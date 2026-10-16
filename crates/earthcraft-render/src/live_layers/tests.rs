use super::*;
use earthcraft_core::{camera::Cam, earth_model::EarthModel};
use std::{sync::mpsc, time::Duration};

pub(super) fn input(model: EarthModel) -> LiveInput {
    let camera = Cam {
        lat: 0.0,
        lon: 0.0,
        heading: 0.0,
        alt: 100.0,
    };
    LiveInput {
        view: LiveView {
            basis: camera.basis_for_model(1.0, model),
            rect: Rect::from_min_size(Pos2::new(20.0, 30.0), Vec2::splat(600.0)),
            altitude_km: camera.alt,
            map_scale: 1.0,
            pixels_per_point: 1.0,
            traffic_width_px: 16,
            traffic_height_px: 16,
        },
        roads: Some(
            vec![Road {
                name: "Road".into(),
                kind: "primary".into(),
                restricted: false,
                points: vec![
                    (0.0, -0.1),
                    (0.0, 0.0),
                    (0.0, 180.0),
                    (0.0, 0.0),
                    (0.0, 0.1),
                ],
            }]
            .into(),
        ),
        vessels: Some(
            vec![Vessel {
                mmsi: "123456789".into(),
                name: "Ship".into(),
                lat: 0.0,
                lon: 0.0,
                course: 45.0,
                speed_knots: Some(8.0),
                observed_unix: 1_700_000_000.0,
            }]
            .into(),
        ),
        satellites: None,
        traffic: None,
        unix_secs: 1_700_000_000.0,
    }
}
#[test]
fn roads_vessels_and_hover_match_the_selected_surface_and_keep_all_visible_fragments() {
    for model in EarthModel::ALL {
        let input = input(model);
        let geometry = LivePreparer::default().prepare(input.clone());
        let expected = input
            .view
            .basis
            .project(input.view.basis.surface_position(0.0, 0.0), input.view.rect)
            .unwrap();
        assert_eq!(geometry.vessels[0].position, expected);
        assert!(
            geometry.vessels[0]
                .icon
                .iter()
                .all(|p| p.x.is_finite() && p.y.is_finite())
        );
        assert_eq!(geometry.roads[0].segments.len(), 2);
        let point = input
            .view
            .basis
            .project(
                input.view.basis.surface_position(0.0, -0.075),
                input.view.rect,
            )
            .unwrap();
        let (source, nearest) = geometry.nearest_road(point, 1.0).unwrap();
        assert_eq!(source, 0);
        assert!((nearest - point).length() < 0.01);
        assert!(
            geometry
                .nearest_road(Pos2::new(-1000.0, -1000.0), 1.0)
                .is_none()
        );
    }
}
#[test]
fn preparer_reuses_components_and_invalidates_changes_to_view_data_and_time() {
    let mut request = input(EarthModel::Wgs84);
    request.traffic = Some(Arc::new(TrafficTiles {
        image: image::RgbaImage::from_pixel(768, 768, image::Rgba([0, 200, 0, 255])),
        zoom: 2,
        left_x: 1,
        top_y: 1,
        loaded: std::time::Instant::now(),
        failed_tiles: 0,
    }));
    let mut preparer = LivePreparer::default();
    let first = preparer.prepare(request.clone());
    request.unix_secs += 1.0;
    assert!(Arc::ptr_eq(&first, &preparer.prepare(request.clone())));
    request.vessels = Some(Vec::<Vessel>::new().into());
    let second = preparer.prepare(request.clone());
    assert!(!Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first.roads, &second.roads));
    assert!(Arc::ptr_eq(
        first.traffic.as_ref().unwrap(),
        second.traffic.as_ref().unwrap()
    ));
    assert!(!Arc::ptr_eq(&first.vessels, &second.vessels));
    request.view.rect = request.view.rect.translate(Vec2::new(10.0, 20.0));
    let third = preparer.prepare(request);
    assert!(!Arc::ptr_eq(&second.roads, &third.roads));
    assert!(!Arc::ptr_eq(
        second.traffic.as_ref().unwrap(),
        third.traffic.as_ref().unwrap()
    ));
    assert!(
        third
            .traffic
            .as_ref()
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] == 229)
    );
}
#[test]
fn orbital_updates_reuse_roads_and_project_only_current_valid_subpoints() {
    let mut request = input(EarthModel::Wgs84);
    let orbit = earthcraft_core::orbit::OrbitalElements {
        inclination_deg: 0.0,
        raan_deg: 0.0,
        eccentricity: 0.001,
        argument_perigee_deg: 0.0,
        mean_anomaly_deg: 0.0,
        mean_motion_rev_per_day: 15.0,
        epoch_unix_secs: request.unix_secs,
    };
    let (lat, lon) = orbit.ground_position(request.unix_secs).unwrap();
    request.view.basis = Cam {
        lat,
        lon,
        alt: 1000.0,
        heading: 0.0,
    }
    .basis_for_model(1.0, EarthModel::Wgs84);
    request.satellites = Some(
        vec![Satellite {
            name: "Orbit".into(),
            norad: "1".into(),
            orbit,
        }]
        .into(),
    );
    let mut preparer = LivePreparer::default();
    let first = preparer.prepare(request.clone());
    assert_eq!(first.satellites.len(), 1);
    request.unix_secs += 1.0;
    let second = preparer.prepare(request);
    assert!(Arc::ptr_eq(&first.roads, &second.roads));
    assert!(!Arc::ptr_eq(&first.satellites, &second.satellites));
    assert_ne!(first.satellites[0].position, second.satellites[0].position);
}
#[test]
fn worker_publishes_before_waking_and_reuses_an_unchanged_request() {
    let (send, receive) = mpsc::channel();
    let worker = LiveWorker::new(move || {
        let _ = send.send(());
    })
    .unwrap();
    let input = input(EarthModel::PerfectSphere);
    worker.submit(input.clone());
    receive.recv_timeout(Duration::from_secs(10)).unwrap();
    let first = worker.take_completed().unwrap();
    assert!(first.input.same_request(&input));
    worker.submit(input);
    receive.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(Arc::ptr_eq(&first, &worker.take_completed().unwrap()));
}
