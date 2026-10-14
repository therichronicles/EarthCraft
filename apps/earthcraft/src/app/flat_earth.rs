//! Hidden 3D-looking, rotatable *parody* disk under a procedurally starry sky.
//! It does not change real-Earth geometry, navigation or geospatial layers.
use super::App;
use earthcraft_core::language::Lang;
use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Key, Pos2, Rect, Sense, TextureHandle, Vec2,
};

const ART: &[u8] = include_bytes!("../../assets/flat_earth_easter_egg.png");
const MIN_ZOOM: f32 = 0.6;
const MAX_ZOOM: f32 = 4.0;
const SECTORS: usize = 96;
const RINGS: usize = 32;
const STARS: usize = 340;

pub(super) struct FlatEarth {
    pub open: bool,
    texture: Option<TextureHandle>,
    zoom: f32,
    offset: Vec2,
    yaw: f32,
    tilt: f32,
}

impl Default for FlatEarth {
    fn default() -> Self {
        Self {
            open: false,
            texture: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
            yaw: 0.0,
            tilt: 0.62,
        }
    }
}

// Parametric perspective of an azimuthal disk. 'height' makes the ice rim
// visibly thick; changing yaw/tilt rotates the surface and side walls.
fn disk_point(center: Pos2, radius: f32, yaw: f32, tilt: f32, x: f32, y: f32, height: f32) -> Pos2 {
    let (sn, cs) = yaw.sin_cos();
    let rx = x * cs - y * sn;
    let ry = x * sn + y * cs;
    Pos2::new(
        center.x + radius * rx,
        center.y + radius * (ry * tilt.sin() - height * tilt.cos()),
    )
}
fn hash01(i: u32) -> f32 {
    let mut a = i.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    a = ((a >> ((a >> 28) + 4)) ^ a).wrapping_mul(277_803_737);
    ((a >> 22) ^ a) as f32 / u32::MAX as f32
}

impl App {
    pub(super) fn flat_earth_shortcuts(&mut self, ctx: &egui::Context) {
        let (toggle, escape) = ctx.input(|input| {
            (
                input.modifiers.shift
                    && (input.modifiers.ctrl || input.modifiers.command)
                    && input.key_pressed(Key::F),
                input.key_pressed(Key::Escape),
            )
        });
        if toggle {
            self.flat_earth.open = !self.flat_earth.open;
            if self.flat_earth.open {
                self.status = self.lang.text("flat-earth-unlocked").into();
            }
            ctx.request_repaint();
        } else if escape && self.flat_earth.open {
            self.flat_earth.open = false;
            ctx.request_repaint();
        }
    }

    pub(super) fn unlock_flat_earth(&mut self, ctx: &egui::Context) {
        self.search_rx = None;
        self.sugg_rx = None;
        self.sugg_open = false;
        self.sugg_sel = None;
        self.suggestions.clear();
        self.remote_sugg.clear();
        self.query.clear();
        self.flat_earth.open = true;
        self.status = self.lang.text("flat-earth-unlocked").into();
        ctx.request_repaint();
    }

    pub(super) fn flat_earth_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(2, 4, 18));
        self.cursor = None;

        if self.flat_earth.texture.is_none()
            && let Ok(img) = image::load_from_memory(ART)
        {
            let pixels = img.to_rgba8();
            let size = [pixels.width() as usize, pixels.height() as usize];
            self.flat_earth.texture = Some(ctx.load_texture(
                "earthcraft-secret-3d-disk",
                ColorImage::from_rgba_unmultiplied(size, pixels.as_raw()),
                egui::TextureOptions::LINEAR,
            ));
        }
        if response.dragged() {
            let delta = ctx.input(|input| input.pointer.delta());
            if ctx.input(|i| i.modifiers.shift) {
                self.flat_earth.offset += delta;
            } else {
                self.flat_earth.yaw += delta.x * 0.007;
                self.flat_earth.tilt = (self.flat_earth.tilt - delta.y * 0.007).clamp(0.12, 1.55);
            }
        }
        if response.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                self.flat_earth.zoom =
                    (self.flat_earth.zoom * (scroll * 0.0015).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
            }
        }
        if response.double_clicked() {
            self.flat_earth.zoom = 1.0;
            self.flat_earth.offset = Vec2::ZERO;
            self.flat_earth.yaw = 0.0;
            self.flat_earth.tilt = 0.62;
        }

        // Deterministic three-dimensional-looking star field, no external assets.
        for i in 0..STARS as u32 {
            let x = rect.left() + rect.width() * hash01(i * 7 + 1);
            let y = rect.top() + rect.height() * hash01(i * 7 + 2);
            let depth = hash01(i * 7 + 3);
            let alpha = 90 + (depth * 150.0) as u8;
            let size = 0.35 + depth * 1.05;
            painter.circle_filled(
                Pos2::new(x, y),
                size,
                Color32::from_rgba_unmultiplied(205, 223, 255, alpha),
            );
        }
        let center = rect.center() + self.flat_earth.offset + Vec2::new(0.0, rect.height() * 0.05);
        let radius = (rect.width() * 0.31).min(rect.height() * 0.42) * self.flat_earth.zoom;
        let (yaw, tilt) = (self.flat_earth.yaw, self.flat_earth.tilt);
        let point = |x, y, z| disk_point(center, radius, yaw, tilt, x, y, z);

        // A visible cylindrical icy rim gives the disk a solid silhouette.
        // Back sections first, then textured surface, then front sections.
        for j in 0..SECTORS {
            let a = (j as f32 + 0.5) * std::f32::consts::TAU / SECTORS as f32;
            let b = (j as f32 + 1.5) * std::f32::consts::TAU / SECTORS as f32;
            let p = [
                point(a.cos(), a.sin(), 0.0),
                point(b.cos(), b.sin(), 0.0),
                point(b.cos(), b.sin(), -0.10),
                point(a.cos(), a.sin(), -0.10),
            ];
            painter.add(egui::Shape::convex_polygon(
                p.to_vec(),
                Color32::from_rgb(117, 181, 206),
                egui::Stroke::NONE,
            ));
        }
        if let Some(tex) = &self.flat_earth.texture {
            // Project a genuinely triangulated textured disk (not a distorted rectangle).
            // Polar-to-equirect UV: centre = northern pole, outer rim = Antarctica.
            let mut mesh = egui::epaint::Mesh::with_texture(tex.id());
            for ring in 0..RINGS {
                let inner = ring as f32 / RINGS as f32;
                let outer = (ring + 1) as f32 / RINGS as f32;
                for sector in 0..SECTORS {
                    let u0 = sector as f32 / SECTORS as f32;
                    let u1 = (sector + 1) as f32 / SECTORS as f32;
                    let t0 = u0 * std::f32::consts::TAU;
                    let t1 = u1 * std::f32::consts::TAU;
                    let start = mesh.vertices.len() as u32;
                    for (r, u, a) in [
                        (inner, u0, t0),
                        (inner, u1, t1),
                        (outer, u1, t1),
                        (outer, u0, t0),
                    ] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: point(r * a.cos(), r * a.sin(), 0.0),
                            uv: Pos2::new(u, r),
                            color: Color32::WHITE,
                        });
                    }
                    mesh.indices.extend_from_slice(&[
                        start,
                        start + 1,
                        start + 2,
                        start,
                        start + 2,
                        start + 3,
                    ]);
                }
            }
            painter.add(egui::Shape::mesh(mesh));
            let mut ice = Vec::with_capacity(SECTORS + 1);
            for i in 0..=SECTORS {
                let a = i as f32 * std::f32::consts::TAU / SECTORS as f32;
                ice.push(point(a.cos(), a.sin(), 0.001));
            }
            painter.add(egui::Shape::line(
                ice,
                egui::Stroke::new(3.0 * self.flat_earth.zoom, Color32::from_rgb(228, 247, 255)),
            ));
        } else {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                self.lang.text("flat-earth-image-error"),
                FontId::proportional(18.0),
                Color32::WHITE,
            );
        }
        let title = self.lang.text("flat-earth-title");
        let description = self.lang.text("flat-earth-disclaimer");
        let hint = self.lang.text("flat-earth-hint");
        let pt = self.lang == Lang::PtBr;
        let top_left = rect.left_top() + Vec2::new(18.0, 16.0);
        let label_bounds =
            Rect::from_min_size(top_left - Vec2::new(10.0, 7.0), Vec2::new(350.0, 76.0));
        painter.rect_filled(
            label_bounds,
            9.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 208),
        );
        painter.text(
            top_left,
            Align2::LEFT_TOP,
            title,
            FontId::proportional(22.0),
            Color32::from_rgb(255, 218, 145),
        );
        painter.text(
            top_left + Vec2::new(0.0, 30.0),
            Align2::LEFT_TOP,
            description,
            FontId::proportional(11.0),
            Color32::from_rgb(229, 231, 236),
        );
        let footer = Rect::from_min_size(
            Pos2::new(rect.left() + 14.0, rect.bottom() - 33.0),
            Vec2::new((rect.width() - 28.0).max(0.0), 25.0),
        );
        painter.rect_filled(footer, 6.0, Color32::from_rgba_unmultiplied(0, 0, 0, 205));
        painter.text(
            footer.center(),
            Align2::CENTER_CENTER,
            hint,
            FontId::proportional(12.0),
            Color32::LIGHT_GRAY,
        );
        let exit_width = if pt { 167.0 } else { 177.0 };
        let close_rect = Rect::from_min_size(
            Pos2::new(
                (rect.right() - exit_width - 16.0).max(rect.left()),
                rect.top() + 18.0,
            ),
            Vec2::new(exit_width, 31.0),
        );
        if ui
            .put(
                close_rect,
                egui::Button::new(self.lang.text("flat-earth-return")),
            )
            .clicked()
        {
            self.flat_earth.open = false;
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_artwork_decodes_as_the_bundled_landscape_texture() {
        let pixels = image::load_from_memory(ART)
            .expect("flat-Earth Easter egg artwork")
            .to_rgba8();
        assert_eq!(pixels.dimensions(), (700, 360));
        assert_eq!(pixels.as_raw().len(), 700 * 360 * 4);
    }
    #[test]
    fn hidden_scene_is_off_by_default() {
        let state = FlatEarth::default();
        assert!(!state.open);
        assert_eq!(state.zoom, 1.0);
    }
    #[test]
    fn perspective_has_depth() {
        let c = Pos2::new(100.0, 100.0);
        assert_ne!(
            disk_point(c, 100.0, 0.0, 0.6, 0.0, 1.0, 0.0),
            disk_point(c, 100.0, 0.0, 0.6, 0.0, 1.0, -0.1)
        );
    }
}
