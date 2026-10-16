use super::*;
use earthcraft_core::camera::Cam;
use earthcraft_data::aircraft::{Aircraft, AircraftSnapshot};
use earthcraft_render::aircraft::AircraftPreparer;

#[test]
fn repeated_ui_passes_share_icon_meshes_and_never_draw_geometry_for_another_view() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let mut input = ProjectionInput {
        view: ProjectionView {
            basis: Cam {
                tilt: 0.0,
                lat: 0.0,
                lon: 0.0,
                heading: 0.0,
                alt: 14_000.0,
            }
            .basis(4.0 / 3.0),
            rect,
            altitude_km: 14_000.0,
            map_scale: 1.0,
            pixels_per_point: 1.0,
        },
        data: Arc::new(AircraftSnapshot {
            aircraft: vec![Aircraft {
                hex: "aa1234".into(),
                callsign: "FLIGHT".into(),
                latitude: 0.0,
                longitude: 0.0,
                heading_deg: 90.0,
                speed_knots: Some(400.0),
                altitude_feet: Some(30_000.0),
                observed_unix: 1000.0,
            }]
            .into(),
            ..Default::default()
        }),
        unix_secs: 1010.0,
    };
    let mut overlay = AircraftOverlay::new(ctx.clone());
    let original = AircraftPreparer::default().prepare(input.clone());
    overlay.accept(original.clone(), input.unix_secs);
    let raw = egui::RawInput {
        screen_rect: Some(rect),
        ..Default::default()
    };
    let _ = crate::app::test_ui::run(&ctx, raw.clone(), |ui| {
        overlay.draw(ui.painter(), input.clone(), Lang::En)
    });
    let egui::Shape::Mesh(first) = &overlay.shapes[0] else {
        panic!("cached icon mesh");
    };
    let first = first.clone();
    assert_eq!(overlay.shapes.len(), 1);
    let _ = crate::app::test_ui::run(&ctx, raw.clone(), |ui| {
        overlay.draw(ui.painter(), input.clone(), Lang::En)
    });
    let egui::Shape::Mesh(second) = &overlay.shapes[0] else {
        panic!("reused icon mesh");
    };
    assert!(Arc::ptr_eq(&first, second));
    let _ = crate::app::test_ui::run(&ctx, raw.clone(), |ui| {
        overlay.draw(ui.painter(), input.clone(), Lang::PtBr)
    });
    let egui::Shape::Mesh(translated) = &overlay.shapes[0] else {
        panic!("refreshed mesh");
    };
    assert!(!Arc::ptr_eq(&first, translated));
    input.unix_secs = 1011.0;
    let newer = AircraftPreparer::default().prepare(input.clone());
    overlay.accept(newer.clone(), input.unix_secs);
    overlay.accept(original, input.unix_secs);
    assert!(Arc::ptr_eq(overlay.geometry.as_ref().unwrap(), &newer));
    input.view.basis = Cam {
        tilt: 0.0,
        lat: 0.0,
        lon: 45.0,
        heading: 0.0,
        alt: 14_000.0,
    }
    .basis(4.0 / 3.0);
    let output = crate::app::test_ui::run(&ctx, raw, |ui| {
        overlay.draw(ui.painter(), input.clone(), Lang::En)
    });
    assert!(
        !output
            .shapes
            .iter()
            .any(|shape| matches!(shape.shape, egui::Shape::Mesh(_)))
    );
}

#[test]
#[ignore = "manual headless egui aircraft mesh submission benchmark; excludes native GPU execution"]
fn aircraft_mesh_repaint_cost() {
    use std::{hint::black_box, time::Instant};
    const ITERATIONS: usize = 100;
    const ROUNDS: usize = 5;
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let data: Vec<_> = (0..20_000)
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
        .collect();
    let request = ProjectionInput {
        view: ProjectionView {
            basis: Cam {
                tilt: 0.0,
                lat: 0.0,
                lon: 0.0,
                heading: 25.0,
                alt: 14_000.0,
            }
            .basis(4.0 / 3.0),
            rect,
            altitude_km: 14_000.0,
            map_scale: 1.0,
            pixels_per_point: 1.0,
        },
        data: Arc::new(AircraftSnapshot {
            aircraft: data.into(),
            ..Default::default()
        }),
        unix_secs: 1010.0,
    };
    let geometry = AircraftPreparer::default().prepare(request.clone());
    let visible = geometry.flights.len();
    let mut overlay = AircraftOverlay::new(ctx.clone());
    overlay.accept(geometry, request.unix_secs);
    let raw = egui::RawInput {
        screen_rect: Some(rect),
        ..Default::default()
    };
    let _ = crate::app::test_ui::run(&ctx, raw.clone(), |_| {});
    let start = Instant::now();
    let _ = crate::app::test_ui::run(&ctx, raw.clone(), |ui| {
        overlay.draw(ui.painter(), request.clone(), Lang::En)
    });
    println!(
        "20,000 aircraft, {visible} visible: first icon-mesh construction {:.3} ms; {} cached shapes",
        start.elapsed().as_secs_f64() * 1000.0,
        overlay.shapes.len()
    );
    let mut samples = Vec::new();
    for _ in 0..ROUNDS {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            black_box(crate::app::test_ui::run(&ctx, raw.clone(), |ui| {
                overlay.draw(ui.painter(), request.clone(), Lang::En)
            }));
        }
        samples.push(start.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "headless cached egui pass: median {:.3} ms; samples {samples:?}",
        samples[ROUNDS / 2]
    );
}
