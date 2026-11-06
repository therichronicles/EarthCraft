fn parse_kml(xml: &str, lang: Lang) -> Result<Vec<Placemark>, crate::Error> {
    Ok(parse_tree(xml, lang)?.markers(false).cloned().collect())
}
fn places(marks: &[Placemark]) -> Places {
    Places {
        roots: marks.iter().cloned().map(PlaceNode::marker).collect(),
    }
}
use super::*;
#[test]
fn cdata_entities_fallback_names_and_incomplete_envelope() {
    let marks = parse_kml("<Placemark><description><![CDATA[A & B]]></description><Point><coordinates>-50,-15,0</coordinates></Point>", Lang::PtBr).unwrap();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].name, "Sem nome");
    assert_eq!(marks[0].desc, "A & B");
    let marks = parse_kml(
        "<Placemark><name>A &amp; B</name><coordinates>1,2</coordinates></Placemark>",
        Lang::En,
    )
    .unwrap();
    assert_eq!(marks[0].name, "A & B");
}

#[test]
fn kml_roundtrip_preserves_names_descriptions_and_coordinate_order() {
    let path = std::env::temp_dir().join(format!(
        "earthcraft-kml-{}-{}.kml",
        std::process::id(),
        earthcraft_core::math::now_unix()
    ));
    let marks = [Placemark {
        name: "São Paulo & <home>".into(),
        desc: "A & B > C".into(),
        lat: -23.5,
        lon: -46.6,
        ..Default::default()
    }];
    save_kml(path.to_str().unwrap(), &places(&marks)).unwrap();
    let loaded: Vec<_> = load_kml(path.to_str().unwrap(), Lang::PtBr)
        .unwrap()
        .markers(false)
        .cloned()
        .collect();
    std::fs::remove_file(path).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, marks[0].name);
    assert_eq!(loaded[0].desc, marks[0].desc);
    assert_eq!((loaded[0].lat, loaded[0].lon), (marks[0].lat, marks[0].lon));
}

#[test]
fn namespaced_nested_folders_keep_marker_order_and_localized_fallbacks() {
    let xml = r#"<k:kml xmlns:k="http://www.opengis.net/kml/2.2"><k:Document>
      <k:name>EarthCraft</k:name><k:Folder><k:name>Folder &amp; name</k:name>
        <k:Placemark><k:name> São Paulo &amp; &lt;home&gt; </k:name>
          <k:description> A &#38; B &amp;amp; C </k:description>
          <k:Point><k:coordinates>-46.6,-23.5,0</k:coordinates></k:Point>
        </k:Placemark>
        <k:Folder><k:Placemark><k:Point><k:coordinates>179.9,89.9</k:coordinates>
          <k:altitudeMode>clampToGround</k:altitudeMode></k:Point></k:Placemark></k:Folder>
      </k:Folder>
      <k:Placemark><k:name></k:name><k:Point><k:coordinates>-179.9,-89.9,10</k:coordinates></k:Point></k:Placemark>
    </k:Document></k:kml>"#;
    let actual = parse_kml(xml, Lang::PtBr).unwrap();
    assert_eq!(actual.len(), 3);
    assert_eq!(actual[0].name, "São Paulo & <home>");
    assert_eq!(actual[0].desc, "A & B &amp; C");
    assert_eq!((actual[0].lat, actual[0].lon), (-23.5, -46.6));
    assert_eq!(actual[1].name, "Sem nome");
    assert_eq!(actual[2].name, "");
}

#[test]
fn empty_points_are_skipped_without_losing_valid_siblings() {
    for coordinates in [
        "",
        "<coordinates/>",
        "<coordinates></coordinates>",
        "<coordinates> </coordinates>",
        "<coordinates><![CDATA[]]></coordinates>",
        "<coordinates><![CDATA[\u{2003}]]></coordinates>",
        "<coordinates>&#x20;&#9;</coordinates>",
        "<coordinates><!-- none --></coordinates>",
        "<coordinates>1,2</coordinates><coordinates/>",
    ] {
        let xml = format!(
            "<kml><Document><Placemark><Point>{coordinates}</Point></Placemark><Placemark><name>Valid</name><Point><coordinates>1,2,0</coordinates></Point></Placemark></Document></kml>"
        );
        let marks = parse_kml(&xml, Lang::En).unwrap();
        assert_eq!(marks.len(), 1, "{coordinates}");
        assert_eq!(marks[0].name, "Valid");
        assert_eq!((marks[0].lat, marks[0].lon), (2.0, 1.0));
    }
    let xml = "<Placemark><Point/></Placemark><Placemark><Point><coordinates><![CDATA[1,2]]></coordinates></Point></Placemark>";
    assert_eq!(parse_kml(xml, Lang::En).unwrap().len(), 1);
}

#[test]
fn incomplete_containers_are_completed_but_truncated_fields_are_errors() {
    for xml in [
        "<Placemark><Point><coordinates>1,2</coordinates></Point>",
        "<Placemark><Point><coordinates>1,2</coordinates>",
        "<kml><Document><Placemark><Point><coordinates>1,2</coordinates></Point></Placemark>",
        "<Placemark><coordinates>1,2</coordinates>",
    ] {
        let marks = parse_kml(xml, Lang::En).unwrap();
        assert_eq!(marks.len(), 1);
        assert_eq!((marks[0].lat, marks[0].lon), (2.0, 1.0));
    }
    for xml in [
        "<Placemark><Point><coordinates>1,2",
        "<Placemark><name>unfinished",
        "<Placemark><Point></wrong>",
    ] {
        assert!(parse_kml(xml, Lang::En).is_err(), "{xml}");
    }
}

#[test]
fn typed_geometry_uses_first_coordinate_without_adding_line_or_polygon_features() {
    for geometry in [
        "<LineString><coordinates>1,2,0 3,4,0</coordinates></LineString>",
        "<LinearRing><coordinates>1,2,0 3,4,0 1,2,0</coordinates></LinearRing>",
        "<Polygon><outerBoundaryIs><LinearRing><coordinates>1,2,0 3,4,0 1,2,0</coordinates></LinearRing></outerBoundaryIs></Polygon>",
        "<MultiGeometry><Point/><MultiGeometry><Point><coordinates>1,2</coordinates></Point><Point><coordinates>3,4</coordinates></Point></MultiGeometry></MultiGeometry>",
    ] {
        let xml = format!("<Placemark><name>Marker</name>{geometry}</Placemark>");
        let marks = parse_kml(&xml, Lang::En).unwrap();
        assert_eq!(marks.len(), 1, "{geometry}");
        assert_eq!((marks[0].lat, marks[0].lon), (2.0, 1.0));
    }
}

#[test]
fn library_errors_are_typed_and_cdata_is_literal_text() {
    let xml = "<Placemark><description><![CDATA[A &amp; B <b>bold</b>]]></description><Point><coordinates>1,2</coordinates></Point></Placemark>";
    assert_eq!(
        parse_kml(xml, Lang::En).unwrap()[0].desc,
        "A &amp; B <b>bold</b>"
    );
    for xml in [
        "<Placemark><Point><coordinates>bad,2</coordinates></Point></Placemark>",
        "<Placemark><Point><altitudeMode>custom-mode</altitudeMode><coordinates>1,2</coordinates></Point></Placemark>",
    ] {
        assert!(matches!(
            parse_kml(xml, Lang::En),
            Err(crate::Error::Kml(_))
        ));
    }
}

#[test]
fn deep_nesting_is_rejected_and_empty_imports_are_valid() {
    let xml = format!(
        "{}<Placemark><Point><coordinates>1,2</coordinates></Point></Placemark>{}",
        "<Folder>".repeat(150),
        "</Folder>".repeat(150)
    );
    assert!(
        parse_kml(&xml, Lang::En)
            .err()
            .unwrap()
            .to_string()
            .contains("nesting")
    );
    for empty in [
        "",
        "<kml><Document></Document></kml>",
        "<Placemark></Placemark>",
    ] {
        assert!(parse_kml(empty, Lang::En).unwrap().is_empty());
    }
}

#[test]
fn invalid_import_and_export_coordinates_use_shared_validation() {
    for coords in ["0,91", "181,0", "NaN,0"] {
        let xml =
            format!("<Placemark><Point><coordinates>{coords}</coordinates></Point></Placemark>");
        assert!(parse_kml(&xml, Lang::En).is_err(), "{coords}");
    }
    let path = std::env::temp_dir().join(format!(
        "earthcraft-invalid-coordinates-{}.kml",
        std::process::id()
    ));
    std::fs::write(&path, "keep original").unwrap();
    let pm = Placemark {
        name: "Invalid".into(),
        desc: String::new(),
        lat: 91.0,
        lon: 0.0,
        ..Default::default()
    };
    assert!(matches!(
        save_kml(path.to_str().unwrap(), &places(&[pm])),
        Err(crate::Error::Coordinates(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep original");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn tree_roundtrip_preserves_distinct_folders_visibility_and_empty_groups() {
    let xml = r#"<kml><Document><name>My places</name><description>A &amp; B</description>
      <Folder><name>Asia</name><visibility>0</visibility><Folder><name>Empty</name></Folder>
      <Placemark><name>Peak</name><visibility>1</visibility><Point><coordinates>86,27</coordinates></Point></Placemark></Folder>
      <Folder><name>Asia</name><Placemark><name>Other</name><visibility>0</visibility><Point><coordinates>87,28</coordinates></Point></Placemark></Folder>
    </Document></kml>"#;
    let tree = parse_tree(xml, Lang::En).unwrap();
    assert_eq!(tree.len(), 2);
    assert_eq!(tree.markers(true).count(), 0);
    let PlaceItem::Folder {
        name,
        description,
        children,
        document,
    } = &tree.roots[0].item
    else {
        panic!("document");
    };
    assert_eq!(name, "My places");
    assert_eq!(description, "A & B");
    assert!(document);
    assert_eq!(children.len(), 2);
    assert!(!children[0].visible);
    assert_ne!(children[0].id(), children[1].id());
    let PlaceItem::Folder {
        children: nested, ..
    } = &children[0].item
    else {
        panic!("folder");
    };
    assert!(matches!(&nested[0].item, PlaceItem::Folder { children, .. } if children.is_empty()));
    let path = std::env::temp_dir().join(format!("earthcraft-tree-{}.kml", std::process::id()));
    save_kml(path.to_str().unwrap(), &tree).unwrap();
    let restored = load_kml(path.to_str().unwrap(), Lang::En).unwrap();
    save_kml(path.to_str().unwrap(), &restored).unwrap();
    let serialized = std::fs::read_to_string(&path).unwrap();
    assert_eq!(serialized.matches("<Document").count(), 1);
    let restored = load_kml(path.to_str().unwrap(), Lang::En).unwrap();
    std::fs::remove_file(path).unwrap();
    let PlaceItem::Folder { children, .. } = &restored.roots[0].item else {
        panic!("document");
    };
    assert_eq!(children.len(), 2);
    assert!(!children[0].visible);
    assert_eq!(restored.markers(true).count(), 0);
    assert_eq!(
        restored
            .markers(false)
            .map(|pm| pm.name.as_str())
            .collect::<Vec<_>>(),
        ["Peak", "Other"]
    );
}

/// Small deterministic generator so the comparison needs no extra dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

fn text(rng: &mut Rng) -> String {
    const PARTS: [&str; 9] = [
        "Alpha", " padded ", "A &amp; B", "caf\u{e9}", "&lt;tag&gt;", "&#38;x", "", "two  spaces", "<![CDATA[raw &amp; <b>]]>",
    ];
    PARTS[rng.below(PARTS.len() as u64) as usize].to_owned()
}

fn field(rng: &mut Rng, p: &str, name: &str) -> String {
    match rng.below(5) {
        0 => String::new(),
        1 => format!("<{p}{name}/>"),
        _ => {
            let body = if name == "visibility" {
                ["0", "1", "false", "true", " 0 ", ""][rng.below(6) as usize].to_owned()
            } else {
                text(rng)
            };
            format!("<{p}{name}>{body}</{p}{name}>")
        }
    }
}

fn geometry(rng: &mut Rng, p: &str) -> String {
    let c = |rng: &mut Rng| {
        format!(
            "{:.4},{:.4},{}",
            rng.below(36000) as f64 / 100.0 - 180.0,
            rng.below(17000) as f64 / 100.0 - 85.0,
            rng.below(100)
        )
    };
    match rng.below(7) {
        0 => String::new(),
        1 => format!("<{p}Point/>"),
        2 | 3 => format!("<{p}Point><{p}coordinates>{}</{p}coordinates></{p}Point>", c(rng)),
        4 => format!(
            "<{p}LineString><{p}coordinates>{} {} {}</{p}coordinates></{p}LineString>",
            c(rng), c(rng), c(rng)
        ),
        5 => format!(
            "<{p}Polygon><{p}outerBoundaryIs><{p}LinearRing><{p}coordinates>{} {} {}</{p}coordinates></{p}LinearRing></{p}outerBoundaryIs></{p}Polygon>",
            c(rng), c(rng), c(rng)
        ),
        _ => format!(
            "<{p}MultiGeometry><{p}Point/><{p}Point><{p}coordinates>{}</{p}coordinates></{p}Point></{p}MultiGeometry>",
            c(rng)
        ),
    }
}

fn placemark(rng: &mut Rng, p: &str) -> String {
    let mut parts = vec![
        field(rng, p, "name"),
        field(rng, p, "description"),
        field(rng, p, "visibility"),
        geometry(rng, p),
    ];
    if rng.chance(20) {
        parts.push(format!("<{p}ExtendedData><{p}Data name=\"k\"><{p}value>v</{p}value></{p}Data></{p}ExtendedData>"));
    }
    if rng.chance(15) {
        parts.push("<!-- note -->".into());
    }
    // Field order varies, like real exports.
    for i in (1..parts.len()).rev() {
        parts.swap(i, rng.below(i as u64 + 1) as usize);
    }
    format!("<{p}Placemark>{}</{p}Placemark>", parts.concat())
}

fn container(rng: &mut Rng, p: &str, depth: u32) -> String {
    let tag = if rng.chance(50) { "Folder" } else { "Document" };
    let mut parts = vec![
        field(rng, p, "name"),
        field(rng, p, "description"),
        field(rng, p, "visibility"),
    ];
    // Repeated metadata: the first occurrence with text must win.
    for name in ["name", "description", "visibility"] {
        if rng.chance(35) {
            parts.push(field(rng, p, name));
        }
    }
    if rng.chance(30) {
        parts.push(format!("<{p}Style id=\"s\"><{p}IconStyle><{p}scale>1</{p}scale></{p}IconStyle></{p}Style>"));
    }
    if rng.chance(10) {
        parts.push(format!("<{p}NetworkLink><{p}name>skip</{p}name><{p}Placemark/></{p}NetworkLink>"));
    }
    for _ in 0..rng.below(5) {
        parts.push(if depth < 4 && rng.chance(30) {
            container(rng, p, depth + 1)
        } else {
            placemark(rng, p)
        });
    }
    for i in (1..parts.len()).rev() {
        parts.swap(i, rng.below(i as u64 + 1) as usize);
    }
    format!("<{p}{tag}>{}</{p}{tag}>", parts.concat())
}

fn document(rng: &mut Rng) -> String {
    let p = if rng.chance(25) { "k:" } else { "" };
    let ns = if p.is_empty() { "" } else { " xmlns:k=\"http://www.opengis.net/kml/2.2\"" };
    let body: String = (0..1 + rng.below(3))
        .map(|_| if rng.chance(70) { container(rng, p, 0) } else { placemark(rng, p) })
        .collect();
    match rng.below(4) {
        0 => body,
        _ => format!("<{p}kml{ns}>{body}</{p}kml>"),
    }
}

fn dump(nodes: &[PlaceNode], out: &mut String) {
    for node in nodes {
        match &node.item {
            PlaceItem::Marker(pm) => out.push_str(&format!(
                "M({:?},{:?},{},{},{});",
                pm.name, pm.desc, pm.lat, pm.lon, node.visible
            )),
            PlaceItem::Folder { name, description, document, children } => {
                out.push_str(&format!("F({name:?},{description:?},{document},{});[", node.visible));
                dump(children, out);
                out.push_str("]");
            }
        }
    }
}

#[test]
fn streaming_reader_matches_whole_document_decoding() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut compared = 0;
    for case in 0..3000 {
        let xml = document(&mut rng);
        let Ok(prepared) = guard::prepare(&xml) else { continue };
        let dom = parse_prepared_dom(prepared.clone(), Lang::En);
        let stream = stream::parse(&prepared, Lang::En);
        match (dom, stream) {
            (Ok(a), Ok(b)) => {
                let (mut x, mut y) = (String::new(), String::new());
                dump(&a.roots, &mut x);
                dump(&b.roots, &mut y);
                assert_eq!(x, y, "case {case}: {xml}");
                compared += 1;
            }
            (Err(_), Err(_)) => {}
            (a, b) => panic!("case {case}: dom ok={} stream ok={}: {xml}", a.is_ok(), b.is_ok()),
        }
    }
    assert!(compared > 2000, "only {compared} comparable documents");
}

#[test]
fn lines_and_outlines_are_imported_and_survive_a_save() {
    let xml = r#"<kml><Document>
      <Placemark><name>Fiber</name><LineString><coordinates>10,20,0 11,21,0 12,22,0</coordinates></LineString></Placemark>
      <Placemark><name>Area</name><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></Placemark>
      <Placemark><name>Bundle</name><MultiGeometry>
        <LineString><coordinates>1,1 2,2</coordinates></LineString>
        <LineString><coordinates>3,3 4,4 5,95 5,5</coordinates></LineString>
        <Point><coordinates>9,9</coordinates></Point></MultiGeometry></Placemark>
      <Placemark><name>Pin</name><Point><coordinates>5,6</coordinates></Point></Placemark>
    </Document></kml>"#;
    let tree = parse_tree(xml, Lang::En).unwrap();
    let marks: Vec<_> = tree.markers(false).cloned().collect();
    assert_eq!(marks.len(), 4);
    assert_eq!((marks[0].lat, marks[0].lon), (20.0, 10.0));
    assert_eq!(marks[0].paths.len(), 1);
    assert!(!marks[0].paths[0].is_closed());
    assert_eq!(marks[0].paths[0].points(), [(20.0, 10.0), (21.0, 11.0), (22.0, 12.0)]);
    assert!(marks[1].paths[0].is_closed());
    // Out-of-range vertices are dropped instead of failing the import.
    assert_eq!(marks[2].paths.len(), 2);
    assert_eq!(marks[2].paths[1].points().len(), 3);
    assert!(marks[3].paths.is_empty());

    let dir = std::env::temp_dir().join(format!("earthcraft-paths-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["paths.kml", "paths.kmz"] {
        let path = dir.join(name);
        save_kml(path.to_str().unwrap(), &tree).unwrap();
        let reloaded = load_kml(path.to_str().unwrap(), Lang::En).unwrap();
        let again: Vec<_> = reloaded.markers(false).cloned().collect();
        assert_eq!(again.len(), 4, "{name}");
        for (before, after) in marks.iter().zip(&again) {
            assert_eq!(before.paths, after.paths, "{name}: {}", before.name);
            assert_eq!((before.lat, before.lon), (after.lat, after.lon));
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
