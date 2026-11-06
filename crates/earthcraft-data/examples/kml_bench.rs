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
    let mut colors = std::collections::BTreeMap::<String, usize>::new();
    let (mut with_desc, mut with_attrs) = (0, 0);
    for pm in places.markers(false) {
        with_desc += usize::from(!pm.desc.is_empty());
        with_attrs += usize::from(!pm.attributes.is_empty());
        let key = pm.line_style.map_or("none".into(), |s| format!("{:?} w{}", s.rgba, s.width));
        *colors.entry(key).or_default() += 1;
    }
    for pm in places.markers(false).take(3) {
        println!("name {:?} attrs {} first {:?}", pm.name, pm.attributes.len(), pm.attributes.first());
    }
    if let Some(pm) = places.markers(false).find(|p| !p.desc.is_empty()) {
        println!("KEPT DESC: {:?}\nATTRS: {:?}", pm.desc, pm.attributes);
    }
    println!("descriptions kept {with_desc}, with attributes {with_attrs}, styles {colors:?}");
    println!("{markers} placemarks, {paths} paths, {vertices} vertices in {elapsed:.2?}");
}
