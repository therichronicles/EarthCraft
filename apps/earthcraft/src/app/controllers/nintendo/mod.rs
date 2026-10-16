//! Bridge Nintendo's vendor-defined full reports into the existing gilrs queue.
//! Standard HID/simple reports and every other controller stay on native gilrs.
mod protocol;
mod reader;

use super::BUTTONS;
use gilrs::{Axis, Event, EventType, GamepadId, Gilrs};
use reader::{PRODUCT_ID, Reader, VENDOR_ID};

pub(super) struct Nintendo {
    reader: Reader,
    target: Option<GamepadId>,
    active: bool,
    buttons: [bool; BUTTONS.len()],
}
impl Nintendo {
    pub fn supports(gamepad: &gilrs::Gamepad<'_>) -> bool {
        gamepad.vendor_id() == Some(VENDOR_ID) && gamepad.product_id() == Some(PRODUCT_ID)
    }

    pub fn new(ctx: &eframe::egui::Context) -> Result<Self, String> {
        Ok(Self {
            reader: Reader::new(ctx)?,
            target: None,
            active: false,
            buttons: [false; BUTTONS.len()],
        })
    }

    /// Returns whether navigation must rearm, and any native input error.
    pub fn poll(&mut self, backend: &mut Gilrs) -> (bool, Option<String>) {
        let mut targets = backend
            .gamepads()
            .filter(|(_, gamepad)| Self::supports(gamepad))
            .map(|(id, _)| id);
        let target = targets.next().filter(|_| targets.next().is_none());
        let snapshot = self.reader.take();
        let reset = self.target != target || snapshot.reset;
        let was_active = self.active;
        if reset {
            self.buttons.fill(false);
            self.active = false;
        }
        self.target = target;
        let Some(id) = target else {
            return (reset && was_active, snapshot.error);
        };
        if let Some(sample) = snapshot.sample {
            // The first sample can be held on connect; require neutral in the app.
            let first = !self.active;
            self.active = true;
            for (axis, value) in [
                Axis::LeftStickX,
                Axis::LeftStickY,
                Axis::RightStickX,
                Axis::RightStickY,
            ]
            .into_iter()
            .zip(sample.axes)
            {
                if let Some(code) = backend.gamepad(id).axis_code(axis) {
                    backend.insert_event(Event::new(id, EventType::AxisChanged(axis, value, code)));
                }
            }
            if first || reset {
                // Resynchronize every button, including releases after overflow.
                for (input, down) in sample.buttons.into_iter().enumerate() {
                    self.button(backend, id, input, down, true);
                }
            } else {
                for (input, down) in snapshot.edges {
                    self.button(backend, id, input, down, false);
                }
            }
            (reset || first, snapshot.error)
        } else {
            if was_active {
                for axis in [
                    Axis::LeftStickX,
                    Axis::LeftStickY,
                    Axis::RightStickX,
                    Axis::RightStickY,
                ] {
                    if let Some(code) = backend.gamepad(id).axis_code(axis) {
                        backend
                            .insert_event(Event::new(id, EventType::AxisChanged(axis, 0.0, code)));
                    }
                }
                for input in 0..BUTTONS.len() {
                    self.button(backend, id, input, false, true);
                }
            }
            self.active = false;
            (reset && was_active, snapshot.error)
        }
    }

    fn button(
        &mut self,
        backend: &mut Gilrs,
        id: GamepadId,
        input: usize,
        down: bool,
        force: bool,
    ) {
        if !force && self.buttons[input] == down {
            return;
        }
        self.buttons[input] = down;
        let button = BUTTONS[input];
        if let Some(code) = backend.gamepad(id).button_code(button) {
            backend.insert_event(Event::new(
                id,
                EventType::ButtonChanged(button, f32::from(down), code),
            ));
            let event = if down {
                EventType::ButtonPressed(button, code)
            } else {
                EventType::ButtonReleased(button, code)
            };
            backend.insert_event(Event::new(id, event));
        }
    }
}
