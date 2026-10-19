//! Isolated synthetic measurements; no services, cache files or timing assertions.
use super::cache::*;
use earthcraft_core::{camera::Cam, language::Lang};
use earthcraft_data::boundaries::{BoundaryData, LabelKind, MapLabel};
use earthcraft_render::overlays::LabelIndex;
use eframe::egui::{self, LayerId, Pos2, Rect, pos2, vec2};
use std::{hint::black_box, sync::Arc, time::Instant};

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

#[test]
#[ignore = "manual overlay benchmark; run alone with --ignored --nocapture"]
fn overlay_projection_and_label_occupancy_time() {
    for (width, height) in [(1920, 1080), (960, 600)] {
        let areas: Vec<Rect> = (0..8000)
            .map(|i| {
                Rect::from_min_size(
                    pos2(((i * 7919) % width) as f32, ((i * 104729) % height) as f32),
                    vec2(64.0, 16.0),
                )
            })
            .collect();
        let mut linear = Vec::new();
        let mut indexed = Vec::new();
        let mut admitted = 0;
        for _ in 0..7 {
            let start = Instant::now();
            let mut accepted: Vec<Rect> = Vec::new();
            let reference: Vec<bool> = areas
                .iter()
                .map(|&area| {
                    let admit =
                        accepted.len() < 500 && !accepted.iter().any(|rect| rect.intersects(area));
                    if admit {
                        accepted.push(area);
                    }
                    admit
                })
                .collect();
            linear.push(start.elapsed().as_secs_f64() * 1000.0);
            admitted = accepted.len();
            let start = Instant::now();
            let mut index = LabelIndex::default();
            let actual: Vec<bool> = areas
                .iter()
                .map(|&area| index.len() < 500 && index.try_insert(area))
                .collect();
            indexed.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(reference, actual);
        }
        println!(
            "{width}x{height}, 8000 label rectangles, {admitted} admitted: linear {:.3} ms, rstar {:.3} ms (median of 7)",
            median(linear),
            median(indexed)
        );
    }

    let data = BoundaryData {
        countries: (0..10_000)
            .map(|i| {
                Arc::new(
                    (0..32)
                        .map(|step| {
                            (
                                -75.0 + (i % 150) as f64,
                                -179.0 + (i % 358) as f64 + step as f64 * 0.1,
                            )
                        })
                        .collect(),
                )
            })
            .collect(),
        labels: (0..8000)
            .map(|i| MapLabel {
                lat: -75.0 + (i % 150) as f64,
                lon: -179.0 + (i % 358) as f64,
                en: format!("Place {i}"),
                pt: format!("Lugar {i}"),
                kind: LabelKind::Country,
                rank: (i % 10) as u8,
                big: true,
            })
            .collect(),
        ..Default::default()
    };
    let cam = Cam {
        tilt: 0.0,
        lat: 0.0,
        lon: 0.0,
        heading: 0.0,
        alt: 14_000.0,
    };
    let view = OverlayView {
        basis: cam.basis(16.0 / 9.0),
        rect: Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0)),
        altitude_km: cam.alt,
        grid_step_deg: None,
        pixels_per_point: 1.0,
    };
    let style = OverlayStyle {
        layers: OverlayLayers {
            countries: true,
            states: false,
            cities: false,
            metro: false,
            labels: true,
        },
        map_scale: 1.0,
        lang: Lang::En,
    };
    let ctx = egui::Context::default();
    let mut cache = OverlayCache::new(ctx.clone());
    let data = Arc::new(data);
    let mut rebuild = Vec::new();
    let mut reuse = Vec::new();
    for round in 0..8 {
        for fresh in [true, false] {
            ctx.begin_pass(egui::RawInput::default());
            let painter = ctx.layer_painter(LayerId::background());
            if fresh {
                cache.invalidate();
            }
            let start = Instant::now();
            loop {
                cache.draw(&painter, view, &data, style);
                if cache.is_ready(view, style) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            if round > 0 {
                if fresh {
                    rebuild.push(elapsed);
                } else {
                    reuse.push(elapsed);
                }
            }
            let mut output = ctx.end_pass();
            output.textures_delta.clear();
            black_box(output);
        }
    }
    println!(
        "10000 32-point lines + 8000 labels: rebuild {:.3} ms, reuse {:.3} ms (median of 7; projection/placement/shape submission only)",
        median(rebuild),
        median(reuse)
    );
}
