//! About and controls help use normal app-owned egui windows.
use crate::app::App;
use eframe::egui;
impl App {
    pub(in crate::app) fn menu_windows(&mut self, ctx: &egui::Context) {
        let lang = self.lang;
        egui::Window::new(lang.text("menu-about-action"))
            .id(egui::Id::new("about-earthcraft"))
            .open(&mut self.show_about)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("EarthCraft");
                ui.label(lang.format(
                    "about-version-label",
                    &[("version", env!("CARGO_PKG_VERSION").into())],
                ));
                ui.label(lang.text("about-description"));
                ui.hyperlink_to(
                    lang.text("about-project-link"),
                    "https://github.com/therichronicles/EarthCraft",
                );
                ui.label(env!("CARGO_PKG_LICENSE"));
            });
        egui::Window::new(lang.text("menu-controls-help"))
            .id(egui::Id::new("controls-help"))
            .open(&mut self.show_shortcuts)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label(lang.text("controls-help-text"));
                ui.label(lang.text("controls-tilt-help-text"));
                ui.separator();
                for action in super::Action::ALL {
                    if let Some(shortcut) = action.shortcut() {
                        ui.horizontal(|ui| {
                            ui.label(action.label(lang));
                            ui.monospace(
                                shortcut
                                    .format(&egui::ModifierNames::NAMES, cfg!(target_os = "macos")),
                            );
                        });
                    }
                }
            });
    }
}
