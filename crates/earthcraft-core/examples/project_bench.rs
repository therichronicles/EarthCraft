//! Time projecting many placemarks: `cargo run --release --example project_bench`
use earthcraft_core::{camera::Cam, earth_model::EarthModel};
use emath::{Rect, pos2};
use std::time::Instant;
fn main() {
    let points: Vec<(f64, f64)> = (0..105_000u64)
        .map(|i| {
            (
                ((i * 7919) % 13000) as f64 / 100.0 - 60.0,
                ((i * 104729) % 36000) as f64 / 100.0 - 180.0,
            )
        })
        .collect();
    let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1920.0, 1080.0));
    for model in [EarthModel::PerfectSphere, EarthModel::default()] {
        let basis = Cam {
            lat: 20.0,
            lon: 0.0,
            heading: 0.0,
            alt: 12000.0,
        }
        .basis_for_model(16.0 / 9.0, model);
        let start = Instant::now();
        let mut visible = 0;
        for &(lat, lon) in &points {
            if basis
                .project(basis.surface_position(lat, lon), rect)
                .is_some()
            {
                visible += 1;
            }
        }
        println!(
            "{model:?}: {} points, {visible} projected in {:.2?}",
            points.len(),
            start.elapsed()
        );
    }
}
