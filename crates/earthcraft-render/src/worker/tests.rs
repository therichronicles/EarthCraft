use super::*;
use earthcraft_data::tiles::Provider;
use std::{sync::mpsc, time::Duration};

const TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn aircraft_geometry_is_prepared_for_the_raster_camera_before_publication() {
    use crate::aircraft::{AircraftPreparer, ProjectionInput, ProjectionView};
    let (wake, wakes) = mpsc::channel();
    let ui_thread = std::thread::current().id();
    let mut preparer = AircraftPreparer::default();
    let worker = RenderWorker::with_prepare(
        |_, buffer| crate::RenderedView {
            pixels: buffer,
            waiting: vec![],
            requests: vec![],
        },
        move || {
            let _ = wake.send(());
        },
        |_| {},
        move |request, _| {
            assert_ne!(ui_thread, std::thread::current().id());
            preparer.prepare(request.aircraft.clone().unwrap())
        },
    )
    .unwrap();
    let mut view = request(20.0, 7);
    let expected = ProjectionView {
        basis: view.basis,
        rect: emath::Rect::from_min_size(emath::Pos2::ZERO, emath::vec2(800.0, 600.0)),
        altitude_km: view.cam.alt,
        map_scale: 1.0,
        pixels_per_point: 2.0,
    };
    view.aircraft = Some(ProjectionInput {
        view: expected,
        data: Arc::new(earthcraft_data::aircraft::AircraftSnapshot::default()),
        unix_secs: 1000.0,
    });
    worker.submit(view);
    wakes.recv_timeout(TEST_TIMEOUT).unwrap();
    let frame = worker.take_completed(7).unwrap();
    assert!(frame.prepared.view == expected);
    assert!(frame.prepared.view.basis == frame.request.basis);
}

fn request(longitude: f64, presentation: u64) -> RenderRequest {
    let cam = Cam {
        lat: -15.0,
        lon: longitude,
        heading: 25.0,
        alt: 14_000.0,
    };
    RenderRequest {
        width: 64,
        height: 48,
        cam,
        basis: cam.basis(4.0 / 3.0),
        cfg: RenderCfg {
            sky: None,
            provider: Provider::Satellite,
            rel: 0,
            grid: false,
            grid_step: 10.0,
            atmo: true,
            sun: None,
        },
        presentation,
        overlays: None,
        aircraft: None,
        live_layers: None,
    }
}

struct ControlledWorker {
    worker: RenderWorker,
    started: mpsc::Receiver<(f64, std::thread::ThreadId)>,
    release: mpsc::Sender<()>,
    repaint: mpsc::Receiver<()>,
    exited: mpsc::Receiver<()>,
    admitted: mpsc::Receiver<u8>,
}

impl ControlledWorker {
    fn new() -> Self {
        let (started_tx, started) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let (repaint_tx, repaint) = mpsc::channel();
        let (exit_tx, exited) = mpsc::channel();
        let (admit_tx, admitted) = mpsc::channel();
        struct ExitSignal(mpsc::Sender<()>);
        impl Drop for ExitSignal {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let exit_signal = ExitSignal(exit_tx);
        let worker = RenderWorker::new(
            move |request, mut buffer| {
                let _keep_alive = &exit_signal;
                started_tx
                    .send((request.cam.lon, std::thread::current().id()))
                    .unwrap();
                release_rx.recv_timeout(TEST_TIMEOUT).unwrap();
                buffer.resize(request.width * request.height * 4, 0);
                buffer.fill(request.cam.lon as u8);
                crate::RenderedView {
                    pixels: buffer,
                    waiting: vec![],
                    requests: vec![],
                }
            },
            move || {
                repaint_tx.send(()).unwrap();
            },
            move |frame| {
                admit_tx.send(frame.pixels[0]).unwrap();
            },
        )
        .unwrap();
        Self {
            worker,
            started,
            release,
            repaint,
            exited,
            admitted,
        }
    }
    fn start(&self) -> (f64, std::thread::ThreadId) {
        self.started.recv_timeout(TEST_TIMEOUT).unwrap()
    }
    fn finish(&self) {
        self.release.send(()).unwrap();
        self.repaint.recv_timeout(TEST_TIMEOUT).unwrap();
    }
}

#[test]
fn camera_requests_coalesce_without_blocking_submission_or_starving_frames() {
    let test = ControlledWorker::new();
    test.worker.submit(request(1.0, 7));
    let (longitude, thread) = test.start();
    assert_eq!(longitude, 1.0);
    assert_ne!(thread, std::thread::current().id());
    assert!(!test.worker.is_idle());
    // The first job is blocked; submitting hundreds of views must still return.
    for longitude in 2..=200 {
        test.worker.submit(request(longitude as f64, 7));
    }
    test.finish();
    assert_eq!(test.start().0, 200.0);
    // Present completed camera snapshots during continuous motion, while the
    // newest view runs. Rejecting every moving frame would freeze rotation.
    let first = test.worker.take_completed(7).unwrap();
    assert_eq!(first.request.cam.lon, 1.0);
    test.finish();
    let last = test.worker.take_completed(7).unwrap();
    assert_eq!(last.request.cam.lon, 200.0);
    assert_eq!(last.pixels, vec![200; 64 * 48 * 4]);
    assert!(test.worker.is_idle());
}

#[test]
fn obsolete_provider_style_or_size_results_are_discarded() {
    let test = ControlledWorker::new();
    test.worker.submit(request(1.0, 7));
    test.start();
    test.worker.submit(request(2.0, 8));
    test.finish();
    test.start();
    assert!(test.worker.take_completed(7).is_none());
    assert!(
        test.admitted.try_recv().is_err(),
        "obsolete frames must not admit downloads"
    );
    test.finish();
    assert_eq!(test.admitted.recv_timeout(TEST_TIMEOUT).unwrap(), 2);
    // Recheck against the UI's current presentation, even when a style changed
    // after the worker completed its job.
    assert!(test.worker.take_completed(9).is_none());
}

#[test]
fn only_the_latest_completed_frame_is_retained_and_completion_wakes_ui() {
    let test = ControlledWorker::new();
    for longitude in [1.0, 2.0] {
        test.worker.submit(request(longitude, 7));
        test.start();
        test.finish();
    }
    let frame = test.worker.take_completed(7).unwrap();
    assert_eq!(frame.request.cam.lon, 2.0);
    assert_eq!((frame.request.width, frame.request.height), (64, 48));
    assert!(test.worker.take_completed(7).is_none());
}

#[test]
fn shutdown_drops_pending_views_without_waiting_for_an_active_raster() {
    let test = ControlledWorker::new();
    test.worker.submit(request(1.0, 7));
    test.start();
    test.worker.submit(request(2.0, 7));
    drop(test.worker);
    // Drop returned before we unblock the active raster. The pending job must
    // never execute, and the worker must release its captured resources.
    test.release.send(()).unwrap();
    test.exited.recv_timeout(TEST_TIMEOUT).unwrap();
    assert!(matches!(
        test.started.recv_timeout(TEST_TIMEOUT),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    assert!(matches!(
        test.repaint.recv_timeout(TEST_TIMEOUT),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn returned_screenshot_buffer_is_reused_for_the_next_frame() {
    let test = ControlledWorker::new();
    test.worker.submit(request(1.0, 7));
    test.start();
    test.finish();
    let frame = test.worker.take_completed(7).unwrap();
    let pointer = frame.pixels.as_ptr();
    test.worker.recycle(frame.pixels);
    test.worker.submit(request(2.0, 7));
    test.start();
    test.finish();
    let next = test.worker.take_completed(7).unwrap();
    assert_eq!(next.pixels.as_ptr(), pointer);
    assert!(next.pixels.iter().all(|&value| value == 2));
}

#[test]
fn frame_preparation_runs_off_ui_and_is_published_with_its_pixels() {
    let caller = std::thread::current().id();
    let (tx, rx) = mpsc::channel();
    let worker = RenderWorker::with_prepare(
        |_, _| crate::RenderedView {
            pixels: vec![10, 20, 30, 255],
            waiting: vec![],
            requests: vec![],
        },
        move || {
            tx.send(()).unwrap();
        },
        |_| {},
        move |request, pixels| {
            assert_ne!(std::thread::current().id(), caller);
            (request.cam.lon, pixels.to_vec())
        },
    )
    .unwrap();
    worker.submit(request(25.0, 7));
    rx.recv_timeout(TEST_TIMEOUT).unwrap();
    let frame = worker.take_completed(7).unwrap();
    assert_eq!(frame.prepared, (25.0, frame.pixels.clone()));
}

#[test]
fn presentation_filtered_buffers_are_reused_and_obsolete_frames_skip_preparation() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let prepared = Arc::new(AtomicUsize::new(0));
    let preparations = prepared.clone();
    let (started, starts) = mpsc::channel();
    let (release, releases) = mpsc::channel();
    let (wake, wakes) = mpsc::channel();
    let worker = RenderWorker::with_prepare(
        move |_, mut buffer| {
            buffer.resize(64 * 48 * 4, 0);
            started.send(buffer.as_ptr() as usize).unwrap();
            releases.recv_timeout(TEST_TIMEOUT).unwrap();
            crate::RenderedView {
                pixels: buffer,
                waiting: vec![],
                requests: vec![],
            }
        },
        move || {
            let _ = wake.send(());
        },
        |_| {},
        move |_, _| {
            preparations.fetch_add(1, Ordering::Relaxed);
        },
    )
    .unwrap();
    worker.submit(request(0.0, 1));
    let pointer = starts.recv_timeout(TEST_TIMEOUT).unwrap();
    worker.submit(request(1.0, 2));
    release.send(()).unwrap();
    wakes.recv_timeout(TEST_TIMEOUT).unwrap();
    assert_eq!(starts.recv_timeout(TEST_TIMEOUT).unwrap(), pointer);
    assert_eq!(prepared.load(Ordering::Relaxed), 0);
    release.send(()).unwrap();
    wakes.recv_timeout(TEST_TIMEOUT).unwrap();
    assert_eq!(prepared.load(Ordering::Relaxed), 1);
    assert!(worker.take_completed(3).is_none());
    worker.submit(request(2.0, 3));
    assert_eq!(starts.recv_timeout(TEST_TIMEOUT).unwrap(), pointer);
    release.send(()).unwrap();
    wakes.recv_timeout(TEST_TIMEOUT).unwrap();
    assert!(worker.take_completed(3).is_some());
}
