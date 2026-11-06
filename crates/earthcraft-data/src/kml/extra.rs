//! Attributes, labels and line styles that accompany placemark geometry.
use ::kml::types::{Element, Style, StyleMap};
use earthcraft_core::placemark::LineStyle;
use std::collections::HashMap;

/// Line styles declared in a document, resolved by `styleUrl`.
#[derive(Default)]
pub(super) struct Styles {
    lines: HashMap<String, LineStyle>,
    maps: HashMap<String, String>,
}
impl Styles {
    pub(super) fn add_style(&mut self, style: &Style) {
        if let (Some(id), Some(line)) = (&style.id, &style.line)
            && let Some(parsed) = line_style(&line.color, line.width)
        {
            self.lines.insert(id.clone(), parsed);
        }
    }
    /// A style map resolves to its `normal` pair.
    pub(super) fn add_map(&mut self, map: &StyleMap) {
        if let (Some(id), Some(pair)) = (&map.id, map.pairs.iter().find(|p| p.key == "normal")) {
            self.maps.insert(id.clone(), pair.style_url.clone());
        }
    }
    pub(super) fn resolve(&self, url: &str) -> Option<LineStyle> {
        let id = url.trim().trim_start_matches('#');
        let id = self.maps.get(id).map_or(id, |next| next.trim_start_matches('#'));
        self.lines.get(id).copied()
    }
}

/// KML colors are `aabbggrr` hexadecimal.
fn line_style(color: &str, width: f64) -> Option<LineStyle> {
    let value = u32::from_str_radix(color.trim(), 16).ok()?;
    let [alpha, blue, green, red] = value.to_be_bytes();
    (width.is_finite() && width > 0.0).then_some(LineStyle {
        rgba: [red, green, blue, alpha],
        width: width as f32,
    })
}

/// The color and width of a placemark's own `<Style><LineStyle>`.
pub(super) fn inline_style(children: &[Element]) -> Option<LineStyle> {
    let line = children
        .iter()
        .find(|e| e.name == "Style")?
        .children
        .iter()
        .find(|e| e.name == "LineStyle")?;
    let field = |name: &str| line.children.iter().find(|e| e.name == name)?.content.as_deref();
    line_style(
        field("color").unwrap_or("ffffffff"),
        field("width").and_then(|w| w.trim().parse().ok()).unwrap_or(1.0),
    )
}

/// Fields from `ExtendedData`: `SimpleData name=".."` and `Data name=".."`.
pub(super) fn attributes(children: &[Element]) -> Vec<(String, String)> {
    fn walk(element: &Element, out: &mut Vec<(String, String)>) {
        let named = element.attrs.get("name").filter(|n| !n.is_empty());
        match (element.name.as_str(), named) {
            ("SimpleData", Some(name)) => {
                let value = element.content.as_deref().unwrap_or_default().trim();
                out.push((name.clone(), value.to_owned()));
            }
            ("Data", Some(name)) => {
                let value = element
                    .children
                    .iter()
                    .find(|e| e.name == "value")
                    .and_then(|e| e.content.as_deref())
                    .or(element.content.as_deref())
                    .unwrap_or_default()
                    .trim();
                out.push((name.clone(), value.to_owned()));
            }
            _ => element.children.iter().for_each(|child| walk(child, out)),
        }
    }
    let mut out = Vec::new();
    for element in children.iter().filter(|e| e.name == "ExtendedData") {
        walk(element, &mut out);
    }
    out
}

/// Exporters such as FME name every feature `kml_1`, `kml_2`, ... Such a name,
/// or none, says nothing, so a `LABEL` field is a better title.
pub(super) fn is_generic_name(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return true;
    }
    ["kml", "placemark"].iter().any(|prefix| {
        lower.strip_prefix(prefix).is_some_and(|rest| {
            let digits = rest.trim_start_matches(['_', '-', ' ']);
            !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
        })
    })
}

/// The first non-empty line of a `LABEL` field.
pub(super) fn label(attributes: &[(String, String)]) -> Option<String> {
    let (_, value) = attributes.iter().find(|(key, _)| key.eq_ignore_ascii_case("label"))?;
    value.lines().map(str::trim).find(|line| !line.is_empty()).map(str::to_owned)
}

fn plain_tokens(html: &str) -> Vec<String> {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                text.push(' ');
            }
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.split_whitespace().map(str::to_owned).collect()
}

/// True when a description is only an HTML table of the placemark's own
/// attributes, which the attribute list already shows. Rows for empty fields
/// may appear in the table without a matching attribute.
pub(super) fn description_repeats(description: &str, attributes: &[(String, String)]) -> bool {
    if attributes.is_empty() || !description.contains("<tr") {
        return false;
    }
    let expected: Vec<Vec<String>> = attributes
        .iter()
        .map(|(key, value)| {
            key.split_whitespace()
                .chain(value.split_whitespace())
                .map(str::to_owned)
                .collect()
        })
        .collect();
    let mut covered = vec![false; expected.len()];
    for row in description.split("</tr>") {
        let tokens = plain_tokens(row);
        if tokens.is_empty() || tokens == ["Attributes"] {
            continue;
        }
        if let Some(index) = expected.iter().position(|e| *e == tokens) {
            covered[index] = true;
        } else {
            let compact: String = row.split_whitespace().collect();
            if !compact.contains("<td></td>") {
                return false;
            }
        }
    }
    covered.iter().all(|&done| done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_read_as_aabbggrr() {
        let style = line_style("ff0000ff", 2.0).unwrap();
        assert_eq!(style.rgba, [255, 0, 0, 255]);
        let style = line_style("80ff8000", 3.0).unwrap();
        assert_eq!(style.rgba, [0, 128, 255, 128]);
        assert!(line_style("zz", 1.0).is_none());
        assert!(line_style("ff0000ff", 0.0).is_none());
    }

    #[test]
    fn generic_names_are_recognized() {
        for name in ["", "  ", "kml_1", "KML_22", "Placemark 3", "placemark-4"] {
            assert!(is_generic_name(name), "{name:?}");
        }
        for name in ["kml", "kml_x", "Main St", "Placemark A", "kml_1a"] {
            assert!(!is_generic_name(name), "{name:?}");
        }
    }

    #[test]
    fn label_uses_the_first_line() {
        let attributes = vec![("LABEL".to_owned(), "\n 72-BFO-2665'\nXD,1-24".to_owned())];
        assert_eq!(label(&attributes).as_deref(), Some("72-BFO-2665'"));
        assert_eq!(label(&[("OWNER".into(), "x".into())]), None);
    }

    #[test]
    fn only_an_attribute_table_counts_as_a_repeat() {
        let attributes = vec![
            ("OWNER".to_owned(), "HICKORYTECH CLEC".to_owned()),
            ("F_Q".to_owned(), "2665".to_owned()),
        ];
        let table = "<center><table><tr><th><em>Attributes</em></th></tr><tr><th>OWNER</th><td>HICKORYTECH CLEC</td></tr><tr><th>F_Q</th><td>2665</td></tr></table></center>";
        assert!(description_repeats(table, &attributes));
        assert!(!description_repeats("<b>Notes</b> from the field", &attributes));
        assert!(!description_repeats("OWNER HICKORYTECH CLEC F_Q 2665", &attributes));
        assert!(!description_repeats(table, &[]));
        // A row for an empty field has no matching attribute.
        let with_empty = table.replace("</table>", "<tr><th>LMS_ROUTE</th><td></td></tr></table>");
        assert!(description_repeats(&with_empty, &attributes));
        // A row with a value the attributes lack is real information.
        let extra = table.replace("</table>", "<tr><th>NOTE</th><td>call first</td></tr></table>");
        assert!(!description_repeats(&extra, &attributes));
    }
}
