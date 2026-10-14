//! Keyless worldwide OSM road geometry; restrictions are not congestion reports.
use earthcraft_core::{
    camera::{Basis, Cam},
    math::ll_to_v3,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke};

#[derive(Default)]
pub(super) struct RoadOverlay {
    pub service: earthcraft_data::roads::LiveRoads,
}
impl RoadOverlay {
    pub fn draw(
        &self,
        painter: &egui::Painter,
        basis: &Basis,
        rect: Rect,
        camera: &Cam,
        pt: bool,
        scale: f32,
    ) {
        let hover = painter
            .ctx()
            .pointer_hover_pos()
            .filter(|p| rect.contains(*p));
        let mut nearest: Option<(&earthcraft_data::roads::Road, Pos2)> = None;
        let mut min_distance = 12.0 * scale;
        for road in self.service.snapshot(camera.lat, camera.lon, camera.alt) {
            let mut visible = Vec::with_capacity(road.points.len());
            for &(rlat, rlon) in &road.points {
                if let Some(pos) = basis.project(ll_to_v3(rlat, rlon), rect) {
                    visible.push(pos);
                } else {
                    draw_segment(painter, &visible, road, scale);
                    visible.clear();
                }
            }
            draw_segment(painter, &visible, road, scale);
            if let Some(mouse) = hover {
                for segment in visible.windows(2) {
                    let a = segment[0];
                    let b = segment[1];
                    let ab = b - a;
                    let factor = if ab.length_sq() > 0.01 {
                        ((mouse - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let near = a + ab * factor;
                    let distance = (near - mouse).length();
                    if distance < min_distance {
                        min_distance = distance;
                        nearest = Some((road, near));
                    }
                }
            }
        }
        if let Some((road, screen)) = nearest {
            let state = if road.restricted {
                if pt {
                    "Acesso restrito/obra registrado no OSM (não é congestionamento)"
                } else {
                    "OSM access restriction/construction (not a traffic jam)"
                }
            } else if pt {
                "Fluxo de veículos: desconhecido (sem dados ao vivo)"
            } else {
                "Vehicle flow: unknown (no live observations)"
            };
            let text = format!("{} • {}\n{}", road.name, road.kind, state);
            let galley =
                painter.layout_no_wrap(text, FontId::proportional(12.0 * scale), Color32::WHITE);
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
fn draw_segment(
    painter: &egui::Painter,
    points: &[Pos2],
    road: &earthcraft_data::roads::Road,
    scale: f32,
) {
    if points.len() < 2 {
        return;
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
    painter.add(egui::Shape::line(
        points.to_vec(),
        Stroke::new(width * scale, color),
    ));
}
