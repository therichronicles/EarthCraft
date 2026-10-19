//! Tile-only disk persistence and coordination with explicit cache clearing.
use super::{Provider, TileKey, unkey};
use crate::wayback::wayback_release;
use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

mod mbtiles;
pub(super) const DEFAULT_DISK_BUDGET_BYTES: u64 = 128 * 1024 * 1024;

const ERROR_LOG_INTERVAL: Duration = Duration::from_secs(30);
const MAX_ERROR_CHARS: usize = 2048;
#[derive(Default)]
struct Diagnostics {
    latest: Option<String>,
    logged_at: Option<Instant>,
}
impl Diagnostics {
    fn record(&mut self, message: String, now: Instant) -> bool {
        self.latest = Some(message.chars().take(MAX_ERROR_CHARS).collect());
        if self
            .logged_at
            .is_none_or(|last| now.saturating_duration_since(last) >= ERROR_LOG_INTERVAL)
        {
            self.logged_at = Some(now);
            true
        } else {
            false
        }
    }
}

pub(super) struct DiskCache {
    tilesets: mbtiles::Mbtiles,
    root: PathBuf,
    epoch: AtomicU64,
    diagnostics: Mutex<Diagnostics>,
    gate: tokio::sync::RwLock<()>,
}

impl DiskCache {
    #[cfg(test)]
    pub(super) fn new(root: PathBuf) -> Self {
        Self {
            tilesets: mbtiles::Mbtiles::new(root.clone()),
            root,
            epoch: AtomicU64::new(0),
            diagnostics: Mutex::new(Diagnostics::default()),
            gate: tokio::sync::RwLock::new(()),
        }
    }
    pub(super) fn with_budget(root: PathBuf, bytes: u64) -> Self {
        Self {
            tilesets: mbtiles::Mbtiles::with_limit(root.clone(), bytes),
            root,
            epoch: AtomicU64::new(0),
            diagnostics: Mutex::new(Diagnostics::default()),
            gate: tokio::sync::RwLock::new(()),
        }
    }
    fn report(&self, operation: &str, error: &crate::Error) {
        let message = format!("Disk tile cache {operation} failed: {error}");
        let mut diagnostics = self.diagnostics.lock().unwrap();
        if diagnostics.record(message, Instant::now()) {
            eprintln!("{}", diagnostics.latest.as_deref().unwrap());
        }
    }
    pub(super) fn latest_error(&self) -> Option<String> {
        self.diagnostics.lock().unwrap().latest.clone()
    }
    pub(super) fn budget_bytes(&self) -> u64 {
        self.tilesets.limit()
    }
    pub(super) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Relaxed)
    }
    pub(super) fn path(&self, key: TileKey) -> Result<PathBuf, crate::Error> {
        let (provider, zoom, x, y, release) = unkey(key);
        let mut base = self.root.join(provider.dir());
        if provider == Provider::Wayback {
            base = base.join(
                wayback_release(release)
                    .ok_or("wayback: release not loaded")?
                    .id
                    .to_string(),
            );
        }
        Ok(base
            .join(zoom.to_string())
            .join(x.to_string())
            .join(format!("{y}.tile")))
    }
    pub(super) async fn read(&self, path: &Path) -> Option<Vec<u8>> {
        let _guard = self.gate.read().await;
        match self.tilesets.read(path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                self.report("read", &error);
                None
            }
        }
    }
    #[cfg(test)]
    pub(super) async fn write(
        &self,
        path: &Path,
        bytes: &[u8],
        epoch: u64,
    ) -> Result<(), crate::Error> {
        self.write_owned(path, bytes.to_vec(), epoch).await
    }
    pub(super) async fn write_owned(
        &self,
        path: &Path,
        bytes: Vec<u8>,
        epoch: u64,
    ) -> Result<(), crate::Error> {
        let _guard = self.gate.read().await;
        if epoch != self.epoch() {
            return Ok(());
        }
        self.tilesets
            .write_owned(path, bytes)
            .await
            .inspect_err(|error| self.report("write", error))
    }
    pub(super) async fn invalidate(&self, path: &Path) -> Result<(), crate::Error> {
        let _guard = self.gate.read().await;
        self.tilesets
            .invalidate(path)
            .await
            .inspect_err(|error| self.report("invalidation", error))
    }
    pub(super) async fn clear(&self) -> Result<(), crate::Error> {
        let _guard = self.gate.write().await;
        self.epoch.fetch_add(1, Ordering::Relaxed);
        self.tilesets
            .clear()
            .await
            .inspect_err(|error| self.report("clear", error))?;
        self.diagnostics.lock().unwrap().latest = None;
        Ok(())
    }
    pub(super) async fn usage_bytes(&self) -> Result<u64, crate::Error> {
        let _guard = self.gate.read().await;
        self.tilesets
            .usage()
            .await
            .inspect_err(|error| self.report("usage measurement", error))
    }
}

// File lengths are a snapshot of encoded cache data, excluding filesystem
// allocation overhead. Never follow links into unrelated directories.
fn count_file_bytes(root: &Path) -> io::Result<u64> {
    let mut pending: Vec<PathBuf> = Provider::ALL
        .iter()
        .map(|provider| root.join(provider.dir()))
        .collect();
    let mut bytes = 0u64;
    while let Some(path) = pending.pop() {
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(path)? {
                pending.push(entry?.path());
            }
        } else if metadata.is_file() {
            bytes = bytes.saturating_add(metadata.len());
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
