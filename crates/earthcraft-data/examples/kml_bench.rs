//! Time a KML/KMZ import: `cargo run --release --example kml_bench -- file.kmz`
use earthcraft_core::language::Lang;
fn main() {
    let path = std::env::args().nth(1).expect("usage: kml_bench <file>");
    let start = std::time::Instant::now();
    let places = earthcraft_data::kml::load_kml(&path, Lang::En).expect("load");
    let elapsed = start.elapsed();
    println!("{} markers in {:.2?}", places.len(), elapsed);
}
