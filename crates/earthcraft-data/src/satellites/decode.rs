//! CelesTrak OMM fields and UTC epoch decoding.
use super::Satellite;
// Satellite epochs use four-digit Gregorian years. Bound arithmetic before conversion.
const MIN_EPOCH_YEAR: i64 = 1;
const MAX_EPOCH_YEAR: i64 = 9999;
fn num(v: &serde_json::Value, k: &str) -> Option<f64> {
    v.get(k)?
        .as_f64()
        .or_else(|| v[k].as_str()?.parse().ok())
        .filter(|n: &f64| n.is_finite())
}
// Days since Unix epoch via proleptic Gregorian calendar (timezone is always UTC).
fn unix_day(y: i64, m: i64, d: i64) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
fn parse_epoch(s: &str) -> Option<f64> {
    let (date, time) = s.split_once('T')?;
    let mut date = date.split('-');
    let y: i64 = date.next()?.parse().ok()?;
    if !(MIN_EPOCH_YEAR..=MAX_EPOCH_YEAR).contains(&y) {
        return None;
    }
    let m: i64 = date.next()?.parse().ok()?;
    let d: i64 = date.next()?.parse().ok()?;
    let mut time = time.trim_end_matches('Z').split(':');
    let hour: f64 = time.next()?.parse().ok()?;
    let minute: f64 = time.next()?.parse().ok()?;
    let second: f64 = time.next()?.parse().ok()?;
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=days).contains(&d)
        || !(0.0..24.0).contains(&hour)
        || !(0.0..60.0).contains(&minute)
        || !(0.0..=60.0).contains(&second)
    {
        return None;
    }
    Some(unix_day(y, m, d) as f64 * 86400.0 + hour * 3600.0 + minute * 60.0 + second)
}
pub(super) fn parse_omm(value: &serde_json::Value, now_unix_secs: f64) -> Vec<Satellite> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let s = Satellite {
                name: v["OBJECT_NAME"].as_str()?.to_owned(),
                norad: v["NORAD_CAT_ID"].to_string().trim_matches('"').to_owned(),
                orbit: earthcraft_core::orbit::OrbitalElements {
                    inclination_deg: num(v, "INCLINATION")?,
                    raan_deg: num(v, "RA_OF_ASC_NODE")?,
                    eccentricity: num(v, "ECCENTRICITY")?,
                    argument_perigee_deg: num(v, "ARG_OF_PERICENTER")?,
                    mean_anomaly_deg: num(v, "MEAN_ANOMALY")?,
                    mean_motion_rev_per_day: num(v, "MEAN_MOTION")?,
                    epoch_unix_secs: parse_epoch(v["EPOCH"].as_str()?)?,
                },
            };
            if s.orbit.mean_motion_rev_per_day > 0.0 && s.ground_position(now_unix_secs).is_some() {
                Some(s)
            } else {
                None
            }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn epoch_reference_and_polar_orbit() {
        assert_eq!(parse_epoch("1970-01-01T00:00:00.000Z"), Some(0.0));
        let sat = Satellite {
            name: "test".into(),
            norad: "1".into(),
            orbit: earthcraft_core::orbit::OrbitalElements {
                inclination_deg: 90.0,
                raan_deg: 0.0,
                eccentricity: 0.001,
                argument_perigee_deg: 0.0,
                mean_anomaly_deg: 0.0,
                mean_motion_rev_per_day: 15.0,
                epoch_unix_secs: 1_700_000_000.0,
            },
        };
        let p = sat.ground_position(sat.orbit.epoch_unix_secs).unwrap();
        assert!(p.0.abs() < 0.001);
        assert!(
            sat.ground_position(sat.orbit.epoch_unix_secs + 86400.0 * 15.0)
                .is_none()
        );
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    #[test]
    fn extreme_epoch_years_are_rejected_before_calendar_arithmetic() {
        for year in [i64::MIN, -1, 0, 10_000, i64::MAX / 2, i64::MAX] {
            assert!(parse_epoch(&format!("{year}-01-01T00:00:00Z")).is_none());
        }
        for epoch in ["0001-01-01T00:00:00Z", "9999-12-31T23:59:59Z"] {
            assert!(parse_epoch(epoch).is_some());
        }
    }
    #[test]
    fn utc_epoch_rejects_invalid_calendar_dates_and_nonfinite_fields() {
        assert!(parse_epoch("2024-02-29T12:00:00.000Z").is_some());
        for epoch in [
            "2023-02-29T12:00:00Z",
            "2024-13-01T00:00:00Z",
            "2024-01-00T00:00:00Z",
            "2024-01-01T24:00:00Z",
            "2024-01-01T00:60:00Z",
            "2024-01-01T00:00:NaNZ",
        ] {
            assert!(parse_epoch(epoch).is_none(), "{epoch}");
        }
    }
    #[test]
    fn omm_snapshot_uses_the_injected_observation_clock_and_retains_provider_ids() {
        let json = serde_json::json!([{"OBJECT_NAME":"TEST","NORAD_CAT_ID":42,"INCLINATION":"90.0","RA_OF_ASC_NODE":0.0,"ECCENTRICITY":0.001,"ARG_OF_PERICENTER":0.0,"MEAN_ANOMALY":0.0,"MEAN_MOTION":15.0,"EPOCH":"2023-11-14T22:13:20.000Z"}]);
        let satellites = parse_omm(&json, 1_700_000_000.0);
        assert_eq!(satellites.len(), 1);
        assert_eq!(satellites[0].norad, "42");
        assert_eq!(satellites[0].orbit.epoch_unix_secs, 1_700_000_000.0);
        assert!(parse_omm(&json, 1_700_000_000.0 + 15.0 * 86400.0).is_empty());
    }
}
