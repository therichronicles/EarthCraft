//! Egui texture ownership and repaint scheduling for data-owned live imagery.
use earthcraft_core::{camera::Basis, language::Lang};
use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, TextureHandle};
use std::time::Duration;
const MIN_PROJECTION_WIDTH_PX: usize = 160;
const MOVING_PROJECTION_WIDTH_PX: usize = 480;
const MAX_PROJECTION_SIDE_PX: usize = 960;

#[derive(Default)]
pub(super) struct LiveMaps {
    service: earthcraft_data::live_maps::LiveMaps,
    projected: Option<TextureHandle>,
    projected_for: Option<(Basis, usize, usize, bool, bool, u64)>,
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
    pub(super) fn draw(
        &mut self,
        painter: &egui::Painter,
        basis: &Basis,
        rect: Rect,
        clouds: bool,
        radar: bool,
    ) {
        let show_clouds = clouds && self.service.cloud().is_some();
        let show_radar = radar && self.service.radar().is_some();
        if !show_clouds && !show_radar {
            return;
        }
        // Projection is cached and updated only when camera or imagery changes.
        let max_width = if painter.ctx().input(|i| i.pointer.any_down()) {
            MOVING_PROJECTION_WIDTH_PX
        } else {
            MAX_PROJECTION_SIDE_PX
        };
        let width = ((rect.width() * painter.ctx().pixels_per_point()) as usize)
            .clamp(MIN_PROJECTION_WIDTH_PX, max_width);
        let height = ((width as f32 * rect.height() / rect.width().max(1.0)) as usize)
            .clamp(1, MAX_PROJECTION_SIDE_PX);
        let key = (
            *basis,
            width,
            height,
            show_clouds,
            show_radar,
            self.service.generation(),
        );
        if self.projected_for != Some(key) {
            let rgba = earthcraft_render::weather::project_live_maps(
                basis,
                rect,
                width,
                height,
                if show_clouds {
                    self.service.cloud()
                } else {
                    None
                },
                if show_radar {
                    self.service.radar()
                } else {
                    None
                },
            );
            let im = ColorImage::from_rgba_unmultiplied([width, height], &rgba);
            if let Some(tex) = self.projected.as_mut() {
                tex.set(im, egui::TextureOptions::LINEAR);
            } else {
                self.projected = Some(painter.ctx().load_texture(
                    "live-weather-cloud-map",
                    im,
                    egui::TextureOptions::LINEAR,
                ));
            }
            self.projected_for = Some(key);
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
