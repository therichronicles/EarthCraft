use super::super::make_key;
use super::*;
use crate::tiles::Provider;
#[test]
fn tile_key_roundtrip() {
    for (p, z, x, y, rel) in [
        (Provider::Wayback, 17u8, 98_765u32, 123_456u32, 4321u16),
        (Provider::Satellite, 0, 0, 0, 0),
        (Provider::Streets, 19, 524_287, 524_287, 0),
        (Provider::Topo, 5, 17, 9, 0),
    ] {
        let k = make_key(p, z, x, y, rel);
        assert_eq!(unkey(k), (p, z, x, y, rel));
    }
    assert_ne!(
        make_key(Provider::Wayback, 10, 1, 1, 1),
        make_key(Provider::Wayback, 10, 1, 1, 2)
    );
}

#[test]
fn explicit_cache_directories_keep_instances_isolated() {
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let root = std::env::temp_dir().join("earthcraft-explicit-roots");
    let first = TileCache::with_cache_dir(|| {}, runtime.handle(), root.join("first"));
    let second = TileCache::with_cache_dir(|| {}, runtime.handle(), root.join("second"));
    let key = make_key(Provider::Satellite, 0, 0, 0, 0);
    assert_eq!(
        first.disk.path(key).unwrap(),
        root.join("first/esri_sat/0/0/0.tile")
    );
    assert_eq!(
        second.disk.path(key).unwrap(),
        root.join("second/esri_sat/0/0/0.tile")
    );
}

fn test_cache(budget: usize) -> (Arc<TileCache>, tokio::sync::mpsc::Receiver<TileKey>) {
    let (tx, rx) = tokio::sync::mpsc::channel(MAX_INFLIGHT);
    (Arc::new(TileCache::with_budget(tx, budget)), rx)
}
fn tiny_tile(value: u8) -> Tile {
    Tile {
        w: 1,
        h: 1,
        px: vec![value, value, value, 255],
    }
}

#[test]
fn cache_evicts_least_recently_used_with_a_byte_weighted_target() {
    let (cache, _rx) = test_cache(8);
    cache.complete(1, Ok(tiny_tile(1)));
    cache.complete(2, Ok(tiny_tile(2)));
    cache.ready.run_pending_tasks();
    assert!(matches!(cache.lookup(1), Lookup::Ready(_)));
    cache.ready.run_pending_tasks();
    cache.complete(3, Ok(tiny_tile(3)));
    cache.ready.run_pending_tasks();
    assert!(matches!(cache.lookup(2), Lookup::Absent));
    assert!(matches!(cache.lookup(1), Lookup::Ready(_)));
    assert_eq!(cache.resident_bytes(), 8);
    cache.complete(1, Ok(tiny_tile(4)));
    cache.ready.run_pending_tasks();
    assert_eq!(cache.resident_bytes(), 8);
    let Lookup::Ready(tile) = cache.lookup(1) else {
        panic!("replacement must be cached")
    };
    assert_eq!(tile.px[0], 4);
}

#[test]
fn failed_requests_and_oversized_tiles_keep_their_retry_delay_without_pixel_weight() {
    let (cache, _rx) = test_cache(4);
    cache.complete(1, Err("network failure".into()));
    cache.complete(
        2,
        Ok(Tile {
            w: 2,
            h: 1,
            px: vec![0; 8],
        }),
    );
    cache.ready.run_pending_tasks();
    assert_eq!(cache.resident_bytes(), 0);
    assert!(matches!(cache.lookup(1), Lookup::Pending));
    assert!(matches!(cache.lookup(2), Lookup::Pending));
    assert!(!cache.request(1));
    assert!(!cache.request(2));
    cache.requests.lock().unwrap().insert(
        1,
        RequestState::Failed(Instant::now() - FAILED_TILE_RETRY_DELAY),
    );
    assert!(matches!(cache.lookup(1), Lookup::Absent));
    assert!(cache.request(1));
    assert_eq!(cache.inflight(), 1);
    cache.complete(1, Err("network failure".into()));
    assert_eq!(cache.inflight(), 0);
    assert_eq!(cache.generation(), 3);
}

#[test]
fn concurrent_requests_for_one_tile_enqueue_once() {
    let (cache, mut rx) = test_cache(8);
    let barrier = std::sync::Barrier::new(8);
    let admitted = std::thread::scope(|scope| {
        let jobs: Vec<_> = (0..8)
            .map(|_| {
                let (cache, barrier) = (&cache, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    cache.request(1)
                })
            })
            .collect();
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .filter(|admitted| *admitted)
            .count()
    });
    assert_eq!(admitted, 1);
    assert_eq!(cache.inflight(), 1);
    assert_eq!(rx.try_recv().unwrap(), 1);
    assert!(rx.try_recv().is_err());
}

#[test]
fn request_limit_completion_and_closed_queue_account_for_inflight_work() {
    let (cache, mut rx) = test_cache(8);
    for key in 0..MAX_INFLIGHT as u64 {
        assert!(cache.request(key));
    }
    assert!(!cache.request(MAX_INFLIGHT as u64));
    assert!(!cache.request(0));
    assert_eq!(cache.inflight(), MAX_INFLIGHT);
    assert_eq!(
        (0..MAX_INFLIGHT).map(|_| rx.try_recv().unwrap()).count(),
        MAX_INFLIGHT
    );
    cache.complete(0, Ok(tiny_tile(1)));
    assert_eq!(cache.inflight(), MAX_INFLIGHT - 1);
    assert_eq!(cache.generation(), 1);
    assert!(!cache.request(0));
    assert!(cache.request(MAX_INFLIGHT as u64));
    assert_eq!(cache.inflight(), MAX_INFLIGHT);

    let (closed, rx) = test_cache(8);
    drop(rx);
    assert!(!closed.request(1));
    assert_eq!(closed.inflight(), 0);
    assert!(matches!(closed.lookup(1), Lookup::Pending));
}

#[test]
fn cached_hits_do_not_wait_for_request_coordination() {
    let (cache, _rx) = test_cache(8);
    cache.complete(1, Ok(tiny_tile(1)));
    let guard = cache.requests.lock().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = cache.clone();
    let job = std::thread::spawn(move || {
        tx.send(matches!(reader.lookup(1), Lookup::Ready(_)))
            .unwrap();
    });
    let result = rx.recv_timeout(Duration::from_secs(5));
    drop(guard);
    job.join().unwrap();
    assert!(result.expect("cached hit should bypass request mutex"));
}

#[test]
fn overview_warmup_yields_to_visible_work_and_visits_the_world_once() {
    use crate::tiles::TileWarmup;
    let (cache, mut rx) = test_cache(4096);
    let mut warmup = TileWarmup::startup_satellite();
    let foreground = make_key(Provider::Satellite, 10, 100, 100, 0);
    let other = make_key(Provider::Satellite, 10, 100, 101, 0);
    assert!(cache.request(foreground));
    assert!(cache.request(other));
    assert_eq!(rx.try_recv().unwrap(), foreground);
    assert_eq!(rx.try_recv().unwrap(), other);
    assert!(warmup.pump(&cache));
    assert!(rx.try_recv().is_err(), "visible work keeps priority");
    cache.complete(foreground, Ok(tiny_tile(1)));
    cache.complete(other, Ok(tiny_tile(1)));

    let mut keys = std::collections::HashSet::new();
    loop {
        let remaining = warmup.pump(&cache);
        assert!(cache.inflight() <= 2, "warmup must leave workers available");
        while let Ok(key) = rx.try_recv() {
            let (provider, z, x, y, rel) = unkey(key);
            assert_eq!(provider, Provider::Satellite);
            assert_eq!(rel, 0);
            assert!(z <= 4 && x < 1 << z && y < 1 << z);
            assert!(keys.insert(key), "warmup must not enqueue duplicates");
            cache.complete(key, Ok(tiny_tile(1)));
        }
        if !remaining {
            break;
        }
    }
    assert_eq!(keys.len(), 341);
    assert!(!warmup.pump(&cache));
    assert_eq!(cache.inflight(), 0);
}

#[test]
fn overview_skips_cached_loading_and_failed_tiles_without_blocking_foreground() {
    use crate::tiles::TileWarmup;
    let (cache, mut rx) = test_cache(4096);
    let cached = make_key(Provider::Satellite, 0, 0, 0, 0);
    let failed = make_key(Provider::Satellite, 1, 0, 0, 0);
    let loading = make_key(Provider::Satellite, 1, 0, 1, 0);
    cache.complete(cached, Ok(tiny_tile(1)));
    cache.complete(failed, Err("fixture failure".into()));
    assert!(cache.request(loading));
    assert_eq!(rx.try_recv().unwrap(), loading);
    let mut warmup = TileWarmup::startup_satellite();
    assert!(warmup.pump(&cache));
    let first = rx.try_recv().unwrap();
    assert_ne!(first, cached);
    assert_ne!(first, failed);
    assert_ne!(first, loading);
    assert!(rx.try_recv().is_err());
    assert!(cache.request(make_key(Provider::Satellite, 12, 1, 1, 0)));
}

#[test]
fn cancelling_overview_warmup_prevents_a_speculative_disk_refill() {
    let (cache, mut rx) = test_cache(4096);
    let mut warmup = crate::tiles::TileWarmup::startup_satellite();
    warmup.cancel();
    assert!(!warmup.pump(&cache));
    assert!(rx.try_recv().is_err());
    assert_eq!(cache.inflight(), 0);
}

#[tokio::test]
async fn clearing_preserves_memory_and_the_original_epoch_of_queued_requests() {
    let (tx, _rx) = tokio::sync::mpsc::channel(MAX_INFLIGHT);
    let mut cache = TileCache::with_budget(tx, 4096);
    cache.disk = DiskCache::new(std::env::temp_dir().join(format!(
        "earthcraft-queued-clear-test-{}",
        std::process::id(),
    )));
    cache.complete(2, Ok(tiny_tile(42)));
    assert!(cache.request(1));
    cache.clear_disk_cache().await.unwrap();
    assert_eq!(cache.disk.epoch(), 1);
    assert!(matches!(cache.lookup(2), Lookup::Ready(_)));
    let requests = cache.requests.lock().unwrap();
    assert!(matches!(requests.get(&1), Some(RequestState::Loading(0))));
}

#[test]
fn new_view_retires_queued_work_preserving_active_downloads_and_overview() {
    let (cache, _tickets) = test_cache(4096);
    let active = make_key(Provider::Satellite, 10, 1, 1, 0);
    let stale = make_key(Provider::Satellite, 10, 2, 1, 0);
    let overview = make_key(Provider::Satellite, 2, 0, 0, 0);
    let visible = make_key(Provider::Satellite, 10, 3, 1, 0);
    assert!(cache.request(active));
    assert_eq!(cache.next_job().unwrap().0, active);
    assert!(cache.request(stale));
    assert!(cache.request(overview));
    assert!(cache.request(visible));
    cache.prioritize_view(&[visible]);
    assert_eq!(cache.inflight(), 3);
    assert!(matches!(cache.lookup(stale), Lookup::Absent));
    assert!(matches!(cache.lookup(active), Lookup::Pending));
    assert_eq!(cache.next_job().unwrap().0, visible);
    assert_eq!(cache.next_job().unwrap().0, overview);
    assert!(cache.next_job().is_none());
    for key in [active, visible, overview] {
        cache.complete(key, Ok(tiny_tile(42)));
    }
    assert_eq!(cache.inflight(), 0);
    assert!(
        cache.request(stale),
        "retired work can be requested again without a failure delay"
    );
}

#[test]
fn retired_wake_tickets_stay_bounded_and_duplicate_priorities_do_not_duplicate_jobs() {
    let (cache, mut tickets) = test_cache(4096);
    for view in 0..100 {
        let keys: Vec<_> = (0..MAX_INFLIGHT)
            .map(|offset| make_key(Provider::Streets, 19, view * 100 + offset as u32, 0, 0))
            .collect();
        cache.prioritize_view(&keys);
        assert_eq!(cache.inflight(), MAX_INFLIGHT);
    }
    let visible = make_key(Provider::Streets, 19, 99999, 0, 0);
    cache.prioritize_view(&[visible, visible]);
    cache.prioritize_view(&[visible, visible]);
    assert_eq!(cache.inflight(), 1);
    let mut count = 0;
    while tickets.try_recv().is_ok() {
        count += 1;
    }
    assert!(count <= MAX_INFLIGHT);
    assert_eq!(cache.next_job().unwrap().0, visible);
    assert!(cache.next_job().is_none());
}
#[test]
fn failed_visible_tiles_keep_retry_deadlines_without_raster_invalidation() {
    let (cache, _rx) = test_cache(4096);
    cache.complete(1, Err("offline".into()));
    assert!(cache.retry_delay(&[1]).unwrap() > Duration::ZERO);
    cache.requests.lock().unwrap().insert(
        1,
        RequestState::Failed(Instant::now() - FAILED_TILE_RETRY_DELAY),
    );
    assert_eq!(cache.retry_delay(&[1]), Some(Duration::ZERO));
    cache.prioritize_view(&[1]);
    assert_eq!(cache.inflight(), 1);
    assert_eq!(cache.retry_delay(&[1]), None);
    cache.complete(1, Ok(tiny_tile(10)));
    assert_eq!(cache.retry_delay(&[1]), None);
    assert_eq!(cache.retry_delay(&[2]), Some(Duration::ZERO));
}

#[test]
fn configured_budgets_reach_both_cache_implementations() {
    let directory = std::env::temp_dir().join("earthcraft-budget-construction");
    let (send, _receive) = tokio::sync::mpsc::channel(1);
    let budgets = CacheBudgets {
        decoded_bytes: 32 * 1024 * 1024,
        disk_bytes: 64 * 1024 * 1024,
    };
    let cache = TileCache::with_budgets_at(send, budgets, directory);
    assert_eq!(cache.budgets(), budgets);
    assert_eq!(
        cache.ready.policy().max_capacity(),
        Some(budgets.decoded_bytes as u64)
    );
}

#[test]
fn paused_admission_retires_queued_overviews_and_preserves_active_downloads() {
    let (cache, _rx) = test_cache(4096);
    let active = make_key(Provider::Satellite, 0, 0, 0, 0);
    let queued = make_key(Provider::Satellite, 1, 0, 0, 0);
    assert!(cache.request(active));
    assert!(cache.request(queued));
    assert_eq!(cache.next_job().unwrap().0, active);
    assert!(cache.set_requests_enabled(false));
    assert_eq!(cache.inflight(), 1);
    assert!(cache.next_job().is_none());
    cache.prioritize_view(&[queued]);
    assert_eq!(cache.inflight(), 1);
    assert!(!cache.request(queued));
    cache.complete(active, Ok(tiny_tile(7)));
    assert!(matches!(cache.lookup(active), Lookup::Ready(_)));
    assert_eq!(cache.inflight(), 0);
    assert!(cache.set_requests_enabled(true));
    assert!(cache.request(queued));
}
