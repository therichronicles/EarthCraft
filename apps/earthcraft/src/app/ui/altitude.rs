//! The desktop owns altitude-entry timing; core owns conversion and validation.
use super::super::App;
use earthcraft_core::{
    MAX_CAMERA_ALTITUDE_KM, MIN_CAMERA_ALTITUDE_KM,
    language::Lang,
    units::{AltitudeUnit, DistanceUnits},
};
use eframe::egui;
const EDITOR_ANCHOR_GAP_POINTS: f32 = 8.0;
const BUTTON_TOP_MARGIN_POINTS: f32 = 8.0;

pub(in crate::app) struct AltitudeEditor {
    text: String,
    unit: AltitudeUnit,
    focus: bool,
    invalid: bool,
    anchor: egui::Pos2,
}
enum Action {
    Pending,
    Cancel,
    Apply { altitude_km: f64 },
}
impl AltitudeEditor {
    pub(super) fn new(altitude_km: f64, units: DistanceUnits, anchor: egui::Pos2) -> Self {
        let unit = AltitudeUnit::for_altitude(altitude_km, units);
        Self {
            text: unit.value(altitude_km).to_string(),
            unit,
            focus: true,
            invalid: false,
            anchor,
        }
    }
    fn show(&mut self, ctx: &egui::Context, lang: Lang) -> Action {
        let mut action = Action::Pending;
        let mut open = true;
        egui::Window::new(lang.text("altitude-edit-title"))
            .id(egui::Id::new("camera_altitude_editor"))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(self.anchor - egui::vec2(0.0, EDITOR_ANCHOR_GAP_POINTS))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let mut enter = false;
                ui.horizontal(|ui| {
                    let mut output = egui::TextEdit::singleline(&mut self.text)
                        .desired_width(180.0)
                        .show(ui);
                    if self.focus {
                        output.response.request_focus();
                        output
                            .state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(self.text.chars().count()),
                            )));
                        output.state.store(ctx, output.response.id);
                        self.focus = false;
                    }
                    if output.response.changed() {
                        self.invalid = false;
                    }
                    enter = (output.response.has_focus() || output.response.lost_focus())
                        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                    ui.label(self.unit.symbol());
                });
                if self.invalid {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        Lang::En.format(
                            "altitude-input-error",
                            &[
                                (
                                    "minimum",
                                    self.unit.value(MIN_CAMERA_ALTITUDE_KM).to_string().into(),
                                ),
                                (
                                    "maximum",
                                    self.unit.value(MAX_CAMERA_ALTITUDE_KM).to_string().into(),
                                ),
                                ("unit", self.unit.symbol().into()),
                            ],
                        ),
                    );
                }
                ui.add_space(BUTTON_TOP_MARGIN_POINTS);
                ui.horizontal(|ui| {
                    if ui.button(lang.text("altitude-apply-action")).clicked() || enter {
                        match self.unit.parse_km(&self.text) {
                            Some(altitude_km) => action = Action::Apply { altitude_km },
                            None => self.invalid = true,
                        }
                    }
                    if ui.button(lang.text("altitude-cancel-action")).clicked() {
                        action = Action::Cancel;
                    }
                });
            });
        if !open || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            Action::Cancel
        } else {
            action
        }
    }
}
impl App {
    pub(in crate::app) fn altitude_window(&mut self, ctx: &egui::Context) {
        let Some(editor) = &mut self.altitude_editor else {
            return;
        };
        match editor.show(ctx, self.lang) {
            Action::Pending => {}
            Action::Cancel => self.altitude_editor = None,
            Action::Apply { altitude_km } => {
                self.fly = None;
                self.cam.alt = altitude_km;
                self.touch();
                self.altitude_editor = None;
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn submit(text: &str) -> (AltitudeEditor, Action) {
        let ctx = egui::Context::default();
        let mut editor =
            AltitudeEditor::new(14000.0, DistanceUnits::Metric, egui::pos2(100.0, 300.0));
        let _ = crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
            let ctx: &egui::Context = ui;
            editor.show(ctx, Lang::En);
        });
        let mut result = Action::Pending;
        let _ = crate::app::test_ui::run(
            &ctx,
            egui::RawInput {
                events: vec![
                    egui::Event::Text(text.into()),
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                let ctx: &egui::Context = ui;
                result = editor.show(ctx, Lang::En);
            },
        );
        (editor, result)
    }
    #[test]
    fn selected_initial_value_is_replaced_and_enter_applies_exact_altitude() {
        let (_, result) = submit("1234.56789");
        assert!(matches!(result, Action::Apply { altitude_km } if altitude_km == 1234.56789));
    }
    #[test]
    fn invalid_input_keeps_editor_open_and_escape_cancels() {
        let (editor, result) = submit("NaN");
        assert!(matches!(result, Action::Pending));
        assert!(editor.invalid);
        let ctx = egui::Context::default();
        let mut editor =
            AltitudeEditor::new(14000.0, DistanceUnits::Metric, egui::pos2(100.0, 300.0));
        let mut result = Action::Pending;
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
            |ui| {
                let ctx: &egui::Context = ui;
                result = editor.show(ctx, Lang::PtBr);
            },
        );
        assert!(matches!(result, Action::Cancel));
    }
}
