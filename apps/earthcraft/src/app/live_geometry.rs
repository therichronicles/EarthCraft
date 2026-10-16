//! Match background live-layer geometry to the displayed globe before drawing.
use super::App;
use earthcraft_core::{
    camera::{Basis, Cam},
    math::now_unix,
};
use earthcraft_render::live_layers::{LiveGeometry, LiveInput, LiveView, LiveWorker};
use eframe::egui;
use std::sync::Arc;

pub(super) struct LiveOverlay {
    worker: LiveWorker,
    requested: Option<LiveInput>,
    latest: Option<Arc<LiveGeometry>>,
}
impl LiveOverlay {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            worker: LiveWorker::new(move || ctx.request_repaint())
                .expect("start live-layer projection"),
            requested: None,
            latest: None,
        }
    }
    pub fn accept(&mut self, geometry: Arc<LiveGeometry>) {
        self.latest = Some(geometry);
        self.requested = None;
    }
    pub fn update(&mut self, input: LiveInput) -> Option<Arc<LiveGeometry>> {
        if let Some(geometry) = self.worker.take_completed()
            && geometry.input.same_request(&input)
        {
            self.accept(geometry);
        }
        if let Some(geometry) = &self.latest
            && geometry.input.same_request(&input)
        {
            return Some(geometry.clone());
        }
        if self
            .requested
            .as_ref()
            .is_none_or(|old| !old.same_request(&input))
        {
            self.worker.submit(input.clone());
            self.requested = Some(input);
        }
        None
    }
}
impl App {
    pub(super) fn live_layer_input(
        &self,
        basis: Basis,
        rect: egui::Rect,
        camera: Cam,
        pixels_per_point: f32,
    ) -> LiveInput {
        let (width, height) = earthcraft_render::weather::traffic_size(rect, pixels_per_point);
        let (weather_width, weather_height) = earthcraft_render::weather::weather_size(
            rect,
            pixels_per_point,
            self.fly.is_some()
                || self.last_interact.elapsed() < earthcraft_render::weather::SETTLE_DELAY,
        );
        let (cloud, radar) = self.live_maps.snapshot(self.show_clouds, self.show_weather);
        LiveInput {
            view: LiveView {
                basis,
                rect,
                altitude_km: camera.alt,
                map_scale: self.settings.map_scale,
                pixels_per_point,
                traffic_width_px: width,
                traffic_height_px: height,
                weather_width_px: weather_width,
                weather_height_px: weather_height,
            },
            roads: self
                .show_roads
                .then(|| {
                    self.roads
                        .service
                        .shared_snapshot(camera.lat, camera.lon, camera.alt)
                })
                .flatten(),
            vessels: self
                .show_vessels
                .then(|| self.live_vessels.shared_snapshot())
                .flatten(),
            satellites: self
                .show_satellites
                .then(|| self.live_satellites.shared_snapshot())
                .flatten(),
            traffic: self
                .show_traffic
                .then(|| self.traffic.service.shared_tiles())
                .flatten(),
            cloud,
            radar,
            unix_secs: now_unix(),
        }
    }
}

pub(super) fn mesh_shapes(painter: &egui::Painter, shapes: Vec<egui::Shape>) -> Vec<egui::Shape> {
    let clipped = shapes
        .into_iter()
        .map(|shape| egui::epaint::ClippedShape {
            clip_rect: painter.clip_rect(),
            shape,
        })
        .collect();
    painter
        .ctx()
        .tessellate(clipped, painter.ctx().pixels_per_point())
        .into_iter()
        .map(|primitive| match primitive.primitive {
            egui::epaint::Primitive::Mesh(mesh) => egui::Shape::mesh(mesh),
            egui::epaint::Primitive::Callback(_) => {
                unreachable!("live overlays contain only styled geometry and labels")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_geometry_publication_for_an_old_view_is_never_drawn_on_a_new_view() {
        let ctx = egui::Context::default();
        let mut overlay = LiveOverlay::new(ctx);
        let cam = Cam {
            lat: 0.0,
            lon: 0.0,
            heading: 0.0,
            alt: 1000.0,
        };
        let input = LiveInput {
            view: LiveView {
                basis: cam.basis(1.0),
                rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 300.0)),
                altitude_km: cam.alt,
                map_scale: 1.0,
                pixels_per_point: 1.0,
                traffic_width_px: 16,
                traffic_height_px: 16,
                weather_width_px: 16,
                weather_height_px: 16,
            },
            roads: None,
            vessels: None,
            satellites: None,
            traffic: None,
            cloud: None,
            radar: None,
            unix_secs: 1_700_000_000.0,
        };
        let geometry =
            earthcraft_render::live_layers::LivePreparer::default().prepare(input.clone());
        overlay.accept(geometry.clone());
        assert!(Arc::ptr_eq(
            &geometry,
            &overlay.update(input.clone()).unwrap()
        ));
        let mut moved = input;
        moved.view.rect = moved.view.rect.translate(egui::vec2(10.0, 0.0));
        assert!(overlay.update(moved).is_none());
    }
}
