//! Ruler units format surface lengths and central angles without changing geometry.
use super::{DistanceUnits, KM_PER_MILE};
use crate::{EARTH_RADIUS_KM, METERS_PER_KM, language::Lang};
const METERS_PER_INCH: f64 = 0.0254;
const INCHES_PER_FOOT: f64 = 12.0;
const FEET_PER_YARD: f64 = 3.0;
const METERS_PER_FOOT: f64 = METERS_PER_INCH * INCHES_PER_FOOT;
const METERS_PER_YARD: f64 = METERS_PER_FOOT * FEET_PER_YARD;
// International nautical mile (NIST SP 811); a smoot is 67 inches (MIT).
const METERS_PER_NAUTICAL_MILE: f64 = 1852.0;
const INCHES_PER_SMOOT: f64 = 67.0;
// The playing field between goal lines, excluding both end zones.
const YARDS_PER_FOOTBALL_FIELD: f64 = 100.0;
const CENTIMETERS_PER_METER: f64 = 100.0;
const ARCSECONDS_PER_DEGREE: f64 = 3600.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RulerUnit {
    #[default]
    Automatic,
    Centimeters,
    Meters,
    Kilometers,
    Inches,
    Feet,
    Yards,
    Miles,
    NauticalMiles,
    Smoots,
    Degrees,
    Arcseconds,
    FootballFields,
}
impl RulerUnit {
    pub const ALL: [Self; 13] = [
        Self::Automatic,
        Self::Centimeters,
        Self::Meters,
        Self::Kilometers,
        Self::Inches,
        Self::Feet,
        Self::Yards,
        Self::Miles,
        Self::NauticalMiles,
        Self::Smoots,
        Self::Degrees,
        Self::Arcseconds,
        Self::FootballFields,
    ];
    /// Stable preference value, independent of UI language.
    pub fn code(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Centimeters => "centimeters",
            Self::Meters => "meters",
            Self::Kilometers => "kilometers",
            Self::Inches => "inches",
            Self::Feet => "feet",
            Self::Yards => "yards",
            Self::Miles => "miles",
            Self::NauticalMiles => "nautical_miles",
            Self::Smoots => "smoots",
            Self::Degrees => "degrees",
            Self::Arcseconds => "arcseconds",
            Self::FootballFields => "football_fields",
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|unit| unit.code() == code)
    }
    pub fn label(self, lang: Lang) -> &'static str {
        lang.text(match self {
            Self::Automatic => "ruler-unit-automatic",
            Self::Centimeters => "ruler-unit-centimeters",
            Self::Meters => "ruler-unit-meters",
            Self::Kilometers => "ruler-unit-kilometers",
            Self::Inches => "ruler-unit-inches",
            Self::Feet => "ruler-unit-feet",
            Self::Yards => "ruler-unit-yards",
            Self::Miles => "ruler-unit-miles",
            Self::NauticalMiles => "ruler-unit-nautical-miles",
            Self::Smoots => "ruler-unit-smoots",
            Self::Degrees => "ruler-unit-degrees",
            Self::Arcseconds => "ruler-unit-arcseconds",
            Self::FootballFields => "ruler-unit-football-fields",
        })
    }
    /// A selector may clarify a unit's name without lengthening measurement labels.
    pub fn selector_label(self, lang: Lang, us_locale: bool) -> &'static str {
        let override_id = match self {
            Self::FootballFields if !us_locale => Some("ruler-unit-american-football-fields"),
            _ => None,
        };
        override_id.map_or_else(|| self.label(lang), |id| lang.text(id))
    }
    pub fn help(self, lang: Lang) -> &'static str {
        lang.text(match self {
            Self::Automatic => "ruler-unit-automatic-help",
            Self::Centimeters => "ruler-unit-centimeters-help",
            Self::Meters => "ruler-unit-meters-help",
            Self::Kilometers => "ruler-unit-kilometers-help",
            Self::Inches => "ruler-unit-inches-help",
            Self::Feet => "ruler-unit-feet-help",
            Self::Yards => "ruler-unit-yards-help",
            Self::Miles => "ruler-unit-miles-help",
            Self::NauticalMiles => "ruler-unit-nautical-miles-help",
            Self::Smoots => "ruler-unit-smoots-help",
            Self::Degrees => "ruler-unit-degrees-help",
            Self::Arcseconds => "ruler-unit-arcseconds-help",
            Self::FootballFields => "ruler-unit-football-fields-help",
        })
    }
    pub fn format_with_angle(
        self,
        distance_km: f64,
        angle_degrees: f64,
        automatic: DistanceUnits,
        lang: Lang,
    ) -> String {
        let angle = if angle_degrees == 0.0 {
            0.0
        } else {
            angle_degrees
        };
        match self {
            Self::Degrees => format!("{angle:.2} °"),
            Self::Arcseconds => format!("{:.2} ″", angle * ARCSECONDS_PER_DEGREE),
            _ => self.format(distance_km, automatic, lang),
        }
    }
    /// Format a segment or path total. Angular units express accumulated central
    /// angle on EarthCraft's mean-radius sphere, not coordinate differences.
    pub fn format(self, distance_km: f64, automatic: DistanceUnits, lang: Lang) -> String {
        // Empty floating-point sums can produce -0.0. Distances have no signed
        // zero, so normalize it before automatic or explicit-unit formatting.
        let distance_km = if distance_km == 0.0 { 0.0 } else { distance_km };
        let meters = distance_km * METERS_PER_KM;
        let (value, suffix) = match self {
            Self::Automatic => return automatic.distance(distance_km),
            Self::Centimeters => (meters * CENTIMETERS_PER_METER, "cm"),
            Self::Meters => (meters, "m"),
            Self::Kilometers => (distance_km, "km"),
            Self::Inches => (meters / METERS_PER_INCH, "in"),
            Self::Feet => (meters / METERS_PER_FOOT, "ft"),
            Self::Yards => (meters / METERS_PER_YARD, "yd"),
            Self::Miles => (distance_km / KM_PER_MILE, "mi"),
            Self::NauticalMiles => (meters / METERS_PER_NAUTICAL_MILE, "nmi"),
            Self::Smoots => {
                return lang.format(
                    "ruler-distance-smoots",
                    &[(
                        "value",
                        format!("{:.2}", meters / (INCHES_PER_SMOOT * METERS_PER_INCH)).into(),
                    )],
                );
            }
            Self::Degrees => ((distance_km / EARTH_RADIUS_KM).to_degrees(), "°"),
            Self::Arcseconds => (
                (distance_km / EARTH_RADIUS_KM).to_degrees() * ARCSECONDS_PER_DEGREE,
                "″",
            ),
            Self::FootballFields => {
                return lang.format(
                    "ruler-distance-football-fields",
                    &[(
                        "value",
                        format!(
                            "{:.2}",
                            meters / (YARDS_PER_FOOTBALL_FIELD * METERS_PER_YARD)
                        )
                        .into(),
                    )],
                );
            }
        };
        format!("{value:.2} {suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_ruler_totals_never_display_negative_zero_in_any_unit() {
        let empty_total: f64 = std::iter::empty::<f64>().sum();
        for distance in [empty_total, -0.0, 0.0] {
            for unit in RulerUnit::ALL {
                for automatic in [DistanceUnits::Metric, DistanceUnits::Imperial] {
                    for lang in Lang::ALL {
                        let label = unit.format(distance, automatic, lang);
                        assert!(!label.starts_with('-'), "{unit:?}: {label}");
                        assert_eq!(label, unit.format(0.0, automatic, lang));
                    }
                }
            }
        }
        let distance = RulerUnit::Automatic.format(empty_total, DistanceUnits::Imperial, Lang::En);
        assert_eq!(distance, "0 ft");
        assert_eq!(
            Lang::En.format(
                "ruler-measurement-summary",
                &[("count", "0".into()), ("distance", distance.into())]
            ),
            "Points: 0   Total: 0 ft"
        );
    }
    #[test]
    fn explicit_units_are_independent_of_global_distance_preferences() {
        for unit in RulerUnit::ALL
            .into_iter()
            .filter(|unit| *unit != RulerUnit::Automatic)
        {
            for lang in Lang::ALL {
                assert_eq!(
                    unit.format(123.45, DistanceUnits::Metric, lang),
                    unit.format(123.45, DistanceUnits::Imperial, lang)
                );
            }
        }
        assert_ne!(
            RulerUnit::Automatic.format(123.45, DistanceUnits::Metric, Lang::En),
            RulerUnit::Automatic.format(123.45, DistanceUnits::Imperial, Lang::En)
        );
    }
    #[test]
    fn ruler_units_match_known_lengths_and_angles() {
        let format = |unit: RulerUnit, km| unit.format(km, DistanceUnits::Metric, Lang::En);
        for (unit, km, expected) in [
            (RulerUnit::Centimeters, 0.001, "100.00 cm"),
            (RulerUnit::Meters, 0.001, "1.00 m"),
            (RulerUnit::Kilometers, 1.0, "1.00 km"),
            (RulerUnit::Inches, 0.0000254, "1.00 in"),
            (RulerUnit::Feet, 0.0003048, "1.00 ft"),
            (RulerUnit::Yards, 0.0009144, "1.00 yd"),
            (RulerUnit::Miles, 1.609344, "1.00 mi"),
            (RulerUnit::NauticalMiles, 1.852, "1.00 nmi"),
            (RulerUnit::Smoots, 0.0017018, "1.00 smoots"),
            (RulerUnit::FootballFields, 0.09144, "1.00 football fields"),
            (
                RulerUnit::Degrees,
                EARTH_RADIUS_KM * std::f64::consts::FRAC_PI_2,
                "90.00 °",
            ),
            (
                RulerUnit::Arcseconds,
                EARTH_RADIUS_KM * (1.0_f64 / 3600.0).to_radians(),
                "1.00 ″",
            ),
        ] {
            assert_eq!(format(unit, km), expected);
        }
        assert_eq!(
            RulerUnit::Automatic.format(1.609344, DistanceUnits::Imperial, Lang::En),
            "1.00 mi"
        );
    }
    #[test]
    fn selector_names_can_differ_from_measurement_labels_by_region() {
        assert!(super::super::is_us_locale("es-US"));
        assert!(super::super::is_us_locale("en_US.UTF-8"));
        assert!(!super::super::is_us_locale("en-GB"));
        assert!(!super::super::is_us_locale("en"));
        for unit in RulerUnit::ALL {
            assert_eq!(RulerUnit::from_code(unit.code()), Some(unit));
            for lang in Lang::ALL {
                assert!(!unit.label(lang).starts_with("ruler-unit-"));
                assert!(!unit.help(lang).starts_with("ruler-unit-"));
                for us_locale in [true, false] {
                    assert!(
                        !unit
                            .selector_label(lang, us_locale)
                            .starts_with("ruler-unit-")
                    );
                    if unit != RulerUnit::FootballFields {
                        assert_eq!(unit.selector_label(lang, us_locale), unit.label(lang));
                    }
                }
            }
        }
        assert_eq!(
            RulerUnit::FootballFields.selector_label(Lang::En, true),
            "Football fields"
        );
        assert_eq!(
            RulerUnit::FootballFields.selector_label(Lang::En, false),
            "American football fields"
        );
        assert_eq!(
            RulerUnit::FootballFields.format(0.18288, DistanceUnits::Metric, Lang::En),
            "2.00 football fields"
        );
        assert_eq!(
            RulerUnit::FootballFields.format(0.18288, DistanceUnits::Metric, Lang::PtBr),
            "2.00 campos de futebol"
        );
    }
}
