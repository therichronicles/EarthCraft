//! Time per-frame path projection for a KML/KMZ: `cargo run --release --example paths_bench -- file.kmz`
use earthcraft_core::{camera::Cam, earth_model::EarthModel, language::Lang};
use emath::{Rect, pos2};
use std::time::Instant;
fn main() {
    let file = std::env::args().nth(1).expect("usage: paths_bench <file>");
    let places = earthcraft_data::kml::load_kml(&file, Lang::En).expect("load");
    let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1920.0, 1080.0));
    for (label, lat, lon, alt) in [
        ("whole continent", 45.0, -94.0, 9000.0),
        ("state", 45.5, -94.0, 1200.0),
        ("metro", 44.97, -93.26, 60.0),
        ("street", 44.97, -93.26, 2.0),
    ] {
        let basis = Cam {
            lat,
            lon,
            heading: 0.0,
            alt,
        }
        .basis_for_model(16.0 / 9.0, EarthModel::Wgs84);
        let start = Instant::now();
        let (mut runs, mut vertices) = (0, 0);
        let mut out = Vec::new();
        for pm in places.markers(true) {
            for path in &pm.paths {
                earthcraft_render::overlays::paths::project_path(&basis, rect, path, &mut out);
                for run in out.drain(..) {
                    runs += 1;
                    vertices += run.len();
                }
            }
        }
        println!(
            "{label:16} {runs:6} runs {vertices:8} vertices {:.2?}",
            start.elapsed()
        );
    }
}
