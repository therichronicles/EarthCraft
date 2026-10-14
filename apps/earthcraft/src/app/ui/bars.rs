use super::super::App;
use earthcraft_core::language::Lang;
use eframe::egui::{self, Key, Rect};
use emath::GuiRounding;
impl App {
    pub(in crate::app) fn top_bar(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        let mut field_rect = Rect::NOTHING;
        const BAR_MARGIN_POINTS: egui::Margin = egui::Margin::symmetric(8, 4);
        egui::Panel::top("top")
            .frame(egui::Frame::side_top_panel(&ui.global_style()).inner_margin(BAR_MARGIN_POINTS))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let sidebar_hint = if self.sidebar_open {
                        lg.text("sidebar-hide-tooltip")
                    } else {
                        lg.text("sidebar-show-tooltip")
                    };
                    ui.toggle_value(&mut self.sidebar_open, "☰")
                        .on_hover_text(sidebar_hint);
                    ui.strong("🌍 EarthCraft");
                    if ui.button(lg.text("top-explore-action")).clicked() {
                        self.explorer.open = true;
                    }
                    for (name, body) in [
                        (lg.text("top-earth"), earthcraft_core::body::Body::Earth),
                        (lg.text("top-moon"), earthcraft_core::body::Body::Moon),
                        (
                            lg.text("top-custom-planet"),
                            earthcraft_core::body::Body::Custom,
                        ),
                    ] {
                        if ui.button(name).clicked() {
                            self.flat_earth.open = false;
                            self.explorer.choose(body, &self.file_jobs);
                            if body == earthcraft_core::body::Body::Custom {
                                self.explorer.open = true;
                            }
                            self.touch();
                        }
                    }
                    ui.separator();

                    // Keys for the suggestion list must be consumed before the TextEdit sees them.
                    let popup = self.sugg_open && !self.suggestions.is_empty();
                    let (mut down, mut up, mut esc) = (false, false, false);
                    if popup {
                        ui.input_mut(|i| {
                            down = i.consume_key(egui::Modifiers::NONE, Key::ArrowDown);
                            up = i.consume_key(egui::Modifiers::NONE, Key::ArrowUp);
                            esc = i.consume_key(egui::Modifiers::NONE, Key::Escape);
                        });
                        let n = self.suggestions.len();
                        if down {
                            self.sugg_sel = Some(self.sugg_sel.map_or(0, |i| (i + 1) % n));
                        }
                        if up {
                            self.sugg_sel = Some(self.sugg_sel.map_or(n - 1, |i| (i + n - 1) % n));
                        }
                        if esc {
                            self.sugg_open = false;
                        }
                    }

                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text(lg.text("search-input-placeholder"))
                            .desired_width(340.0 * self.settings.ui_scale.max(1.0)),
                    );
                    field_rect = r.rect;
                    if r.changed() {
                        self.on_query_changed();
                    }
                    if r.gained_focus()
                        && !self.query.trim().is_empty()
                        && !self.suggestions.is_empty()
                    {
                        self.sugg_open = true;
                    }
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    if enter {
                        match (self.sugg_open, self.sugg_sel) {
                            (true, Some(i)) => self.choose_suggestion(i),
                            _ => {
                                self.sugg_open = false;
                                self.start_search(ui);
                            }
                        }
                    }
                    if ui.button(lg.text("search-submit-action")).clicked() {
                        self.sugg_open = false;
                        self.start_search(ui);
                    }
                    ui.separator();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("⚙")
                            .on_hover_text(lg.text("settings-open-tooltip"))
                            .clicked()
                        {
                            self.show_settings = !self.show_settings;
                        }
                        let mut chosen = self.lang;
                        egui::ComboBox::from_id_salt("lang_select")
                            .selected_text(format!("🌐 {}", chosen.label()))
                            .show_ui(ui, |ui| {
                                super::selection::size_menu(
                                    ui,
                                    Lang::ALL.into_iter().map(|lang| lang.label()),
                                );
                                for l in Lang::ALL {
                                    super::selection::full_width_value(
                                        ui,
                                        &mut chosen,
                                        l,
                                        l.label(),
                                    );
                                }
                            });
                        if chosen != self.lang {
                            self.set_lang(chosen);
                        }
                        let compact = match &self.status_file {
                            Some(file) if file.message == self.status => {
                                let full_path = file.path.to_string_lossy();
                                self.status.replace(
                                    full_path.as_ref(),
                                    &super::paths::short_folder(&full_path),
                                )
                            }
                            _ => self.status.clone(),
                        };
                        let available = ui.available_width().max(0.0);
                        let compact = super::paths::fit_tail(ui, &compact, available);
                        ui.add_sized(
                            [available, ui.spacing().interact_size.y],
                            egui::Label::new(compact).truncate(),
                        )
                        .on_hover_text(&self.status);
                    });
                });
            });
        self.suggestion_popup(ui, field_rect);
    }

    /// Status below the map.
    pub(in crate::app) fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        egui::Panel::bottom("bottom").show(ui, |ui| {
            let bounds = ui.available_rect_before_wrap();
            let heading = self
                .renderer
                .displayed_view()
                .map_or(self.cam.heading, |(cam, _)| cam.heading);
            let font = egui::TextStyle::Monospace.resolve(ui.style());
            let heading_width = ui
                .painter()
                .layout_no_wrap("359° NW".into(), font, ui.visuals().text_color())
                .size()
                .x
                + ui.spacing().item_spacing.x * 2.0;
            let split = (bounds.right() - heading_width).max(bounds.left());
            let heading_rect = Rect::from_min_max(egui::pos2(split, bounds.top()), bounds.max);
            let left_rect = Rect::from_min_max(
                bounds.min,
                egui::pos2(split - ui.spacing().item_spacing.x, bounds.bottom()),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                ui.horizontal(|ui| {
                    ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
                    let sidebar_right =
                        egui::containers::panel::PanelState::load(ui, egui::Id::new("side"))
                            .map_or(
                                ui.content_rect().left()
                                    + super::sidebar::DEFAULT_SIDEBAR_WIDTH_POINTS,
                                |state| state.outer_rect.right(),
                            );
                    // Keep the cursor divider on the inner edge of the sidebar separator.
                    const SIDEBAR_SEPARATOR_INSET_POINTS: f32 = 1.0;
                    let sidebar_border_x = (sidebar_right - SIDEBAR_SEPARATOR_INSET_POINTS)
                        .round_to_pixels(ui.pixels_per_point());
                    super::cursor::cursor_readout(ui, self.cursor, sidebar_border_x);
                    ui.monospace(lg.format(
                        "statusbar-camera-center",
                        &[
                            ("latitude", format!("{:.4}", self.cam.lat).into()),
                            ("longitude", format!("{:.4}", self.cam.lon).into()),
                        ],
                    ));
                    ui.separator();
                    let altitude = lg.format(
                        "statusbar-camera-altitude",
                        &[(
                            "distance",
                            self.distance_units().altitude(self.cam.alt).into(),
                        )],
                    );
                    let altitude_response = ui
                        .add(
                            egui::Label::new(egui::RichText::new(altitude).monospace())
                                .sense(egui::Sense::click()),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(lg.text("altitude-edit-tooltip"));
                    if altitude_response.clicked() {
                        self.altitude_editor = Some(super::altitude::AltitudeEditor::new(
                            self.cam.alt,
                            self.distance_units(),
                            altitude_response.rect.left_top(),
                        ));
                    }
                    ui.separator();
                    let n = self.cache.inflight();
                    if n > 0 {
                        ui.monospace(lg.format(
                            "statusbar-tiles-loading",
                            &[("count", n.to_string().into())],
                        ));
                    } else {
                        ui.monospace("✔ tiles ok");
                    }
                });
            });
            ui.scope_builder(egui::UiBuilder::new().max_rect(heading_rect), |ui| {
                if super::compass::heading_readout(ui, heading, lg) {
                    self.cam.heading = 0.0;
                    self.fly = None;
                    self.touch();
                    ui.request_repaint();
                }
            });
        });
    }

    // ───────── Globe / Globo ─────────
}
