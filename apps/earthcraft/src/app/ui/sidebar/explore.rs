use crate::app::App;
use crate::app::ui::paths::short_folder;
use eframe::egui;

impl App {
    pub(super) fn sidebar_explore(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        let mut goto = None;
        let mut shot = false;
        egui::CollapsingHeader::new(lg.text("navigation-section-title"))
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button(lg.text("navigation-reset-north-action"))
                        .clicked()
                    {
                        self.cam.heading = 0.0;
                    }
                    if ui
                        .button(lg.text("navigation-global-view-action"))
                        .clicked()
                    {
                        self.fly_to(self.cam.lat, self.cam.lon, 14_000.0);
                    }
                });
                if self.explorer.active == earthcraft_core::body::Body::Earth {
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut self.cam.tilt,
                                0.0..=earthcraft_core::camera::MAX_TILT_DEG,
                            )
                            .text(lg.text("navigation-tilt-label"))
                            .suffix("°"),
                        )
                        .on_hover_text(lg.text("navigation-tilt-help"))
                        .changed()
                    {
                        self.fly = None;
                        self.touch();
                    }
                    if ui
                        .button(lg.text("navigation-reset-view-action"))
                        .on_hover_text(lg.text("navigation-reset-view-help"))
                        .clicked()
                    {
                        self.fly = None;
                        self.cam.reset_view();
                        self.touch();
                    }
                }
                ui.checkbox(
                    &mut self.auto_rotate,
                    lg.text("layers-auto-rotation-toggle"),
                );
                if !self.results.is_empty() {
                    ui.separator();
                    ui.label(lg.text("search-results-label"));
                    for r in &self.results {
                        let short: String = r.name.chars().take(48).collect();
                        if ui.button(short).on_hover_text(&r.name).clicked() {
                            goto = Some((r.lat, r.lon, r.alt));
                        }
                    }
                }
            });

        super::super::ruler::section(ui, lg, self.ruler_reveal_requested, |ui| {
            ui.checkbox(&mut self.ruler, lg.text("ruler-enable-toggle"));
            let selected = super::super::ruler::unit_selector(
                ui,
                self.settings.ruler_unit,
                self.us_locale,
                lg,
            );
            if selected != self.settings.ruler_unit {
                self.settings.ruler_unit = selected;
            }
            if ui.button(lg.text("ruler-points-edit-action")).clicked() {
                self.ruler_paths.end_move();
                self.ruler_drag = None;
                self.ruler_point_editor = Some(super::super::ruler::PointEditor::new((
                    self.cam.lat,
                    self.cam.lon,
                )));
            }
            let model = self.active_earth_model();
            let total = self.ruler_paths.distance_km(model);
            ui.label(
                lg.format(
                    "ruler-measurement-summary",
                    &[
                        ("count", self.ruler_paths.point_count().to_string().into()),
                        (
                            "distance",
                            self.settings
                                .ruler_unit
                                .format_with_angle(
                                    total,
                                    self.ruler_paths.angular_distance_degrees(model),
                                    self.distance_units(),
                                    lg,
                                )
                                .into(),
                        ),
                    ],
                ),
            );
            ui.weak(lg.text(if cfg!(target_os = "macos") {
                "ruler-point-edit-help-macos"
            } else {
                "ruler-point-edit-help"
            }));
            if let Some(pair) = self.ruler_paths.last_leg() {
                let bearing = model
                    .initial_bearing(pair[0], pair[1])
                    .map(|bearing| earthcraft_core::compass::heading_text(bearing, lg))
                    .unwrap_or_else(|| "—".into());
                ui.label(lg.format("ruler-bearing-label", &[("bearing", bearing.into())]))
                    .on_hover_text(lg.text("ruler-bearing-tooltip"));
            }
            ui.horizontal(|ui| {
                for (command, label, enabled) in [
                    (
                        crate::app::ruler_history::HistoryCommand::Undo,
                        "edit-undo-action",
                        self.ruler_paths.can_undo(),
                    ),
                    (
                        crate::app::ruler_history::HistoryCommand::Redo,
                        "edit-redo-action",
                        self.ruler_paths.can_redo(),
                    ),
                ] {
                    if ui
                        .add_enabled(enabled, egui::Button::new(lg.text(label)))
                        .clicked()
                    {
                        self.apply_ruler_history(command, ui);
                    }
                }
            });
            if ui.button(lg.text("ruler-clear-action")).clicked() {
                self.ruler_paths.clear();
                self.ruler_drag = None;
            }
        });

        self.ruler_reveal_requested = false;

        egui::CollapsingHeader::new(lg.text("export-section-title"))
            .default_open(false)
            .show(ui, |ui| {
                if ui.button(lg.text("screenshot-save-action")).clicked() {
                    shot = true;
                }
                ui.weak(lg.format(
                    "export-folder-label",
                    &[("path", short_folder(&self.settings.screenshot_dir).into())],
                ))
                .on_hover_text(&self.settings.screenshot_dir);
                if ui.button(lg.text("settings-folders-action")).clicked() {
                    self.show_settings = true;
                }
            });

        ui.checkbox(&mut self.show_help, lg.text("controls-help-toggle"));
        if self.show_help {
            ui.label(lg.text("controls-help-text"));
            ui.label(lg.text("controls-tilt-help-text"));
        }
        if shot {
            self.save_screenshot();
        }
        if let Some((lat, lon, alt)) = goto {
            self.fly_to(lat, lon, alt);
        }
    }
}
