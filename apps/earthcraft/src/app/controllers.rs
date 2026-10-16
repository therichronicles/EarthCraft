//! Native gamepad adapter. Bindings and navigation math live in core.
use super::{App, menu::Action};
use earthcraft_core::controller::{ControllerAction, ControllerMotion, ControllerState};
use earthcraft_data::settings::controllers::ControllerPreferences;
use eframe::egui;
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
use std::time::{Duration, Instant};
#[cfg(target_os = "macos")]
mod nintendo;
const ACTIVE_POLL_INTERVAL: Duration = Duration::from_millis(16);
const HOTPLUG_POLL_INTERVAL: Duration = Duration::from_millis(250);
const MAX_EVENTS_PER_FRAME: usize = 512;
const BIND_TIMEOUT: Duration = Duration::from_secs(10);
const BIND_AXIS_THRESHOLD: f64 = 0.75;
const NEUTRAL_THRESHOLD: f64 = 0.2;
pub(super) const BUTTONS: [Button; 19] = [
    Button::South,
    Button::East,
    Button::North,
    Button::West,
    Button::LeftTrigger,
    Button::RightTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::LeftThumb,
    Button::RightThumb,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
    Button::Mode,
    Button::C,
    Button::Z,
];
pub(super) const AXIS_LABEL_IDS: [&str; 6] = [
    "controllers-left-x",
    "controllers-left-y",
    "controllers-right-x",
    "controllers-right-y",
    "controllers-left-trigger",
    "controllers-right-trigger",
];
pub(super) struct Device {
    pub(super) id: GamepadId,
    pub(super) key: String,
    pub(super) name: String,
    pub(super) state: ControllerState,
}
pub(super) struct BindingCapture {
    pub(super) action: usize,
    pub(super) axis: bool,
    deadline: Instant,
    armed: [bool; 6],
}
pub(super) struct Controllers {
    backend: Option<Gilrs>,
    #[cfg(target_os = "macos")]
    nintendo: Option<Result<nintendo::Nintendo, String>>,
    pub(super) devices: Vec<Device>,
    pub(super) error: Option<String>,
    pub(super) capture: Option<BindingCapture>,
    pub(super) conflict: Option<(usize, usize)>,
    suspended: bool,
    selected: String,
}
impl Controllers {
    pub(super) fn new() -> Self {
        let (backend, error) = match Gilrs::new() {
            Ok(backend) => (Some(backend), None),
            Err(error) => (
                None,
                Some(format!("Controller initialization failed: {error}")),
            ),
        };
        let devices = backend
            .as_ref()
            .map(|backend| {
                backend
                    .gamepads()
                    .map(|(id, gamepad)| device(id, gamepad))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            backend,
            #[cfg(target_os = "macos")]
            nintendo: None,
            devices,
            error,
            capture: None,
            conflict: None,
            suspended: true,
            selected: String::new(),
        }
    }
    pub(super) fn listen(&mut self, action: usize, axis: bool) {
        self.conflict = None;
        self.capture = Some(BindingCapture {
            action,
            axis,
            deadline: Instant::now() + BIND_TIMEOUT,
            armed: [false; 6],
        });
        self.suspended = true;
    }
    pub(super) fn poll(
        &mut self,
        ctx: &egui::Context,
        prefs: &mut ControllerPreferences,
        allowed: bool,
    ) -> (ControllerMotion, Vec<ControllerAction>) {
        if prefs.selected.is_empty()
            && let Some(device) = self.devices.first()
        {
            prefs.selected = device.key.clone();
        }
        if self.selected != prefs.selected {
            self.selected = prefs.selected.clone();
            self.suspended = true;
            self.capture = None;
            self.conflict = None;
        }
        if !allowed || !prefs.enabled {
            self.suspended = true;
        }
        let mut actions = Vec::new();
        if self
            .capture
            .as_ref()
            .is_some_and(|capture| Instant::now() >= capture.deadline)
        {
            self.capture = None;
        }
        if let Some(backend) = &mut self.backend {
            #[cfg(target_os = "macos")]
            {
                if self.nintendo.is_none()
                    && backend
                        .gamepads()
                        .any(|(_, gamepad)| nintendo::Nintendo::supports(&gamepad))
                {
                    self.nintendo = Some(nintendo::Nintendo::new(ctx));
                }
                if let Some(nintendo) = &mut self.nintendo {
                    match nintendo {
                        Ok(nintendo) => {
                            let (reset, error) = nintendo.poll(backend);
                            self.suspended |= reset;
                            self.error = error;
                        }
                        Err(error) => self.error = Some(error.clone()),
                    }
                }
            }
            for _ in 0..MAX_EVENTS_PER_FRAME {
                let Some(event) = backend.next_event() else {
                    break;
                };
                match event.event {
                    EventType::Connected => {
                        self.devices.retain(|device| device.id != event.id);
                        self.devices
                            .push(device(event.id, backend.gamepad(event.id)));
                        self.suspended = true;
                    }
                    EventType::Disconnected => {
                        if let Some(device) =
                            self.devices.iter_mut().find(|device| device.id == event.id)
                        {
                            device.state.release();
                        }
                        self.devices.retain(|device| device.id != event.id);
                        self.capture = None;
                        self.suspended = true;
                    }
                    _ => {}
                }
                let Some(device) = self.devices.iter_mut().find(|device| device.id == event.id)
                else {
                    continue;
                };
                let selected = device.key == prefs.selected;
                match event.event {
                    EventType::AxisChanged(axis, value, _) => {
                        if let Some(input) = [
                            Axis::LeftStickX,
                            Axis::LeftStickY,
                            Axis::RightStickX,
                            Axis::RightStickY,
                        ]
                        .iter()
                        .position(|candidate| *candidate == axis)
                        {
                            device.state.axes[input] = f64::from(value);
                        }
                    }
                    EventType::ButtonChanged(button, value, _) => {
                        if button == Button::LeftTrigger2 {
                            device.state.axes[4] = f64::from(value);
                        }
                        if button == Button::RightTrigger2 {
                            device.state.axes[5] = f64::from(value);
                        }
                    }
                    EventType::ButtonPressed(button, _) | EventType::ButtonReleased(button, _) => {
                        let down = matches!(event.event, EventType::ButtonPressed(..));
                        if let Some(input) =
                            BUTTONS.iter().position(|candidate| *candidate == button)
                        {
                            let profile = prefs.profile(&device.key);
                            let action = device.state.button(input, down, &profile);
                            if selected
                                && down
                                && self.capture.as_ref().is_some_and(|capture| !capture.axis)
                            {
                                let capture = self.capture.take().unwrap();
                                let profile = prefs.profiles.entry(device.key.clone()).or_default();
                                if !profile.move_button(capture.action, input, false) {
                                    self.conflict = Some((capture.action, input));
                                }
                            } else if selected
                                && allowed
                                && prefs.enabled
                                && !self.suspended
                                && self.capture.is_none()
                                && self.conflict.is_none()
                                && let Some(action) = action
                            {
                                actions.push(action);
                            }
                        }
                    }
                    _ => {}
                }
                if selected
                    && let Some(capture) = &mut self.capture
                    && capture.axis
                {
                    for (input, value) in device.state.axes.iter().enumerate() {
                        capture.armed[input] |= value.abs() < NEUTRAL_THRESHOLD;
                        if capture.armed[input] && value.abs() >= BIND_AXIS_THRESHOLD {
                            prefs.profiles.entry(device.key.clone()).or_default().axes
                                [capture.action]
                                .input = Some(input);
                            self.capture = None;
                            break;
                        }
                    }
                }
            }
            backend.inc();
        }
        ctx.request_repaint_after(if self.devices.is_empty() {
            HOTPLUG_POLL_INTERVAL
        } else {
            ACTIVE_POLL_INTERVAL
        });
        let Some(device) = self
            .devices
            .iter()
            .find(|device| device.key == prefs.selected)
        else {
            return (ControllerMotion::default(), actions);
        };
        if self.suspended
            && allowed
            && prefs.enabled
            && self.capture.is_none()
            && self.conflict.is_none()
            && neutral(&device.state, &prefs.profile(&device.key))
        {
            self.suspended = false;
        }
        let motion = if self.suspended || self.capture.is_some() || self.conflict.is_some() {
            ControllerMotion::default()
        } else {
            prefs.profile(&device.key).motion(&device.state)
        };
        (motion, actions)
    }
}
fn device(id: GamepadId, gamepad: gilrs::Gamepad<'_>) -> Device {
    // gilrs UUID identifies a model, not a unique physical serial number.
    let uuid: String = gamepad
        .uuid()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let key = format!(
        "{uuid}-{:04x}-{:04x}-{}",
        gamepad.vendor_id().unwrap_or_default(),
        gamepad.product_id().unwrap_or_default(),
        gamepad.name()
    );
    Device {
        id,
        key,
        name: gamepad.name().into(),
        state: ControllerState::default(),
    }
}
impl App {
    pub(super) fn controller_input(&mut self, ctx: &egui::Context, height_points: f32) -> bool {
        if ctx.current_pass_index() != 0 {
            return false;
        }
        let allowed = ctx.input(|input| input.focused)
            && !ctx.egui_wants_keyboard_input()
            && !self.show_settings
            && !self.show_about
            && !self.show_shortcuts
            && !self.explorer.open
            && self.altitude_editor.is_none()
            && self.ruler_point_editor.is_none()
            && !self.flat_earth.open
            && !self.update_restart.is_saving()
            && !egui::Popup::is_any_open(ctx);
        let (motion, actions) = self
            .controllers
            .poll(ctx, &mut self.settings.controllers, allowed);
        for action in actions {
            let menu = match action {
                ControllerAction::North => Some(Action::North),
                ControllerAction::Ruler => Some(Action::Ruler),
                ControllerAction::Places => Some(Action::ShowPlaces),
                ControllerAction::Settings => Some(Action::Settings),
                ControllerAction::Placemark => Some(Action::Placemark),
                ControllerAction::Cancel => {
                    self.fly = None;
                    self.ruler_paths.end_move();
                    self.ruler_drag = None;
                    None
                }
                ControllerAction::Reset => {
                    self.fly = None;
                    self.cam.reset_view();
                    None
                }
                ControllerAction::None => None,
            };
            if let Some(menu) = menu {
                self.run_menu_action(menu, ctx);
            }
        }
        if motion.is_active() && !self.show_settings {
            self.fly = None;
            let dt = super::frame_timing::navigation_delta_secs(ctx);
            motion.apply(
                &mut self.cam,
                dt,
                height_points,
                (self.explorer.active == earthcraft_core::body::Body::Earth)
                    .then_some(self.settings.earth_model),
            );
            return true;
        }
        false
    }
}

fn neutral(
    state: &ControllerState,
    profile: &earthcraft_core::controller::ControllerProfile,
) -> bool {
    profile.axes.iter().all(|binding| {
        binding.input.is_none_or(|input| {
            state
                .axes
                .get(input)
                .is_none_or(|value| value.abs() < NEUTRAL_THRESHOLD.max(binding.deadzone))
        })
    }) && state
        .buttons
        .iter()
        .zip(profile.buttons)
        .all(|(down, action)| !down || action == ControllerAction::None)
}
#[cfg(test)]
mod tests;
