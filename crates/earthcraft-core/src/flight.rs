//! Camera flight interpolation, independent of UI timing and input.
use crate::{
    MAX_CAMERA_ALTITUDE_KM, MAX_CAMERA_LATITUDE_DEG, MIN_CAMERA_ALTITUDE_KM, camera::Cam,
    earth_model::EarthModel, math::wrap_lon,
};
use std::f64::consts::PI;
const FLY_ARC_ALTITUDE_RATIO: f64 = 0.7;
const FLY_DISTANCE_SCALE_KM: f64 = 8000.0;
const FLY_DURATION_SCALE_SECS: f64 = 2.0;
const MIN_FLY_DURATION_SECS: f64 = 1.0;
const MAX_FLY_DURATION_SECS: f64 = 3.5;

pub struct Flight {
    a: Cam,
    b: Cam,
    t: f64,
    dur: f64,
    bump: f64,
    model: EarthModel,
    distance_km: f64,
    bearing_deg: f64,
}
impl Flight {
    pub fn new(from: Cam, lat: f64, lon: f64, alt: f64) -> Self {
        Self::new_for_model(from, lat, lon, alt, EarthModel::PerfectSphere)
    }
    pub fn new_for_model(from: Cam, lat: f64, lon: f64, alt: f64, model: EarthModel) -> Self {
        let a = from;
        let b = Cam {
            lat: lat.clamp(-MAX_CAMERA_LATITUDE_DEG, MAX_CAMERA_LATITUDE_DEG),
            lon: wrap_lon(lon),
            heading: 0.0,
            alt: alt.clamp(MIN_CAMERA_ALTITUDE_KM, MAX_CAMERA_ALTITUDE_KM),
        };
        let dist = model.distance_km((a.lat, a.lon), (b.lat, b.lon));
        let bump = if dist * FLY_ARC_ALTITUDE_RATIO > a.alt.max(b.alt) {
            (dist * FLY_ARC_ALTITUDE_RATIO)
                .min(MAX_CAMERA_ALTITUDE_KM)
                .ln()
                - (a.alt.ln() + b.alt.ln()) / 2.0
        } else {
            0.0
        };
        let dur = (MIN_FLY_DURATION_SECS + dist / FLY_DISTANCE_SCALE_KM * FLY_DURATION_SCALE_SECS)
            .clamp(MIN_FLY_DURATION_SECS, MAX_FLY_DURATION_SECS);
        Self {
            a,
            b,
            t: 0.0,
            dur,
            bump,
            model,
            distance_km: dist,
            bearing_deg: model.route_bearing((a.lat, a.lon), (b.lat, b.lon)),
        }
    }

    pub fn advance(&mut self, dt: f64) -> Cam {
        self.t += dt / self.dur;
        let t = self.t.min(1.0);
        let s = t * t * (3.0 - 2.0 * t);
        let (a, b) = (self.a, self.b);
        let (la, lb) = (a.alt.ln(), b.alt.ln());
        if self.is_finished() {
            return b;
        }
        let (lat, lon) = if self.model == EarthModel::PerfectSphere {
            (
                a.lat + (b.lat - a.lat) * s,
                wrap_lon(a.lon + wrap_lon(b.lon - a.lon) * s),
            )
        } else {
            self.model
                .destination((a.lat, a.lon), self.bearing_deg, self.distance_km * s)
        };
        Cam {
            lat,
            lon,
            heading: a.heading + wrap_lon(b.heading - a.heading) * s,
            alt: (la + (lb - la) * s + (PI * t).sin() * self.bump).exp(),
        }
    }
    pub fn is_finished(&self) -> bool {
        self.t >= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ellipsoid_flight_follows_its_geodesic_and_finishes_exactly() {
        let model = EarthModel::Wgs84;
        for (start, end) in [
            ((45.0, 179.0), (-20.0, -170.0)),
            ((90.0, 30.0), (50.0, -100.0)),
        ] {
            let from = Cam {
                lat: start.0,
                lon: start.1,
                heading: 30.0,
                alt: 500.0,
            };
            let mut flight = Flight::new_for_model(from, end.0, end.1, 400.0, model);
            let middle = flight.advance(flight.dur * 0.5);
            let half_distance = model.distance_km(start, (middle.lat, middle.lon));
            assert!((half_distance - model.distance_km(start, end) * 0.5).abs() < 1e-8);
            let result = flight.advance(flight.dur);
            assert!(flight.is_finished());
            assert_eq!((result.lat, result.lon, result.alt), (end.0, end.1, 400.0));
        }
    }
    #[test]
    fn flight_crosses_dateline_by_short_path_and_finishes_at_clamped_destination() {
        let from = Cam {
            lat: 0.0,
            lon: 179.0,
            heading: 0.0,
            alt: 500.0,
        };
        let mut flight = Flight::new(from, 0.0, -179.0, 500.0);
        let halfway = flight.advance(flight.dur / 2.0);
        assert!((halfway.lon.abs() - 180.0).abs() < 1e-9);
        assert!(!flight.is_finished());
        let end = flight.advance(10.0);
        assert_eq!(end.lon, -179.0);
        assert_eq!(end.alt, 500.0);
        assert!(flight.is_finished());
        let mut bounds = Flight::new(from, 100.0, 540.0, 0.0);
        let end = bounds.advance(100.0);
        assert_eq!(end.lat, MAX_CAMERA_LATITUDE_DEG);
        assert_eq!(end.lon, -180.0);
        assert_eq!(end.alt, MIN_CAMERA_ALTITUDE_KM);
    }
}
