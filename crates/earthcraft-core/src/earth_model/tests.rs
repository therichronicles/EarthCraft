use super::*;

#[test]
fn curvature_from_surface_normals_agrees_with_geodetic_latitude() {
    for model in EarthModel::ALL {
        let geometry = model.geometry();
        for step in -900..=900 {
            let latitude = step as f64 / 10.0;
            let normal = geometry.normal(geometry.position(latitude, 179.9));
            let (expected_m, expected_n) = geometry.curvature_km(latitude);
            let (actual_m, actual_n) = geometry.curvature_km_from_latitude_sine(normal.z);
            assert!((actual_m - expected_m).abs() < 1e-10);
            assert!((actual_n - expected_n).abs() < 1e-10);
        }
    }
}

#[test]
fn wgs84_direct_and_inverse_match_published_geographiclib_example() {
    // geographiclib-rs GeodesicLine documentation, based on Karney's test data:
    // https://docs.rs/geographiclib-rs/0.2.7/geographiclib_rs/struct.GeodesicLine.html
    let (start, end) = ((-11.95887, -116.94513), (4.57352, 7.16501));
    let distance_km = 13_834.722580140138;
    let bearing_deg = 92.71261983045255;
    let model = EarthModel::Wgs84;
    assert!((model.distance_km(start, end) - distance_km).abs() < 1e-8);
    assert!((model.initial_bearing(start, end).unwrap() - bearing_deg).abs() < 1e-10);
    let actual = model.destination(start, bearing_deg, distance_km);
    assert!((actual.0 - end.0).abs() < 1e-10);
    assert!((actual.1 - end.1).abs() < 1e-10);
}

#[test]
fn path_samples_follow_evenly_spaced_shortest_geodesics() {
    const SEGMENTS: usize = 48;
    for model in EarthModel::ALL {
        let geometry = model.geometry();
        for (start, end) in [
            ((0.0, 0.0), (0.0, 180.0)),
            ((90.0, 70.0), (-90.0, -110.0)),
            ((-11.95887, -116.94513), (4.57352, 7.16501)),
            ((23.0, -14.0), (-23.0, 165.999999)),
            ((89.99, 179.9), (89.999, -179.9)),
            ((10.0, 20.0), (10.0, 20.0)),
        ] {
            let path = model.path(start, end, SEGMENTS);
            let length = model.distance_km(start, end);
            let coordinates: Vec<_> = path.into_iter().map(|p| geometry.coordinates(p)).collect();
            for (index, point) in coordinates.iter().enumerate() {
                let expected = length * index as f64 / SEGMENTS as f64;
                assert!(
                    (model.distance_km(start, *point) - expected).abs() < 1e-6,
                    "{model:?} sample {index}: {start:?} -> {end:?}"
                );
            }
            let sampled: f64 = coordinates
                .array_windows::<2>()
                .map(|pair| model.distance_km(pair[0], pair[1]))
                .sum();
            assert!(
                (sampled - length).abs() < 1e-6,
                "{model:?}: {start:?} -> {end:?}"
            );
        }
    }
}
#[test]
fn spherical_distances_retain_precision_near_antipodes() {
    for longitude in [179.999_f64, 179.999999, 180.0] {
        let expected = EARTH_RADIUS_KM * longitude.to_radians();
        let actual = EarthModel::PerfectSphere.distance_km((0.0, 0.0), (0.0, longitude));
        assert!(
            (actual - expected).abs() < 1e-8,
            "{longitude}: {actual} != {expected}"
        );
    }
}
#[test]
fn paths_stay_on_the_surface_at_antipodes_and_with_no_segments() {
    for model in EarthModel::ALL {
        for (start, end) in [
            ((0.0, 0.0), (0.0, 180.0)),
            ((90.0, 0.0), (-90.0, 0.0)),
            ((23.0, -14.0), (-23.0, 165.999999)),
            ((0.0, 0.0), (0.0, 0.0000001)),
        ] {
            let geometry = model.geometry();
            for count in [0, 48] {
                let path = model.path(start, end, count);
                assert_eq!(path.len(), count + 1, "{model:?}: {start:?} -> {end:?}");
                assert!(path[0].abs_diff_eq(geometry.position(start.0, start.1), 1e-12));
                for point in &path {
                    assert!(point.is_finite());
                    assert!(
                        (point.dot(*point * geometry.inverse_axes_squared()) - 1.0).abs() < 1e-12,
                        "{model:?}: {start:?} -> {end:?}: {point:?}"
                    );
                }
                if count > 0 {
                    assert!(path[count].abs_diff_eq(geometry.position(end.0, end.1), 1e-12));
                }
            }
        }
    }
}
#[test]
fn intersections_choose_forward_hits_and_reject_invalid_rays() {
    for model in EarthModel::ALL {
        let geometry = model.geometry();
        let surface = geometry.position(0.0, 0.0);
        assert!(
            geometry
                .intersect(DVec3::ZERO, DVec3::X)
                .unwrap()
                .abs_diff_eq(surface, 1e-12)
        );
        assert!(
            geometry
                .intersect(surface * 0.5, DVec3::X)
                .unwrap()
                .abs_diff_eq(surface, 1e-12)
        );
        assert!(geometry.intersect(surface * 2.0, DVec3::X).is_none());
        for (origin, direction) in [
            (DVec3::ZERO, DVec3::ZERO),
            (DVec3::NAN, DVec3::X),
            (DVec3::ZERO, DVec3::INFINITY),
        ] {
            assert!(geometry.intersect(origin, direction).is_none());
        }
    }
}
#[test]
fn known_geodesics_distinguish_models_and_cover_antipodes() {
    let model = EarthModel::Wgs84;
    assert!((model.distance_km((90.0, 0.0), (-90.0, 0.0)) - 20003.931458625).abs() < 1e-8);
    assert!((model.distance_km((0.0, 0.0), (0.0, 1.0)) - 111.319490793).abs() < 1e-8);
    assert!((model.distance_km((0.0, 0.0), (0.0, 180.0)) - 20003.931458625).abs() < 1e-8);
    for end in [(0.01, 179.99), (89.99, 20.0), (-35.0, -179.9)] {
        let start = (10.0, 1.0);
        let bearing = model.initial_bearing(start, end).unwrap();
        let actual = model.destination(start, bearing, model.distance_km(start, end));
        assert!((actual.0 - end.0).abs() < 1e-10 && (actual.1 - end.1).abs() < 1e-10);
    }
    assert_eq!(
        EarthModel::PerfectSphere.distance_km((0.0, 0.0), (0.0, 1.0)),
        math::haversine_km((0.0, 0.0), (0.0, 1.0))
    );
}
#[test]
fn surface_coordinates_normals_and_intersections_agree() {
    for model in EarthModel::ALL {
        let geometry = model.geometry();
        for (lat, lon) in [(0.0, 0.0), (45.0, 179.9), (89.999, -179.9), (-90.0, 60.0)] {
            let point = geometry.position(lat, lon);
            let normal = math::ll_to_v3(lat, lon);
            for direction in [
                normal,
                -normal,
                normal * 2.0,
                normal.any_orthonormal_vector(),
            ] {
                let hit = geometry.intersect(point, direction).unwrap();
                assert!(hit.abs_diff_eq(point, 1e-12), "{model:?}: {lat}, {lon}");
            }
            let hit = geometry.intersect(point + normal * 0.1, -normal).unwrap();
            assert!(hit.abs_diff_eq(point, 1e-12));
            let (a, b) = geometry.coordinates(hit);
            assert!((a - lat).abs() < 1e-8 && (b - lon).abs() < 1e-8);
            assert!((point.dot(point * geometry.inverse_axes_squared()) - 1.0).abs() < 1e-14);
        }
    }
}
