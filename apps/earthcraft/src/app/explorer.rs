//! Optional explorer utilities. Does not modify the CPU globe renderer.
mod files;
use super::App;
use earthcraft_core::body::Body;
use earthcraft_data::{
    files::{FileJobs, ImageSource, ImageTarget},
    links::photo_search,
};
use eframe::egui::{self, TextureHandle};
use std::{process::Command, sync::Arc};

#[derive(Default)]
pub(super) struct Explorer {
    pub open: bool,
    name: String,
    image_path: String,
    sky_path: String,
    image_bytes: Option<Arc<Vec<u8>>>,
    sky_bytes: Option<Arc<Vec<u8>>>,
    pending: bool,
    activate_on_completion: bool,
    moon_loading: bool,
    pub viewer: Option<TextureHandle>,
    pub active: Body,
    moon: Option<TextureHandle>,
    pub sky: Option<TextureHandle>,
    pub(super) message: String,
    tour_text: String,
    generated_tour_text: String,
}

impl Explorer {
    pub fn choose(&mut self, body: Body, jobs: &FileJobs) {
        self.active = body;
        self.activate_on_completion = false;
        if body == Body::Moon && self.moon.is_none() && !self.moon_loading {
            match jobs.load_image(
                ImageTarget::Moon,
                ImageSource::Embedded(include_bytes!("../../assets/lroc_color_2k.jpg")),
            ) {
                Ok(()) => self.moon_loading = true,
                Err(error) => self.message = format!("Moon image request failed: {error}"),
            }
        }
    }
    pub fn active_texture(&self) -> Option<&TextureHandle> {
        match self.active {
            Body::Moon => self.moon.as_ref(),
            Body::Custom => self.viewer.as_ref(),
            Body::Earth => None,
        }
    }
}
fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(url).spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
}

fn speak(text: &str, _lang: earthcraft_core::language::Lang) -> Result<(), String> {
    // System TTS, without uploading the narration to any cloud service.
    #[cfg(target_os = "macos")]
    let result = Command::new("say").arg(text).spawn();
    #[cfg(target_os="windows")]
    let result = Command::new("powershell").args(["-NoProfile", "-Command", "Add-Type -AssemblyName System.Speech; $s=New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Speak($args[0])", text]).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let result = Command::new("espeak")
        .args([
            "-v",
            if _lang == earthcraft_core::language::Lang::PtBr {
                "pt-br"
            } else {
                "en"
            },
            text,
        ])
        .spawn();
    result
        .map(|_| ())
        .map_err(|e| format!("TTS unavailable (install espeak on Linux): {e}"))
}

impl App {
    pub(super) fn explorer_window(&mut self, ctx: &egui::Context) {
        if !self.explorer.open {
            return;
        }
        let lg = self.lang;
        let mut open = self.explorer.open;
        egui::Window::new(lg.text("explorer-title"))
            .open(&mut open).default_width(390.0).resizable(true).vscroll(true)
            .show(ctx, |ui| {
                ui.heading(lg.text("explorer-selected-location"));
                let selected=self.results.first();
                let label=selected.map(|r| r.name.as_str()).unwrap_or(self.query.as_str());
                ui.label(if label.trim().is_empty() {lg.text("explorer-search-place")} else {label});
                if let Some(r)=selected {
                    ui.monospace(lg.format("explorer-latlon", &[("latitude",format!("{:.6}",r.lat).into()), ("longitude",format!("{:.6}",r.lon).into())]));
                } else {
                    ui.monospace(lg.format("explorer-center", &[("latitude",format!("{:.6}",self.cam.lat).into()), ("longitude",format!("{:.6}",self.cam.lon).into())]));
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button(lg.text("explorer-place-photos")).clicked() {self.explorer.message=open_url(&photo_search(label)).map(|_|lg.text("explorer-photos-opened").into()).unwrap_or_else(|e|e);}
                    if ui.button(lg.text("explorer-view-osm")).clicked() {
                        let (lat,lon)=selected.map(|r|(r.lat,r.lon)).unwrap_or((self.cam.lat,self.cam.lon));
                        self.explorer.message=open_url(&format!("https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=16/{lat}/{lon}")).map(|_|lg.text("explorer-map-opened").into()).unwrap_or_else(|e|e);
                    }
                });
                ui.small(lg.text("explorer-photo-copyright"));
                ui.separator();
                ui.heading(lg.text("explorer-narrated-tour"));
                if !label.trim().is_empty() &&
                    (self.explorer.tour_text.is_empty() || self.explorer.tour_text == self.explorer.generated_tour_text) {
                    // Preserve user-written narration; only refresh our autogenerated message.
                    let generated = lg.format("explorer-tour-default", &[("place",label.into())]);
                    self.explorer.tour_text = generated.clone();
                    self.explorer.generated_tour_text = generated;
                }
                ui.add(egui::TextEdit::multiline(&mut self.explorer.tour_text).desired_rows(3));
                if ui.button(lg.text("explorer-read-aloud")).clicked(){self.explorer.message=speak(&self.explorer.tour_text,lg).map(|_|lg.text("explorer-speech-sent").into()).unwrap_or_else(|e|e);}
                ui.separator();
                ui.heading(lg.text("explorer-globe-layers"));
                ui.small(lg.text("explorer-layer-info"));
                ui.checkbox(&mut self.show_flights, lg.text("layers-flights-toggle"));
                if self.show_flights {
                    ui.small(self.live_aircraft.status(lg == earthcraft_core::language::Lang::PtBr));
                    ui.hyperlink_to("OpenSky Network", "https://opensky-network.org/");
                    ui.hyperlink_to("ADSB.lol (ODbL)", "https://www.adsb.lol/");
                }
                ui.checkbox(&mut self.show_weather, lg.text("layers-weather-toggle"));
                ui.checkbox(&mut self.show_clouds, lg.text("layers-clouds-toggle"));
                if self.show_clouds || self.show_weather {
                    ui.small(self.live_maps.status(self.show_clouds, self.show_weather, lg));
                    if self.show_clouds {
                        ui.label(lg.text("explorer-cloud-credit"));
                        ui.hyperlink_to(lg.text("explorer-cloud-source"), "https://github.com/matteason/live-cloud-maps");
                        ui.small(lg.text("explorer-cloud-license"));
                    }
                    if self.show_weather {
                        ui.label(lg.text("explorer-radar-credit"));
                        ui.hyperlink_to(lg.text("explorer-radar-source"), "https://www.rainviewer.com/");
                        ui.small(lg.text("explorer-radar-explanation"));
                    }
                }
                ui.separator();
                ui.heading(lg.text("explorer-bodies"));
                ui.horizontal(|ui| {
                    for (body, label) in [
                        (Body::Earth, "explorer-earth"),
                        (Body::Moon, "explorer-moon"),
                        (Body::Custom, "explorer-custom-planet"),
                    ] {
                        if ui.button(lg.text(label)).clicked() {
                            self.explorer.choose(body, &self.file_jobs);
                        }
                    }
                });
                ui.small(lg.text("explorer-moon-fixed"));
                if self.explorer.pending || self.explorer.moon_loading {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(lg.text("explorer-file-working"));
                    });
                }
                ui.add_enabled_ui(!self.explorer.pending, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(lg.text("explorer-name"));
                        ui.text_edit_singleline(&mut self.explorer.name);
                    });
                    ui.horizontal(|ui| {
                        ui.label(lg.text("explorer-image"));
                        let hint = if self.explorer.image_bytes.is_some() {
                            lg.text("explorer-embedded-image")
                        } else {
                            ""
                        };
                        if ui.add(egui::TextEdit::singleline(&mut self.explorer.image_path)
                            .hint_text(hint).desired_width(200.0)).changed()
                        {
                            self.explorer.image_bytes = None;
                        }
                        if ui.button(lg.text("explorer-browse")).clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg"]).pick_file()
                        {
                            self.explorer.image_path = path.to_string_lossy().into();
                            self.explorer.image_bytes = None;
                        }
                    });
                    if ui.button(lg.text("explorer-display-planet")).clicked() {
                        self.explorer.request_image(ImageTarget::Planet, &self.file_jobs);
                    }
                    if let Some(texture) = &self.explorer.viewer {
                        ui.image((texture.id(), egui::vec2(260.0, 150.0)));
                    }
                    ui.horizontal(|ui| {
                        if ui.button(lg.text("explorer-save-planet")).clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("EarthCraft planet", &["earthplanet"])
                                .set_file_name("my-planet.earthplanet").save_file()
                        {
                            self.explorer.request_save(path, &self.file_jobs);
                        }
                        if ui.button(lg.text("explorer-load-planet")).clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("EarthCraft planet", &["earthplanet"]).pick_file()
                        {
                            self.explorer.request_open(path, &self.file_jobs);
                        }
                    });
                    ui.separator();
                    ui.heading(lg.text("explorer-sky-galaxy"));
                    ui.horizontal(|ui| {
                        let hint = lg.text(if self.explorer.sky_bytes.is_some() {
                            "explorer-embedded-image"
                        } else {
                            "explorer-image-path"
                        });
                        if ui.add(egui::TextEdit::singleline(&mut self.explorer.sky_path)
                            .hint_text(hint).desired_width(210.0)).changed()
                        {
                            self.explorer.sky_bytes = None;
                        }
                        if ui.button(lg.text("explorer-browse")).clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg"]).pick_file()
                        {
                            self.explorer.sky_path = path.to_string_lossy().into();
                            self.explorer.sky_bytes = None;
                        }
                    });
                    if ui.button(lg.text("explorer-display-galaxy")).clicked() {
                        self.explorer.request_image(ImageTarget::Sky, &self.file_jobs);
                    }
                    if let Some(texture) = &self.explorer.sky {
                        ui.image((texture.id(), egui::vec2(260.0, 120.0)));
                    }
                    ui.small(lg.text("explorer-sky-preview"));
                });
                if !self.explorer.message.is_empty(){ui.separator();ui.label(&self.explorer.message);}
            });
        self.explorer.open = open;
    }
}
