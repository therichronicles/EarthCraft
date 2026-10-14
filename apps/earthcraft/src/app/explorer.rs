//! Optional explorer utilities. Does not modify the CPU globe renderer.
use super::App;
use earthcraft_core::body::Body;
use earthcraft_data::{
    links::photo_search,
    planet::{load_planet, save_planet},
};
use eframe::egui::{self, ColorImage, TextureHandle};
use std::{fs, path::Path, process::Command};

#[derive(Default)]
pub(super) struct Explorer {
    pub open: bool,
    name: String,
    image_path: String,
    sky_path: String,
    pub viewer: Option<TextureHandle>,
    pub active: Body,
    moon: Option<TextureHandle>,
    pub sky: Option<TextureHandle>,
    pub(super) message: String,
    tour_text: String,
    generated_tour_text: String,
}

fn texture(ctx: &egui::Context, file: &Path) -> Result<TextureHandle, String> {
    let data = fs::read(file).map_err(|e| e.to_string())?;
    let im = image::load_from_memory(&data)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let size = [im.width() as usize, im.height() as usize];
    Ok(ctx.load_texture(
        file.to_string_lossy(),
        ColorImage::from_rgba_unmultiplied(size, im.as_raw()),
        Default::default(),
    ))
}

fn texture_bytes(ctx: &egui::Context, label: &str, data: &[u8]) -> Result<TextureHandle, String> {
    let im = image::load_from_memory(data)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let size = [im.width() as usize, im.height() as usize];
    Ok(ctx.load_texture(
        label,
        ColorImage::from_rgba_unmultiplied(size, im.as_raw()),
        Default::default(),
    ))
}
impl Explorer {
    pub fn choose(&mut self, body: Body, ctx: &egui::Context) {
        self.active = body;
        let item = match body {
            Body::Moon => Some((
                &mut self.moon,
                "Moon",
                include_bytes!("../../assets/lroc_color_2k.jpg").as_slice(),
            )),
            _ => None,
        };
        if let Some((slot, label, bytes)) = item
            && slot.is_none()
        {
            *slot = texture_bytes(ctx, label, bytes).ok();
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
                    if ui.button(lg.text("explorer-earth")).clicked() {self.explorer.choose(Body::Earth,ctx)}
                    if ui.button(lg.text("explorer-moon")).clicked() {self.explorer.choose(Body::Moon,ctx)}
                    if ui.button(lg.text("explorer-custom-planet")).clicked() {self.explorer.choose(Body::Custom,ctx)}
                });
                ui.small(lg.text("explorer-moon-fixed"));
                ui.horizontal(|ui|{ui.label(lg.text("explorer-name"));ui.text_edit_singleline(&mut self.explorer.name);});
                ui.horizontal(|ui|{ui.label(lg.text("explorer-image"));ui.add(egui::TextEdit::singleline(&mut self.explorer.image_path).desired_width(200.0));
                    if ui.button(lg.text("explorer-browse")).clicked()&& let Some(p)=rfd::FileDialog::new().add_filter("Image", &["png","jpg","jpeg"]).pick_file(){self.explorer.image_path=p.to_string_lossy().into();}
                });
                if ui.button(lg.text("explorer-display-planet")).clicked() {
                    self.explorer.message=match texture(ctx,Path::new(&self.explorer.image_path)) {Ok(t)=>{self.explorer.viewer=Some(t);self.explorer.active=Body::Custom;lg.text("explorer-planet-displayed").into()},Err(e)=>e};
                }
                if let Some(t)=&self.explorer.viewer {ui.image((t.id(),egui::vec2(260.0,150.0)));}
                ui.horizontal(|ui| {
                    if ui.button(lg.text("explorer-save-planet")).clicked()
                        && let Some(path)=rfd::FileDialog::new().add_filter("EarthCraft planet", &["earthplanet"]).set_file_name("my-planet.earthplanet").save_file(){
                            self.explorer.message=save_planet(&path,&self.explorer.name,&self.explorer.image_path,&self.explorer.sky_path).unwrap_or_else(|e|e.to_string());
                        }
                    if ui.button(lg.text("explorer-load-planet")).clicked()
                        && let Some(path)=rfd::FileDialog::new().add_filter("EarthCraft planet", &["earthplanet"]).pick_file(){
                            self.explorer.message=match load_planet(&path){Ok(loaded)=>{
                                self.explorer.name=loaded.name;self.explorer.viewer=Some(upload_image(ctx,"loaded-planet",loaded.planet));self.explorer.sky=loaded.sky.map(|image|upload_image(ctx,"loaded-sky",image));self.explorer.active=Body::Custom;
                                lg.text("explorer-planet-loaded").into()
                            },Err(e)=>e.to_string()};
                        }
                });
                ui.separator();
                ui.heading(lg.text("explorer-sky-galaxy"));
                ui.horizontal(|ui|{ui.add(egui::TextEdit::singleline(&mut self.explorer.sky_path).hint_text(lg.text("explorer-image-path")).desired_width(210.0));
                    if ui.button(lg.text("explorer-browse")).clicked()&& let Some(p)=rfd::FileDialog::new().add_filter("Image", &["png","jpg","jpeg"]).pick_file(){self.explorer.sky_path=p.to_string_lossy().into();}
                });
                if ui.button(lg.text("explorer-display-galaxy")).clicked(){self.explorer.message=match texture(ctx,Path::new(&self.explorer.sky_path)){Ok(t)=>{self.explorer.sky=Some(t);lg.text("explorer-sky-applied").into()},Err(e)=>e};}
                if let Some(t)=&self.explorer.sky{ui.image((t.id(),egui::vec2(260.0,120.0)));}
                ui.small(lg.text("explorer-sky-preview"));
                if !self.explorer.message.is_empty(){ui.separator();ui.label(&self.explorer.message);}
            });
        self.explorer.open = open;
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
