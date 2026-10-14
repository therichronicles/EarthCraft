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
