//! Time a KML/KMZ import: `cargo run --release --example kml_bench -- file.kmz`
use earthcraft_core::language::Lang;
fn main() {
    let path = std::env::args().nth(1).expect("usage: kml_bench <file>");
    let start = std::time::Instant::now();
    let places = earthcraft_data::kml::load_kml(&path, Lang::En).expect("load");
    let elapsed = start.elapsed();
    let (mut markers, mut paths, mut vertices) = (0, 0, 0);
    for pm in places.markers(false) {
        markers += 1;
        paths += pm.paths.len();
        vertices += pm.paths.iter().map(|p| p.points().len()).sum::<usize>();
    }
    println!("{markers} placemarks, {paths} paths, {vertices} vertices in {elapsed:.2?}");
}
