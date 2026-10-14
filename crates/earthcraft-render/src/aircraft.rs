//! Project actual flight telemetry on the same globe as the base imagery.
use earthcraft_core::{EARTH_RADIUS_KM, camera::Basis, math::ll_to_v3};
use earthcraft_data::aircraft::Aircraft;
use emath::{Pos2, Rect, Vec2};

const ICON_SIZE_POINTS: f32 = 7.0;
const TRAIL_SIZE_POINTS: f32 = 12.0;
const WING_WIDTH_RATIO: f32 = 0.4;
const EXTRAPOLATION_MAX_SECS: f64 = 25.0;

pub struct ProjectedFlight {
    pub code: String,
    pub hex: String,
    pub speed_knots: Option<f64>,
    pub altitude_feet: Option<f64>,
    pub age_secs: f64,
    pub heading_deg: f64,
    pub position: Pos2,
    pub icon: [Pos2; 4],
    pub trail: [Pos2; 2],
}

pub fn project_flights(
    basis: &Basis,
    rect: Rect,
    aircraft: &[Aircraft],
    unix_secs: f64,
    map_scale: f32,
) -> Vec<ProjectedFlight> {
    aircraft
        .iter()
        .filter_map(|flight| {
            let age = (unix_secs - flight.observed_unix).max(0.0);
            // Never invent an entire route between remote updates. Small extrapolation
            // is allowed for recent transponder reports, then the marker freezes.
            let speed_km_s = flight.speed_knots.unwrap_or(0.0).clamp(0.0, 1200.0) * 1.852 / 3600.0;
            let displacement = speed_km_s * age.min(EXTRAPOLATION_MAX_SECS);
            let (lat, lon) = destination(
                flight.latitude,
                flight.longitude,
                flight.heading_deg,
                displacement,
            );
            let screen = basis.project(ll_to_v3(lat, lon), rect)?;
            if !rect.contains(screen) {
                return None;
            }
            // Project a small step along the actual aircraft track to orient its icon
            // correctly even when the camera is rotated or near a pole.
            let (ahead_lat, ahead_lon) = destination(lat, lon, flight.heading_deg, 3.0);
            let heading = basis
                .project(ll_to_v3(ahead_lat, ahead_lon), rect)
                .map(|ahead| (ahead.y - screen.y).atan2(ahead.x - screen.x))
                .unwrap_or(-std::f32::consts::FRAC_PI_2);
            let forward = Vec2::angled(heading) * (ICON_SIZE_POINTS * map_scale);
            let wing = Vec2::new(-forward.y, forward.x) * WING_WIDTH_RATIO;
            let tail = Vec2::angled(heading) * (TRAIL_SIZE_POINTS * map_scale);
            Some(ProjectedFlight {
                code: if flight.callsign.is_empty() {
                    flight.hex.to_ascii_uppercase()
                } else {
                    flight.callsign.clone()
                },
                hex: flight.hex.clone(),
                speed_knots: flight.speed_knots,
                altitude_feet: flight.altitude_feet,
                age_secs: age,
                heading_deg: flight.heading_deg,
                position: screen,
                icon: [
                    screen + forward,
                    screen + wing - forward * 0.25,
                    screen - forward * 0.85,
                    screen - wing - forward * 0.25,
                ],
                trail: [screen - tail * 0.5, screen - tail * 1.1],
            })
        })
        .collect()
}

fn destination(lat_deg: f64, lon_deg: f64, heading_deg: f64, km: f64) -> (f64, f64) {
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
mod tests {
    use super::*;
    #[test]
    fn eastbound_plane_moves_east_and_westbound_wraps_dateline() {
        let (lat, lon) = destination(0.0, 0.0, 90.0, 110.0);
        assert!(lat.abs() < 0.01);
        assert!(lon > 0.9 && lon < 1.1);
        let (_, lon) = destination(0.0, 179.9, 90.0, 50.0);
        assert!(lon < -179.0);
    }
}
