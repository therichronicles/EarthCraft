//! Display units only. Geographic calculations and persisted coordinates stay metric.
use crate::{
    METERS_PER_KM,
    math::{fmt_alt, fmt_dist},
};
mod altitude;
pub use altitude::AltitudeUnit;
mod ruler;
pub use ruler::RulerUnit;

const KM_PER_MILE: f64 = 1.609_344;
const FEET_PER_MILE: f64 = 5280.0;
// EarthCraft's regional display policy includes mixed/traditional mile usage.
// This deliberately extends CLDR's general length defaults (US/GB); it does not
// describe statutory measurement systems. Users can override it in Settings.
const MILE_DEFAULT_REGIONS: &[&str] = &[
    "US", "GB", "LR", "MM", "BZ", "BS", "VI", "GU", "PR", "KY", "GG", "JE", "IM",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceUnits {
    Metric,
    Imperial,
}
impl DistanceUnits {
    /// Resolve the app's regional display default; language alone has no region.
    /// See the regional unit policy in docs/ARCHITECTURE.md.
    pub fn for_locale(locale: &str) -> Self {
        if locale_region(locale)
            .is_some_and(|region| MILE_DEFAULT_REGIONS.contains(&region.as_str()))
        {
            Self::Imperial
        } else {
            Self::Metric
        }
    }
    pub fn distance(self, km: f64) -> String {
        match self {
            Self::Metric => fmt_dist(km),
            Self::Imperial => {
                let miles = km / KM_PER_MILE;
                if (miles * FEET_PER_MILE).round() < FEET_PER_MILE {
                    format!("{:.0} ft", miles * FEET_PER_MILE)
                } else {
                    format!("{miles:.2} mi")
                }
            }
        }
    }
    pub fn altitude(self, km: f64) -> String {
        match self {
            Self::Metric => fmt_alt(km),
            Self::Imperial => {
                let miles = km / KM_PER_MILE;
                if (miles * FEET_PER_MILE).round() < FEET_PER_MILE {
                    format!("{:.0} ft", miles * FEET_PER_MILE)
                } else {
                    format!("{miles:.0} mi")
                }
            }
        }
    }
    /// Choose feet for short scale bars and miles for larger ones.
    pub fn scale_unit_meters(self, target_m: f64) -> f64 {
        match self {
            Self::Metric => {
                if target_m >= METERS_PER_KM {
                    METERS_PER_KM
                } else {
                    1.0
                }
            }
            Self::Imperial => {
                if target_m >= KM_PER_MILE * METERS_PER_KM {
                    KM_PER_MILE * METERS_PER_KM
                } else {
                    KM_PER_MILE * METERS_PER_KM / FEET_PER_MILE
                }
            }
        }
    }
    pub fn scale_label(self, distance_m: f64) -> String {
        let divisor = self.scale_unit_meters(distance_m * (1.0 + f64::EPSILON));
        let unit = match self {
            Self::Metric => {
                if divisor == 1.0 {
                    "m"
                } else {
                    "km"
                }
            }
            Self::Imperial => {
                if divisor < 1.0 {
                    "ft"
                } else {
                    "mi"
                }
            }
        };
        format!("{} {unit}", (distance_m / divisor * 100.0).round() / 100.0)
    }
}
/// Select US-specific ruler naming independently of the UI language.
pub fn is_us_locale(locale: &str) -> bool {
    locale_region(locale).is_some_and(|region| region.as_str() == "US")
}
fn locale_region(locale: &str) -> Option<unic_langid::subtags::Region> {
    locale
        .split(['.', '@'])
        .next()
        .unwrap_or(locale)
        .replace('_', "-")
        .parse::<unic_langid::LanguageIdentifier>()
        .ok()
        .and_then(|id| id.region)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rounded_small_units_promote_at_the_display_boundary() {
        assert_eq!(DistanceUnits::Metric.distance(0.9994), "999 m");
        assert_eq!(DistanceUnits::Metric.distance(0.9996), "1.00 km");
        assert_eq!(DistanceUnits::Metric.altitude(0.9996), "1 km");
        let below = (FEET_PER_MILE - 0.6) / FEET_PER_MILE * KM_PER_MILE;
        let rounds_up = (FEET_PER_MILE - 0.4) / FEET_PER_MILE * KM_PER_MILE;
        assert_eq!(DistanceUnits::Imperial.distance(below), "5279 ft");
        assert_eq!(DistanceUnits::Imperial.distance(rounds_up), "1.00 mi");
        assert_eq!(DistanceUnits::Imperial.altitude(rounds_up), "1 mi");
    }
    #[test]
    fn region_not_ui_language_selects_default() {
        for locale in [
            "en-US",
            "es_US.UTF-8",
            "en_Latn_US",
            "en-GB",
            "cy-GB",
            "en-LR",
            "my-MM",
            "en-BZ",
            "en-BS",
            "en-VI",
            "en-GU",
            "es-PR",
            "en-KY",
            "en-GG",
            "en-JE",
            "en-IM",
        ] {
            assert_eq!(DistanceUnits::for_locale(locale), DistanceUnits::Imperial);
        }
        for locale in ["en-CA", "en-AU", "pt-BR", "fr-FR", "en", "", "C", "invalid"] {
            assert_eq!(DistanceUnits::for_locale(locale), DistanceUnits::Metric);
        }
    }
    #[test]
    fn display_conversion_preserves_geographic_values() {
        assert_eq!(DistanceUnits::Imperial.distance(KM_PER_MILE), "1.00 mi");
        assert_eq!(
            DistanceUnits::Imperial.distance(KM_PER_MILE / 2.0),
            "2640 ft"
        );
        assert_eq!(DistanceUnits::Metric.distance(1.5), "1.50 km");
        assert_eq!(DistanceUnits::Metric.altitude(14000.0), "14000 km");
        assert_eq!(
            DistanceUnits::Imperial.scale_label(KM_PER_MILE * METERS_PER_KM),
            "1 mi"
        );
    }
}
