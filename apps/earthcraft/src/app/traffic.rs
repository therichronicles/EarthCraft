//! Texture lifecycle for live, key-authenticated TomTom road traffic.
use earthcraft_core::camera::Basis;
use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, TextureHandle};
#[derive(Default)]
pub(super) struct TrafficOverlay {
    pub service: earthcraft_data::traffic::LiveTraffic,
    texture: Option<TextureHandle>,
    projected_for: Option<(Basis,usize,usize,u64)>,
}
impl TrafficOverlay {
    pub fn draw(&mut self,painter:&egui::Painter,basis:&Basis,rect:Rect) {
        let Some(tiles) = self.service.tiles() else {return;};
        let width = ((rect.width()*painter.ctx().pixels_per_point()) as usize).clamp(160,800);
        let height = ((width as f32 * rect.height()/rect.width().max(1.0)) as usize).clamp(1,800);
        let key = (*basis,width,height,self.service.generation());
        if self.projected_for != Some(key) {
            let pixels = earthcraft_render::weather::project_traffic_tiles(basis,rect,width,height,tiles);
            let image = ColorImage::from_rgba_unmultiplied([width,height],&pixels);
            if let Some(tex) = self.texture.as_mut() {tex.set(image,egui::TextureOptions::LINEAR);}
            else {self.texture = Some(painter.ctx().load_texture("tomtom-live-traffic",image,egui::TextureOptions::LINEAR));}
            self.projected_for = Some(key);
        }
        if let Some(tex) = &self.texture {
            painter.image(tex.id(),rect,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0)),Color32::WHITE);
        }
    }
}
