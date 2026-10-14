use earthcraft_render::aircraft::{AircraftGeometry, ProjectedFlight};
use eframe::egui::{self, Align2, Color32, FontId, Shape, Stroke, Vec2};
use std::sync::OnceLock;

const TRAIL_WIDTH_POINTS: f32 = 1.2;
const LABEL_FONT_POINTS: f32 = 11.0;
const LABEL_OFFSET_POINTS: Vec2 = Vec2::new(9.0, -6.0);
const SHADOW_OFFSET_POINTS: Vec2 = Vec2::new(1.0, 1.0);
// The plane geometry is taken from the bundled SVG, not the former yellow diamond.
const AIRPLANE_SVG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/airplane.svg"
));
const SVG_CENTER_POINTS: f32 = 12.0;
const AIRPLANE_SCALE_POINTS: f32 = 0.78;
const AIRPLANE_OUTLINE_SCALE: f32 = 1.11;
const AIRPLANE_FILL: Color32 = Color32::from_rgb(243, 250, 255);
const AIRPLANE_OUTLINE: Color32 = Color32::from_rgb(22, 52, 72);
const LABEL_COLOR: Color32 = Color32::from_rgb(227, 245, 255);
const TRAIL_RGBA: [u8; 4] = [114, 211, 249, 90];

struct SvgAirplane {
    // Each SVG polygon is convex, allowing efficient, indexed triangle fans.
    polygons: Vec<Vec<Vec2>>,
}

fn airplane_symbol() -> &'static SvgAirplane {
    static SYMBOL: OnceLock<SvgAirplane> = OnceLock::new();
    SYMBOL.get_or_init(|| {
        let polygons = AIRPLANE_SVG
            .split("<polygon")
            .skip(1)
            .map(|element| {
                let (_, attributes) = element
                    .split_once("points=\"")
                    .expect("bundled airplane SVG must have polygon points");
                let (points, _) = attributes
                    .split_once('"')
                    .expect("bundled airplane SVG must close polygon points");
                let polygon: Vec<Vec2> = points
                    .split_ascii_whitespace()
                    .map(|pair| {
                        let (x, y) = pair.split_once(',').expect("bundled SVG point must be x,y");
                        Vec2::new(
                            x.parse().expect("SVG x must be a number"),
                            y.parse().expect("SVG y must be a number"),
                        )
                    })
                    .collect();
                assert!(
                    polygon.len() >= 3,
                    "airplane SVG polygons need three points"
                );
                polygon
            })
            .collect();
        SvgAirplane { polygons }
    })
}

fn add_airplane_mesh(
    mesh: &mut egui::Mesh,
    flight: &ProjectedFlight,
    map_scale: f32,
    symbol: &SvgAirplane,
) {
    // Existing projection already computes the direction of the aircraft on screen.
    // Reuse it so the SVG turns as the camera/globe turns, even near the poles.
    let forward = (flight.icon[0] - flight.position).normalized();
    let right = Vec2::new(-forward.y, forward.x);
    let icon_size = AIRPLANE_SCALE_POINTS * map_scale;

    // Draw the dark silhouette beneath all SVG parts, then the light silhouette.
    // This creates an exterior outline without outlines between overlapping wings.
    for (scale, color) in [
        (AIRPLANE_OUTLINE_SCALE, AIRPLANE_OUTLINE),
        (1.0, AIRPLANE_FILL),
    ] {
        for polygon in &symbol.polygons {
            let base = mesh.vertices.len() as u32;
            for point in polygon {
                let dx = (point.x - SVG_CENTER_POINTS) * icon_size * scale;
                let dy = (SVG_CENTER_POINTS - point.y) * icon_size * scale;
                mesh.colored_vertex(flight.position + right * dx + forward * dy, color);
            }
            for i in 1..polygon.len() - 1 {
                mesh.indices
                    .extend_from_slice(&[base, base + i as u32, base + i as u32 + 1]);
            }
        }
    }
}

pub(super) fn build(painter: &egui::Painter, geometry: &AircraftGeometry) -> Vec<Shape> {
    #[cfg(feature = "profiling")]
    let _profile = earthcraft_data::profiling::Span::new("ui.aircraft_shapes");
    let ms = geometry.view.map_scale;
    let [r, g, b, a] = TRAIL_RGBA;
    let trail_stroke = Stroke::new(
        TRAIL_WIDTH_POINTS * ms,
        Color32::from_rgba_unmultiplied(r, g, b, a),
    );
    let mut output = Vec::new();
    let mut trails = Vec::new();
    let mut planes = egui::Mesh::default();
    let symbol = airplane_symbol();
    let mut labels = geometry.labels.iter().copied().peekable();
    for (index, flight) in geometry.flights.iter().enumerate() {
        trails.push(Shape::line_segment(flight.trail, trail_stroke));
        add_airplane_mesh(&mut planes, flight, ms, symbol);
        if labels.peek() == Some(&index) {
            labels.next();
            flush_icons(painter, &mut trails, &mut planes, &mut output);
            let sample = &geometry.data.aircraft[flight.source_index];
            let code = if sample.callsign.is_empty() {
                sample.hex.to_uppercase()
            } else {
                sample.callsign.to_string()
            };
            let position = flight.position + LABEL_OFFSET_POINTS * ms;
            for (offset, color) in [
                (SHADOW_OFFSET_POINTS, Color32::BLACK),
                (Vec2::ZERO, LABEL_COLOR),
            ] {
                output.push(painter.fonts_mut(|fonts| {
                    Shape::text(
                        fonts,
                        position + offset,
                        Align2::LEFT_BOTTOM,
                        &code,
                        FontId::proportional(LABEL_FONT_POINTS * ms),
                        color,
                    )
                }));
            }
        }
    }
    flush_icons(painter, &mut trails, &mut planes, &mut output);
    output
}

fn flush_icons(
    painter: &egui::Painter,
    trails: &mut Vec<Shape>,
    planes: &mut egui::Mesh,
    output: &mut Vec<Shape>,
) {
    if !planes.indices.is_empty() {
        trails.push(Shape::mesh(std::mem::take(planes)));
    }
    if trails.is_empty() {
        return;
    }
    let clipped = std::mem::take(trails)
        .into_iter()
        .map(|shape| egui::epaint::ClippedShape {
            clip_rect: painter.clip_rect(),
            shape,
        })
        .collect();
    // Cache the planes/trails as meshes; retain label Galleys and original ordering.
    for primitive in painter
        .ctx()
        .tessellate(clipped, painter.ctx().pixels_per_point())
    {
        match primitive.primitive {
            egui::epaint::Primitive::Mesh(mesh) => output.push(Shape::mesh(mesh)),
            egui::epaint::Primitive::Callback(_) => {
                unreachable!("aircraft contain only polygons and lines")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_svg_contains_real_airplane_wings_and_fuselage() {
        let symbol = airplane_symbol();
        assert_eq!(symbol.polygons.len(), 5);
        assert!(symbol.polygons.iter().all(|polygon| polygon.len() >= 4));
        assert!(
            symbol
                .polygons
                .iter()
                .flatten()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
    }
}
