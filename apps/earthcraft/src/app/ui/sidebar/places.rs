use crate::app::App;
use crate::app::ui::paths::short_folder;
use eframe::egui::{self, Color32, Key, Stroke};
const PLACES_DETAILS_HEIGHT_POINTS: f32 = 135.0;
const PLACES_ADD_HEIGHT_POINTS: f32 = 84.0;
const MIN_TREE_HEIGHT_POINTS: f32 = 64.0;

impl App {
    pub(super) fn sidebar_places(&mut self, ui: &mut egui::Ui) {
        let lg = self.lang;
        let (mut save_now, mut save_as, mut open_dialog, mut open_typed) =
            (false, false, false, false);
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
        let files = files_footer(ui, lg, &mut self.kml_path, &self.settings.kml_dir);
        open_typed |= files.open_typed;
        let details_height = if self.place_selection.is_some() {
            PLACES_DETAILS_HEIGHT_POINTS
        } else {
            0.0
        };
        let tree_height = (ui.available_height() - PLACES_ADD_HEIGHT_POINTS - details_height)
            .max(MIN_TREE_HEIGHT_POINTS);
        let goto = egui::ScrollArea::vertical()
            .id_salt("places-viewer")
            .auto_shrink([false, false])
            .show(ui, |ui| self.places_tree(ui, tree_height))
            .inner;
        if files.clear.is_some_and(|response| response.clicked()) {
            self.clear_places();
        }
        self.place_reveal_requested = false;
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
        if let Some((lat, lon, alt)) = goto {
            self.fly_to(lat, lon, alt);
        }
    }
}

const FILES_FOOTER_CLEARANCE_POINTS: f32 = 12.0;
const FILES_FOOTER_HEIGHT_LINES: f32 = 11.0;
const FILES_MAX_HEIGHT_FRACTION: f32 = 0.5;
struct FileControls {
    open_typed: bool,
    clear: Option<egui::Response>,
}
/// Reserve the actual file controls before sizing the scrolling placemark viewer.
/// The footer has its own bounded scroll area for short pinned Places panels.
fn files_footer(
    ui: &mut egui::Ui,
    lg: earthcraft_core::language::Lang,
    path: &mut String,
    folder: &str,
) -> FileControls {
    let mut controls = FileControls {
        open_typed: false,
        clear: None,
    };
    let max_height = ui.available_height() * FILES_MAX_HEIGHT_FRACTION;
    egui::Panel::bottom("places-files-footer")
        .frame(egui::Frame::NONE)
        .default_size(
            ui.text_style_height(&egui::TextStyle::Body) * FILES_FOOTER_HEIGHT_LINES
                + FILES_FOOTER_CLEARANCE_POINTS,
        )
        .size_range(0.0..=max_height)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("places-files-scroll")
                .show(ui, |ui| {
                    egui::CollapsingHeader::new(lg.text("sidebar-places-files-title"))
                        .id_salt("places-files")
                        .default_open(true)
                        .show(ui, |ui| {
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
                            egui::Frame::NONE
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
                                    let r = super::super::paths::file_path_input(ui, path);
                                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                        controls.open_typed = true;
                                    }
                                });
                            ui.weak(lg.format(
                                "export-folder-label",
                                &[("path", short_folder(folder).into())],
                            ))
                            .on_hover_text(folder);
                            controls.clear = Some(ui.button(lg.text("places-clear-all-action")));
                        });
                });
        });
    controls
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::language::Lang;
    #[test]
    fn files_are_expanded_and_visible_without_scrolling_the_tree() {
        for lang in Lang::ALL {
            for height in [260.0, 600.0] {
                let ctx = egui::Context::default();
                let mut path = String::new();
                for _ in 0..3 {
                    crate::app::test_ui::run(
                        &ctx,
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(240.0, height),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            let files = files_footer(ui, lang, &mut path, "Documents");
                            if height >= 600.0 {
                                let clear = files.clear.as_ref().expect("expanded file controls");
                                assert!(
                                    clear.interact_rect.contains_rect(clear.rect),
                                    "Clear All must fit without scrolling: {clear:?}"
                                );
                            }
                            assert!(
                                ui.available_height() > height * 0.4,
                                "viewer keeps usable space"
                            );
                        },
                    );
                    let path = ctx
                        .read_response(egui::Id::new("places-files-path"))
                        .unwrap();
                    assert!(
                        path.interact_rect.contains_rect(path.rect),
                        "path must start fully visible: {path:?}"
                    );
                }
            }
        }
    }
}
