use earthcraft_core::language::Lang;
use std::sync::{Arc, Mutex};
mod brazil;
mod geojson;
mod load;
mod updates;
mod world;
pub use geojson::dedupe_city_labels;
pub use load::{BoundaryLoader, load_boundaries_async};
pub use updates::{BoundarySnapshot, BoundaryUpdates};
pub type Line = Arc<Vec<(f64, f64)>>;

#[derive(Clone, Default)]
pub struct BoundaryData {
    pub countries: Vec<Line>,
    pub states: Vec<Line>, // admin-1 of "ordinary" countries (shown at medium zoom)
    pub states_big: Vec<Line>, // admin-1 of the largest countries (shown even with the whole Earth in view)
    pub cities: Vec<Line>,
    pub world_cities: Vec<Line>,
    pub metro: Vec<Line>,
    pub labels: Vec<MapLabel>,
}

/// States/provinces of these countries (the 10 largest by area) are visible when zoomed far out.
const BIG_COUNTRIES_A3: [&str; 10] = [
    "RUS", "CAN", "USA", "CHN", "BRA", "AUS", "IND", "ARG", "KAZ", "DZA",
];
const BIG_COUNTRIES_NAME: [&str; 10] = [
    "Russia",
    "Canada",
    "United States of America",
    "China",
    "Brazil",
    "Australia",
    "India",
    "Argentina",
    "Kazakhstan",
    "Algeria",
];
/// Below this camera altitude (km) the states of every other country appear too.
pub const OTHER_STATES_MAX_ALTITUDE_KM: f64 = 3000.0;
const BIG_STATE_LABEL_MAX_ALTITUDE_KM: f64 = 22_000.0;

const METRO_LABEL_MAX_ALTITUDE_KM: f64 = 5000.0;
const CITY_RANK_0_1_MAX_ALTITUDE_KM: f64 = 9000.0;
const CITY_RANK_2_3_MAX_ALTITUDE_KM: f64 = 4000.0;
const CITY_RANK_4_5_MAX_ALTITUDE_KM: f64 = 2000.0;
const CITY_RANK_6_7_MAX_ALTITUDE_KM: f64 = 900.0;
const OTHER_CITY_MAX_ALTITUDE_KM: f64 = 300.0;

pub(super) fn is_big_country(feature: &serde_json::Value) -> bool {
    let Some(props) = feature.get("properties") else {
        return false;
    };
    for k in ["adm0_a3", "ADM0_A3", "sov_a3", "SOV_A3"] {
        if let Some(a3) = props.get(k).and_then(|v| v.as_str()) {
            return BIG_COUNTRIES_A3.contains(&a3);
        }
    }
    for k in ["admin", "ADMIN", "geonunit"] {
        if let Some(n) = props.get(k).and_then(|v| v.as_str()) {
            return BIG_COUNTRIES_NAME.contains(&n);
        }
    }
    false
}

#[derive(Clone)]
pub struct MapLabel {
    pub lat: f64,
    pub lon: f64,
    pub en: String,
    pub pt: String,
    pub kind: LabelKind,
    pub rank: u8,
    pub big: bool,
}
impl MapLabel {
    pub fn text(&self, lg: Lang) -> &str {
        match lg {
            Lang::En => &self.en,
            Lang::PtBr => &self.pt,
        }
    }
    /// Whether the label should be shown at the given camera altitude (km).
    /// Bigger places (lower rank) appear first, smaller ones only when zoomed in.
    pub fn visible_at(&self, alt: f64) -> bool {
        match self.kind {
            LabelKind::Country => true,
            LabelKind::State => {
                alt < if self.big {
                    BIG_STATE_LABEL_MAX_ALTITUDE_KM
                } else {
                    OTHER_STATES_MAX_ALTITUDE_KM
                }
            }
            LabelKind::Metro => alt < METRO_LABEL_MAX_ALTITUDE_KM,
            LabelKind::City => {
                alt < match self.rank {
                    0..=1 => CITY_RANK_0_1_MAX_ALTITUDE_KM,
                    2..=3 => CITY_RANK_2_3_MAX_ALTITUDE_KM,
                    4..=5 => CITY_RANK_4_5_MAX_ALTITUDE_KM,
                    6..=7 => CITY_RANK_6_7_MAX_ALTITUDE_KM,
                    _ => OTHER_CITY_MAX_ALTITUDE_KM,
                }
            }
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
pub enum LabelKind {
    Country,
    State,
    City,
    Metro,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Layer {
    Countries,
    States,
    Cities,
    WorldCities,
    Metro,
}

/// One finished piece of overlay data, sent from a loader thread to the UI thread.
pub struct Patch {
    pub layer: Layer,
    pub lines: Vec<Line>,
    pub lines_big: Vec<Line>,
    pub labels: Vec<MapLabel>,
}

/// Human-readable load log shown in the Layers panel, so failures are never silent.
pub type StatusLog = Arc<Mutex<Vec<String>>>;

pub(super) fn log_status(log: &StatusLog, msg: String) {
    if let Ok(mut v) = log.lock() {
        v.push(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn big_countries() {
        let f = serde_json::json!({"properties": {"adm0_a3": "BRA", "admin": "Brazil"}});
        assert!(is_big_country(&f));
        let f = serde_json::json!({"properties": {"adm0_a3": "FRA", "admin": "France"}});
        assert!(!is_big_country(&f));
    }
}
