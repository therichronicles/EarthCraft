//! Keyless worldwide OSM road geometry; restrictions are not congestion reports.
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct RoadOverlay {
    pub service: earthcraft_data::roads::LiveRoads,
    projected: Option<Arc<Vec<earthcraft_render::live_layers::ProjectedRoad>>>,
    shapes: Vec<egui::Shape>,
}
impl RoadOverlay {
    pub(super) fn invalidate_text(&mut self) {
        self.projected = None;
    }
    pub fn draw(
        &mut self,
        painter: &egui::Painter,
        geometry: &earthcraft_render::live_layers::LiveGeometry,
        lang: earthcraft_core::language::Lang,
        scale: f32,
    ) {
        let rect = geometry.input.view.rect;
        let Some(roads) = &geometry.input.roads else {
            return;
        };
        let hover = painter
            .ctx()
            .pointer_hover_pos()
            .filter(|p| rect.contains(*p));
        if self
            .projected
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &geometry.roads))
        {
            let shapes = geometry
                .roads
                .iter()
                .flat_map(|projected| {
                    projected.segments.iter().filter_map(|line| {
                        segment_shape(line, &roads[projected.source_index], scale)
                    })
                })
                .collect();
            self.shapes = super::live_geometry::mesh_shapes(painter, shapes);
            self.projected = Some(geometry.roads.clone());
        }
        painter.extend(self.shapes.iter().cloned());
        let nearest = hover
            .and_then(|mouse| geometry.nearest_road(mouse, 12.0 * scale))
            .map(|(index, point)| (&roads[index], point));
        if let Some((road, screen)) = nearest {
            let state = lang.text(if road.restricted {
                "roads-restricted-help"
            } else {
                "roads-flow-unknown-help"
            });
            let text = lang.format(
                "roads-hover-tooltip",
                &[
                    (
                        "name",
                        if road.name.is_empty() {
                            lang.text("roads-unnamed-label")
                        } else {
                            road.name.as_str()
                        }
                        .into(),
                    ),
                    ("kind", road.kind.as_str().into()),
                    ("state", state.into()),
                ],
            );
            let galley = painter.layout_no_wrap(
                text,
                crate::app::fonts::map_font(painter.ctx(), 12.0 * scale),
                Color32::WHITE,
            );
            let pos = Pos2::new(
                (screen.x + 16.0)
                    .min(rect.right() - galley.size().x - 10.0)
                    .max(rect.left() + 5.0),
                (screen.y + 12.0)
                    .min(rect.bottom() - galley.size().y - 10.0)
                    .max(rect.top() + 5.0),
            );
            painter.rect_filled(
                Rect::from_min_size(pos, galley.size()).expand(5.0),
                5.0,
                Color32::from_rgba_unmultiplied(9, 19, 30, 230),
            );
            painter.galley(pos, galley, Color32::WHITE);
        }
    }
}
fn segment_shape(
    points: &[Pos2],
    road: &earthcraft_data::roads::Road,
    scale: f32,
) -> Option<egui::Shape> {
    if points.len() < 2 {
        return None;
    }
    let major = matches!(
        road.kind.as_str(),
        "motorway" | "trunk" | "primary" | "secondary"
    );
    let width = if major { 2.5 } else { 1.5 };
    // Neutral gray means unknown, not free-flowing. Red means documented restriction/works.
    let color = if road.restricted {
        Color32::from_rgb(240, 83, 70)
    } else if major {
        Color32::from_rgba_unmultiplied(205, 209, 222, 212)
    } else {
        Color32::from_rgba_unmultiplied(165, 174, 191, 165)
    };
    Some(egui::Shape::line(
        points.to_vec(),
        Stroke::new(width * scale, color),
    ))
}
