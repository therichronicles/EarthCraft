use super::App;
use earthcraft_core::{language::Lang, math::now_unix};
use earthcraft_data::files::{FileCompletion, FileKind, FileOutcome};
use eframe::egui;
use std::path::PathBuf;
/// A file status keeps its full path for hover while the toolbar abbreviates it.
pub(super) struct FileStatus {
    pub(super) message: String,
    pub(super) path: PathBuf,
}
impl App {
    pub(super) fn resolve_kml_path(&self, p: &str) -> PathBuf {
        let path = PathBuf::from(p.trim());
        if path.is_absolute() {
            path
        } else {
            PathBuf::from(&self.settings.kml_dir).join(path)
        }
    }

    pub(super) fn save_kml_to(&mut self, path: PathBuf) {
        if let Err(error) = self.file_jobs.save_kml(path, &self.placemarks) {
            self.status = Lang::En.format("kml-save-error", &[("error", error.to_string().into())]);
        }
    }

    pub(super) fn load_kml_from(&mut self, path: &std::path::Path) {
        if let Err(error) = self.file_jobs.open_kml(path.to_path_buf(), self.lang) {
            self.status = Lang::En.format("kml-open-error", &[("error", error.to_string().into())]);
        }
    }

    pub(super) fn poll_files(&mut self, ctx: &egui::Context) {
        while let Ok(FileCompletion { kind, result }) = self.file_results.try_recv() {
            match result {
                Ok(FileOutcome::ImageLoaded { target, image }) => {
                    self.explorer.image_loaded(target, image, ctx, self.lang);
                }
                Ok(FileOutcome::PlanetLoaded { planet }) => {
                    self.explorer.planet_loaded(planet, ctx, self.lang);
                }
                Ok(FileOutcome::PlanetSaved { path }) => {
                    self.explorer.planet_saved(path, self.lang);
                }
                Ok(FileOutcome::KmlSaved { path }) => {
                    self.kml_path = path.to_string_lossy().into_owned();
                    self.status = self.lang.format(
                        "kml-save-success",
                        &[("path", path.display().to_string().into())],
                    );
                    self.status_file = Some(FileStatus {
                        message: self.status.clone(),
                        path,
                    });
                }
                Ok(FileOutcome::KmlLoaded { path, placemarks }) => {
                    self.status = self.lang.format(
                        "kml-import-success",
                        &[
                            ("count", placemarks.len().into()),
                            ("path", path.display().to_string().into()),
                        ],
                    );
                    self.status_file = Some(FileStatus {
                        message: self.status.clone(),
                        path: path.clone(),
                    });
                    self.kml_path = path.to_string_lossy().into_owned();
                    if let Some(first) = placemarks.first() {
                        self.fly_to(
                            first.lat,
                            first.lon,
                            if placemarks.len() == 1 { 3.0 } else { 2000.0 },
                        );
                    }
                    self.placemarks.extend(placemarks);
                }
                Ok(FileOutcome::ScreenshotSaved { path }) => {
                    self.status = self.lang.format(
                        "screenshot-save-success",
                        &[("path", path.display().to_string().into())],
                    );
                    self.status_file = Some(FileStatus {
                        message: self.status.clone(),
                        path,
                    });
                }
                Err(error) => {
                    let id = match kind {
                        FileKind::OpenKml => "kml-open-error",
                        FileKind::SaveKml => "kml-save-error",
                        FileKind::Screenshot => "screenshot-save-error",
                        FileKind::Image(_) | FileKind::OpenPlanet | FileKind::SavePlanet => {
                            self.explorer.file_failed(kind, error);
                            continue;
                        }
                    };
                    self.status = Lang::En.format(id, &[("error", error.to_string().into())]);
                }
            }
        }
    }

    pub(super) fn pick_kml_open(&mut self) {
        let dir = PathBuf::from(&self.settings.kml_dir);
        let mut dlg = rfd::FileDialog::new()
            .add_filter("KML", &["kml", "KML"])
            .set_title(self.lang.text("kml-open-dialog-title"));
        if dir.is_dir() {
            dlg = dlg.set_directory(&dir);
        }
        if let Some(path) = dlg.pick_file() {
            self.load_kml_from(&path);
        }
    }

    pub(super) fn pick_kml_save_as(&mut self) {
        let dir = PathBuf::from(&self.settings.kml_dir);
        let name = std::path::Path::new(&self.kml_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("placemarks.kml")
            .to_string();
        let mut dlg = rfd::FileDialog::new()
            .add_filter("KML", &["kml"])
            .set_file_name(name)
            .set_title(self.lang.text("kml-save-dialog-title"));
        if dir.is_dir() {
            dlg = dlg.set_directory(&dir);
        }
        if let Some(mut path) = dlg.save_file() {
            if path.extension().is_none() {
                path.set_extension("kml");
            }
            self.save_kml_to(path);
        }
    }

    pub(super) fn save_screenshot(&mut self) {
        match &self.image {
            Some((width, height, pixels)) => {
                let path = PathBuf::from(&self.settings.screenshot_dir)
                    .join(format!("earthcraft_{}.png", now_unix() as u64));
                if let Err(error) =
                    self.file_jobs
                        .screenshot(path, *width as u32, *height as u32, pixels)
                {
                    self.status = Lang::En.format(
                        "screenshot-save-error",
                        &[("error", error.to_string().into())],
                    );
                }
            }
            None => self.status = Lang::En.text("screenshot-empty-error").into(),
        }
    }

    pub(super) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            let path = f.path();
            let is_kml = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("kml"));
            if is_kml {
                self.load_kml_from(path);
            } else {
                self.status = Lang::En.text("kml-drop-type-error").into();
            }
        }
    }
}
