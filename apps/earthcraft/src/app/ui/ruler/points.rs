//! Exact point entry; core validates decimal degrees and owns undoable edits.
use crate::app::App;
use earthcraft_core::{
    coordinates::parse_coordinates,
    language::Lang,
    ruler::{Ruler, RulerPointId},
};
use eframe::egui;
const EDITOR_WIDTH_POINTS: f32 = 440.0;
const POINT_LIST_HEIGHT_POINTS: f32 = 160.0;
const BUTTON_MARGIN_POINTS: f32 = 8.0;

pub(in crate::app) struct PointEditor {
    latitude: String,
    longitude: String,
    selected: Option<(RulerPointId, (f64, f64))>,
    new_path: bool,
    error: Option<String>,
}
#[derive(Debug, PartialEq)]
enum Action {
    Pending,
    Close,
    Add {
        position: (f64, f64),
        new_path: bool,
    },
    Edit {
        id: RulerPointId,
        expected: (f64, f64),
        position: (f64, f64),
    },
}
impl PointEditor {
    pub(in crate::app::ui) fn new(position: (f64, f64)) -> Self {
        Self {
            latitude: position.0.to_string(),
            longitude: position.1.to_string(),
            selected: None,
            new_path: false,
            error: None,
        }
    }
    fn select(&mut self, id: RulerPointId, position: (f64, f64)) {
        self.latitude = position.0.to_string();
        self.longitude = position.1.to_string();
        self.selected = Some((id, position));
        self.error = None;
    }
    fn submit(&mut self, ruler: &Ruler) -> Action {
        if self.selection_changed(ruler) {
            self.error =
                Some("Ruler points changed. Select a point again before applying an edit.".into());
            return Action::Pending;
        }
        match parse_coordinates(&self.latitude, &self.longitude) {
            Ok(position) => {
                self.error = None;
                match self.selected {
                    Some((id, expected)) => Action::Edit {
                        id,
                        expected,
                        position,
                    },
                    None => Action::Add {
                        position,
                        new_path: self.new_path,
                    },
                }
            }
            Err(error) => {
                self.error = Some(error.to_string());
                Action::Pending
            }
        }
    }
    fn show(&mut self, ctx: &egui::Context, ruler: &Ruler, lang: Lang) -> Action {
        let mut open = true;
        let mut action = Action::Pending;
        egui::Window::new(lang.text("ruler-points-title"))
            .id(egui::Id::new("ruler_points_editor"))
            .default_width(EDITOR_WIDTH_POINTS)
            .default_pos(
                ctx.content_rect().center()
                    - egui::vec2(EDITOR_WIDTH_POINTS * 0.5, POINT_LIST_HEIGHT_POINTS),
            )
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(POINT_LIST_HEIGHT_POINTS)
                    .show(ui, |ui| {
                        for (path, points) in ruler.paths().enumerate() {
                            if points.is_empty() {
                                continue;
                            }
                            ui.strong(lang.format(
                                "ruler-path-heading",
                                &[("number", (path + 1).to_string().into())],
                            ));
                            for (point, position) in points.iter().copied().enumerate() {
                                let id = RulerPointId { path, point };
                                let label = lang.format(
                                    "ruler-point-row",
                                    &[
                                        ("number", (point + 1).to_string().into()),
                                        ("latitude", format!("{:.8}", position.0).into()),
                                        ("longitude", format!("{:.8}", position.1).into()),
                                    ],
                                );
                                if ui
                                    .selectable_label(
                                        self.selected.is_some_and(|(selected, _)| selected == id),
                                        label,
                                    )
                                    .clicked()
                                {
                                    self.select(id, position);
                                }
                            }
                        }
                    });
                ui.separator();
                ui.label(lang.text(if self.selected.is_some() {
                    "ruler-point-edit-mode"
                } else {
                    "ruler-point-add-mode"
                }));
                let mut enter = false;
                for (label, text) in [
                    ("ruler-point-latitude", &mut self.latitude),
                    ("ruler-point-longitude", &mut self.longitude),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(lang.text(label));
                        let response = ui.add(
                            egui::TextEdit::singleline(text)
                                .id_source(label)
                                .desired_width(ui.available_width()),
                        );
                        if response.changed() {
                            self.error = None;
                        }
                        enter |= (response.has_focus() || response.lost_focus())
                            && ui.input_mut(|input| {
                                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                            });
                    });
                }
                if self.selected.is_none() {
                    ui.checkbox(&mut self.new_path, lang.text("ruler-point-new-path-toggle"));
                }
                ui.weak(lang.text("ruler-point-input-help"));
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.add_space(BUTTON_MARGIN_POINTS);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(lang.text(if self.selected.is_some() {
                            "ruler-point-apply-action"
                        } else {
                            "ruler-point-add-action"
                        }))
                        .clicked()
                        || enter
                    {
                        action = self.submit(ruler);
                    }
                    if self.selected.is_some()
                        && ui.button(lang.text("ruler-point-new-action")).clicked()
                    {
                        self.selected = None;
                        self.error = None;
                    }
                    if ui.button(lang.text("ruler-point-close-action")).clicked() {
                        action = Action::Close;
                    }
                });
            });
        if !open
            || ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            Action::Close
        } else {
            action
        }
    }
}
impl PointEditor {
    // Undo, deletion or a map drag can invalidate index-based selection.
    fn selection_changed(&self, ruler: &Ruler) -> bool {
        self.selected
            .is_some_and(|(id, original)| ruler.point(id) != Some(original))
    }
    fn apply(&mut self, action: Action, ruler: &mut Ruler) -> bool {
        match action {
            Action::Add { position, new_path } => {
                ruler.end_move();
                ruler.add_point(position, new_path);
                self.new_path = false;
            }
            Action::Edit {
                id,
                expected,
                position,
            } if ruler.point(id) == Some(expected) => {
                ruler.end_move();
                ruler.move_point(id, position);
                self.select(id, position);
            }
            _ => return false,
        }
        true
    }
}
impl App {
    pub(in crate::app) fn ruler_points_window(&mut self, ctx: &egui::Context) {
        let Some(editor) = &mut self.ruler_point_editor else {
            return;
        };
        let action = editor.show(ctx, &self.ruler_paths, self.lang);
        if action == Action::Close {
            self.ruler_point_editor = None;
        } else if editor.apply(action, &mut self.ruler_paths) {
            self.ruler_drag = None;
            self.ruler = true;
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_add_edit_and_separate_paths_are_undoable_without_rounding() {
        let original = (89.99999999999999, -179.12345678901235);
        let edited = (-45.123456789012345, 0.0000000000001);
        let mut ruler = Ruler::default();
        let mut editor = PointEditor::new(original);
        let action = editor.submit(&ruler);
        assert!(editor.apply(action, &mut ruler));
        let id = RulerPointId { path: 0, point: 0 };
        editor.select(id, ruler.point(id).unwrap());
        assert_eq!(
            parse_coordinates(&editor.latitude, &editor.longitude),
            Ok(original)
        );
        editor.latitude = edited.0.to_string();
        editor.longitude = edited.1.to_string();
        let action = editor.submit(&ruler);
        assert!(editor.apply(action, &mut ruler));
        assert_eq!(ruler.point(id), Some(edited));
        assert!(ruler.undo());
        assert_eq!(ruler.point(id), Some(original));
        assert!(ruler.redo());
        assert_eq!(ruler.point(id), Some(edited));
        editor.selected = None;
        editor.new_path = true;
        let action = editor.submit(&ruler);
        assert!(editor.apply(action, &mut ruler));
        assert_eq!(ruler.paths().count(), 2);
        assert_eq!(ruler.total_distance_km(), 0.0);
        assert!(!editor.new_path);
        assert!(ruler.undo());
        assert_eq!(ruler.paths().count(), 1);
    }
    #[test]
    fn validation_and_stale_selection_never_mutate_or_add_points() {
        let mut ruler = Ruler::default();
        ruler.add_point((10.0, 20.0), false);
        ruler.add_point((30.0, 40.0), false);
        let mut editor = PointEditor::new((0.0, 0.0));
        editor.latitude = "NaN".into();
        assert_eq!(editor.submit(&ruler), Action::Pending);
        assert!(editor.error.is_some());
        let id = RulerPointId { path: 0, point: 0 };
        editor.select(id, (10.0, 20.0));
        editor.latitude = "15".into();
        let action = editor.submit(&ruler);
        ruler.remove_point(id);
        assert!(!editor.apply(action, &mut ruler));
        assert_eq!(editor.submit(&ruler), Action::Pending);
        assert_eq!(ruler.point_count(), 1);
        assert_eq!(ruler.point(id), Some((30.0, 40.0)));
        editor.select(id, (30.0, 40.0));
        let action = editor.submit(&ruler);
        assert!(editor.apply(action, &mut ruler));
        // An unchanged Apply does not consume a history step.
        assert!(ruler.undo());
        assert_eq!(ruler.point_count(), 2);
    }
    #[test]
    fn editor_renders_in_both_languages_and_escape_cancels() {
        let ctx = egui::Context::default();
        let mut ruler = Ruler::default();
        ruler.add_point((90.0, -180.0), false);
        for lang in Lang::ALL {
            let mut editor = PointEditor::new((0.0, 0.0));
            let _ = crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
                let ctx: &egui::Context = ui;
                assert_eq!(editor.show(ctx, &ruler, lang), Action::Pending);
            });
            let _ = crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| assert_eq!(editor.show(ui, &ruler, lang), Action::Close),
            );
            assert_eq!(ruler.point_count(), 1);
        }
    }
}
