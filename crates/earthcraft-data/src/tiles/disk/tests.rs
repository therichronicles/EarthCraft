use super::*;
use std::sync::atomic::AtomicUsize;

static FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);
pub(super) struct Fixture(pub(super) PathBuf);
impl Fixture {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "earthcraft-disk-test-{}-{}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn disk(&self) -> DiskCache {
        DiskCache::new(self.0.clone())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[tokio::test]
async fn clear_removes_all_provider_tiles_and_preserves_other_data() {
    let fixture = Fixture::new();
    let disk = fixture.disk();
    for provider in Provider::ALL {
        let path = fixture.0.join(provider.dir()).join("release/0/0/0.tile");
        disk.write(&path, b"cached imagery", disk.epoch())
            .await
            .unwrap();
    }
    let personal = [
        "settings.json",
        "lang.txt",
        "boundaries.json",
        "placemarks.kml",
    ];
    for filename in personal {
        std::fs::write(fixture.0.join(filename), b"preserve me").unwrap();
    }
    disk.clear().await.unwrap();
    for provider in Provider::ALL {
        assert!(!fixture.0.join(provider.dir()).exists());
    }
    for filename in personal {
        assert_eq!(
            std::fs::read(fixture.0.join(filename)).unwrap(),
            b"preserve me"
        );
    }
    disk.clear().await.unwrap(); // Already empty is a successful clear.
}

#[tokio::test]
async fn clear_blocks_old_jobs_from_writing_back_but_allows_new_downloads() {
    let fixture = Fixture::new();
    let disk = fixture.disk();
    let path = fixture.0.join("esri_sat/0/0/0.tile");
    let old_epoch = disk.epoch();
    disk.write(&path, b"old download", old_epoch).await.unwrap();
    assert_eq!(disk.read(&path).await.unwrap(), b"old download");
    disk.clear().await.unwrap();
    // A job that was still queued or fetching when clear began may finish later.
    disk.write(&path, b"late download", old_epoch)
        .await
        .unwrap();
    assert!(disk.read(&path).await.is_none());
    disk.write(&path, b"new download", disk.epoch())
        .await
        .unwrap();
    assert_eq!(disk.read(&path).await.unwrap(), b"new download");
}

#[tokio::test]
async fn clear_reports_filesystem_errors_and_preserves_unexpected_files() {
    let fixture = Fixture::new();
    let disk = fixture.disk();
    let path = fixture.0.join("esri_sat");
    std::fs::write(&path, b"unexpected file").unwrap();
    assert!(disk.clear().await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"unexpected file");
}

#[cfg(unix)]
#[tokio::test]
async fn clear_removes_provider_symlinks_without_deleting_their_targets() {
    let fixture = Fixture::new();
    let disk = fixture.disk();
    let external = fixture.0.join("other-data");
    std::fs::create_dir_all(&external).unwrap();
    std::fs::write(external.join("keep.tile"), b"unmanaged imagery").unwrap();
    std::os::unix::fs::symlink(&external, fixture.0.join("esri_sat")).unwrap();
    assert_eq!(disk.usage_bytes().await.unwrap(), 0);
    disk.clear().await.unwrap();
    assert!(!fixture.0.join("esri_sat").exists());
    assert_eq!(
        std::fs::read(external.join("keep.tile")).unwrap(),
        b"unmanaged imagery"
    );
}

#[tokio::test]
async fn disk_usage_counts_managed_files_and_refreshes_after_clearing() {
    let fixture = Fixture::new();
    let disk = fixture.disk();
    assert_eq!(disk.usage_bytes().await.unwrap(), 0);
    disk.write(
        &fixture.0.join("esri_sat/0/0/0.tile"),
        b"ocean",
        disk.epoch(),
    )
    .await
    .unwrap();
    disk.write(&fixture.0.join("osm/1/0/1.tile"), b"land", disk.epoch())
        .await
        .unwrap();
    std::fs::write(fixture.0.join("settings.json"), b"exclude settings").unwrap();
    std::fs::create_dir_all(fixture.0.join("geo")).unwrap();
    std::fs::write(fixture.0.join("geo/boundaries.json"), b"exclude boundaries").unwrap();
    assert!(
        disk.usage_bytes().await.unwrap() > 9,
        "usage includes SQLite pages and metadata"
    );
    disk.clear().await.unwrap();
    assert_eq!(disk.usage_bytes().await.unwrap(), 0);
}

#[tokio::test]
async fn persistence_failures_remain_visible_while_reads_can_fall_back() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("osm"), b"blocked directory").unwrap();
    let disk = fixture.disk();
    let path = fixture.0.join("osm/3/0/0.tile");
    assert!(disk.write(&path, b"pixels", disk.epoch()).await.is_err());
    assert!(
        disk.latest_error()
            .unwrap()
            .starts_with("Disk tile cache write failed:")
    );
    assert!(disk.read(&path).await.is_none());
    assert!(disk.latest_error().unwrap().contains("failed:"));
}
#[test]
fn diagnostics_rate_limit_logs_but_keep_the_latest_bounded_message() {
    let mut diagnostics = Diagnostics::default();
    let now = Instant::now();
    assert!(diagnostics.record("first".into(), now));
    assert!(!diagnostics.record("second".into(), now));
    assert_eq!(diagnostics.latest.as_deref(), Some("second"));
    assert!(diagnostics.record("x".repeat(MAX_ERROR_CHARS * 2), now + ERROR_LOG_INTERVAL));
    assert_eq!(diagnostics.latest.unwrap().len(), MAX_ERROR_CHARS);
}
