//! Submit immutable inputs and apply file results only on the UI thread.
#[cfg(test)]
mod tests;
use super::Explorer;
use earthcraft_core::{body::Body, language::Lang};
use earthcraft_data::{
    files::{FileJobs, FileKind, ImageSource, ImageTarget, LoadedImage, PlanetSave},
    planet::LoadedPlanet,
};
use eframe::egui::{self, ColorImage, TextureHandle};
use std::{path::PathBuf, sync::Arc};

fn source(path: &str, encoded: &Option<Arc<Vec<u8>>>) -> ImageSource {
    match encoded {
        Some(bytes) => ImageSource::Encoded(bytes.clone()),
        None => ImageSource::Path(PathBuf::from(path)),
    }
}

impl Explorer {
    fn submitted(&mut self, result: Result<(), earthcraft_data::Error>, activate: bool) {
        match result {
            Ok(()) => {
                self.pending = true;
                self.activate_on_completion = activate;
            }
            Err(error) => self.message = format!("Explorer file request failed: {error}"),
        }
    }

    pub(super) fn request_image(&mut self, target: ImageTarget, jobs: &FileJobs) {
        let (path, encoded) = match target {
            ImageTarget::Planet => (&self.image_path, &self.image_bytes),
            ImageTarget::Sky => (&self.sky_path, &self.sky_bytes),
            ImageTarget::Moon => {
                self.choose(Body::Moon, jobs);
                return;
            }
        };
        let input = if path.is_empty() {
            source(path, encoded)
        } else {
            ImageSource::Path(PathBuf::from(path))
        };
        self.submitted(
            jobs.load_image(target, input),
            target == ImageTarget::Planet,
        );
    }

    pub(super) fn request_open(&mut self, path: PathBuf, jobs: &FileJobs) {
        self.submitted(jobs.open_planet(path), true);
    }

    pub(super) fn request_save(&mut self, path: PathBuf, jobs: &FileJobs) {
        let snapshot = PlanetSave {
            path,
            name: self.name.clone(),
            texture: source(&self.image_path, &self.image_bytes),
            sky: if self.sky_path.trim().is_empty() && self.sky_bytes.is_none() {
                None
            } else {
                Some(source(&self.sky_path, &self.sky_bytes))
            },
        };
        self.submitted(jobs.save_planet(snapshot), false);
    }

    fn finished(&mut self) {
        if self.activate_on_completion {
            self.active = Body::Custom;
        }
        self.pending = false;
        self.activate_on_completion = false;
    }

    pub(in crate::app) fn image_loaded(
        &mut self,
        target: ImageTarget,
        image: LoadedImage,
        ctx: &egui::Context,
        lang: Lang,
    ) {
        let label = match target {
            ImageTarget::Planet => "custom-planet",
            ImageTarget::Sky => "custom-sky",
            ImageTarget::Moon => "Moon",
        };
        let texture = upload_image(ctx, label, image.pixels);
        match target {
            ImageTarget::Planet => {
                self.viewer = Some(texture);
                self.image_bytes = Some(image.encoded);
                self.message = lang.text("explorer-planet-displayed").into();
            }
            ImageTarget::Sky => {
                self.sky = Some(texture);
                self.sky_bytes = Some(image.encoded);
                self.message = lang.text("explorer-sky-applied").into();
            }
            ImageTarget::Moon => {
                self.moon = Some(texture);
                self.moon_loading = false;
                return;
            }
        }
        self.finished();
    }

    pub(in crate::app) fn planet_loaded(
        &mut self,
        planet: LoadedPlanet,
        ctx: &egui::Context,
        lang: Lang,
    ) {
        self.name = planet.name;
        self.viewer = Some(upload_image(ctx, "loaded-planet", planet.planet));
        self.sky = planet
            .sky
            .map(|image| upload_image(ctx, "loaded-sky", image));
        self.image_path.clear();
        self.sky_path.clear();
        self.image_bytes = Some(planet.assets.texture);
        self.sky_bytes = (!planet.assets.sky.is_empty()).then_some(planet.assets.sky);
        self.message = lang.text("explorer-planet-loaded").into();
        self.finished();
    }

    pub(in crate::app) fn planet_saved(&mut self, path: PathBuf, lang: Lang) {
        self.message = lang.format(
            "explorer-planet-saved",
            &[("path", path.display().to_string().into())],
        );
        self.finished();
    }

    pub(in crate::app) fn file_failed(&mut self, kind: FileKind, error: earthcraft_data::Error) {
        if kind == FileKind::Image(ImageTarget::Moon) {
            self.moon_loading = false;
        } else {
            self.pending = false;
            self.activate_on_completion = false;
        }
        self.message = format!("Explorer file operation failed: {error}");
    }
}

fn upload_image(ctx: &egui::Context, label: &str, image: image::RgbaImage) -> TextureHandle {
    let size = [image.width() as usize, image.height() as usize];
    ctx.load_texture(
        label,
        ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
        Default::default(),
    )
}
