//! Egui texture ownership and repaint scheduling for data-owned live imagery.
use earthcraft_core::language::Lang;
use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, TextureHandle};
use std::sync::Arc;
use std::time::Duration;
#[derive(Default)]
pub(super) struct LiveMaps {
    service: earthcraft_data::live_maps::LiveMaps,
    projected: Option<TextureHandle>,
    projected_for: Option<Arc<Vec<u8>>>,
}
impl LiveMaps {
    pub(super) fn update(
        &mut self,
        ctx: &egui::Context,
        runtime: &earthcraft_data::runtime::ServiceRuntime,
        clouds: bool,
        radar: bool,
    ) {
        let wake = ctx.clone();
        self.service
            .update(runtime, clouds, radar, move || wake.request_repaint());
        if clouds || radar {
            ctx.request_repaint_after(Duration::from_secs(60));
        }
    }
    pub(super) fn snapshot(
        &self,
        clouds: bool,
        radar: bool,
    ) -> (Option<Arc<image::RgbaImage>>, Option<Arc<image::RgbaImage>>) {
        (
            clouds.then(|| self.service.shared_cloud()).flatten(),
            radar.then(|| self.service.shared_radar()).flatten(),
        )
    }
    pub(super) fn draw(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        geometry: Option<&earthcraft_render::live_layers::LiveGeometry>,
    ) {
        let Some(geometry) = geometry else {
            return;
        };
        let Some(rgba) = &geometry.weather else {
            return;
        };
        if self
            .projected_for
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, rgba))
        {
            let im = ColorImage::from_rgba_unmultiplied(
                [
                    geometry.input.view.weather_width_px,
                    geometry.input.view.weather_height_px,
                ],
                rgba,
            );
            if let Some(tex) = self.projected.as_mut() {
                tex.set(im, egui::TextureOptions::LINEAR);
            } else {
                self.projected = Some(painter.ctx().load_texture(
                    "live-weather-cloud-map",
                    im,
                    egui::TextureOptions::LINEAR,
                ));
            }
            self.projected_for = Some(rgba.clone());
        }
        if let Some(tex) = &self.projected {
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    pub(super) fn status(&self, clouds: bool, radar: bool, lang: Lang) -> String {
        self.service.status(clouds, radar, lang)
    }
}
