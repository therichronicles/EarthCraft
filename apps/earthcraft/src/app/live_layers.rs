use super::App;
use earthcraft_core::{camera::Basis, math::now_unix};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Stroke, Vec2};
use std::time::Duration;

const FLIGHT_REPAINT_INTERVAL: Duration = Duration::from_secs(1);
const FLIGHT_LABEL_MAX_ALTITUDE_KM: f64 = 650.0;
const FLIGHT_LABEL_MIN_SEPARATION_PX: f32 = 46.0;
const FLIGHT_HOVER_RADIUS_PX: f32 = 15.0;
const MAX_FLIGHT_LABELS: usize = 110;

impl App {
    pub(super) fn draw_live_layers(
        &self,
        painter: &egui::Painter,
        basis: &Basis,
        rect: Rect,
        altitude_km: f64,
    ) {
        if !self.show_flights {
            return;
        }
        self.draw_flight_overlay(painter, basis, rect, altitude_km);
        painter.ctx().request_repaint_after(FLIGHT_REPAINT_INTERVAL);
    }

    fn draw_flight_overlay(
        &self,
        painter: &egui::Painter,
        basis: &Basis,
        rect: Rect,
        altitude_km: f64,
    ) {
        let planes = self.live_aircraft.snapshot();
        let projected = earthcraft_render::aircraft::project_flights(
            basis,
            rect,
            &planes,
            now_unix(),
            self.settings.map_scale,
        );
        let ms = self.settings.map_scale;
        let hover = painter
            .ctx()
            .pointer_hover_pos()
            .filter(|p| rect.contains(*p));
        let mut nearest = None;
        let mut min_distance_sq = (FLIGHT_HOVER_RADIUS_PX * ms).powi(2);
        let mut label_positions: Vec<egui::Pos2> = Vec::new();

        for flight in &projected {
            let screen = flight.position;
            painter.line_segment(
                flight.trail,
                Stroke::new(1.2 * ms, Color32::from_rgba_unmultiplied(255, 197, 33, 95)),
            );
            painter.add(egui::Shape::convex_polygon(
                flight.icon.to_vec(),
                Color32::from_rgb(255, 206, 45),
                Stroke::new(0.9 * ms, Color32::from_rgb(36, 38, 39)),
            ));

            if let Some(pos) = hover {
                let dsq = (pos - screen).length_sq();
                if dsq < min_distance_sq {
                    min_distance_sq = dsq;
                    nearest = Some(flight);
                }
            }

            // Dense global views would otherwise produce thousands of overlapping
            // text labels. All aircraft remain visible as icons at every zoom.
            if altitude_km <= FLIGHT_LABEL_MAX_ALTITUDE_KM
                && label_positions.len() < MAX_FLIGHT_LABELS
                && label_positions
                    .iter()
                    .all(|other| (*other - screen).length() > FLIGHT_LABEL_MIN_SEPARATION_PX * ms)
            {
                label_positions.push(screen);
                let tp = screen + Vec2::new(9.0 * ms, -6.0 * ms);
                painter.text(
                    tp + Vec2::new(1.0, 1.0),
                    Align2::LEFT_BOTTOM,
                    &flight.code,
                    FontId::proportional(11.0 * ms),
                    Color32::BLACK,
                );
                painter.text(
                    tp,
                    Align2::LEFT_BOTTOM,
                    &flight.code,
                    FontId::proportional(11.0 * ms),
                    Color32::from_rgb(255, 235, 149),
                );
            }
        }
        if let Some(f) = nearest {
            let pt = self.lang == earthcraft_core::language::Lang::PtBr;
            let alt = f.altitude_feet.map_or("—".into(), |a| format!("{a:.0} ft"));
            let speed = f.speed_knots.map_or("—".into(), |s| format!("{s:.0} kt"));
            let text = if pt {
                format!(
                    "{}  (ICAO {})\nAltitude: {}  |  Velocidade: {}\nRumo: {:.0}°  |  Posição há {:.0} s",
                    f.code,
                    f.hex.to_uppercase(),
                    alt,
                    speed,
                    f.heading_deg,
                    f.age_secs
                )
            } else {
                format!(
                    "{}  (ICAO {})\nAltitude: {}  |  Speed: {}\nHeading: {:.0}°  |  Position {:.0} s old",
                    f.code,
                    f.hex.to_uppercase(),
                    alt,
                    speed,
                    f.heading_deg,
                    f.age_secs
                )
            };
            let font = FontId::proportional(12.0 * ms);
            let galley = painter.layout_no_wrap(text, font, Color32::WHITE);
            let position = egui::pos2(
                (f.position.x + 18.0 * ms)
                    .min(rect.max.x - galley.size().x - 16.0 * ms)
                    .max(rect.min.x + 6.0 * ms),
                (f.position.y + 12.0 * ms)
                    .min(rect.max.y - galley.size().y - 16.0 * ms)
                    .max(rect.min.y + 6.0 * ms),
            );
            let bounds = Rect::from_min_size(position, galley.size()).expand(6.0 * ms);
            painter.rect_filled(
                bounds,
                5.0 * ms,
                Color32::from_rgba_unmultiplied(13, 24, 35, 235),
            );
            painter.galley(position, galley, Color32::WHITE);
        }
    }
}
