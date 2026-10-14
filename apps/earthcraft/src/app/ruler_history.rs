//! Ruler history commands and keyboard routing; text edits keep their own history.
use super::App;
use eframe::egui::{self, Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HistoryCommand {
    Undo,
    Redo,
}

pub(super) fn text_shortcut_event(command: HistoryCommand) -> egui::Event {
    egui::Event::Key {
        // egui 0.27's text undo matcher also accepts Shift+Command+Z.
        // Use its distinct redo event when forwarding a native Redo command.
        key: if command == HistoryCommand::Redo {
            Key::Y
        } else {
            Key::Z
        },
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    }
}

fn consume_shortcut(ctx: &egui::Context) -> Option<HistoryCommand> {
    if ctx.egui_wants_keyboard_input() {
        return None;
    }
    ctx.input_mut(|input| {
        if input.consume_shortcut(&egui::KeyboardShortcut::new(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::Z,
        )) || input.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Y))
        {
            Some(HistoryCommand::Redo)
        } else if input.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Z)) {
            Some(HistoryCommand::Undo)
        } else {
            None
        }
    })
}

impl App {
    pub(super) fn apply_ruler_history(&mut self, command: HistoryCommand, ctx: &egui::Context) {
        let changed = match command {
            HistoryCommand::Undo => self.ruler_paths.undo(),
            HistoryCommand::Redo => self.ruler_paths.redo(),
        };
        if changed {
            self.ruler_drag = None;
            ctx.request_repaint();
        }
    }
    pub(super) fn ruler_shortcuts(&mut self, ctx: &egui::Context) {
        // AppKit dispatches native accelerators through the menu event queue.
        #[cfg(target_os = "macos")]
        if self.native_menu.is_some() {
            return;
        }
        if let Some(command) = consume_shortcut(ctx) {
            self.run_menu_action(
                match command {
                    HistoryCommand::Undo => super::menu::Action::Undo,
                    HistoryCommand::Redo => super::menu::Action::Redo,
                },
                ctx,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_history_events_reach_text_edits_own_undo_and_redo() {
        let ctx = egui::Context::default();
        let mut text = String::new();
        let frame = |time, events, text: &mut String| {
            let _ = crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.text_edit_singleline(text).request_focus();
                    });
                },
            );
        };
        frame(0.0, vec![], &mut text);
        frame(1.0, vec![egui::Event::Text("hello".into())], &mut text);
        frame(2.0, vec![], &mut text);
        assert_eq!(text, "hello");
        frame(
            3.0,
            vec![text_shortcut_event(HistoryCommand::Undo)],
            &mut text,
        );
        assert_eq!(text, "");
        frame(
            4.0,
            vec![text_shortcut_event(HistoryCommand::Redo)],
            &mut text,
        );
        assert_eq!(text, "hello");
    }
    #[test]
    fn shortcuts_respect_text_focus_and_distinguish_redo() {
        for (key, modifiers, expected) in [
            (Key::Z, Modifiers::COMMAND, HistoryCommand::Undo),
            (
                Key::Z,
                Modifiers::COMMAND | Modifiers::SHIFT,
                HistoryCommand::Redo,
            ),
            (Key::Y, Modifiers::COMMAND, HistoryCommand::Redo),
        ] {
            for text_focus in [false, true] {
                let ctx = egui::Context::default();
                let mut text = "text".to_string();
                let frame = |events, text: &mut String| {
                    let mut command = None;
                    let _ = crate::app::test_ui::run(
                        &ctx,
                        egui::RawInput {
                            events,
                            ..Default::default()
                        },
                        |ui| {
                            let ctx: &egui::Context = ui;
                            command = consume_shortcut(ctx);
                            if text_focus {
                                egui::CentralPanel::default().show(ui, |ui| {
                                    ui.text_edit_singleline(text).request_focus();
                                });
                            }
                        },
                    );
                    command
                };
                let _ = frame(vec![], &mut text);
                let command = frame(
                    vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    &mut text,
                );
                assert_eq!(command, if text_focus { None } else { Some(expected) });
            }
        }
    }
}
