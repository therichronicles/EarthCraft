use crate::EARTH_RADIUS_KM;
use crate::METERS_PER_KM;
use glam::DVec3;
use std::time::{SystemTime, UNIX_EPOCH};
const ANTIPODAL_CROSS_EPSILON: f64 = 16.0 * f64::EPSILON;
const SECONDS_PER_DAY: f64 = 86400.0;
const UNIX_EPOCH_TO_J2000_DAYS: f64 = 10957.5;
// Coefficients of the approximate solar-position model, in degrees and days.
const MEAN_LONGITUDE_AT_J2000_DEG: f64 = 280.460;
const MEAN_LONGITUDE_RATE_DEG_PER_DAY: f64 = 0.9856474;
const MEAN_ANOMALY_AT_J2000_DEG: f64 = 357.528;
const MEAN_ANOMALY_RATE_DEG_PER_DAY: f64 = 0.9856003;
const LONGITUDE_FIRST_HARMONIC_DEG: f64 = 1.915;
const LONGITUDE_SECOND_HARMONIC_DEG: f64 = 0.020;
const OBLIQUITY_AT_J2000_DEG: f64 = 23.439;
const OBLIQUITY_RATE_DEG_PER_DAY: f64 = 0.0000004;
const SIDEREAL_ANGLE_AT_J2000_DEG: f64 = 280.46061837;
const SIDEREAL_RATE_DEG_PER_DAY: f64 = 360.98564736629;

// ───────────────────────────── Math ─────────────────────────────

pub fn ll_to_v3(lat: f64, lon: f64) -> DVec3 {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    DVec3::new(la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin())
}
pub fn v3_to_ll(p: DVec3) -> (f64, f64) {
    (
        p.z.clamp(-1.0, 1.0).asin().to_degrees(),
        p.y.atan2(p.x).to_degrees(),
    )
}
pub fn wrap_lon(l: f64) -> f64 {
    (l + 180.0).rem_euclid(360.0) - 180.0
}
pub fn haversine_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, la2) = (a.0.to_radians(), b.0.to_radians());
    let dlat = la2 - la1;
    let dlon = (b.1 - a.1).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlon / 2.0).sin().powi(2);
    // Haversine preserves short baselines, but asin(sqrt(h)) loses the
    // complement of the angle as h approaches one. Use vector atan2 there.
    if h > 0.5 {
        let (pa, pb) = (ll_to_v3(a.0, a.1), ll_to_v3(b.0, b.1));
        return EARTH_RADIUS_KM * pa.cross(pb).length().atan2(pa.dot(pb));
    }
    2.0 * EARTH_RADIUS_KM * h.sqrt().min(1.0).asin()
}
pub fn great_circle(a: (f64, f64), b: (f64, f64), n: usize) -> Vec<DVec3> {
    let (pa, pb) = (ll_to_v3(a.0, a.1), ll_to_v3(b.0, b.1));
    if n == 0 {
        return vec![pa];
    }
    let cross = pa.cross(pb);
    let sine = cross.length();
    let cosine = pa.dot(pb);
    let angle = sine.atan2(cosine);
    // Exact antipodes have no unique shortest arc. Choose a deterministic plane;
    // otherwise the cross product defines the arc, including tiny baselines.
    let tangent = if sine <= ANTIPODAL_CROSS_EPSILON && cosine < 0.0 {
        pa.any_orthonormal_vector()
    } else if sine > 0.0 {
        (cross / sine).cross(pa)
    } else {
        return vec![pa; n + 1];
    };
    (0..=n)
        .map(|i| {
            if i == 0 {
                pa
            } else if i == n {
                pb
            } else {
                let (sine, cosine) = (angle * i as f64 / n as f64).sin_cos();
                pa * cosine + tangent * sine
            }
        })
        .collect()
}
pub fn fmt_dist(km: f64) -> String {
    if km < 1.0 {
        format!("{:.0} m", km * METERS_PER_KM)
    } else {
        format!("{:.2} km", km)
    }
}
pub fn fmt_alt(km: f64) -> String {
    if km < 1.0 {
        format!("{:.0} m", km * METERS_PER_KM)
    } else {
        format!("{:.0} km", km)
    }
}

/// Approximate subsolar point (lat, lon) for the given Unix time.
pub fn subsolar(unix: f64) -> (f64, f64) {
    let n = unix / SECONDS_PER_DAY - UNIX_EPOCH_TO_J2000_DAYS; // Days since J2000 / dias desde J2000
    let l = (MEAN_LONGITUDE_AT_J2000_DEG + MEAN_LONGITUDE_RATE_DEG_PER_DAY * n).rem_euclid(360.0);
    let g = (MEAN_ANOMALY_AT_J2000_DEG + MEAN_ANOMALY_RATE_DEG_PER_DAY * n)
        .rem_euclid(360.0)
        .to_radians();
    let lam = (l
        + LONGITUDE_FIRST_HARMONIC_DEG * g.sin()
        + LONGITUDE_SECOND_HARMONIC_DEG * (2.0 * g).sin())
    .to_radians();
    let eps = (OBLIQUITY_AT_J2000_DEG - OBLIQUITY_RATE_DEG_PER_DAY * n).to_radians();
    let dec = (eps.sin() * lam.sin()).asin();
    let ra = (eps.cos() * lam.sin()).atan2(lam.cos()).to_degrees();
    let gmst = (SIDEREAL_ANGLE_AT_J2000_DEG + SIDEREAL_RATE_DEG_PER_DAY * n).rem_euclid(360.0);
    (dec.to_degrees(), wrap_lon(ra - gmst))
}
pub fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Destination on the current spherical Earth model, in degrees and kilometers.
#[inline]
pub fn destination(lat_deg: f64, lon_deg: f64, heading_deg: f64, km: f64) -> (f64, f64) {
    let delta = km / EARTH_RADIUS_KM;
    let lat = lat_deg.to_radians();
    let lon = lon_deg.to_radians();
    let heading = heading_deg.to_radians();
    let next_lat = (lat.sin() * delta.cos() + lat.cos() * delta.sin() * heading.cos())
        .clamp(-1.0, 1.0)
        .asin();
    let next_lon = lon
        + (heading.sin() * delta.sin() * lat.cos()).atan2(delta.cos() - lat.sin() * next_lat.sin());
    (
        next_lat.to_degrees(),
        (next_lon.to_degrees() + 180.0).rem_euclid(360.0) - 180.0,
    )
}

#[cfg(test)]
mod destination_tests {
    use super::*;
    #[test]
    fn destination_keeps_distance_and_coordinate_bounds_at_poles_and_dateline() {
        for (lat, lon) in [(0.0, 179.9), (89.9, -150.0), (-90.0, 0.0)] {
            for heading in [0.0, 90.0, 180.0, 270.0] {
                let next = destination(lat, lon, heading, 100.0);
                assert!(crate::coordinates::validate_coordinates(next.0, next.1).is_ok());
                assert!((haversine_km((lat, lon), next) - 100.0).abs() < 0.000001);
            }
        }
    }
}
