use super::super::App;
use super::paths::short_folder;
use earthcraft_data::tiles::Provider;
use eframe::egui::{self, Color32, Key, Stroke};
pub(super) const DEFAULT_SIDEBAR_WIDTH_POINTS: f32 = 270.0;
impl App {
    pub(in crate::app) fn side_panel(&mut self, ui: &mut egui::Ui) {
        let openness =
            ui.animate_bool_responsive(egui::Id::new("side").with("animation"), self.sidebar_open);
        self.sidebar_animating = openness != if self.sidebar_open { 1.0 } else { 0.0 };
        let mut expanded = self.sidebar_open;
        let lg = self.lang;
        let mut goto: Option<(f64, f64, f64)> = None;
        let (mut save_now, mut save_as, mut open_dialog, mut open_typed, mut shot) =
            (false, false, false, false, false);
        self.kml_drop_rect = egui::Rect::NOTHING;
        egui::Panel::show_switched(
            ui,
            &mut expanded,
            egui::Panel::left("side_collapsed")
                .frame(egui::Frame::NONE)
                .exact_size(0.0),
            expanded_sidebar_panel(),
            |ui, _expanded| {
                if self.sidebar_animating || !self.sidebar_open {
                    return;
                }
                sidebar_scroll_area().show(ui, |ui| {
                    egui::CollapsingHeader::new(lg.text("layers-section-title"))
                        .default_open(true)
                        .show(ui, |ui| {
                            for p in Provider::ALL {
                                ui.radio_value(&mut self.provider, p, p.name(lg));
                            }
                            if self.provider == Provider::Wayback {
                                ui.indent("wayback_ui", |ui| {
                                    if self.wayback_list.is_empty() {
                                        ui.weak(lg.text("wayback-loading-status"));
                                    } else {
                                        let n = self.wayback_list.len();
                                        let mut idx = self.wayback_idx;
                                        ui.horizontal(|ui| {
                                            if ui
                                                .small_button("◀")
                                                .on_hover_text(lg.text("wayback-older-tooltip"))
                                                .clicked()
                                                && idx + 1 < n
                                            {
                                                idx += 1;
                                            }
                                            egui::ComboBox::from_id_salt("wayback_date")
                                                .selected_text(self.wayback_list[idx].date.clone())
                                                .height(320.0)
                                                .show_ui(ui, |ui| {
                                                    super::selection::size_menu(
                                                        ui,
                                                        self.wayback_list
                                                            .iter()
                                                            .map(|release| release.date.as_str()),
                                                    );
                                                    for (i, r) in
                                                        self.wayback_list.iter().enumerate()
                                                    {
                                                        super::selection::full_width_value(
                                                            ui, &mut idx, i, &r.date,
                                                        );
                                                    }
                                                });
                                            if ui
                                                .small_button("▶")
                                                .on_hover_text(lg.text("wayback-newer-tooltip"))
                                                .clicked()
                                                && idx > 0
                                            {
                                                idx -= 1;
                                            }
                                        });
                                        ui.weak(lg.format(
                                            "wayback-availability-status",
                                            &[("count", n.to_string().into())],
                                        ));
                                        ui.weak(lg.text("wayback-fallback-help"));
                                        self.set_wayback(idx);
                                    }
                                    if let Some(e) = &self.wayback_err {
                                        ui.colored_label(
                                            Color32::from_rgb(255, 120, 120),
                                            format!("✖ {e}"),
                                        );
                                    }
                                });
                            }
                            ui.separator();
                            ui.checkbox(&mut self.show_grid, lg.text("layers-grid-toggle"));
                            if self.show_grid {
                                ui.indent("grid_numbers", |ui| {
                                    ui.checkbox(
                                        &mut self.settings.grid_numbers,
                                        lg.text("layers-grid-numbers-toggle"),
                                    );
                                });
                            }
                            ui.checkbox(
                                &mut self.show_country_boundaries,
                                lg.text("layers-country-borders-toggle"),
                            );
                            ui.checkbox(
                                &mut self.show_state_boundaries,
                                lg.text("layers-state-borders-toggle"),
                            );
                            ui.checkbox(
                                &mut self.show_place_labels,
                                lg.text("layers-place-labels-toggle"),
                            );
                            ui.checkbox(
                                &mut self.show_city_boundaries,
                                lg.text("layers-municipal-borders-toggle"),
                            );
                            ui.checkbox(
                                &mut self.show_metro_boundaries,
                                lg.text("layers-metro-borders-toggle"),
                            );
                            if self.boundary_loader.is_loading() {
                                ui.weak(lg.text("boundaries-loading-status"));
                            }
                            if let Ok(log) = self.boundary_log.lock() {
                                for line in log.iter() {
                                    ui.weak(egui::RichText::new(line).small());
                                }
                            }
                            ui.checkbox(&mut self.show_atmo, lg.text("layers-atmosphere-toggle"));
                            ui.checkbox(&mut self.show_sun, lg.text("layers-sunlight-toggle"));
                            ui.checkbox(
                                &mut self.auto_rotate,
                                lg.text("layers-auto-rotation-toggle"),
                            );
                            ui.checkbox(&mut self.show_vessels, lg.text("layers-vessels-toggle"));
                            if self.show_vessels {
                                ui.small(
                                    self.live_vessels
                                        .status(lg == earthcraft_core::language::Lang::PtBr),
                                );
                                ui.hyperlink_to("OpenSeaFeed AIS", "https://openseafeed.com/");
                                ui.hyperlink_to(
                                    "Fintraffic (contingência)",
                                    "https://www.digitraffic.fi/en/marine-traffic/",
                                );
                            }
                            ui.checkbox(
                                &mut self.show_satellites,
                                lg.text("layers-satellites-toggle"),
                            );
                            if self.show_satellites {
                                ui.small(
                                    self.live_satellites
                                        .status(lg == earthcraft_core::language::Lang::PtBr),
                                );
                                ui.hyperlink_to("CelesTrak", "https://celestrak.org/");
                            }
                            ui.checkbox(&mut self.show_roads, lg.text("layers-roads-toggle"));
                            if self.show_roads {
                                ui.small(self.roads.service.status(
                                    self.cam.alt,
                                    lg == earthcraft_core::language::Lang::PtBr,
                                ));
                                ui.hyperlink_to(
                                    "© OpenStreetMap contributors (ODbL)",
                                    "https://www.openstreetmap.org/copyright",
                                );
                            }
                            ui.checkbox(&mut self.show_traffic, lg.text("layers-traffic-toggle"));
                            if self.show_traffic {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.tomtom_key)
                                        .password(true)
                                        .hint_text(lg.text("layers-traffic-key")),
                                );
                                ui.small(self.traffic.service.status(
                                    &self.tomtom_key,
                                    self.cam.alt,
                                    lg == earthcraft_core::language::Lang::PtBr,
                                ));
                                ui.hyperlink_to("TomTom Traffic", "https://developer.tomtom.com/");
                            }
                            ui.checkbox(&mut self.show_weather, lg.text("layers-weather-toggle"));
                            if self.show_weather {
                                ui.hyperlink_to(
                                    lg.text("sidebar-radar-source"),
                                    "https://www.rainviewer.com/",
                                );
                            }
                            ui.checkbox(&mut self.show_clouds, lg.text("layers-clouds-toggle"));
                            if self.show_clouds {
                                ui.hyperlink_to(
                                    lg.text("sidebar-cloud-source"),
                                    "https://github.com/matteason/live-cloud-maps",
                                );
                            }
                            ui.checkbox(&mut self.show_flights, lg.text("layers-flights-toggle"));
                            if self.show_flights {
                                super::aircraft::draw_status(
                                    ui,
                                    &self.live_aircraft.snapshot(),
                                    lg,
                                );
                                ui.hyperlink_to("OpenSky Network", "https://opensky-network.org/");
                                ui.hyperlink_to("ADSB.lol (ODbL)", "https://www.adsb.lol/");
                            }
                            ui.small(lg.text("layers-live-overlays-help"));
                        });

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

                    egui::CollapsingHeader::new(lg.text("placemarks-section-title"))
                        .default_open(true)
                        .show(ui, |ui| {
                            if let Some(target) = self.places_tree(ui) {
                                goto = Some(target);
                            }
                            ui.separator();
                            // Drop zone: drag a .kml file from the file manager onto this box.
                            let hovering_file = ui.input(|i| !i.raw.hovered_files.is_empty());
                            let (stroke_col, fill) = if hovering_file {
                                (
                                    Color32::from_rgb(90, 170, 255),
                                    Color32::from_rgba_unmultiplied(60, 120, 220, 45),
                                )
                            } else {
                                (Color32::from_gray(90), Color32::from_black_alpha(40))
                            };
                            let zone = egui::Frame::NONE
                                .fill(fill)
                                .stroke(Stroke::new(
                                    if hovering_file { 2.0_f32 } else { 1.0_f32 },
                                    stroke_col,
                                ))
                                .corner_radius(6.0)
                                .inner_margin(egui::Margin::same(8))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.vertical_centered(|ui| {
                                        ui.label(lg.text("kml-drop-zone-hint"));
                                    });
                                    let r = ui.add(
                                        egui::TextEdit::singleline(&mut self.kml_path)
                                            .desired_width(f32::INFINITY),
                                    );
                                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                        open_typed = true;
                                    }
                                });
                            self.kml_drop_rect = zone.response.rect;
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .button(lg.text("kml-save-action"))
                                    .on_hover_text(lg.text("kml-save-tooltip"))
                                    .clicked()
                                {
                                    save_now = true;
                                }
                                if ui.button(lg.text("kml-save-as-action")).clicked() {
                                    save_as = true;
                                }
                                if ui.button(lg.text("kml-open-action")).clicked() {
                                    open_dialog = true;
                                }
                            });
                            ui.weak(lg.format(
                                "export-folder-label",
                                &[("path", short_folder(&self.settings.kml_dir).into())],
                            ))
                            .on_hover_text(&self.settings.kml_dir);
                            if ui.button(lg.text("places-clear-all-action")).clicked() {
                                self.clear_places();
                            }
                        });

                    egui::CollapsingHeader::new(lg.text("ruler-section-title"))
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.checkbox(&mut self.ruler, lg.text("ruler-enable-toggle"));
                            let selected = super::ruler::unit_selector(
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
                                self.ruler_point_editor = Some(super::ruler::PointEditor::new((
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
                                        (
                                            "count",
                                            self.ruler_paths.point_count().to_string().into(),
                                        ),
                                        (
                                            "distance",
                                            self.settings
                                                .ruler_unit
                                                .format_with_angle(
                                                    total,
                                                    self.ruler_paths
                                                        .angular_distance_degrees(model),
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
                                    .map(|bearing| {
                                        earthcraft_core::compass::heading_text(bearing, lg)
                                    })
                                    .unwrap_or_else(|| "—".into());
                                ui.label(
                                    lg.format(
                                        "ruler-bearing-label",
                                        &[("bearing", bearing.into())],
                                    ),
                                )
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
                    }
                });
            },
        );
        self.sidebar_open = expanded;
        if save_now {
            let path = self.resolve_kml_path(&self.kml_path.clone());
            self.save_kml_to(path);
        }
        if save_as {
            self.pick_kml_save_as();
        }
        if open_dialog {
            self.pick_kml_open();
        }
        if open_typed {
            let path = self.resolve_kml_path(&self.kml_path.clone());
            if path.is_file() {
                self.load_kml_from(&path);
            }
        }
        if shot {
            self.save_screenshot();
        }
        if let Some((la, lo, al)) = goto {
            self.fly_to(la, lo, al);
        }
    }
}

fn expanded_sidebar_panel() -> egui::Panel {
    egui::Panel::left("side").default_size(DEFAULT_SIDEBAR_WIDTH_POINTS)
}
fn sidebar_scroll_area() -> egui::ScrollArea {
    // Keep the allocated viewport when sections close, and scroll oversized
    // widgets rather than letting them resize the panel when sections reopen.
    egui::ScrollArea::both().auto_shrink([false, false])
}

#[cfg(test)]
mod tests;
