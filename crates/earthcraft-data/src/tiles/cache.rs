use super::{KeyBuild, Tile, TileKey, disk::DiskCache, unkey};
use crate::paths::cache_dir;
use moka::{policy::EvictionPolicy, sync::Cache};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
const TILE_WORKER_COUNT: usize = 6;
const FAILED_TILE_RETRY_DELAY: Duration = Duration::from_secs(15);
/// Best-effort decoded-pixel target; Moka may temporarily exceed it during eviction.
pub const DEFAULT_TILE_CACHE_BUDGET_BYTES: usize = 256 * 1024 * 1024;
const MAX_TILE_RESPONSE_BYTES: u64 = 8_000_000;
pub const MAX_INFLIGHT: usize = 48;

enum RequestState {
    Loading(u64),
    Failed(Instant),
}
impl RequestState {
    fn is_pending(&self) -> bool {
        match self {
            Self::Loading(_) => true,
            Self::Failed(time) => time.elapsed() < FAILED_TILE_RETRY_DELAY,
        }
    }
}
pub enum Lookup {
    Ready(Arc<Tile>),
    Pending,
    Absent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheBudgets {
    pub decoded_bytes: usize,
    pub disk_bytes: u64,
}
impl Default for CacheBudgets {
    fn default() -> Self {
        Self {
            decoded_bytes: DEFAULT_TILE_CACHE_BUDGET_BYTES,
            disk_bytes: super::disk::DEFAULT_DISK_BUDGET_BYTES,
        }
    }
}

pub struct TileCache {
    ready: Cache<TileKey, Arc<Tile>, KeyBuild>,
    // Synchronizes request admission only; decoded-cache hits bypass this lock.
    requests: Mutex<HashMap<TileKey, RequestState, KeyBuild>>,
    // Always lock after requests; channel messages are wake tickets, not FIFO jobs.
    queue: Mutex<VecDeque<TileKey>>,
    budget: usize,
    disk: DiskCache,
    tx: tokio::sync::mpsc::Sender<TileKey>,
    generation: AtomicU64,
    inflight: AtomicUsize,
}

impl TileCache {
    #[cfg(test)]
    fn with_budget(tx: tokio::sync::mpsc::Sender<TileKey>, budget: usize) -> Self {
        Self::with_budget_at(tx, budget, cache_dir())
    }
    #[cfg(test)]
    fn with_budget_at(
        tx: tokio::sync::mpsc::Sender<TileKey>,
        budget: usize,
        directory: PathBuf,
    ) -> Self {
        Self::with_budgets_at(
            tx,
            CacheBudgets {
                decoded_bytes: budget,
                ..CacheBudgets::default()
            },
            directory,
        )
    }
    fn with_budgets_at(
        tx: tokio::sync::mpsc::Sender<TileKey>,
        budgets: CacheBudgets,
        directory: PathBuf,
    ) -> Self {
        let budget = budgets.decoded_bytes;
        Self {
            ready: Cache::builder()
                .name("earthcraft-decoded-tiles")
                .max_capacity(budget as u64)
                .weigher(|_: &TileKey, tile: &Arc<Tile>| {
                    u32::try_from(tile.px.len()).unwrap_or(u32::MAX)
                })
                // LRU favors imagery revisited during map browsing.
                .eviction_policy(EvictionPolicy::lru())
                .build_with_hasher(KeyBuild::default()),
            requests: Mutex::new(HashMap::default()),
            queue: Mutex::new(VecDeque::new()),
            budget,
            disk: DiskCache::with_budget(directory, budgets.disk_bytes),
            tx,
            generation: AtomicU64::new(0),
            inflight: AtomicUsize::new(0),
        }
    }
    pub fn new(
        repaint: impl Fn() + Send + Sync + 'static,
        runtime: &tokio::runtime::Handle,
    ) -> Arc<Self> {
        Self::with_cache_dir(repaint, runtime, cache_dir())
    }

    /// Use an explicit imagery directory, including for isolated render fixtures.
    pub fn with_cache_dir(
        repaint: impl Fn() + Send + Sync + 'static,
        runtime: &tokio::runtime::Handle,
        directory: PathBuf,
    ) -> Arc<Self> {
        Self::with_budgets_in(repaint, runtime, directory, CacheBudgets::default())
    }
    pub fn with_budgets(
        repaint: impl Fn() + Send + Sync + 'static,
        runtime: &tokio::runtime::Handle,
        budgets: CacheBudgets,
    ) -> Arc<Self> {
        Self::with_budgets_in(repaint, runtime, cache_dir(), budgets)
    }
    fn with_budgets_in(
        repaint: impl Fn() + Send + Sync + 'static,
        runtime: &tokio::runtime::Handle,
        directory: PathBuf,
        budgets: CacheBudgets,
    ) -> Arc<Self> {
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        let (tx, rx) = tokio::sync::mpsc::channel(MAX_INFLIGHT);
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        let cache = Arc::new(Self::with_budgets_at(tx, budgets, directory));
        for _ in 0..TILE_WORKER_COUNT {
            let (c, r, x) = (cache.clone(), rx.clone(), repaint.clone());
            runtime.spawn(worker(Arc::downgrade(&c), r, x));
        }
        cache
    }

    pub fn budgets(&self) -> CacheBudgets {
        CacheBudgets {
            decoded_bytes: self.budget,
            disk_bytes: self.disk.budget_bytes(),
        }
    }
    /// Short-lived decoded handles for a renderer upload. Never retain this as a second RAM cache.
    pub fn ready_tiles(
        &self,
        provider: super::Provider,
        release: u16,
    ) -> Vec<(TileKey, Arc<Tile>)> {
        self.ready
            .iter()
            .filter_map(|(key, tile)| {
                let (p, _, _, _, r) = unkey(*key);
                (p == provider && r == release).then_some((*key, tile))
            })
            .collect()
    }

    pub fn lookup(&self, key: TileKey) -> Lookup {
        if let Some(tile) = self.ready.get(&key) {
            return Lookup::Ready(tile);
        }
        match self.requests.lock().unwrap().get(&key) {
            Some(state) if state.is_pending() => Lookup::Pending,
            _ => Lookup::Absent,
        }
    }
    /// Estimated decoded-cache bytes; Moka maintenance may lag inserts and evictions.
    /// Renderer references and other allocations are not included in this metric.
    pub fn resident_bytes(&self) -> usize {
        usize::try_from(self.ready.weighted_size()).unwrap_or(usize::MAX)
    }
    pub fn request(&self, key: TileKey) -> bool {
        self.request_under_limit(key, MAX_INFLIGHT)
    }
    // Check under the admission lock, so background work cannot race past its limit.
    pub(super) fn request_under_limit(&self, key: TileKey, limit: usize) -> bool {
        let mut requests = self.requests.lock().unwrap();
        if self.ready.contains_key(&key)
            || requests.get(&key).is_some_and(RequestState::is_pending)
            || self.inflight.load(Ordering::Relaxed) >= limit.min(MAX_INFLIGHT)
        {
            return false;
        }
        requests.insert(key, RequestState::Loading(self.disk.epoch()));
        self.queue.lock().unwrap().push_back(key);
        self.inflight.fetch_add(1, Ordering::Relaxed);
        // If wake tickets are full, queued tickets already cover the bounded
        // job queue. Retiring a view must not accumulate unbounded stale tickets.
        if matches!(
            self.tx.try_send(key),
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_))
        ) {
            self.queue.lock().unwrap().retain(|&queued| queued != key);
            requests.insert(key, RequestState::Failed(Instant::now()));
            self.inflight.fetch_sub(1, Ordering::Relaxed);
            return false;
        }
        true
    }
    /// Put the latest view first; retire old queued work while keeping active
    /// downloads and the small satellite overview queue. Keys arrive coarse-first.
    pub fn prioritize_view(&self, keys: &[TileKey]) {
        #[cfg(feature = "profiling")]
        let _profile = crate::profiling::Span::new("tiles.admission");
        let wanted: HashSet<_> = keys.iter().copied().collect();
        {
            #[cfg(feature = "profiling")]
            let waiting = std::time::Instant::now();
            let mut requests = self.requests.lock().unwrap();
            #[cfg(feature = "profiling")]
            crate::profiling::Span::elapsed("tiles.admission_lock", waiting, Default::default());
            let mut queue = self.queue.lock().unwrap();
            queue.retain(|key| {
                let (provider, zoom, _, _, _) = unkey(*key);
                let keep =
                    wanted.contains(key) || (provider == super::Provider::Satellite && zoom <= 4);
                if !keep && matches!(requests.remove(key), Some(RequestState::Loading(_))) {
                    self.inflight.fetch_sub(1, Ordering::Relaxed);
                }
                keep
            });
            let queued: HashSet<_> = queue.iter().copied().collect();
            let mut selected = HashSet::new();
            let mut ordered: VecDeque<_> = keys
                .iter()
                .copied()
                .filter(|key| queued.contains(key) && selected.insert(*key))
                .collect();
            ordered.extend(queue.iter().copied().filter(|key| !wanted.contains(key)));
            *queue = ordered;
        }
        for &key in keys {
            if !self.request(key) && self.inflight() >= MAX_INFLIGHT {
                break;
            }
        }
    }
    /// Schedule failed visible dependencies without rerasterizing the globe.
    /// Full admission relies on completion wakeups rather than a busy retry loop.
    pub fn retry_delay(&self, keys: &[TileKey]) -> Option<Duration> {
        let requests = self.requests.lock().unwrap();
        keys.iter()
            .filter_map(|key| match requests.get(key) {
                Some(RequestState::Failed(time)) => {
                    Some(FAILED_TILE_RETRY_DELAY.saturating_sub(time.elapsed()))
                }
                Some(RequestState::Loading(_)) => None,
                None if self.inflight() < MAX_INFLIGHT && !self.ready.contains_key(key) => {
                    Some(Duration::ZERO)
                }
                None => None,
            })
            .min()
    }

    fn next_job(&self) -> Option<(TileKey, u64)> {
        let requests = self.requests.lock().unwrap();
        let key = self.queue.lock().unwrap().pop_front()?;
        let Some(RequestState::Loading(epoch)) = requests.get(&key) else {
            return None;
        };
        Some((key, *epoch))
    }

    fn complete(&self, key: TileKey, result: Result<Tile, crate::Error>) {
        let admitted = match result {
            Ok(tile) if tile.px.len() <= self.budget => {
                self.ready.insert(key, Arc::new(tile));
                true
            }
            _ => {
                self.ready.invalidate(&key);
                false
            }
        };
        // Publish the decoded tile before removing Loading to avoid duplicate requests.
        let mut requests = self.requests.lock().unwrap();
        self.queue.lock().unwrap().retain(|&queued| queued != key);
        let was_loading = matches!(requests.remove(&key), Some(RequestState::Loading(_)));
        requests.retain(|_, state| state.is_pending());
        if !admitted {
            requests.insert(key, RequestState::Failed(Instant::now()));
        }
        if was_loading {
            self.inflight.fetch_sub(1, Ordering::Relaxed);
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
    /// Changes whenever a tile request finishes, including failures.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
    pub fn inflight(&self) -> usize {
        self.inflight.load(Ordering::Relaxed)
    }
    /// Remove this instance's downloaded tile files while preserving decoded
    /// tiles and other application data. Old jobs cannot write back afterward.
    pub async fn clear_disk_cache(&self) -> Result<(), crate::Error> {
        self.disk.clear().await?;
        Ok(())
    }
    /// Snapshot of encoded file bytes in the managed provider directories.
    /// Filesystem allocation overhead and other application data are excluded.
    /// Latest bounded diagnostic; optional persistence failures never discard ready imagery.
    pub fn disk_error(&self) -> Option<String> {
        self.disk.latest_error()
    }
    pub async fn disk_usage_bytes(&self) -> Result<u64, crate::Error> {
        self.disk.usage_bytes().await
    }
}

struct FetchedTile {
    tile: Tile,
    persistence: Option<(std::path::PathBuf, Vec<u8>)>,
}
async fn decode(bytes: Vec<u8>) -> Result<(Tile, Vec<u8>), crate::Error> {
    crate::cpu::run(crate::cpu::Service::Tiles, move || {
        Tile::decode(&bytes).map(|tile| (tile, bytes))
    })
    .await
}
async fn fetch_tile(k: TileKey, disk: &DiskCache) -> Result<FetchedTile, crate::Error> {
    let (provider, zoom, x, y, release) = unkey(k);
    let path = disk.path(k)?;
    if let Some(bytes) = disk.read(&path).await {
        match decode(bytes).await {
            Ok((tile, _)) => {
                return Ok(FetchedTile {
                    tile,
                    persistence: None,
                });
            }
            Err(_) => {
                let _ = disk.invalidate(&path).await;
            }
        }
    }
    let url = provider.url(zoom, x, y, release);
    if url.is_empty() {
        return Err("empty tile URL".into());
    }
    let bytes = crate::http::body(
        crate::http::client(crate::http::Policy::Tile)?.get(url),
        MAX_TILE_RESPONSE_BYTES,
    )
    .await?;
    let (tile, bytes) = decode(bytes).await?;
    Ok(FetchedTile {
        tile,
        persistence: Some((path, bytes)),
    })
}
async fn worker(
    cache: std::sync::Weak<TileCache>,
    rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<TileKey>>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
    loop {
        let Some(_ticket) = rx.lock().await.recv().await else {
            return;
        };
        let Some(cache) = cache.upgrade() else {
            return;
        };
        let Some((key, epoch)) = cache.next_job() else {
            continue;
        };
        match fetch_tile(key, &cache.disk).await {
            Ok(FetchedTile { tile, persistence }) => {
                // Publish before optional SQLite persistence, so writes/eviction do
                // not delay imagery readiness. Persistence keeps its original epoch.
                cache.complete(key, Ok(tile));
                repaint();
                if let Some((path, bytes)) = persistence
                    && cache.disk.write_owned(&path, bytes, epoch).await.is_err()
                {
                    repaint();
                }
            }
            Err(error) => {
                cache.complete(key, Err(error));
                repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmarks;
