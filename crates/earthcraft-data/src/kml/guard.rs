//! Prepare XML for kml 0.14: its Point reader assumes a coordinate and some
//! container loops do not stop at EOF. KmlReader owns all KML decoding.
use quick_xml::{
    Reader, Writer,
    events::{BytesStart, Event},
};

const MAX_XML_DEPTH: usize = 128;
pub(super) const FOLDER_ATTRIBUTE: &str = "earthcraft-prepared-folder";

struct Frame {
    tag: BytesStart<'static>,
    start: usize,
    coordinate_text: bool,
    has_coordinates: bool,
    synthetic: bool,
}
impl Frame {
    fn is(&self, name: &str) -> bool {
        self.tag.local_name().as_ref() == name
    }
}

fn start(
    tag: BytesStart<'_>,
    synthetic: bool,
    stack: &mut Vec<Frame>,
    writer: &mut Writer<Vec<u8>>,
) -> Result<(), crate::Error> {
    if stack.len() >= MAX_XML_DEPTH {
        return Err("KML nesting exceeds the supported depth".into());
    }
    let mut tag = tag.into_owned();
    // kml 0.14's Folder reader discards unknown feature fields, including
    // visibility. Its Document reader preserves them. Normalize only this
    // container and mark it so typed decoding can restore the original kind.
    if tag.local_name().as_ref() == "Folder" {
        tag.set_name("Document");
        tag.push_attribute((FOLDER_ATTRIBUTE, "1"));
    }
    let frame = Frame {
        tag,
        start: writer.get_ref().len(),
        coordinate_text: false,
        has_coordinates: false,
        synthetic,
    };
    writer.write_event(Event::Start(frame.tag.borrow()))?;
    stack.push(frame);
    Ok(())
}

fn close(stack: &mut Vec<Frame>, writer: &mut Writer<Vec<u8>>) -> Result<(), crate::Error> {
    let frame = stack
        .pop()
        .expect("close is only called for an open element");
    if frame.is("Point") && !frame.has_coordinates {
        writer.get_mut().truncate(frame.start);
    } else {
        writer.write_event(Event::End(frame.tag.to_end()))?;
    }
    if frame.is("coordinates")
        && let Some(point) = stack.last_mut().filter(|f| f.is("Point"))
    {
        point.has_coordinates = frame.coordinate_text;
        if point.synthetic {
            close(stack, writer)?;
        }
    }
    Ok(())
}

pub(super) fn prepare(txt: &str) -> Result<Vec<u8>, crate::Error> {
    let mut reader = Reader::from_str(txt);
    reader.config_mut().expand_empty_elements = true;
    let mut writer = Writer::new(Vec::new());
    let mut stack = Vec::new();
    loop {
        let event = reader.read_event()?;
        match event {
            Event::Start(tag) => {
                // Older EarthCraft files/fragments could omit the Point wrapper.
                if tag.local_name().as_ref() == "coordinates"
                    && stack.last().is_some_and(|f: &Frame| f.is("Placemark"))
                {
                    start(BytesStart::new("Point"), true, &mut stack, &mut writer)?;
                }
                start(tag, false, &mut stack, &mut writer)?;
            }
            Event::End(_) => {
                if stack.is_empty() {
                    return Err("KML contains an unexpected closing tag".into());
                }
                close(&mut stack, &mut writer)?;
            }
            Event::Eof => {
                // Keep incomplete outer envelopes importable, but do not accept
                // truncated field contents. Balanced output prevents EOF loops.
                if stack
                    .iter()
                    .any(|f| f.is("coordinates") || f.is("name") || f.is("description"))
                {
                    return Err("KML ends inside a text or coordinates field".into());
                }
                while !stack.is_empty() {
                    close(&mut stack, &mut writer)?;
                }
                return Ok(writer.into_inner());
            }
            event => {
                if let Some(field) = stack.last_mut().filter(|f| f.is("coordinates")) {
                    let nonempty = match &event {
                        Event::Text(text) => !text.trim().is_empty(),
                        Event::CData(text) => !text.trim().is_empty(),
                        Event::GeneralRef(reference) => reference
                            .resolve_char_ref()?
                            .is_none_or(|c| !c.is_whitespace()),
                        _ => false,
                    };
                    field.coordinate_text |= nonempty;
                }
                writer.write_event(event)?;
            }
        }
    }
}
