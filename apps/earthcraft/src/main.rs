// EarthCraft — 3D globe in the style of Google Earth / globo 3D estilo Google Earth (Windows, macOS, Linux)
//
// Rendering: CPU ray-casting (parallel via rayon) of a sphere textured with
// Web Mercator tiles downloaded on demand (Esri / OSM / OpenTopoMap).
// UI: egui/eframe.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod presentation;
use app::App;
use eframe::egui;
const INITIAL_WINDOW_SIZE_POINTS: [f32; 2] = [1280.0, 800.0];
const APP_ICON_PNG: &[u8] = std::cfg_select! {
    target_os = "macos" => include_bytes!("../../../assets/macos/icon.png"),
    _ => include_bytes!("../../../assets/icon_256.png"),
};

fn main() -> Result<(), eframe::Error> {
    #[cfg(feature = "profiling")]
    earthcraft_data::profiling::init();
    let settings = earthcraft_data::settings::Settings::load();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(INITIAL_WINDOW_SIZE_POINTS)
        .with_title("EarthCraft")
        .with_app_id("earthcraft");
    if let Ok(img) = image::load_from_memory(APP_ICON_PNG) {
        let img = img.to_rgba8();
        viewport = viewport.with_icon(egui::IconData {
            width: img.width(),
            height: img.height(),
            rgba: img.into_raw(),
        });
    }
    let options = eframe::NativeOptions {
        viewport,
        wgpu_options: presentation::configuration(settings.uncap_fps),
        ..Default::default()
    };
    eframe::run_native(
        "EarthCraft",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, settings)))),
    )
}
