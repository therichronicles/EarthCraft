//! Road geometry and static access restrictions from OpenStreetMap/Overpass.
//! No account/API key. OSM is NOT a source of live vehicle speeds or jams.
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
const MAX_ALTITUDE_KM: f64 = 180.0;
const REFRESH: Duration = Duration::from_secs(20 * 60);
const MOVE_RETRY: Duration = Duration::from_secs(35);
const ERROR_RETRY: Duration = Duration::from_secs(90);
const MAX_CACHE_AGE: Duration = Duration::from_secs(35 * 60);

#[derive(Clone, Debug)]
pub struct Road {
    pub name: String,
    pub kind: String,
    pub restricted: bool,
    pub points: Vec<(f64, f64)>,
}
struct RoadSnapshot {
    roads: Arc<[Road]>,
    latitude: f64,
    longitude: f64,
}

#[derive(Default)]
pub struct LiveRoads {
    current: Arc<[Road]>,
    loaded: Option<Instant>,
    attempt: Option<Instant>,
    requested: Option<(f64, f64)>,
    rx: Option<mpsc::Receiver<Result<RoadSnapshot, crate::Error>>>,
    error: Option<crate::Error>,
}
impl LiveRoads {
    pub fn snapshot(&self, lat: f64, lon: f64, alt: f64) -> &[Road] {
        if alt > MAX_ALTITUDE_KM
            || self.loaded.is_none_or(|t| t.elapsed() > MAX_CACHE_AGE)
            || !self
                .requested
                .is_some_and(|(a, b)| nearby(lat, lon, a, b, coverage_radius(alt) * 1.6))
        {
            return &[];
        }
        &self.current
    }
    pub fn shared_snapshot(&self, lat: f64, lon: f64, alt: f64) -> Option<Arc<[Road]>> {
        (!self.snapshot(lat, lon, alt).is_empty()).then(|| self.current.clone())
    }
    pub fn status(
        &self,
        alt: f64,
        lang: earthcraft_core::language::Lang,
        units: earthcraft_core::units::DistanceUnits,
    ) -> String {
        if alt > MAX_ALTITUDE_KM {
            return lang.format(
                "roads-zoom-help",
                &[("altitude", units.altitude(MAX_ALTITUDE_KM).into())],
            );
        }
        if let Some(error) = &self.error {
            return format!("OpenStreetMap/Overpass: {error}");
        }
        if self.loaded.is_none() {
            return lang.text("roads-loading-status").into();
        }
        lang.format(
            "roads-loaded-status",
            &[("count", self.current.len().into())],
        )
    }
    pub fn update(
        &mut self,
        runtime: &crate::runtime::ServiceRuntime,
        enabled: bool,
        lat: f64,
        lon: f64,
        alt: f64,
        repaint: impl Fn() + Send + Sync + 'static,
    ) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Ok(snapshot)) => {
                    self.current = snapshot.roads;
                    self.loaded = Some(Instant::now());
                    self.requested = Some((snapshot.latitude, snapshot.longitude));
                    self.error = None;
                    self.rx = None;
                }
                Ok(Err(e)) => {
                    self.error = Some(e);
                    self.rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("Road download disconnected".into());
                    self.rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if !enabled || alt > MAX_ALTITUDE_KM || lat.abs() > 84.5 {
            return;
        }
        let radius = coverage_radius(alt);
        let moved = self
            .requested
            .is_none_or(|(a, b)| !nearby(lat, lon, a, b, radius * 0.6));
        let due = self.attempt.is_none_or(|t| {
            t.elapsed()
                >= if self.error.is_some() {
                    ERROR_RETRY
                } else if moved {
                    MOVE_RETRY
                } else {
                    REFRESH
                }
        });
        if self.rx.is_none() && due {
            self.attempt = Some(Instant::now());
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            runtime.spawn(async move {
                let _ = tx.send(download(lat, lon, alt).await.map(|roads| RoadSnapshot {
                    roads,
                    latitude: lat,
                    longitude: lon,
                }));
                repaint();
            });
        }
    }
}
fn coverage_radius(alt: f64) -> f64 {
    (alt.max(0.1) / 700.0).clamp(0.015, 0.21)
}
fn nearby(lat: f64, lon: f64, a: f64, b: f64, radius: f64) -> bool {
    let dlon = (lon - b + 180.0).rem_euclid(360.0) - 180.0;
    (lat - a).abs() <= radius && dlon.abs() * lat.to_radians().cos().abs().max(0.1) <= radius
}
async fn download(lat: f64, lon: f64, alt: f64) -> Result<Arc<[Road]>, crate::Error> {
    let delta = coverage_radius(alt);
    let lat0 = (lat - delta).max(-85.0);
    let lat1 = (lat + delta).min(85.0);
    // Deliberately avoid wrapping one bbox across the 180th meridian.
    let lon0 = (lon - delta / lat.to_radians().cos().abs().max(0.2)).max(-180.0);
    let lon1 = (lon + delta / lat.to_radians().cos().abs().max(0.2)).min(180.0);
    let ways = if alt > 65.0 {
        "motorway|trunk|primary|secondary|tertiary|construction"
    } else {
        "motorway|trunk|primary|secondary|tertiary|residential|unclassified|service|living_street|motorway_link|trunk_link|primary_link|secondary_link|tertiary_link|construction"
    };
    let query = format!(
        "[out:json][timeout:23];way[\"highway\"~\"^({ways})$\"]({lat0:.5},{lon0:.5},{lat1:.5},{lon1:.5});out geom 4500;"
    );
    let client = crate::http::client(crate::http::Policy::Boundary)?;
    let mut last = None;
    for url in [
        "https://overpass.kumi.systems/api/interpreter",
        "https://overpass-api.de/api/interpreter",
    ] {
        match crate::http::body(
            client.post(url).form(&[("data", query.as_str())]),
            9_000_000,
        )
        .await
        {
            Ok(bytes) => match crate::http::parse_json(bytes).await {
                Ok(json) if json["elements"].is_array() => {
                    return crate::cpu::run(crate::cpu::Service::Features, move || {
                        Ok(parse_overpass(&json).into())
                    })
                    .await;
                }
                Ok(_) => last = Some("Unexpected Overpass response".into()),
                Err(e) => last = Some(e),
            },
            Err(e) => last = Some(e),
        }
    }
    Err(last.expect("at least one server"))
}
fn parse_overpass(json: &serde_json::Value) -> Vec<Road> {
    json["elements"]
        .as_array()
        .into_iter()
        .flatten()
        .take(4500)
        .filter_map(|item| {
            if item["type"].as_str() != Some("way") {
                return None;
            }
            let points: Vec<_> = item["geometry"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    let lat = p["lat"].as_f64()?;
                    let lon = p["lon"].as_f64()?;
                    (lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0)
                        .then_some((lat, lon))
                })
                .take(400)
                .collect();
            if points.len() < 2 {
                return None;
            }
            let tags = &item["tags"];
            let kind = tags["highway"].as_str().unwrap_or("road").to_owned();
            let restricted = kind == "construction"
                || tags["access"].as_str() == Some("no")
                || tags["motor_vehicle"].as_str() == Some("no");
            let name = tags["name"]
                .as_str()
                .or_else(|| tags["ref"].as_str())
                .unwrap_or("")
                .chars()
                .take(100)
                .collect();
            Some(Road {
                name,
                kind,
                restricted,
                points,
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_named_and_restricted_roads_with_real_geometry() {
        let input = serde_json::json!({"elements":[{"type":"way","tags":{"name":"BR-381","highway":"trunk"},
            "geometry":[{"lat":-19.4,"lon":-42.5},{"lat":-19.5,"lon":-42.6}]},
            {"type":"way","tags":{"highway":"construction"},"geometry":[{"lat":1.0,"lon":2.0},{"lat":1.1,"lon":2.1}]}]});
        let roads = parse_overpass(&input);
        assert_eq!(roads.len(), 2);
        assert_eq!(roads[0].name, "BR-381");
        assert!(!roads[0].restricted);
        assert!(roads[1].restricted);
    }
    #[test]
    fn antimeridian_distance_uses_wrapped_longitude() {
        assert!(nearby(0.0, 179.99, 0.0, -179.99, 0.04));
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;
    use earthcraft_core::{language::Lang, units::DistanceUnits};
    #[test]
    fn zoom_hint_follows_metric_and_imperial_units_in_both_languages() {
        let roads = LiveRoads::default();
        for lang in Lang::ALL {
            let metric = roads.status(MAX_ALTITUDE_KM + 1.0, lang, DistanceUnits::Metric);
            let imperial = roads.status(MAX_ALTITUDE_KM + 1.0, lang, DistanceUnits::Imperial);
            assert!(metric.contains("180 km"), "{metric}");
            assert!(imperial.contains("112 mi"), "{imperial}");
            assert!(!imperial.contains("km"));
            assert_eq!(
                roads.status(MAX_ALTITUDE_KM, lang, DistanceUnits::Imperial),
                lang.text("roads-loading-status")
            );
        }
    }
    #[test]
    fn immutable_snapshots_are_shared_and_outlive_refresh_without_expanding_coverage() {
        let road = Road {
            name: "Old".into(),
            kind: "primary".into(),
            restricted: false,
            points: vec![(0.0, 0.0), (0.1, 0.1)],
        };
        let mut roads = LiveRoads {
            current: vec![road].into(),
            loaded: Some(Instant::now()),
            requested: Some((0.0, 0.0)),
            ..Default::default()
        };
        let old = roads.shared_snapshot(0.0, 0.0, 1.0).unwrap();
        assert!(Arc::ptr_eq(
            &old,
            &roads.shared_snapshot(0.0, 0.0, 1.0).unwrap()
        ));
        roads.current = Arc::from([]);
        assert_eq!(old[0].name, "Old");
        assert!(roads.shared_snapshot(0.0, 0.0, 1.0).is_none());
        assert!(
            roads
                .shared_snapshot(0.0, 0.0, MAX_ALTITUDE_KM + 1.0)
                .is_none()
        );
    }
}
