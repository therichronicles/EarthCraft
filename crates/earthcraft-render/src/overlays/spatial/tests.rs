use super::*;
use earthcraft_core::camera::Cam;

#[test]
fn conservative_bounds_keep_visible_interiors_across_poles_dateline_and_horizon() {
    let lines: Vec<Line> = [
        vec![(0.0, -100.0), (0.0, 100.0)],
        vec![(80.0, -179.0), (89.0, 179.0)],
        vec![(-40.0, 170.0), (40.0, -170.0)],
        vec![(-85.0, -170.0), (-85.0, 170.0)],
        vec![(-60.0, -50.0), (60.0, 50.0)],
    ]
    .into_iter()
    .map(Arc::new)
    .collect();
    let data = BoundaryData {
        countries: lines.clone(),
        ..Default::default()
    };
    let index = BoundaryIndex::new(&data);
    for model in earthcraft_core::earth_model::EarthModel::ALL {
        for latitude in [-89.9, -40.0, 0.0, 40.0, 89.9] {
            for longitude in [-180.0, -90.0, 0.0, 90.0, 179.9] {
                for altitude in [0.15, 500.0, 14000.0, 40000.0] {
                    for heading in [0.0, 90.0, 170.0] {
                        let basis = Cam {
                            lat: latitude,
                            lon: longitude,
                            alt: altitude,
                            heading,
                        }
                        .basis_for_model(4.0 / 3.0, model);
                        let rect =
                            Rect::from_min_size(emath::Pos2::ZERO, emath::vec2(800.0, 600.0));
                        let visible = index.visible(&basis, rect);
                        for line in &lines {
                            let bounds = Bounds::line(line);
                            let (a, b) = (line[0], line[1]);
                            let mut delta = b.1 - a.1;
                            if delta > 180.0 {
                                delta -= 360.0;
                            }
                            if delta < -180.0 {
                                delta += 360.0;
                            }
                            for step in 0..=200 {
                                let t = step as f64 / 200.0;
                                let point =
                                    basis.surface_position(a.0 + (b.0 - a.0) * t, a.1 + delta * t);
                                let margin = DVec3::splat(model.geometry().displacement_bound());
                                assert!(
                                    point.cmpge(bounds.min - margin).all()
                                        && point.cmple(bounds.max + margin).all()
                                );
                                if basis.project(point, rect).is_some_and(|p| rect.contains(p)) {
                                    assert!(
                                        visible.ids.contains(&(Arc::as_ptr(line) as usize)),
                                        "visible interpolation must survive culling"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn hidden_and_offscreen_lines_are_rejected_without_reordering_survivors() {
    let data = BoundaryData {
        countries: [
            vec![(0.0, 0.0), (0.1, 0.1)],
            vec![(0.0, 179.0), (0.1, 179.1)],
            vec![(0.0, 20.0), (0.1, 20.1)],
        ]
        .into_iter()
        .map(Arc::new)
        .collect(),
        ..Default::default()
    };
    let index = BoundaryIndex::new(&data);
    let basis = Cam {
        lat: 0.0,
        lon: 0.0,
        alt: 500.0,
        heading: 0.0,
    }
    .basis(1.0);
    let rect = Rect::from_min_size(emath::Pos2::ZERO, emath::vec2(600.0, 600.0));
    let visible = index.visible(&basis, rect);
    assert!(
        visible
            .ids
            .contains(&(Arc::as_ptr(&data.countries[0]) as usize))
    );
    assert!(
        !visible
            .ids
            .contains(&(Arc::as_ptr(&data.countries[1]) as usize))
    );
    assert!(
        !visible
            .ids
            .contains(&(Arc::as_ptr(&data.countries[2]) as usize))
    );
}

#[test]
#[ignore = "manual static boundary-index benchmark"]
fn static_boundary_index_time() {
    use std::{hint::black_box, time::Instant};
    let data = BoundaryData {
        countries: (0..10000)
            .map(|id| {
                let lat = (id % 150) as f64 - 75.0;
                let lon = ((id * 97) % 360) as f64 - 180.0;
                Arc::new(
                    (0..32)
                        .map(|step| (lat + step as f64 * 0.01, lon + step as f64 * 0.01))
                        .collect(),
                )
            })
            .collect(),
        ..Default::default()
    };
    let start = Instant::now();
    let index = BoundaryIndex::new(&data);
    let construction = start.elapsed();
    struct IndexedBounds(usize, Bounds);
    impl rstar::RTreeObject for IndexedBounds {
        type Envelope = rstar::AABB<[f64; 2]>;
        fn envelope(&self) -> Self::Envelope {
            rstar::AABB::from_corners([self.1.min.x, self.1.min.y], [self.1.max.x, self.1.max.y])
        }
    }
    let objects: Vec<_> = index
        .entries
        .iter()
        .map(|&(id, b)| IndexedBounds(id, b))
        .collect();
    let start = Instant::now();
    let reference = rstar::RTree::bulk_load(objects);
    let reference_construction = start.elapsed();
    let mut indexed = std::time::Duration::ZERO;
    let mut linear = std::time::Duration::ZERO;
    let mut reference_query = std::time::Duration::ZERO;
    let rect = Rect::from_min_size(emath::Pos2::ZERO, emath::vec2(800.0, 600.0));
    for lon in 0..360 {
        let basis = Cam {
            lat: 20.0,
            lon: lon as f64 - 180.0,
            alt: 500.0,
            heading: 0.0,
        }
        .basis(4.0 / 3.0);
        let center = basis.c.normalize();
        let radius = (2.0 - 2.0 / basis.c.length()).sqrt();
        let query = rstar::AABB::from_corners(
            [center.x - radius, center.y - radius],
            [center.x + radius, center.y + radius],
        );
        let start = Instant::now();
        let reference_result: HashSet<_> = reference
            .locate_in_envelope_intersecting(query)
            .filter(|entry| entry.1.visible(&basis, rect))
            .map(|entry| entry.0)
            .collect();
        black_box(&reference_result);
        reference_query += start.elapsed();
        let start = Instant::now();
        let result = index.visible(&basis, rect);
        indexed += start.elapsed();
        let start = Instant::now();
        let expected: HashSet<_> = index
            .entries
            .iter()
            .filter(|(_, b)| b.visible(&basis, rect))
            .map(|(id, _)| *id)
            .collect();
        linear += start.elapsed();
        assert_eq!(result.ids, expected);
        assert_eq!(result.ids, reference_result);
    }
    println!(
        "10k lines: bounds+geo-index build {:.3} ms / rstar index-only build {:.3} ms; moving-view query geo-index+planes {:.3} ms / linear planes {:.3} ms / rstar+planes {:.3} ms; packed index {} bytes (bounds/IDs separate)",
        construction.as_secs_f64() * 1000.0,
        reference_construction.as_secs_f64() * 1000.0,
        indexed.as_secs_f64() * 1000.0 / 360.0,
        linear.as_secs_f64() * 1000.0 / 360.0,
        reference_query.as_secs_f64() * 1000.0 / 360.0,
        index.tree.as_ref().unwrap().as_ref().len()
    );
}
