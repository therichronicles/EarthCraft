//! Hidden 3D-looking, rotatable *parody* disk under a procedurally starry sky.
//! It does not change real-Earth geometry, navigation or geospatial layers.
mod scene;
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
const ROTATION_RADIANS_PER_POINT: f32 = 0.007;
const ZOOM_EXPONENT_PER_POINT: f32 = 0.0015;
const MIN_TILT_RADIANS: f32 = 0.12;
const MAX_TILT_RADIANS: f32 = 1.55;
const INITIAL_TILT_RADIANS: f32 = 0.62;
const SCROLL_THRESHOLD_POINTS: f32 = 0.1;

pub(super) struct FlatEarth {
    pub open: bool,
    texture: Option<TextureHandle>,
    zoom: f32,
    offset: Vec2,
    yaw: f32,
    tilt: f32,
    scene: scene::Scene,
    image_requested: bool,
    pub(super) image_failed: bool,
}

impl Default for FlatEarth {
    fn default() -> Self {
        Self {
            open: false,
            texture: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
            yaw: 0.0,
            tilt: INITIAL_TILT_RADIANS,
            scene: scene::Scene::default(),
            image_requested: false,
            image_failed: false,
        }
    }
}

impl FlatEarth {
    pub(super) fn image_loaded(
        &mut self,
        ctx: &egui::Context,
        image: earthcraft_data::files::LoadedImage,
    ) {
        self.image_failed = false;
        let pixels = image.pixels;
        let size = [pixels.width() as usize, pixels.height() as usize];
        self.texture = Some(ctx.load_texture(
            "earthcraft-secret-3d-disk",
            ColorImage::from_rgba_unmultiplied(size, pixels.as_raw()),
            egui::TextureOptions::LINEAR,
        ));
    }
}

fn hash01(i: u32) -> f32 {
    let mut a = i.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    a = ((a >> ((a >> 28) + 4)) ^ a).wrapping_mul(277_803_737);
    ((a >> 22) ^ a) as f32 / u32::MAX as f32
}

impl App {
    pub(super) fn flat_earth_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
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

        if !self.flat_earth.image_requested {
            match self.file_jobs.load_image(
                earthcraft_data::files::ImageTarget::FlatEarth,
                earthcraft_data::files::ImageSource::Embedded(ART),
            ) {
                Ok(()) => self.flat_earth.image_requested = true,
                Err(error) => self.status = format!("Flat Earth artwork request failed: {error}"),
            }
        }
        if response.dragged() {
            let delta = ctx.input(|input| input.pointer.delta());
            if ctx.input(|i| i.modifiers.shift) {
                self.flat_earth.offset += delta;
            } else {
                self.flat_earth.yaw += delta.x * ROTATION_RADIANS_PER_POINT;
                self.flat_earth.tilt = (self.flat_earth.tilt
                    - delta.y * ROTATION_RADIANS_PER_POINT)
                    .clamp(MIN_TILT_RADIANS, MAX_TILT_RADIANS);
            }
        }
        if response.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > SCROLL_THRESHOLD_POINTS {
                self.flat_earth.zoom = (self.flat_earth.zoom
                    * (scroll * ZOOM_EXPONENT_PER_POINT).exp())
                .clamp(MIN_ZOOM, MAX_ZOOM);
            }
        }
        if response.double_clicked() {
            self.flat_earth.zoom = 1.0;
            self.flat_earth.offset = Vec2::ZERO;
            self.flat_earth.yaw = 0.0;
            self.flat_earth.tilt = INITIAL_TILT_RADIANS;
        }

        self.flat_earth.scene.draw(
            &painter,
            scene::View {
                rect,
                zoom: self.flat_earth.zoom,
                offset: self.flat_earth.offset,
                yaw: self.flat_earth.yaw,
                tilt: self.flat_earth.tilt,
                texture: self.flat_earth.texture.as_ref().map(TextureHandle::id),
                pixels_per_point: ctx.pixels_per_point(),
            },
        );
        if self.flat_earth.image_failed {
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
}

#[cfg(test)]
mod image_tests {
    use super::*;
    #[test]
    fn embedded_image_uses_the_file_worker_and_completion_does_not_reopen_the_scene() {
        use earthcraft_data::files::{FileJobs, FileOutcome, ImageSource, ImageTarget};
        let runtime = earthcraft_data::runtime::ServiceRuntime::new().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let (jobs, results) = FileJobs::new(runtime.handle(), move || {
            let _ = send.send(());
        });
        jobs.load_image(ImageTarget::FlatEarth, ImageSource::Embedded(ART))
            .unwrap();
        let result = results
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        let FileOutcome::ImageLoaded {
            target: ImageTarget::FlatEarth,
            image,
        } = result.result.unwrap()
        else {
            panic!("illustration image");
        };
        receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        let mut illustration = FlatEarth::default();
        illustration.image_loaded(&egui::Context::default(), image);
        assert!(!illustration.open);
        assert!(illustration.texture.is_some());
        assert!(!illustration.image_failed);
    }
}
