//! Read and write KML/KMZ places through the shared GeoRust KML pipeline.
use ::kml::{
    Kml, KmlReader, KmlWriter,
    types::{
        Coord, Element, Folder, Geometry, KmlDocument, KmlVersion, Placemark as KmlPlacemark, Point,
    },
};
use earthcraft_core::{
    language::Lang,
    placemark::{PlaceItem, PlaceNode, Placemark, Places},
};
use quick_xml::{
    Writer,
    events::{BytesDecl, Event},
};

mod guard;
mod kmz;

fn element(name: &str, content: &str) -> Kml<f64> {
    Kml::Element(Element {
        name: name.into(),
        content: Some(content.into()),
        ..Default::default()
    })
}
fn encode(node: &PlaceNode) -> Result<Kml<f64>, crate::Error> {
    let visibility = element("visibility", if node.visible { "1" } else { "0" });
    Ok(match &node.item {
        PlaceItem::Marker(pm) => {
            earthcraft_core::coordinates::validate_coordinates(pm.lat, pm.lon)?;
            let Kml::Element(visibility) = visibility else {
                unreachable!()
            };
            Kml::Placemark(KmlPlacemark {
                name: Some(pm.name.clone()),
                description: Some(pm.desc.clone()),
                geometry: Some(Geometry::Point(Point::new(pm.lon, pm.lat, Some(0.0)))),
                children: vec![visibility],
                ..Default::default()
            })
        }
        PlaceItem::Folder {
            name,
            description,
            document,
            children,
        } => {
            let children = children.iter().map(encode).collect::<Result<Vec<_>, _>>()?;
            if *document {
                Kml::Document {
                    attrs: Default::default(),
                    elements: [
                        element("name", name),
                        element("description", description),
                        visibility,
                    ]
                    .into_iter()
                    .chain(children)
                    .collect(),
                }
            } else {
                Kml::Folder(Folder {
                    name: Some(name.clone()),
                    description: Some(description.clone()),
                    elements: std::iter::once(visibility).chain(children).collect(),
                    ..Default::default()
                })
            }
        }
    })
}
pub fn save_kml(path: &str, places: &Places) -> Result<(), crate::Error> {
    let mut elements = places
        .roots
        .iter()
        .map(encode)
        .collect::<Result<Vec<_>, _>>()?;
    if !matches!(elements.as_slice(), [Kml::Document { .. }]) {
        elements = vec![Kml::Document {
            attrs: Default::default(),
            elements: std::iter::once(element("name", "EarthCraft"))
                .chain(elements)
                .collect(),
        }];
    }
    let document = Kml::KmlDocument(KmlDocument {
        version: KmlVersion::V22,
        attrs: [("xmlns".into(), "http://www.opengis.net/kml/2.2".into())].into(),
        elements,
    });
    // Validate and encode fully before touching the destination.
    let mut bytes = Vec::new();
    let mut xml = Writer::new(&mut bytes);
    xml.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    drop(xml);
    KmlWriter::from_writer(&mut bytes).write(&document)?;
    let bytes = if is_kmz(path) {
        kmz::encode(&bytes)?
    } else {
        bytes
    };
    Ok(std::fs::write(path, bytes)?)
}
fn metadata(elements: &[Kml<f64>], field: &str) -> Option<String> {
    elements.iter().find_map(|entry| match entry {
        Kml::Element(element) if element.name == field => element.content.clone(),
        _ => None,
    })
}
fn visible(value: Option<&str>) -> bool {
    !matches!(value.map(str::trim), Some("0" | "false"))
}
fn collect(document: Kml<f64>, lg: Lang, out: &mut Vec<PlaceNode>) -> Result<(), crate::Error> {
    match document {
        Kml::KmlDocument(doc) => {
            for element in doc.elements {
                collect(element, lg, out)?;
            }
        }
        Kml::Document { elements, attrs } => {
            let name = metadata(&elements, "name").unwrap_or_else(|| {
                lg.text(if attrs.contains_key(guard::FOLDER_ATTRIBUTE) {
                    "places-folder-name"
                } else {
                    "places-document-name"
                })
                .into()
            });
            let description = metadata(&elements, "description").unwrap_or_default();
            let visibility = visible(metadata(&elements, "visibility").as_deref());
            let mut children = Vec::new();
            for element in elements {
                collect(element, lg, &mut children)?;
            }
            let mut node = PlaceNode::folder(
                name,
                description,
                !attrs.contains_key(guard::FOLDER_ATTRIBUTE),
                children,
            );
            node.visible = visibility;
            out.push(node);
        }
        Kml::Folder(folder) => {
            let visibility = visible(metadata(&folder.elements, "visibility").as_deref());
            let mut children = Vec::new();
            for element in folder.elements {
                collect(element, lg, &mut children)?;
            }
            let mut node = PlaceNode::folder(
                folder
                    .name
                    .unwrap_or_else(|| lg.text("places-folder-name").into()),
                folder.description.unwrap_or_default(),
                false,
                children,
            );
            node.visible = visibility;
            out.push(node);
        }
        Kml::Placemark(pm) => {
            if let Some(coord) = pm.geometry.as_ref().and_then(first_coordinate) {
                let (lat, lon) =
                    earthcraft_core::coordinates::validate_coordinates(coord.y, coord.x)?;
                let visibility = visible(
                    pm.children
                        .iter()
                        .find(|entry| entry.name == "visibility")
                        .and_then(|entry| entry.content.as_deref()),
                );
                let mut node = PlaceNode::marker(Placemark {
                    name: pm
                        .name
                        .map(|s| s.trim().to_owned())
                        .unwrap_or_else(|| lg.text("placemarks-fallback-name").into()),
                    desc: pm.description.unwrap_or_default().trim().to_owned(),
                    lat,
                    lon,
                });
                node.visible = visibility;
                out.push(node);
            }
        }
        _ => {}
    }
    Ok(())
}

// EarthCraft imports one marker per Placemark, at its first coordinate. Decoding
// more KML geometry types does not add polygon/line rendering to the application.
fn first_coordinate(geometry: &Geometry<f64>) -> Option<Coord<f64>> {
    match geometry {
        Geometry::Point(point) => Some(point.coord),
        Geometry::LineString(line) => line.coords.first().copied(),
        Geometry::LinearRing(ring) => ring.coords.first().copied(),
        Geometry::Polygon(polygon) => polygon.outer.coords.first().copied(),
        Geometry::MultiGeometry(collection) => {
            collection.geometries.iter().find_map(first_coordinate)
        }
        _ => None,
    }
}

fn parse_tree(txt: &str, lg: Lang) -> Result<Places, crate::Error> {
    let xml = guard::prepare(txt)?;
    let document = match KmlReader::<_, f64>::from_reader(xml.as_slice()).read() {
        Ok(document) => document,
        Err(::kml::Error::NoElements) => return Ok(Places::default()),
        Err(error) => return Err(error.into()),
    };
    places_from_document(document, lg)
}

fn places_from_document(document: Kml<f64>, lg: Lang) -> Result<Places, crate::Error> {
    let mut out = Vec::new();
    collect(document, lg, &mut out)?;
    Ok(Places { roots: out })
}

fn is_kmz(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("kmz"))
}

pub fn load_kml(path: &str, lg: Lang) -> Result<Places, crate::Error> {
    if is_kmz(path) {
        let mut reader = KmlReader::<_, f64>::from_kmz_path(path).map_err(|error| match error {
            // Preserve NotFound for startup pruning of missing KMZ sources.
            ::kml::Error::IoError(error) => crate::Error::Io(error),
            error => error.into(),
        })?;
        places_from_document(reader.read()?, lg)
    } else {
        parse_tree(&std::fs::read_to_string(path)?, lg)
    }
}

#[cfg(test)]
mod tests;
