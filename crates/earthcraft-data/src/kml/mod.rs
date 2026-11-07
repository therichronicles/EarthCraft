//! Read and write KML/KMZ places through the shared GeoRust KML pipeline.
use ::kml::{
    Kml, KmlWriter,
    types::{
        Coord, Element, Folder, Geometry, KmlDocument, KmlVersion, LineString, LinearRing,
        Placemark as KmlPlacemark, Point, Polygon,
    },
};
use earthcraft_core::{
    language::Lang,
    placemark::{GeoPath, PlaceItem, PlaceNode, Placemark, Places},
};
use quick_xml::{
    Writer,
    events::{BytesDecl, Event},
};

mod extra;
mod guard;
mod kmz;
mod stream;
use extra::Styles;

fn element(name: &str, content: &str) -> Kml<f64> {
    Kml::Element(Element {
        name: name.into(),
        content: Some(content.into()),
        ..Default::default()
    })
}
/// `visibility`, the attributes as `ExtendedData`, and the line color as an
/// inline `Style`, so a saved file keeps what the import read.
fn marker_children(visibility: Element, pm: &Placemark) -> Vec<Element> {
    let text = |name: &str, content: &str| Element {
        name: name.into(),
        content: Some(content.into()),
        ..Default::default()
    };
    let mut children = vec![visibility];
    if !pm.attributes.is_empty() {
        children.push(Element {
            name: "ExtendedData".into(),
            children: pm
                .attributes
                .iter()
                .map(|(key, value)| Element {
                    name: "Data".into(),
                    attrs: [("name".to_owned(), key.clone())].into(),
                    children: vec![text("value", value)],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        });
    }
    if let Some(style) = pm.line_style {
        let [red, green, blue, alpha] = style.rgba;
        children.push(Element {
            name: "Style".into(),
            children: vec![Element {
                name: "LineStyle".into(),
                children: vec![
                    text(
                        "color",
                        &format!("{alpha:02x}{blue:02x}{green:02x}{red:02x}"),
                    ),
                    text("width", &style.width.to_string()),
                ],
                ..Default::default()
            }],
            ..Default::default()
        });
    }
    children
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
                geometry: Some(match pm.paths.as_slice() {
                    [] => Geometry::Point(Point::new(pm.lon, pm.lat, Some(0.0))),
                    [path] => path_geometry(path),
                    paths => Geometry::MultiGeometry(::kml::types::MultiGeometry::new(
                        paths.iter().map(path_geometry).collect(),
                    )),
                }),
                children: marker_children(visibility, pm),
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
fn collect(
    document: Kml<f64>,
    lg: Lang,
    styles: &Styles,
    out: &mut Vec<PlaceNode>,
) -> Result<(), crate::Error> {
    match document {
        Kml::KmlDocument(doc) => {
            for element in doc.elements {
                collect(element, lg, styles, out)?;
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
                collect(element, lg, styles, &mut children)?;
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
                collect(element, lg, styles, &mut children)?;
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
                let attributes = extra::attributes(&pm.children);
                let titled = pm.name.as_deref().is_none_or(extra::is_generic_name);
                let name = match extra::label(&attributes) {
                    Some(label) if titled => label,
                    _ => pm
                        .name
                        .map(|s| s.trim().to_owned())
                        .unwrap_or_else(|| lg.text("placemarks-fallback-name").into()),
                };
                let desc = pm.description.unwrap_or_default().trim().to_owned();
                let desc = if extra::description_repeats(&desc, &attributes) {
                    String::new()
                } else {
                    desc
                };
                let line_style = extra::inline_style(&pm.children)
                    .or_else(|| pm.style_url.as_deref().and_then(|url| styles.resolve(url)));
                let mut node = PlaceNode::marker(Placemark {
                    name,
                    desc,
                    lat,
                    lon,
                    paths: pm.geometry.as_ref().map(geometry_paths).unwrap_or_default(),
                    attributes,
                    line_style,
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

/// Lines and outlines a placemark draws. Points contribute none, and vertices
/// outside the valid coordinate range are dropped rather than failing the import.
fn geometry_paths(geometry: &Geometry<f64>) -> Vec<GeoPath> {
    fn ring(closed: bool, coords: &[Coord<f64>]) -> Option<GeoPath> {
        GeoPath::new(closed, coords.iter().map(|c| (c.y, c.x)))
    }
    match geometry {
        Geometry::LineString(line) => ring(false, &line.coords).into_iter().collect(),
        Geometry::LinearRing(r) => ring(true, &r.coords).into_iter().collect(),
        Geometry::Polygon(polygon) => std::iter::once(&polygon.outer)
            .chain(&polygon.inner)
            .filter_map(|r| ring(true, &r.coords))
            .collect(),
        Geometry::MultiGeometry(collection) => collection
            .geometries
            .iter()
            .flat_map(geometry_paths)
            .collect(),
        _ => Vec::new(),
    }
}

fn path_geometry(path: &GeoPath) -> Geometry<f64> {
    let coords: Vec<Coord<f64>> = path
        .points()
        .iter()
        .map(|&(lat, lon)| Coord::new(lon, lat, None))
        .collect();
    if path.is_closed() {
        Geometry::Polygon(Polygon::new(LinearRing::from(coords), Vec::new()))
    } else {
        Geometry::LineString(LineString::from(coords))
    }
}

#[cfg(test)]
fn parse_tree(txt: &str, lg: Lang) -> Result<Places, crate::Error> {
    parse_prepared(guard::prepare(txt)?, lg)
}

/// Like `parse_tree`, but frees the source text as soon as the prepared copy
/// exists so a large import does not hold both for the whole parse.
fn parse_owned(txt: String, lg: Lang) -> Result<Places, crate::Error> {
    let xml = guard::prepare(&txt)?;
    drop(txt);
    parse_prepared(xml, lg)
}

fn parse_prepared(xml: Vec<u8>, lg: Lang) -> Result<Places, crate::Error> {
    // Placemarks are decoded one at a time, so the whole document is never held
    // as a DOM beside the prepared text.
    stream::parse(&xml, lg)
}

/// The original whole-document decoder, kept to check the streaming reader.
#[cfg(test)]
fn parse_prepared_dom(xml: Vec<u8>, lg: Lang) -> Result<Places, crate::Error> {
    use ::kml::KmlReader;
    let document = match KmlReader::<_, f64>::from_reader(xml.as_slice()).read() {
        Ok(document) => document,
        Err(::kml::Error::NoElements) => return Ok(Places::default()),
        Err(error) => return Err(error.into()),
    };
    drop(xml);
    places_from_document(document, lg)
}

#[cfg(test)]
fn places_from_document(document: Kml<f64>, lg: Lang) -> Result<Places, crate::Error> {
    let mut out = Vec::new();
    collect(document, lg, &Styles::default(), &mut out)?;
    Ok(Places { roots: out })
}

fn is_kmz(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("kmz"))
}

pub fn load_kml(path: &str, lg: Lang) -> Result<Places, crate::Error> {
    // Both formats pass through the guard: kml 0.14 can panic, loop or recurse
    // without bound on unprepared XML. Missing files stay `Error::Io(NotFound)`
    // for startup pruning of missing sources.
    let txt = if is_kmz(path) {
        kmz::decode(path)?
    } else {
        std::fs::read_to_string(path)?
    };
    parse_owned(txt, lg)
}

#[cfg(test)]
mod tests;
