//! Pointer ownership stays in the desktop adapter; projection stays in render.
use super::App;
use earthcraft_core::{
    camera::Basis,
    ruler::{Ruler, RulerPointId},
};
use eframe::egui::{self, Key, Rect};
const POINT_HIT_RADIUS_POINTS: f32 = 10.0;

impl App {
    pub(super) fn edit_ruler(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        aspect: f64,
    ) -> bool {
        if !self.ruler
            || self.sidebar_animating
            || self.explorer.active != earthcraft_core::body::Body::Earth
        {
            self.ruler_paths.end_move();
            self.ruler_drag = None;
            return false;
        }
        let basis = self.renderer.picking_basis(self.cam.basis(aspect));
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let hovered = pointer
            .filter(|p| response.hovered() && rect.contains(*p))
            .and_then(|p| {
                earthcraft_render::overlays::ruler::point_at(
                    &basis,
                    rect,
                    &self.ruler_paths,
                    p,
                    POINT_HIT_RADIUS_POINTS,
                )
            });
        if hovered.is_some() || self.ruler_drag.is_some() {
            let starting_path = ui.input(|i| i.modifiers.alt) && self.ruler_drag.is_none();
            ui.output_mut(|o| {
                o.cursor_icon = if starting_path {
                    egui::CursorIcon::Crosshair
                } else {
                    egui::CursorIcon::Grab
                }
            });
        }
        if let Some(index) = hovered.filter(|_| !ui.egui_wants_keyboard_input())
            && ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, Key::Delete)
                    || i.consume_key(egui::Modifiers::NONE, Key::Backspace)
            })
        {
            self.ruler_paths.remove_point(index);
            self.ruler_drag = None;
            return true;
        }
        if ui.input(|i| i.pointer.primary_pressed()) {
            self.ruler_drag = pressed_handle(response, &basis, rect, &self.ruler_paths);
            if self.ruler_drag.is_some() {
                self.ruler_paths.begin_move();
                self.fly = None;
            }
        }
        let captured = self.ruler_drag.is_some();
        if let Some(index) = self.ruler_drag {
            if let Some(point) = pointer
                .filter(|_| {
                    response.dragged_by(egui::PointerButton::Primary)
                        || response.drag_stopped_by(egui::PointerButton::Primary)
                })
                .filter(|p| rect.contains(*p))
                .and_then(|p| basis.pick(rect, p))
            {
                self.ruler_paths.move_point(index, point);
            }
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
            ui.request_repaint();
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            self.ruler_paths.end_move();
            self.ruler_drag = None;
        }
        captured
    }
}

pub(super) fn starts_new_path(response: &egui::Response) -> bool {
    response.ctx.input(|input| {
        input
            .events
            .iter()
            .rev()
            .find_map(|event| match event {
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                    ..
                } => Some(modifiers.alt),
                _ => None,
            })
            .unwrap_or(input.modifiers.alt)
    })
}

fn pressed_handle(
    response: &egui::Response,
    basis: &Basis,
    rect: Rect,
    points: &Ruler,
) -> Option<RulerPointId> {
    // Use egui's input ownership so controls layered above the map keep clicks.
    if !(response.is_pointer_button_down_on()
        || response.clicked_by(egui::PointerButton::Primary)
        || response.drag_started_by(egui::PointerButton::Primary)
        || response.drag_stopped_by(egui::PointerButton::Primary))
    {
        return None;
    }
    // Several pointer events can arrive in one frame. The latest position may
    // already be far from the handle, so hit-test the primary press event itself.
    let press = response.ctx.input(|input| {
        input.events.iter().find_map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            } if !modifiers.alt => Some(*pos),
            _ => None,
        })
    })?;
    if !rect.contains(press) {
        return None;
    }
    earthcraft_render::overlays::ruler::point_at(
        basis,
        rect,
        points,
        press,
        POINT_HIT_RADIUS_POINTS,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::camera::Cam;
    #[test]
    fn alt_click_on_a_handle_starts_a_path_instead_of_dragging_it() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 600.0));
        let basis = Cam {
            lat: 0.0,
            lon: 0.0,
            heading: 0.0,
            alt: 14000.0,
        }
        .basis(1.0);
        let mut points = Ruler::default();
        points.add_point((0.0, 0.0), false);
        points.add_point((0.0, 1.0), false);
        let distance = points.total_distance_km();
        let pos = rect.center();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::ALT,
        };
        for events in [
            vec![egui::Event::PointerMoved(pos)],
            vec![button(true)],
            vec![button(false)],
        ] {
            let _ = crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                        assert!(pressed_handle(&response, &basis, rect, &points).is_none());
                        if response.clicked() {
                            assert!(starts_new_path(&response));
                            points.add_point(
                                basis.pick(rect, pos).unwrap(),
                                starts_new_path(&response),
                            );
                        }
                    });
                },
            );
        }
        assert_eq!(points.paths().count(), 2);
        assert_eq!(points.point_count(), 3);
        assert_eq!(points.total_distance_km(), distance);
        assert!(points.last_leg().is_none());
    }
    fn handle_on_press(start: egui::Pos2, end: egui::Pos2) -> Option<RulerPointId> {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 600.0));
        let basis = Cam {
            lat: 0.0,
            lon: 0.0,
            heading: 0.0,
            alt: 14000.0,
        }
        .basis(1.0);
        let mut points = Ruler::default();
        points.add_point((0.0, 0.0), false);
        let mut selected = None;
        for events in [
            vec![egui::Event::PointerMoved(start)],
            vec![
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(end),
            ],
        ] {
            let _ = crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(rect),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                        selected = pressed_handle(&response, &basis, rect, &points);
                    });
                },
            );
        }
        selected
    }
    #[test]
    fn fast_drag_captures_the_point_at_press_not_at_frame_end() {
        assert_eq!(
            handle_on_press(egui::pos2(300.0, 300.0), egui::pos2(350.0, 320.0)),
            Some(RulerPointId { path: 0, point: 0 })
        );
    }
    #[test]
    fn dragging_onto_a_point_from_empty_map_does_not_capture_it() {
        assert_eq!(
            handle_on_press(egui::pos2(350.0, 320.0), egui::pos2(300.0, 300.0)),
            None
        );
    }
}
