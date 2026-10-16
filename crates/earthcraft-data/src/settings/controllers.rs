//! Local per-model profiles; device identifiers never leave this settings file.
use earthcraft_core::controller::ControllerProfile;
use serde::Serialize;
use std::collections::BTreeMap;
const MAX_PROFILES: usize = 64;
const MAX_DEVICE_KEY_BYTES: usize = 256;
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ControllerPreferences {
    pub enabled: bool,
    pub selected: String,
    pub profiles: BTreeMap<String, ControllerProfile>,
}
impl Default for ControllerPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            selected: String::new(),
            profiles: BTreeMap::new(),
        }
    }
}
impl ControllerPreferences {
    pub fn profile(&self, device: &str) -> ControllerProfile {
        self.profiles.get(device).cloned().unwrap_or_default()
    }
    pub(super) fn apply(&mut self, value: &serde_json::Value) {
        if let Some(enabled) = value.get("enabled").and_then(|v| v.as_bool()) {
            self.enabled = enabled;
        }
        if let Some(selected) = value
            .get("selected")
            .and_then(|v| v.as_str())
            .filter(|s| s.len() <= MAX_DEVICE_KEY_BYTES)
        {
            self.selected = selected.into();
        }
        if let Some(profiles) = value.get("profiles").and_then(|v| v.as_object()) {
            for (key, value) in profiles.iter().take(MAX_PROFILES) {
                if key.len() <= MAX_DEVICE_KEY_BYTES
                    && let Ok(mut profile) =
                        serde_json::from_value::<ControllerProfile>(value.clone())
                {
                    profile.normalize();
                    self.profiles.insert(key.clone(), profile);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_profiles_survive_reconnect_roundtrip_and_bad_siblings() {
        let mut prefs = ControllerPreferences::default();
        let mut one = ControllerProfile::default();
        one.axes[0].inverted = true;
        prefs.profiles.insert("uuid-one".into(), one.clone());
        prefs
            .profiles
            .insert("uuid-two".into(), ControllerProfile::default());
        prefs.selected = "uuid-one".into();
        let mut loaded = ControllerPreferences::default();
        loaded.apply(&serde_json::to_value(&prefs).unwrap());
        assert_eq!(loaded, prefs);
        loaded.apply(&serde_json::json!({"profiles":{"broken":42,"uuid-two":{"axes":"wrong"}},"enabled":false}));
        assert_eq!(loaded.profile("uuid-one"), one);
        assert_eq!(loaded.profiles.len(), 2);
        assert!(!loaded.enabled);
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    #[test]
    fn legacy_five_axis_profiles_keep_custom_settings_and_gain_tilt() {
        let mut profile = ControllerProfile::default();
        profile.axes[0].inverted = true;
        let mut value = serde_json::to_value(&profile).unwrap();
        value["axes"].as_array_mut().unwrap().pop();
        let loaded: ControllerProfile = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.axes[0], profile.axes[0]);
        assert_eq!(loaded.axes[5].input, Some(3));
    }
}
