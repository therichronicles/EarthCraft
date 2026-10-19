//! Device-independent Explore bindings. Native adapters supply normalized samples.
use crate::{MAX_CAMERA_ALTITUDE_KM, MIN_CAMERA_ALTITUDE_KM, camera::Cam, math::wrap_lon};
use serde::{Deserialize, Serialize};

pub const AXIS_COUNT: usize = 6;
pub const BUTTON_COUNT: usize = 19;
pub const MAX_DEADZONE: f64 = 0.8;
pub const MIN_SENSITIVITY: f64 = 0.1;
pub const MAX_SENSITIVITY: f64 = 4.0;
const PAN_VIEW_HEIGHTS_PER_SEC: f64 = 0.5;
const ROTATION_DEG_PER_SEC: f64 = 90.0;
const TILT_DEG_PER_SEC: f64 = 60.0;
const ZOOM_EXPONENT_PER_SEC: f64 = 1.0;
const MAX_STEP_SECS: f64 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerAction {
    None,
    North,
    Reset,
    Ruler,
    Places,
    Settings,
    Cancel,
    Placemark,
}
impl ControllerAction {
    pub const ALL: [Self; 8] = [
        Self::None,
        Self::North,
        Self::Reset,
        Self::Ruler,
        Self::Places,
        Self::Settings,
        Self::Cancel,
        Self::Placemark,
    ];
    pub fn label_id(self) -> &'static str {
        match self {
            Self::None => "controllers-action-none",
            Self::Reset => "navigation-reset-view-action",
            Self::North => "navigation-reset-north-action",
            Self::Ruler => "ruler-enable-toggle",
            Self::Places => "sidebar-places-title",
            Self::Settings => "settings-title",
            Self::Cancel => "controllers-action-cancel",
            Self::Placemark => "menu-add-placemark",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AxisBinding {
    /// Index in the adapter's six normalized axes; None disables this action.
    pub input: Option<usize>,
    pub inverted: bool,
    pub deadzone: f64,
    pub sensitivity: f64,
    /// 1 is linear; 2 makes small movements gentler.
    pub curve: f64,
}
impl Default for AxisBinding {
    fn default() -> Self {
        Self {
            input: None,
            inverted: false,
            deadzone: 0.15,
            sensitivity: 1.0,
            curve: 1.0,
        }
    }
}
impl AxisBinding {
    pub fn normalize(&mut self) {
        self.input = self.input.filter(|input| *input < AXIS_COUNT);
        self.deadzone = finite_or(self.deadzone, 0.15).clamp(0.0, MAX_DEADZONE);
        self.sensitivity = finite_or(self.sensitivity, 1.0).clamp(MIN_SENSITIVITY, MAX_SENSITIVITY);
        self.curve = finite_or(self.curve, 1.0).clamp(1.0, 3.0);
    }
    fn sample(self, axes: &[f64; AXIS_COUNT]) -> f64 {
        let mut binding = self;
        binding.normalize();
        let Some(input) = binding.input else {
            return 0.0;
        };
        let value = finite_or(axes[input], 0.0).clamp(-1.0, 1.0);
        let magnitude = ((value.abs() - binding.deadzone).max(0.0) / (1.0 - binding.deadzone))
            .powf(binding.curve);
        magnitude * value.signum() * binding.sensitivity * if binding.inverted { -1.0 } else { 1.0 }
    }
}
fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControllerProfile {
    /// Pan horizontal, pan vertical, heading, zoom out, zoom in, tilt.
    #[serde(deserialize_with = "deserialize_axes")]
    pub axes: [AxisBinding; 6],
    pub buttons: [ControllerAction; BUTTON_COUNT],
}
fn deserialize_axes<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<[AxisBinding; 6], D::Error> {
    let axes = Vec::<AxisBinding>::deserialize(deserializer)?;
    if axes.len() != 5 && axes.len() != 6 {
        return Err(serde::de::Error::custom(
            "expected five legacy or six axis bindings",
        ));
    }
    let mut result = ControllerProfile::default().axes;
    for (target, binding) in result.iter_mut().zip(axes) {
        *target = binding;
    }
    Ok(result)
}
impl Default for ControllerProfile {
    fn default() -> Self {
        let mut buttons = [ControllerAction::None; BUTTON_COUNT];
        buttons[0] = ControllerAction::Placemark;
        buttons[1] = ControllerAction::Cancel;
        buttons[2] = ControllerAction::Reset;
        buttons[3] = ControllerAction::Ruler;
        buttons[8] = ControllerAction::Places;
        buttons[9] = ControllerAction::Settings;
        Self {
            axes: [Some(0), Some(1), Some(2), Some(4), Some(5), Some(3)].map(|input| AxisBinding {
                input,
                ..Default::default()
            }),
            buttons,
        }
    }
}
impl ControllerProfile {
    /// Moving an existing action never silently overwrites another button action.
    pub fn move_button(&mut self, source: usize, target: usize, replace: bool) -> bool {
        if source >= BUTTON_COUNT || target >= BUTTON_COUNT {
            return false;
        }
        if source != target && self.buttons[target] != ControllerAction::None && !replace {
            return false;
        }
        let action = self.buttons[source];
        self.buttons[source] = ControllerAction::None;
        self.buttons[target] = action;
        true
    }
    pub fn normalize(&mut self) {
        for axis in &mut self.axes {
            axis.normalize();
        }
    }
    /// Assigning one physical axis to several actions is allowed and explicit.
    pub fn motion(&self, state: &ControllerState) -> ControllerMotion {
        let values = self.axes.map(|binding| binding.sample(&state.axes));
        ControllerMotion {
            pan_x: values[0],
            pan_y: values[1],
            heading: values[2],
            zoom: values[3] - values[4],
            tilt: values[5],
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct ControllerState {
    pub axes: [f64; AXIS_COUNT],
    pub buttons: [bool; BUTTON_COUNT],
}
impl ControllerState {
    pub fn release(&mut self) {
        *self = Self::default();
    }
    /// Rising edges only: held buttons cannot repeatedly toggle tools.
    pub fn button(
        &mut self,
        input: usize,
        down: bool,
        profile: &ControllerProfile,
    ) -> Option<ControllerAction> {
        let pressed = self.buttons.get_mut(input)?;
        let rising = down && !*pressed;
        *pressed = down;
        rising
            .then_some(profile.buttons[input])
            .filter(|action| *action != ControllerAction::None)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControllerMotion {
    pub pan_x: f64,
    pub pan_y: f64,
    pub heading: f64,
    pub zoom: f64,
    pub tilt: f64,
}
impl ControllerMotion {
    pub fn is_active(self) -> bool {
        self != Self::default()
    }
    /// Same validated camera primitives as mouse/keyboard.
    pub fn apply(
        self,
        camera: &mut Cam,
        delta_secs: f64,
        viewport_height_points: f32,
        earth_model: Option<crate::earth_model::EarthModel>,
    ) {
        let dt = finite_or(delta_secs, 0.0).clamp(0.0, MAX_STEP_SECS);
        let height = f64::from(viewport_height_points.max(1.0));
        let dx = -self.pan_x * height * PAN_VIEW_HEIGHTS_PER_SEC * dt;
        let dy = self.pan_y * height * PAN_VIEW_HEIGHTS_PER_SEC * dt;
        if let Some(model) = earth_model {
            camera.pan_for_model(dx, dy, viewport_height_points, model);
        } else {
            camera.orbit_body(dx, dy, viewport_height_points);
        }
        if earth_model.is_some() {
            camera.tilt_by(self.tilt * TILT_DEG_PER_SEC * dt);
        }
        camera.heading = wrap_lon(camera.heading + self.heading * ROTATION_DEG_PER_SEC * dt);
        camera.alt = (camera.alt * (self.zoom * ZOOM_EXPONENT_PER_SEC * dt).exp())
            .clamp(MIN_CAMERA_ALTITUDE_KM, MAX_CAMERA_ALTITUDE_KM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn button_remapping_requires_explicit_conflict_replacement() {
        let mut profile = ControllerProfile::default();
        let original = profile.clone();
        assert!(!profile.move_button(3, 2, false));
        assert_eq!(profile, original);
        assert!(profile.move_button(3, 2, true));
        assert_eq!(profile.buttons[2], ControllerAction::Ruler);
        assert_eq!(profile.buttons[3], ControllerAction::None);
        assert!(!profile.move_button(BUTTON_COUNT, 0, true));
    }
    #[test]
    fn neutral_noise_deadzone_inversion_and_release() {
        let mut profile = ControllerProfile::default();
        let mut state = ControllerState::default();
        state.axes[0] = 0.149;
        assert!(!profile.motion(&state).is_active());
        state.axes[0] = 1.0;
        assert_eq!(profile.motion(&state).pan_x, 1.0);
        profile.axes[0].inverted = true;
        profile.axes[0].sensitivity = 2.0;
        assert_eq!(profile.motion(&state).pan_x, -2.0);
        assert_eq!(
            state.button(3, true, &profile),
            Some(ControllerAction::Ruler)
        );
        assert_eq!(state.button(3, true, &profile), None);
        state.release();
        assert!(!profile.motion(&state).is_active());
        assert!(state.buttons.iter().all(|down| !down));
        assert_eq!(state.button(BUTTON_COUNT, true, &profile), None);
    }
    #[test]
    fn invalid_axis_settings_are_bounded_and_camera_steps_are_finite() {
        let mut binding = AxisBinding {
            input: Some(999),
            deadzone: 1.0,
            sensitivity: f64::NAN,
            curve: 99.0,
            inverted: false,
        };
        binding.normalize();
        assert_eq!(binding.input, None);
        assert_eq!(binding.deadzone, MAX_DEADZONE);
        assert_eq!(binding.sensitivity, 1.0);
        let mut cam = Cam {
            tilt: 0.0,
            lat: 89.0,
            lon: 179.0,
            alt: 100.0,
            heading: 0.0,
        };
        let motion = ControllerMotion {
            pan_x: 1.0,
            pan_y: 1.0,
            heading: 1.0,
            zoom: 1.0,
            tilt: 1.0,
        };
        motion.apply(
            &mut cam,
            0.0,
            600.0,
            Some(crate::earth_model::EarthModel::Wgs84),
        );
        assert_eq!(cam.alt, 100.0);
        motion.apply(
            &mut cam,
            9.0,
            600.0,
            Some(crate::earth_model::EarthModel::Wgs84),
        );
        assert!(cam.lat.abs() <= crate::MAX_CAMERA_LATITUDE_DEG);
        assert!((-180.0..180.0).contains(&cam.lon));
        assert!((cam.heading - 9.0).abs() < 1e-10);
        assert!((cam.alt - 100.0 * 0.1_f64.exp()).abs() < 1e-10);
    }
}

#[cfg(test)]
mod tilt_tests {
    use super::*;
    #[test]
    fn default_right_y_tilts_without_zoom_or_heading_and_release_stops_motion() {
        let profile = ControllerProfile::default();
        let mut state = ControllerState::default();
        state.axes[3] = 1.0;
        let motion = profile.motion(&state);
        assert_eq!((motion.heading, motion.zoom), (0.0, 0.0));
        assert_eq!(motion.tilt, 1.0);
        let mut camera = Cam {
            lat: 0.0,
            lon: 0.0,
            heading: 0.0,
            alt: 100.0,
            tilt: 0.0,
        };
        motion.apply(
            &mut camera,
            0.1,
            600.0,
            Some(crate::earth_model::EarthModel::Wgs84),
        );
        assert_eq!(camera.tilt, 6.0);
        state.release();
        assert!(!profile.motion(&state).is_active());
        motion.apply(&mut camera, 0.1, 600.0, None);
        assert_eq!(camera.tilt, 6.0);
    }
}
