//! Aircraft geometry is prepared in render; styled shapes and hover stay in egui.
mod shapes;
#[cfg(test)]
mod tests;
use super::App;
use earthcraft_core::{camera::Basis, language::Lang, math::now_unix};
use earthcraft_render::aircraft::{
    AircraftGeometry, AircraftWorker, ProjectionInput, ProjectionView,
};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Stroke, Vec2};
use std::{sync::Arc, time::Duration};

pub(super) const FLIGHT_REPAINT_INTERVAL: Duration = Duration::from_secs(1);
const HOVER_RADIUS_POINTS: f32 = 15.0;
const HOVER_FONT_POINTS: f32 = 12.0;
const HOVER_OFFSET_POINTS: Vec2 = Vec2::new(18.0, 12.0);
const HOVER_EDGE_MARGIN_POINTS: f32 = 16.0;
const HOVER_MIN_MARGIN_POINTS: f32 = 6.0;
const HOVER_PADDING_POINTS: f32 = 6.0;
const HOVER_CORNER_POINTS: f32 = 5.0;
const HOVER_FILL_RGBA: [u8; 4] = [13, 24, 35, 235];

pub(super) struct AircraftOverlay {
    geometry: Option<Arc<AircraftGeometry>>,
    requested: Option<ProjectionInput>,
    worker: AircraftWorker,
    shapes: Vec<egui::Shape>,
    shape_key: Option<(usize, Lang)>,
}
impl AircraftOverlay {
    pub(super) fn new(ctx: egui::Context) -> Self {
        Self {
            geometry: None,
            requested: None,
            worker: AircraftWorker::new(move || ctx.request_repaint())
                .expect("start aircraft projection"),
            shapes: Vec::new(),
            shape_key: None,
        }
    }
    pub(super) fn accept(&mut self, geometry: Arc<AircraftGeometry>, unix_secs: f64) {
        if !geometry.is_recent(unix_secs)
            || self.geometry.as_ref().is_some_and(|old| {
                old.matches(geometry.view, &geometry.data)
                    && old.is_recent(unix_secs)
                    && old.unix_secs.floor() >= geometry.unix_secs.floor()
            })
        {
            return;
        }
        self.geometry = Some(geometry);
        self.shape_key = None;
    }
    fn draw(&mut self, painter: &egui::Painter, input: ProjectionInput, lang: Lang) {
        if let Some(completed) = self.worker.take_completed()
            && completed.matches(input.view, &input.data)
        {
            self.accept(completed, input.unix_secs);
        }
        let current = self.geometry.as_ref().filter(|geometry| {
            geometry.matches(input.view, &input.data) && geometry.is_recent(input.unix_secs)
        });
        let needs_refresh =
            current.is_none_or(|geometry| geometry.unix_secs.floor() != input.unix_secs.floor());
        if needs_refresh
            && self
                .requested
                .as_ref()
                .is_none_or(|old| !old.same_request(&input))
        {
            self.worker.submit(input.clone());
            self.requested = Some(input.clone());
        }
        let Some(geometry) = current else {
            return;
        };
        let key = (Arc::as_ptr(geometry) as usize, lang);
        if self.shape_key != Some(key) {
            self.shapes = shapes::build(painter, geometry);
            self.shape_key = Some(key);
        }
        painter.extend(self.shapes.iter().cloned());
        draw_hover(painter, geometry, lang);
    }
}

impl App {
    pub(super) fn aircraft_input(
        &self,
        basis: Basis,
        rect: Rect,
        altitude_km: f64,
        pixels_per_point: f32,
    ) -> ProjectionInput {
        ProjectionInput {
            view: ProjectionView {
                basis,
                rect,
                altitude_km,
                map_scale: self.settings.map_scale,
                pixels_per_point,
            },
            data: self.live_aircraft.snapshot(),
            unix_secs: now_unix(),
        }
    }
    pub(super) fn draw_live_layers(
        &mut self,
        painter: &egui::Painter,
        basis: &Basis,
        rect: Rect,
        altitude_km: f64,
    ) {
        if self.show_flights {
            let input =
                self.aircraft_input(*basis, rect, altitude_km, painter.ctx().pixels_per_point());
            self.aircraft_overlay.draw(painter, input, self.lang);
            painter.ctx().request_repaint_after(FLIGHT_REPAINT_INTERVAL);
        }
        if self.show_vessels {
            self.draw_vessel_overlay(painter, basis, rect, altitude_km);
        }
        if self.show_satellites {
            self.draw_satellite_overlay(painter, basis, rect, altitude_km);
            // Orbit predictions need to move even without another network fetch.
            painter.ctx().request_repaint_after(Duration::from_secs(1));
        }
    }
}

fn draw_hover(painter: &egui::Painter, geometry: &AircraftGeometry, lang: Lang) {
    let view = geometry.view;
    let ms = view.map_scale;
    let Some(hover) = painter
        .ctx()
        .pointer_hover_pos()
        .filter(|point| view.rect.contains(*point))
    else {
        return;
    };
    let mut nearest = None;
    let mut minimum = (HOVER_RADIUS_POINTS * ms).powi(2);
    for flight in &geometry.flights {
        let distance = (hover - flight.position).length_sq();
        if distance < minimum {
            minimum = distance;
            nearest = Some(flight);
        }
    }
    let Some(flight) = nearest else {
        return;
    };
    let sample = &geometry.data.aircraft[flight.source_index];
    let code = if sample.callsign.is_empty() {
        sample.hex.to_uppercase()
    } else {
        sample.callsign.to_string()
    };
    let text = lang.format(
        "aircraft-hover-tooltip",
        &[
            ("code", code.into()),
            ("hex", sample.hex.to_uppercase().into()),
            (
                "altitude",
                sample
                    .altitude_feet
                    .map_or("—".into(), |alt| format!("{alt:.0} ft"))
                    .into(),
            ),
            (
                "speed",
                sample
                    .speed_knots
                    .map_or("—".into(), |speed| format!("{speed:.0} kt"))
                    .into(),
            ),
            (
                "heading",
                earthcraft_core::compass::heading_text(sample.heading_deg, lang).into(),
            ),
            (
                "age",
                format!("{:.0}", (now_unix() - sample.observed_unix).max(0.0)).into(),
            ),
        ],
    );
    let galley = painter.layout_no_wrap(
        text,
        FontId::proportional(HOVER_FONT_POINTS * ms),
        Color32::WHITE,
    );
    let position = egui::pos2(
        (flight.position.x + HOVER_OFFSET_POINTS.x * ms)
            .min(view.rect.max.x - galley.size().x - HOVER_EDGE_MARGIN_POINTS * ms)
            .max(view.rect.min.x + HOVER_MIN_MARGIN_POINTS * ms),
        (flight.position.y + HOVER_OFFSET_POINTS.y * ms)
            .min(view.rect.max.y - galley.size().y - HOVER_EDGE_MARGIN_POINTS * ms)
            .max(view.rect.min.y + HOVER_MIN_MARGIN_POINTS * ms),
    );
    let bounds = Rect::from_min_size(position, galley.size()).expand(HOVER_PADDING_POINTS * ms);
    let [r, g, b, a] = HOVER_FILL_RGBA;
    let fill = Color32::from_rgba_unmultiplied(r, g, b, a);
    painter.rect_filled(bounds, HOVER_CORNER_POINTS * ms, fill);
    painter.galley(position, galley, Color32::WHITE);
}

// Additional worldwide AIS and predicted orbital-position layers.
impl App {
    fn draw_vessel_overlay(&self, painter: &egui::Painter, basis: &Basis, rect: Rect, altitude: f64) {
        use earthcraft_core::math::ll_to_v3;
        let ms = self.settings.map_scale;
        let hover = painter.ctx().pointer_hover_pos();
        let mut nearest: Option<(&earthcraft_data::vessels::Vessel,egui::Pos2)> = None;
        let mut distance = (14.0 * ms).powi(2);
        let mut labels = 0;
        for ship in self.live_vessels.snapshot() {
            let Some(at) = basis.project(ll_to_v3(ship.lat,ship.lon), rect) else {continue;};
            if !rect.contains(at) {continue;}
            let (lat_ahead, lon_ahead) = earthcraft_core::math::destination(
                ship.lat, ship.lon, ship.course, 2.0,
            );
            let forward = basis.project(ll_to_v3(lat_ahead, lon_ahead), rect)
                .map(|p| p - at)
                .filter(|v| v.length_sq() > 0.0001)
                .map(|v| v.normalized() * 8.0 * ms)
                .unwrap_or(Vec2::new(0.0, -8.0 * ms));
            let right = Vec2::new(-forward.y, forward.x) * 0.55;
            painter.add(egui::Shape::convex_polygon(
                vec![at+forward,at+right-forward*0.8,at-forward*0.65,at-right-forward*0.8],
                Color32::from_rgb(52,224,229),Stroke::new(1.0*ms,Color32::from_rgb(5,60,65))));
            if let Some(h) = hover {
                let d = (h-at).length_sq();
                if d < distance {distance=d;nearest=Some((ship,at));}
            }
            if altitude<75.0 && labels<75 {
                let text = if ship.name.is_empty() {&ship.mmsi} else {&ship.name};
                painter.text(at+Vec2::new(8.0*ms,0.0),Align2::LEFT_CENTER,text,
                    FontId::proportional(10.0*ms),Color32::from_rgb(120,245,245));
                labels+=1;
            }
        }
        if let Some((ship,at)) = nearest {
            let name = if ship.name.is_empty() {"AIS vessel"} else {&ship.name};
            let age_minutes=((now_unix()-ship.observed_unix).max(0.0)/60.0).round();
            let observed=if self.lang==earthcraft_core::language::Lang::PtBr {
                format!("Último sinal AIS há {age_minutes:.0} min • cobertura depende da fonte")
            } else {format!("Last AIS report {age_minutes:.0} min ago • coverage varies")};
            let text=format!("{} • MMSI {}\n{:.3}°, {:.3}° • {:.0}° • {} kt\n{}",
                name,ship.mmsi,ship.lat,ship.lon,ship.course,
                ship.speed_knots.map_or("?".into(),|s|format!("{s:.1}")),observed);
            hover_label(painter,rect,at,&text,ms);
        }
    }
    fn draw_satellite_overlay(&self, painter:&egui::Painter,basis:&Basis,rect:Rect,altitude:f64) {
        use earthcraft_core::math::{ll_to_v3,now_unix};
        let ms=self.settings.map_scale;
        let now=now_unix();
        let hover=painter.ctx().pointer_hover_pos();
        let mut nearest:Option<(&earthcraft_data::satellites::Satellite,egui::Pos2)>=None;
        let mut distance=(13.0*ms).powi(2);
        let mut labels=0;
        for sat in self.live_satellites.snapshot() {
            let Some((lat,lon))=sat.ground_position(now) else {continue;};
            let Some(at)=basis.project(ll_to_v3(lat,lon),rect) else {continue;};
            if !rect.contains(at) {continue;}
            let s=5.0*ms;
            painter.add(egui::Shape::convex_polygon(
                vec![at+Vec2::new(0.0,-s),at+Vec2::new(s,0.0),
                    at+Vec2::new(0.0,s),at+Vec2::new(-s,0.0)],
                Color32::from_rgb(207,159,255),Stroke::new(ms,Color32::from_rgb(62,22,91))));
            if let Some(h)=hover {
                let d=(h-at).length_sq();
                if d<distance {distance=d;nearest=Some((sat,at));}
            }
            if altitude<900.0 && labels<60 {
                painter.text(at+Vec2::new(8.0*ms,0.0),Align2::LEFT_CENTER,&sat.name,
                    FontId::proportional(10.0*ms),Color32::from_rgb(219,178,252));
                labels+=1;
            }
        }
        if let Some((sat,at))=nearest {
            let pt=self.lang==earthcraft_core::language::Lang::PtBr;
            let text=if pt {format!("{} (NORAD {})\nPosição orbital estimada • CelesTrak",sat.name,sat.norad)}
            else {format!("{} (NORAD {})\nEstimated orbital position • CelesTrak",sat.name,sat.norad)};
            hover_label(painter,rect,at,&text,ms);
        }
    }
}
fn hover_label(painter:&egui::Painter,rect:Rect,screen:egui::Pos2,text:&str,ms:f32) {
    let galley=painter.layout_no_wrap(text.to_string(),FontId::proportional(12.0*ms),Color32::WHITE);
    let pos=egui::pos2((screen.x+15.0).min(rect.right()-galley.size().x-10.0)
        .max(rect.left()+5.0),(screen.y+12.0).min(rect.bottom()-galley.size().y-10.0)
        .max(rect.top()+5.0));
    painter.rect_filled(Rect::from_min_size(pos,galley.size()).expand(5.0),5.0,
        Color32::from_rgba_unmultiplied(12,22,36,236));
    painter.galley(pos,galley,Color32::WHITE);
}
