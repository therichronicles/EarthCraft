//! Texture lifecycle for live, key-authenticated TomTom road traffic.
use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, TextureHandle};
use std::sync::Arc;
#[derive(Default)]
pub(super) struct TrafficOverlay {
    pub(super) service: earthcraft_data::traffic::LiveTraffic,
    texture: Option<TextureHandle>,
    projected_for: Option<Arc<Vec<u8>>>,
}
impl TrafficOverlay {
    pub(super) fn draw(
        &mut self,
        painter: &egui::Painter,
        geometry: &earthcraft_render::live_layers::LiveGeometry,
    ) {
        let Some(pixels) = &geometry.traffic else {
            return;
        };
        let rect = geometry.input.view.rect;
        let width = geometry.input.view.traffic_width_px;
        let height = geometry.input.view.traffic_height_px;
        if self
            .projected_for
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, pixels))
        {
            let image = ColorImage::from_rgba_unmultiplied([width, height], pixels);
            if let Some(tex) = self.texture.as_mut() {
                tex.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture = Some(painter.ctx().load_texture(
                    "tomtom-live-traffic",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
            self.projected_for = Some(pixels.clone());
        }
        if let Some(tex) = &self.texture {
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}
