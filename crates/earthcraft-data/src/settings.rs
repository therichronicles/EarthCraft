mod writer;
use crate::paths::{cache_dir, default_out_dir};
use earthcraft_core::{earth_model::EarthModel, language::Lang, units::RulerUnit};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub use writer::SettingsWriter;
pub const MIN_UI_SCALE: f32 = 0.7;
pub const MAX_UI_SCALE: f32 = 2.5;
pub const MIN_MAP_SCALE: f32 = 0.6;
pub const MAX_MAP_SCALE: f32 = 3.0;

fn lang_file() -> PathBuf {
    cache_dir().join("lang.txt")
}
pub fn load_lang() -> Lang {
    match std::fs::read_to_string(lang_file())
        .ok()
        .as_deref()
        .map(str::trim)
    {
        Some("pt-BR") => Lang::PtBr,
        _ => Lang::En, // English is the default
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitPreference {
    #[default]
    System,
    Metric,
    Imperial,
}
impl UnitPreference {
    pub fn resolve(
        self,
        system: earthcraft_core::units::DistanceUnits,
    ) -> earthcraft_core::units::DistanceUnits {
        use earthcraft_core::units::DistanceUnits;
        match self {
            Self::System => system,
            Self::Metric => DistanceUnits::Metric,
            Self::Imperial => DistanceUnits::Imperial,
        }
    }
}
#[derive(Clone, PartialEq, Serialize)]
pub struct Settings {
    pub screenshot_dir: String,
    pub kml_dir: String,
    pub ui_scale: f32,  // text size of buttons/panels
    pub map_scale: f32, // text size of map labels
    pub uncap_fps: bool,
    pub grid_numbers: bool,
    pub hide_navigation_controls: bool,
    pub distance_units: UnitPreference,
    #[serde(serialize_with = "serialize_earth_model")]
    pub earth_model: EarthModel,
    #[serde(serialize_with = "serialize_ruler_unit")]
    pub ruler_unit: RulerUnit,
}
fn serialize_earth_model<S: serde::Serializer>(
    model: &EarthModel,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(model.code())
}
fn serialize_ruler_unit<S: serde::Serializer>(
    unit: &RulerUnit,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(unit.code())
}
impl Default for Settings {
    fn default() -> Self {
        Settings {
            screenshot_dir: default_out_dir("Pictures"),
            kml_dir: default_out_dir("Documents"),
            ui_scale: 1.0,
            map_scale: 1.0,
            uncap_fps: false,
            grid_numbers: false,
            hide_navigation_controls: false,
            distance_units: UnitPreference::System,
            earth_model: EarthModel::default(),
            ruler_unit: RulerUnit::Automatic,
        }
    }
}
fn settings_file() -> PathBuf {
    cache_dir().join("settings.json")
}
#[derive(Default, Deserialize)]
struct SettingsInput {
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    screenshot_dir: Option<String>,
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    kml_dir: Option<String>,
    #[serde(default, deserialize_with = "crate::json_fields::number")]
    ui_scale: Option<f64>,
    #[serde(default, deserialize_with = "crate::json_fields::number")]
    map_scale: Option<f64>,
    #[serde(default, deserialize_with = "crate::json_fields::boolean")]
    uncap_fps: Option<bool>,
    #[serde(default, deserialize_with = "crate::json_fields::boolean")]
    grid_numbers: Option<bool>,
    #[serde(default, deserialize_with = "crate::json_fields::boolean")]
    hide_navigation_controls: Option<bool>,
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    distance_units: Option<String>,
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    earth_model: Option<String>,
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    ruler_unit: Option<String>,
}
impl Settings {
    fn apply_json(&mut self, bytes: &[u8]) -> Result<(), crate::Error> {
        let input: SettingsInput = serde_json::from_slice(bytes)?;
        if let Some(path) = input.screenshot_dir.filter(|s| !s.is_empty()) {
            self.screenshot_dir = path;
        }
        if let Some(path) = input.kml_dir.filter(|s| !s.is_empty()) {
            self.kml_dir = path;
        }
        if let Some(scale) = input.ui_scale {
            self.ui_scale = (scale as f32).clamp(MIN_UI_SCALE, MAX_UI_SCALE);
        }
        if let Some(scale) = input.map_scale {
            self.map_scale = (scale as f32).clamp(MIN_MAP_SCALE, MAX_MAP_SCALE);
        }
        if let Some(uncap) = input.uncap_fps {
            self.uncap_fps = uncap;
        }
        if let Some(hide) = input.hide_navigation_controls {
            self.hide_navigation_controls = hide;
        }
        if let Some(model) = input.earth_model.as_deref().and_then(EarthModel::from_code) {
            self.earth_model = model;
        }
        if let Some(units) = input.distance_units {
            self.distance_units = match units.as_str() {
                "metric" => UnitPreference::Metric,
                "imperial" => UnitPreference::Imperial,
                "system" => UnitPreference::System,
                _ => self.distance_units,
            };
        }
        if let Some(unit) = input.ruler_unit.as_deref().and_then(RulerUnit::from_code) {
            self.ruler_unit = unit;
        }
        if let Some(value) = input.grid_numbers {
            self.grid_numbers = value;
        }
        Ok(())
    }
    pub fn load() -> Self {
        let mut settings = Self::default();
        if let Ok(bytes) = std::fs::read(settings_file()) {
            let _ = settings.apply_json(&bytes);
        }
        settings
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn earth_models_roundtrip_and_tolerate_unknown_preferences() {
        let mut settings = Settings::default();
        assert_eq!(settings.earth_model, EarthModel::Wgs84);
        settings.apply_json(br#"{}"#).unwrap();
        assert_eq!(settings.earth_model, EarthModel::Wgs84);
        for model in EarthModel::ALL {
            settings.earth_model = model;
            let mut restored = Settings::default();
            restored
                .apply_json(&serde_json::to_vec(&settings).unwrap())
                .unwrap();
            assert_eq!(restored.earth_model, model);
            for bytes in [
                br#"{}"#.as_slice(),
                br#"{"earth_model":"unknown"}"#,
                br#"{"earth_model":42}"#,
            ] {
                restored.apply_json(bytes).unwrap();
                assert_eq!(restored.earth_model, model);
            }
        }
    }
    #[test]
    fn grid_preference_roundtrips_and_tolerates_missing_or_invalid_fields() {
        let mut settings = Settings::default();
        assert!(!settings.grid_numbers);
        settings.apply_json(br#"{"grid_numbers":true}"#).unwrap();
        for input in [
            br#"{"navigation_compass":false}"#.as_slice(),
            br#"{"grid_numbers":42}"#,
        ] {
            settings.apply_json(input).unwrap();
            assert!(settings.grid_numbers);
        }
        let mut restored = Settings::default();
        restored
            .apply_json(&serde_json::to_vec(&settings).unwrap())
            .unwrap();
        assert!(restored.grid_numbers);
        restored.apply_json(br#"{"grid_numbers":false}"#).unwrap();
        assert!(!restored.grid_numbers);
    }

    #[test]
    fn every_ruler_preference_roundtrips_and_invalid_values_keep_the_selection() {
        let mut settings = Settings::default();
        settings.apply_json(br#"{}"#).unwrap();
        assert_eq!(settings.ruler_unit, RulerUnit::Automatic);
        for unit in RulerUnit::ALL {
            settings.ruler_unit = unit;
            let bytes = serde_json::to_vec(&settings).unwrap();
            let mut loaded = Settings::default();
            loaded.apply_json(&bytes).unwrap();
            assert_eq!(loaded.ruler_unit, unit);
            loaded
                .apply_json(br#"{"distance_units":"imperial"}"#)
                .unwrap();
            assert_eq!(loaded.ruler_unit, unit);
            loaded.apply_json(br#"{"ruler_unit":"unknown"}"#).unwrap();
            assert_eq!(loaded.ruler_unit, unit);
            loaded.apply_json(br#"{"ruler_unit":42}"#).unwrap();
            assert_eq!(loaded.ruler_unit, unit);
        }
    }
    #[test]
    fn units_roundtrip_and_tolerate_old_or_invalid_preferences() {
        use earthcraft_core::units::DistanceUnits;
        let mut settings = Settings::default();
        settings.apply_json(br#"{}"#).unwrap();
        assert_eq!(settings.distance_units, UnitPreference::System);
        settings
            .apply_json(br#"{"distance_units":"imperial"}"#)
            .unwrap();
        assert_eq!(
            settings.distance_units.resolve(DistanceUnits::Metric),
            DistanceUnits::Imperial
        );
        for bytes in [
            br#"{"distance_units":42}"#.as_slice(),
            br#"{"distance_units":"unknown"}"#,
        ] {
            settings.apply_json(bytes).unwrap();
            assert_eq!(settings.distance_units, UnitPreference::Imperial);
        }
        let bytes = serde_json::to_vec(&settings).unwrap();
        let mut loaded = Settings::default();
        loaded.apply_json(&bytes).unwrap();
        assert_eq!(loaded.distance_units, UnitPreference::Imperial);
        loaded
            .apply_json(br#"{"distance_units":"system"}"#)
            .unwrap();
        assert_eq!(
            loaded.distance_units.resolve(DistanceUnits::Metric),
            DistanceUnits::Metric
        );
        assert_eq!(
            loaded.distance_units.resolve(DistanceUnits::Imperial),
            DistanceUnits::Imperial
        );
    }
    #[test]
    fn partial_invalid_settings_preserve_valid_fields_and_clamping() {
        let mut settings = Settings {
            screenshot_dir: "pictures".into(),
            kml_dir: "documents".into(),
            ui_scale: 1.0,
            map_scale: 1.0,
            uncap_fps: false,
            grid_numbers: false,
            hide_navigation_controls: false,
            distance_units: UnitPreference::System,
            earth_model: EarthModel::default(),
            ruler_unit: RulerUnit::Automatic,
        };
        settings
            .apply_json(
                br#"{"screenshot_dir":123,"kml_dir":"custom","ui_scale":9,"map_scale":"wrong","uncap_fps":"wrong"}"#,
            )
            .unwrap();
        assert_eq!(settings.screenshot_dir, "pictures");
        assert_eq!(settings.kml_dir, "custom");
        assert_eq!(settings.ui_scale, MAX_UI_SCALE);
        assert_eq!(settings.map_scale, 1.0);
        assert!(!settings.uncap_fps);
        assert!(!settings.hide_navigation_controls);
        settings
            .apply_json(br#"{"kml_dir":"","ui_scale":-5,"map_scale":9,"uncap_fps":true,"hide_navigation_controls":true}"#)
            .unwrap();
        assert_eq!(settings.kml_dir, "custom");
        assert_eq!(settings.ui_scale, MIN_UI_SCALE);
        assert_eq!(settings.map_scale, MAX_MAP_SCALE);
        assert!(settings.uncap_fps);
        settings.apply_json(br#"{}"#).unwrap();
        assert!(settings.uncap_fps);
        assert!(settings.hide_navigation_controls);
        settings
            .apply_json(br#"{"hide_navigation_controls":"wrong"}"#)
            .unwrap();
        assert!(settings.hide_navigation_controls);
        let json: serde_json::Value =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(json["kml_dir"], "custom");
        assert_eq!(json["uncap_fps"], true);
        assert_eq!(json["hide_navigation_controls"], true);
        let mut restored = Settings::default();
        restored
            .apply_json(&serde_json::to_vec(&settings).unwrap())
            .unwrap();
        assert!(restored.hide_navigation_controls);
    }
}
