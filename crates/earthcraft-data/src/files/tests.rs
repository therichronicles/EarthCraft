use super::*;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[test]
fn screenshot_submission_retains_pixels_without_copying() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(PENDING_FILE_JOBS);
    let jobs = FileJobs { sender };
    let pixels = Arc::new(vec![10, 20, 30, 255]);
    jobs.screenshot(PathBuf::from("shot.png"), 1, 1, &pixels)
        .unwrap();
    let Job::Screenshot(_, _, _, retained) = receiver.try_recv().unwrap() else {
        panic!("screenshot job");
    };
    assert!(Arc::ptr_eq(&pixels, &retained));
}
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn jobs_preserve_order_snapshot_inputs_and_wake_after_success_and_failure() {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "earthcraft-file-jobs-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir_all(&fixture.0).unwrap();
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let (wake_tx, wakes) = mpsc::channel();
    let (jobs, results) = FileJobs::new(runtime.handle(), move || {
        let _ = wake_tx.send(());
    });
    let kml = fixture.0.join("saved/marks.kml");
    let mut marks = Places::default();
    marks.push(earthcraft_core::placemark::Placemark {
        name: "Saved snapshot".into(),
        desc: "".into(),
        lat: 20.0,
        lon: 30.0,
    });
    jobs.save_kml(kml.clone(), &marks).unwrap();
    *marks.roots[0].name_mut() = "Edited later".into();
    jobs.open_kml(kml.clone(), Lang::En).unwrap();
    jobs.open_kml(fixture.0.join("missing.kml"), Lang::En)
        .unwrap();
    assert!(matches!(
        results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result,
        Ok(FileOutcome::KmlSaved { .. })
    ));
    let FileOutcome::KmlLoaded { path, placemarks } = results
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap()
    else {
        panic!("ordered KML load");
    };
    assert_eq!(path, kml);
    assert_eq!(placemarks.first().unwrap().name, "Saved snapshot");
    assert!(
        results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result
            .is_err()
    );
    for _ in 0..3 {
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    let pixels = Arc::new(vec![10, 20, 30, 255]);
    jobs.screenshot(fixture.0.join("shot.png"), 1, 1, &pixels)
        .unwrap();
    drop(pixels);
    assert!(matches!(
        results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result,
        Ok(FileOutcome::ScreenshotSaved { .. })
    ));
    wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    let image = image::open(fixture.0.join("shot.png")).unwrap().to_rgba8();
    assert_eq!(image.as_raw(), &[10, 20, 30, 255]);
}

fn fixture() -> Fixture {
    let path = std::env::temp_dir().join(format!(
        "earthcraft-explorer-jobs-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&path).unwrap();
    Fixture(path)
}

#[test]
fn restore_reopens_sources_in_order_prunes_missing_and_retains_other_errors() {
    let fixture = fixture();
    let session = fixture.0.join("open-kml.json");
    let mut paths = Vec::new();
    // A single restoration job supports more documents than the queue capacity.
    for index in 0..PENDING_FILE_JOBS + 2 {
        let path = fixture.0.join(format!("{index}.kml"));
        let mut places = Places::default();
        places.push(earthcraft_core::placemark::Placemark {
            name: format!("Source {index}"),
            desc: String::new(),
            lat: index as f64,
            lon: 0.0,
        });
        crate::kml::save_kml(path.to_str().unwrap(), &places).unwrap();
        paths.push(path);
    }
    paths.insert(1, fixture.0.join("missing.kml"));
    paths.push(paths[0].clone());
    let unreadable = fixture.0.join("directory.kml");
    std::fs::create_dir(&unreadable).unwrap();
    paths.push(unreadable.clone());
    std::fs::write(&session, serde_json::to_vec(&paths).unwrap()).unwrap();
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let (wake_tx, wakes) = mpsc::channel();
    let (jobs, results) = FileJobs::new(runtime.handle(), move || {
        let _ = wake_tx.send(());
    });
    jobs.reserve()
        .unwrap()
        .send(Job::Restore(session, Lang::En));
    let FileOutcome::KmlRestored { files } = results
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap()
    else {
        panic!("restored KML sources");
    };
    assert_eq!(files.len(), PENDING_FILE_JOBS + 3);
    for (index, file) in files.iter().take(PENDING_FILE_JOBS + 2).enumerate() {
        assert_eq!(
            file.result.as_ref().unwrap().first().unwrap().name,
            format!("Source {index}")
        );
    }
    let failed = files.last().unwrap();
    assert_eq!(failed.path, unreadable);
    assert!(matches!(failed.result, Err(crate::Error::Io(_))));
    wakes.recv_timeout(Duration::from_secs(10)).unwrap();
}

#[test]
fn restore_accepts_a_missing_session_but_reports_corrupt_lists() {
    let fixture = fixture();
    let session = fixture.0.join("open-kml.json");
    assert!(
        matches!(Job::Restore(session.clone(), Lang::En).run().unwrap(),
        FileOutcome::KmlRestored { files } if files.is_empty())
    );
    std::fs::write(&session, "invalid JSON").unwrap();
    assert!(matches!(
        Job::Restore(session, Lang::En).run(),
        Err(crate::Error::Json(_))
    ));
}

#[test]
fn planet_jobs_preserve_original_encoded_assets_after_source_files_are_deleted() {
    let fixture = fixture();
    let texture = fixture.0.join("texture.jpg");
    let sky = fixture.0.join("sky.png");
    image::RgbImage::from_pixel(3, 2, image::Rgb([80, 120, 160]))
        .save(&texture)
        .unwrap();
    image::RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 128]))
        .save(&sky)
        .unwrap();
    let texture_bytes = std::fs::read(&texture).unwrap();
    let sky_bytes = std::fs::read(&sky).unwrap();
    let original = fixture.0.join("original.earthplanet");
    crate::planet::save_planet(
        &original,
        "Original",
        texture.to_str().unwrap(),
        sky.to_str().unwrap(),
    )
    .unwrap();
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let (wake_tx, wakes) = mpsc::channel();
    let (jobs, results) = FileJobs::new(runtime.handle(), move || {
        let _ = wake_tx.send(());
    });
    jobs.open_planet(original).unwrap();
    let FileOutcome::PlanetLoaded { planet } = results
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap()
    else {
        panic!("loaded planet");
    };
    std::fs::remove_file(texture).unwrap();
    std::fs::remove_file(sky).unwrap();
    let saved = fixture.0.join("saved.earthplanet");
    jobs.save_planet(PlanetSave {
        path: saved.clone(),
        name: "Renamed".into(),
        texture: ImageSource::Encoded(planet.assets.texture),
        sky: Some(ImageSource::Encoded(planet.assets.sky)),
    })
    .unwrap();
    assert!(
        matches!(results.recv_timeout(Duration::from_secs(10)).unwrap().result,
        Ok(FileOutcome::PlanetSaved { path }) if path == saved)
    );
    let loaded = crate::planet::load_planet(&saved).unwrap();
    assert_eq!(loaded.name, "Renamed");
    assert_eq!(*loaded.assets.texture, texture_bytes);
    assert_eq!(*loaded.assets.sky, sky_bytes);
    assert_eq!(loaded.planet.dimensions(), (3, 2));
    assert_eq!(loaded.sky.unwrap().get_pixel(0, 0).0, [1, 2, 3, 128]);
    for _ in 0..2 {
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    }
}

struct ReleaseWorker(Option<mpsc::Sender<()>>);
impl Drop for ReleaseWorker {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[test]
fn image_jobs_queue_without_waiting_for_the_file_worker_and_wake_on_failure() {
    let fixture = fixture();
    let path = fixture.0.join("image.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([9, 8, 7, 255]))
        .save(&path)
        .unwrap();
    let runtime = crate::runtime::ServiceRuntime::new().unwrap();
    let (release_tx, release_rx) = mpsc::channel();
    let release = ReleaseWorker(Some(release_tx));
    let (started_tx, started) = mpsc::channel();
    runtime.spawn(async move {
        crate::cpu::run(crate::cpu::Service::Files, move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        })
        .await
        .unwrap();
    });
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    let (wake_tx, wakes) = mpsc::channel();
    let (jobs, results) = FileJobs::new(runtime.handle(), move || {
        let _ = wake_tx.send(());
    });
    jobs.load_image(ImageTarget::Planet, ImageSource::Path(path))
        .unwrap();
    jobs.load_image(
        ImageTarget::Sky,
        ImageSource::Encoded(Arc::new(vec![1, 2, 3])),
    )
    .unwrap();
    assert!(matches!(results.try_recv(), Err(mpsc::TryRecvError::Empty)));
    drop(release);
    let FileOutcome::ImageLoaded { target, image } = results
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap()
    else {
        panic!("decoded image");
    };
    assert_eq!(target, ImageTarget::Planet);
    assert_eq!(image.pixels.get_pixel(0, 0).0, [9, 8, 7, 255]);
    let failed = results.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(failed.kind, FileKind::Image(ImageTarget::Sky));
    assert!(matches!(failed.result, Err(crate::Error::Image(_))));
    for _ in 0..2 {
        wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    }
}
