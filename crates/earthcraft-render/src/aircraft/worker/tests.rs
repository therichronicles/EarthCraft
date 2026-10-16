use super::*;
use earthcraft_core::camera::Cam;
use earthcraft_data::aircraft::Aircraft;
use emath::{Pos2, Vec2};
use std::{sync::mpsc, time::Duration};

fn input() -> ProjectionInput {
    let data = (0..200)
        .map(|index| Aircraft {
            hex: format!("{index:06x}").into(),
            callsign: format!("FLIGHT{index}").into(),
            latitude: (index % 20) as f64 - 10.0,
            longitude: (index / 20) as f64 - 5.0,
            heading_deg: 90.0,
            speed_knots: Some(440.0),
            altitude_feet: Some(35_000.0),
            observed_unix: 1000.0,
        })
        .collect::<Vec<_>>()
        .into();
    ProjectionInput {
        view: ProjectionView {
            basis: Cam {
                tilt: 0.0,
                lat: 0.0,
                lon: 0.0,
                heading: 0.0,
                alt: 600.0,
            }
            .basis(4.0 / 3.0),
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
            altitude_km: 600.0,
            map_scale: 1.0,
            pixels_per_point: 1.0,
        },
        data: Arc::new(AircraftSnapshot {
            aircraft: data,
            ..Default::default()
        }),
        unix_secs: 1010.1,
    }
}

#[test]
fn unchanged_view_reuses_geometry_and_time_data_dpi_and_scale_invalidate_it() {
    let mut preparer = AircraftPreparer::default();
    let mut request = input();
    let first = preparer.prepare(request.clone());
    assert!(first.is_recent(1010.1) && first.is_recent(1011.9));
    assert!(!first.is_recent(1009.9) && !first.is_recent(1012.2));
    request.unix_secs = 1010.9;
    assert!(Arc::ptr_eq(&first, &preparer.prepare(request.clone())));
    request.unix_secs = 1011.0;
    let timed = preparer.prepare(request.clone());
    assert!(!Arc::ptr_eq(&first, &timed));
    request.view.pixels_per_point = 2.0;
    assert!(!Arc::ptr_eq(&timed, &preparer.prepare(request.clone())));
    request.view.map_scale = 1.5;
    let scaled = preparer.prepare(request.clone());
    request.data = Arc::new(AircraftSnapshot {
        aircraft: request.data.aircraft.clone(),
        ..Default::default()
    });
    assert!(!scaled.matches(request.view, &request.data));
    assert!(!Arc::ptr_eq(&scaled, &preparer.prepare(request)));
}

#[test]
fn labels_keep_source_order_distance_and_count_limits_and_overview_has_no_labels() {
    let mut preparer = AircraftPreparer::default();
    let mut request = input();
    let geometry = preparer.prepare(request.clone());
    assert!(!geometry.labels.is_empty());
    assert!(geometry.labels.len() <= MAX_LABELS);
    assert!(geometry.labels.array_windows::<2>().all(|[a, b]| a < b));
    for (index, &first) in geometry.labels.iter().enumerate() {
        for &other in &geometry.labels[index + 1..] {
            assert!(
                (geometry.flights[first].position - geometry.flights[other].position).length()
                    > LABEL_MIN_SEPARATION_POINTS
            );
        }
    }
    request.view.altitude_km = 1000.0;
    assert!(preparer.prepare(request).labels.is_empty());
}

#[test]
fn background_projection_coalesces_pending_views_and_uses_the_latest_displayed_camera() {
    let (wake, wakes) = mpsc::channel();
    let (release, releases) = mpsc::channel();
    let ui_thread = std::thread::current().id();
    let worker = AircraftWorker::new(move || {
        assert_ne!(ui_thread, std::thread::current().id());
        let _ = wake.send(());
        let _ = releases.recv_timeout(Duration::from_secs(10));
    })
    .unwrap();
    let mut request = input();
    worker.submit(request.clone());
    wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    for heading in 1..=100 {
        request.view.basis = Cam {
            tilt: 0.0,
            lat: 0.0,
            lon: 0.0,
            heading: f64::from(heading),
            alt: 600.0,
        }
        .basis(4.0 / 3.0);
        worker.submit(request.clone());
    }
    release.send(()).unwrap();
    wakes.recv_timeout(Duration::from_secs(10)).unwrap();
    let geometry = worker.take_completed().unwrap();
    assert!(geometry.matches(request.view, &request.data));
    let wrong = input();
    assert!(!geometry.matches(wrong.view, &request.data));
    release.send(()).unwrap();
}
