use super::MAX_UI_SLIDER_SCALE;
use crate::app::App;
use earthcraft_core::language::{Lang, LanguagePreference};
use earthcraft_data::settings::{MAX_MAP_SCALE, MIN_MAP_SCALE, MIN_UI_SCALE, UnitPreference};
use eframe::egui;

impl App {
    pub(super) fn settings_general(
        &mut self,
        ui: &mut egui::Ui,
        language_preference: &mut LanguagePreference,
    ) {
        let lg = self.lang;
        ui.horizontal(|ui| {
            ui.label(lg.text("settings-language-label"));
            let choices = LanguagePreference::ALL
                .map(|choice| (choice, choice.label(lg, &self.system_locale)));
            egui::ComboBox::from_id_salt("settings_language")
                .selected_text((*language_preference).label(lg, &self.system_locale))
                .show_ui(ui, |ui| {
                    super::super::selection::size_menu(
                        ui,
                        choices.iter().map(|(_, label)| label.as_str()),
                    );
                    for (choice, label) in &choices {
                        super::super::selection::full_width_value(
                            ui,
                            language_preference,
                            *choice,
                            label.as_str(),
                        );
                    }
                });
        });
    }
    pub(super) fn settings_appearance(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        ui.horizontal(|ui| {
            ui.label(lg.text("settings-theme-label"));
            let choices = earthcraft_data::settings::appearance::ThemePreference::ALL;
            let name = |theme| {
                lg.text(match theme {
                    earthcraft_data::settings::appearance::ThemePreference::System => {
                        "settings-theme-system"
                    }
                    earthcraft_data::settings::appearance::ThemePreference::Dark => {
                        "settings-theme-dark"
                    }
                    earthcraft_data::settings::appearance::ThemePreference::Light => {
                        "settings-theme-light"
                    }
                })
            };
            egui::ComboBox::from_id_salt("theme")
                .selected_text(name(self.settings.appearance.theme))
                .show_ui(ui, |ui| {
                    super::super::selection::size_menu(ui, choices.map(name));
                    for choice in choices {
                        super::super::selection::full_width_value(
                            ui,
                            &mut self.settings.appearance.theme,
                            choice,
                            name(choice),
                        );
                    }
                });
        });
        for (id, label, selected) in [
            (
                "ui-font",
                "settings-ui-font-label",
                &mut self.settings.appearance.ui_font,
            ),
            (
                "map-font",
                "settings-map-font-label",
                &mut self.settings.appearance.map_font,
            ),
        ] {
            ui.horizontal(|ui| {
                ui.label(lg.text(label));
                egui::ComboBox::from_id_salt(id)
                    .selected_text(if selected.is_empty() {
                        lg.text("settings-font-system")
                    } else {
                        selected.as_str()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            selected,
                            String::new(),
                            lg.text("settings-font-system"),
                        );
                        for family in &self.fonts.families {
                            ui.selectable_value(selected, family.clone(), family);
                        }
                    });
            });
        }
        if self.fonts.discovering {
            ui.spinner();
            ui.weak(lg.text("settings-fonts-loading"));
        }
        if let Some(error) = &self.fonts.error {
            ui.label(error);
        }
        ui.horizontal(|ui| {
            ui.label(lg.text("settings-distance-units-label"));
            let label = |units| match units {
                UnitPreference::System => lg.format(
                    "settings-units-system",
                    &[(
                        "unit",
                        match self.system_units {
                            earthcraft_core::units::DistanceUnits::Metric => "km",
                            earthcraft_core::units::DistanceUnits::Imperial => "mi",
                        }
                        .into(),
                    )],
                ),
                UnitPreference::Metric => lg.text("settings-units-metric").to_owned(),
                UnitPreference::Imperial => lg.text("settings-units-imperial").to_owned(),
            };
            let choices = [
                UnitPreference::System,
                UnitPreference::Metric,
                UnitPreference::Imperial,
            ]
            .map(|units| (units, label(units)));
            egui::ComboBox::from_id_salt("distance_units")
                .selected_text(label(self.settings.distance_units))
                .show_ui(ui, |ui| {
                    super::super::selection::size_menu(
                        ui,
                        choices.iter().map(|(_, label)| label.as_str()),
                    );
                    for (units, label) in &choices {
                        super::super::selection::full_width_value(
                            ui,
                            &mut self.settings.distance_units,
                            *units,
                            label.as_str(),
                        );
                    }
                });
        });
        self.settings_earth_model(ui);
        super::super::coordinate_display::controls(ui, &mut self.settings.coordinate_display, lg);
        ui.strong(lg.text("settings-text-section-title"));
        egui::Grid::new("settings_text")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(lg.text("settings-ui-scale-label"));
                // The window auto-sizes to its (scaled) content, so changing the scale while dragging
                // moves the slider under the cursor and feeds back into itself (shaking).
                // Fix: only commit the new scale when the drag ends.
                let r = super::super::scale_slider::relative_scale_slider(
                    ui,
                    &mut self.ui_scale_edit,
                    MIN_UI_SCALE,
                    MAX_UI_SLIDER_SCALE,
                );
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    self.settings.ui_scale = self.ui_scale_edit;
                }
                ui.end_row();
                ui.label(lg.text("settings-map-scale-label"));
                super::super::scale_slider::relative_scale_slider(
                    ui,
                    &mut self.settings.map_scale,
                    MIN_MAP_SCALE,
                    MAX_MAP_SCALE,
                );
                ui.end_row();
            });
    }
    fn settings_earth_model(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        ui.horizontal(|ui| {
            ui.label(lg.text("settings-earth-model-label"));
            let models = earthcraft_core::earth_model::EarthModel::ALL;
            egui::ComboBox::from_id_salt("earth_model")
                .selected_text(lg.text(self.settings.earth_model.name_id()))
                .show_ui(ui, |ui| {
                    super::super::selection::size_menu(
                        ui,
                        models.into_iter().map(|model| lg.text(model.name_id())),
                    );
                    for model in models {
                        super::super::selection::full_width_value(
                            ui,
                            &mut self.settings.earth_model,
                            model,
                            lg.text(model.name_id()),
                        )
                        .on_hover_text(lg.text(model.help_id()));
                    }
                });
        });
    }
    pub(super) fn settings_graphics(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        ui.checkbox(
            &mut self.settings.uncap_fps,
            lg.text("settings-uncap-fps-toggle"),
        );
        ui.weak(lg.text("settings-frame-sync-help"));
    }
    pub(super) fn settings_files(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        let folder_row = |ui: &mut egui::Ui, label: &str, value: &mut String, title: &str| {
            ui.label(label);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(value).desired_width(300.0));
                if ui
                    .button(lg.text("settings-folder-browse-action"))
                    .clicked()
                {
                    let mut dlg = rfd::FileDialog::new().set_title(title);
                    if std::path::Path::new(value.as_str()).is_dir() {
                        dlg = dlg.set_directory(value.as_str());
                    }
                    if let Some(dir) = dlg.pick_folder() {
                        *value = dir.to_string_lossy().into_owned();
                    }
                }
            });
        };
        folder_row(
            ui,
            lg.text("settings-screenshot-folder-label"),
            &mut self.settings.screenshot_dir,
            lg.text("settings-screenshot-folder-dialog-title"),
        );
        folder_row(
            ui,
            lg.text("settings-kml-folder-label"),
            &mut self.settings.kml_dir,
            lg.text("settings-kml-folder-dialog-title"),
        );
        ui.weak(lg.text("settings-folder-creation-help"));
    }
    pub(super) fn settings_cache(&mut self, ui: &mut egui::Ui, confirm_cache: &mut bool) {
        let lg = self.lang;
        let ctx = ui.ctx().clone();
        ui.label(lg.text("settings-cache-budgets-label"));
        for (label, value, max) in [
            (
                "settings-cache-decoded-label",
                &mut self.settings.cache.decoded_mib,
                earthcraft_data::settings::cache::MAX_DECODED_CACHE_MIB,
            ),
            (
                "settings-cache-disk-label",
                &mut self.settings.cache.disk_mib,
                earthcraft_data::settings::cache::MAX_DISK_CACHE_MIB,
            ),
        ] {
            ui.horizontal(|ui| {
                ui.label(lg.text(label));
                ui.add(
                    egui::DragValue::new(value)
                        .range(earthcraft_data::settings::cache::MIN_CACHE_MIB..=max)
                        .suffix(" MiB"),
                );
            });
        }
        ui.weak(lg.text("settings-cache-restart-help"));
        ui.label(lg.format(
            "settings-cache-resident-label",
            &[(
                "size",
                crate::app::cache::cache_size_text(self.cache.resident_bytes() as u64).into(),
            )],
        ));
        ui.separator();
        ui.weak(lg.text("settings-cache-clear-help"));
        let clearing = self.cache_controls.clearing();
        ui.horizontal(|ui| {
            let label = lg.text(if clearing {
                "settings-cache-clearing-label"
            } else {
                "settings-cache-clear-action"
            });
            if ui
                .add_enabled(!clearing, egui::Button::new(label))
                .clicked()
            {
                *confirm_cache = true;
            }
            if clearing {
                ui.spinner();
            }
            match &self.cache_controls.usage {
                Some(Ok(bytes)) => {
                    ui.label(lg.format(
                        "settings-cache-disk-usage",
                        &[("size", crate::app::cache::cache_size_text(*bytes).into())],
                    ));
                }
                Some(Err(error)) => {
                    ui.label(Lang::En.text("settings-cache-usage-unavailable"))
                        .on_hover_text(error);
                }
                None => {
                    ui.weak(lg.text("settings-cache-measuring-label"));
                }
            }
        });
        if let Some(error) = self.cache.disk_error() {
            ui.label(Lang::En.format(
                "settings-cache-persistence-error",
                &[("error", error.into())],
            ));
        }
        if *confirm_cache {
            ui.label(lg.text("settings-cache-confirm"));
            ui.horizontal(|ui| {
                if ui.button(lg.text("settings-confirm-action")).clicked() {
                    self.clear_disk_cache(&ctx);
                    *confirm_cache = false;
                }
                if ui.button(lg.text("settings-cancel-action")).clicked() {
                    *confirm_cache = false;
                }
            });
        }
    }
    pub(super) fn settings_navigation(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        ui.checkbox(
            &mut self.settings.hide_navigation_controls,
            lg.text("settings-hide-navigation-controls-toggle"),
        );
        egui::Grid::new("navigation-sensitivity")
            .num_columns(2)
            .show(ui, |ui| {
                for (label, value) in [
                    (
                        "settings-pan-sensitivity",
                        &mut self.settings.navigation.pan_sensitivity,
                    ),
                    (
                        "settings-rotation-sensitivity",
                        &mut self.settings.navigation.rotation_sensitivity,
                    ),
                    (
                        "settings-zoom-sensitivity",
                        &mut self.settings.navigation.zoom_sensitivity,
                    ),
                ] {
                    ui.label(lg.text(label));
                    ui.add(
                        egui::Slider::new(
                            value,
                            earthcraft_data::settings::navigation::MIN_SENSITIVITY
                                ..=earthcraft_data::settings::navigation::MAX_SENSITIVITY,
                        )
                        .suffix("×"),
                    );
                    ui.end_row();
                }
            });
        ui.checkbox(
            &mut self.settings.navigation.invert_scroll,
            lg.text("settings-invert-scroll"),
        );
        ui.separator();
        ui.label(lg.text("settings-startup-view-label"));
        let camera = self.settings.navigation.startup.camera();
        match self
            .settings
            .coordinate_display
            .format(camera.lat, camera.lon)
        {
            Ok(position) => {
                ui.label(position);
            }
            Err(error) => {
                ui.label(error.to_string());
            }
        }
        ui.horizontal(|ui| {
            if ui
                .button(lg.text("settings-startup-current-action"))
                .clicked()
            {
                self.settings.navigation.startup =
                    earthcraft_data::settings::navigation::StartupView::from_camera(self.cam);
            }
            if ui
                .button(lg.text("settings-startup-default-action"))
                .clicked()
            {
                self.settings.navigation.startup =
                    earthcraft_data::settings::navigation::StartupView::default();
            }
        });
        ui.weak(lg.text("settings-startup-help"));
    }
}
