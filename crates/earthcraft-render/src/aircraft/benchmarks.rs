use super::*;
use earthcraft_core::camera::Cam;
use std::{hint::black_box, time::Instant};

const AIRCRAFT_COUNT: usize = 20_000;
const ROUNDS: usize = 5;
const ITERATIONS: usize = 100;

#[test]
#[ignore = "manual isolated aircraft projection benchmark"]
fn aircraft_projection_cost() {
    let flights = flights();
    let basis = Cam {
        tilt: 0.0,
        lat: 0.0,
        lon: 0.0,
        heading: 25.0,
        alt: 14_000.0,
    }
    .basis(4.0 / 3.0);
    let rect = Rect::from_min_size(Pos2::ZERO, emath::vec2(800.0, 600.0));
    let visible = project_flights(&basis, rect, &flights, 1010.0, 1.0).len();
    let mut samples = Vec::new();
    for _ in 0..ROUNDS {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            black_box(project_flights(&basis, rect, &flights, 1010.0, 1.0));
        }
        samples.push(start.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "20,000 aircraft, {visible} visible, 800x600 overview projection: median {:.3} ms; samples {samples:?}",
        samples[ROUNDS / 2]
    );
}

#[test]
#[ignore = "manual isolated aircraft geometry reuse benchmark"]
fn aircraft_geometry_reuse_cost() {
    let mut preparer = AircraftPreparer::default();
    let view = ProjectionView {
        basis: Cam {
            tilt: 0.0,
            lat: 0.0,
            lon: 0.0,
            heading: 25.0,
            alt: 14_000.0,
        }
        .basis(4.0 / 3.0),
        rect: Rect::from_min_size(Pos2::ZERO, emath::vec2(800.0, 600.0)),
        altitude_km: 14_000.0,
        map_scale: 1.0,
        pixels_per_point: 1.0,
    };
    let request = ProjectionInput {
        view,
        data: std::sync::Arc::new(earthcraft_data::aircraft::AircraftSnapshot {
            aircraft: flights().into(),
            ..Default::default()
        }),
        unix_secs: 1010.0,
    };
    let geometry = preparer.prepare(request.clone());
    let mut samples = Vec::new();
    for _ in 0..ROUNDS {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            black_box(preparer.prepare(request.clone()));
        }
        samples.push(start.elapsed().as_secs_f64() * 1_000_000.0 / ITERATIONS as f64);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "20,000 aircraft, {} visible cached geometry: median {:.3} us; samples {samples:?}",
        geometry.flights.len(),
        samples[ROUNDS / 2]
    );
}

pub(super) fn flights() -> Vec<Aircraft> {
    (0..AIRCRAFT_COUNT)
        .map(|index| Aircraft {
            hex: format!("{index:06x}").into(),
            callsign: format!("FLIGHT{index}").into(),
            latitude: (index % 160) as f64 - 80.0,
            longitude: (index % 360) as f64 - 180.0,
            heading_deg: 90.0,
            speed_knots: Some(440.0),
            altitude_feet: Some(35_000.0),
            observed_unix: 1000.0,
        })
        .collect()
}
