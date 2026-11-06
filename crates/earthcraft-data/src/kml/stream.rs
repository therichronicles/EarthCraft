//! Memory-bounded KML import. The guarded XML from `guard::prepare` is walked
//! once; container structure (kml, Document, Folder) is tracked here and each
//! Placemark or metadata element is decoded on its own by the GeoRust reader.
//! Only the current element is ever materialized, never the document, and
//! subtrees EarthCraft does not import (styles, overlays, schemas) are skipped.
//! Behavior matches decoding the whole document, except that malformed content
//! inside skipped subtrees no longer fails the import.
use super::{collect, guard, visible};
use ::kml::{Kml, KmlReader};
use earthcraft_core::{
    language::Lang,
    placemark::{PlaceNode, Places},
};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};

/// An open `kml` envelope (transparent) or `Document`/`Folder` (a place folder).
struct Frame {
    envelope: bool,
    folder: bool,
    name: Option<String>,
    description: Option<String>,
    visibility: Option<String>,
    children: Vec<PlaceNode>,
}
impl Frame {
    fn new(envelope: bool, folder: bool) -> Self {
        Self {
            envelope,
            folder,
            name: None,
            description: None,
            visibility: None,
            children: Vec::new(),
        }
    }
    fn finish(self, lg: Lang) -> Vec<PlaceNode> {
        if self.envelope {
            return self.children;
        }
        let name = self.name.unwrap_or_else(|| {
            lg.text(if self.folder {
                "places-folder-name"
            } else {
                "places-document-name"
            })
            .into()
        });
        let mut node = PlaceNode::folder(
            name,
            self.description.unwrap_or_default(),
            !self.folder,
            self.children,
        );
        node.visible = visible(self.visibility.as_deref());
        vec![node]
    }
}

fn decode(xml: &[u8]) -> Result<Kml<f64>, crate::Error> {
    Ok(KmlReader::<_, f64>::from_reader(xml).read()?)
}

fn open(tag: &BytesStart<'_>) -> Frame {
    let folder = tag
        .attributes()
        .flatten()
        .any(|attribute| attribute.key.as_ref() == guard::FOLDER_ATTRIBUTE);
    Frame::new(tag.local_name().as_ref() == "kml", folder)
}

pub(super) fn parse(xml: &[u8], lg: Lang) -> Result<Places, crate::Error> {
    let mut reader = Reader::from_reader(xml);
    let mut stack: Vec<Frame> = Vec::new();
    let mut roots: Vec<PlaceNode> = Vec::new();
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event()? {
            Event::Start(tag) => match tag.local_name().as_ref() {
                "kml" | "Document" => stack.push(open(&tag)),
                name @ ("Placemark" | "name" | "description" | "visibility") => {
                    reader.read_to_end(tag.name())?;
                    // The reader now sits just past the closing tag.
                    let element = decode(&xml[start..reader.buffer_position() as usize])?;
                    let siblings = stack.last_mut();
                    match (name, element, siblings) {
                        ("Placemark", element, Some(frame)) => {
                            collect(element, lg, &mut frame.children)?
                        }
                        ("Placemark", element, None) => collect(element, lg, &mut roots)?,
                        (field, Kml::Element(element), Some(frame)) if !frame.envelope => {
                            let slot = match field {
                                "name" => &mut frame.name,
                                "description" => &mut frame.description,
                                _ => &mut frame.visibility,
                            };
                            // The first occurrence that has text wins.
                            if slot.is_none() {
                                *slot = element.content;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {
                    reader.read_to_end(tag.name())?;
                }
            },
            Event::End(_) => {
                if let Some(frame) = stack.pop() {
                    let nodes = frame.finish(lg);
                    stack.last_mut().map_or(&mut roots, |p| &mut p.children).extend(nodes);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    while let Some(frame) = stack.pop() {
        let nodes = frame.finish(lg);
        stack.last_mut().map_or(&mut roots, |p| &mut p.children).extend(nodes);
    }
    Ok(Places { roots })
}
