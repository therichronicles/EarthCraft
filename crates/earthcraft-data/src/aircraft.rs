//! Real aircraft snapshots from OpenSky (worldwide) and ADSB.lol (near the camera).
//! Both providers are independent, best-effort services; no fabricated flights.
use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const OPENSKY_STATES_URL: &str = "https://opensky-network.org/api/states/all";
const ADSB_LOCAL_RADIUS_NM: u32 = 250;
// OpenSky anonymous worldwide queries cost four credits; 17 min stays under
// the published 400-credits/day budget for a continuously running instance.
const GLOBAL_REFRESH: Duration = Duration::from_secs(17 * 60);
const LOCAL_REFRESH: Duration = Duration::from_secs(90);
const FAILURE_RETRY: Duration = Duration::from_secs(3 * 60);
const MOVE_RETRY_MIN: Duration = Duration::from_secs(30);
const LOCAL_MAX_CAMERA_ALT_KM: f64 = 1800.0;
const GLOBAL_MAX_AGE_SECS: f64 = 20.0 * 60.0;
const LOCAL_MAX_AGE_SECS: f64 = 3.0 * 60.0;
const MAX_POSITION_AGE_SECS: f64 = 120.0;
const GLOBAL_RESPONSE_LIMIT: u64 = 20 * 1024 * 1024;
const LOCAL_RESPONSE_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Aircraft {
    pub hex: String,
    pub callsign: String,
    pub latitude: f64,
    pub longitude: f64,
    pub heading_deg: f64,
    pub speed_knots: Option<f64>,
    pub altitude_feet: Option<f64>,
    pub observed_unix: f64,
}

#[derive(Default)]
pub struct LiveAircraft {
    global: Vec<Aircraft>,
    local: Vec<Aircraft>,
    global_rx: Option<mpsc::Receiver<Result<Vec<Aircraft>, crate::Error>>>,
    local_rx: Option<mpsc::Receiver<Result<Vec<Aircraft>, crate::Error>>>,
    global_attempt: Option<Instant>,
    local_attempt: Option<Instant>,
    global_loaded: Option<Instant>,
    local_loaded: Option<Instant>,
    local_center: Option<(f64, f64)>,
    global_error: Option<String>,
    local_error: Option<String>,
}

impl LiveAircraft {
    pub fn update(
        &mut self,
        runtime: &crate::runtime::ServiceRuntime,
        enabled: bool,
        center_lat: f64,
        center_lon: f64,
        camera_alt_km: f64,
        repaint: impl Fn() + Send + Sync + 'static,
    ) {
        let repaint = std::sync::Arc::new(repaint);
        if let Some(rx) = &self.global_rx {
            match rx.try_recv() {
                Ok(Ok(planes)) => {
                    self.global = planes;
                    self.global_loaded = Some(Instant::now());
                    self.global_error = None;
                    self.global_rx = None;
                }
                Ok(Err(e)) => {
                    self.global_error = Some(e.to_string());
                    self.global_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.global_error = Some("OpenSky request interrupted".into());
                    self.global_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &self.local_rx {
            match rx.try_recv() {
                Ok(Ok(planes)) => {
                    self.local = planes;
                    self.local_loaded = Some(Instant::now());
                    self.local_error = None;
                    self.local_rx = None;
                }
                Ok(Err(e)) => {
                    self.local_error = Some(e.to_string());
                    self.local_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.local_error = Some("ADSB.lol request interrupted".into());
                    self.local_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if !enabled {
            return;
        }

        if self.global_rx.is_none()
            && due(
                self.global_attempt,
                self.global_error.is_none(),
                GLOBAL_REFRESH,
            )
        {
            let (tx, rx) = mpsc::channel();
            self.global_rx = Some(rx);
            self.global_attempt = Some(Instant::now());
            let wake = repaint.clone();
            runtime.spawn(async move {
                let result = download_global().await;
                let _ = tx.send(result);
                wake();
            });
        }

        // ADSB.lol's public proximity endpoint is limited to a 250nm radius.
        // Use it only when zoomed in, rather than hammering the service for the globe.
        if camera_alt_km <= LOCAL_MAX_CAMERA_ALT_KM && self.local_rx.is_none() {
            let moved = self.local_center.is_some_and(|center| {
                geographic_distance_km(center, (center_lat, center_lon)) > 140.0
            });
            let refresh = due(
                self.local_attempt,
                self.local_error.is_none(),
                LOCAL_REFRESH,
            );
            let movement_ready = moved
                && self
                    .local_attempt
                    .is_none_or(|t| t.elapsed() >= MOVE_RETRY_MIN);
            if refresh || movement_ready {
                let (tx, rx) = mpsc::channel();
                self.local_rx = Some(rx);
                self.local_attempt = Some(Instant::now());
                self.local_center = Some((center_lat, center_lon));
                let wake = repaint.clone();
                runtime.spawn(async move {
                    let result = download_local(center_lat, center_lon).await;
                    let _ = tx.send(result);
                    wake();
                });
            }
        }
    }

    /// Current non-expired provider samples, deduplicated by ICAO hex code.
    /// Nearby ADS-B reports override older OpenSky world samples.
    pub fn snapshot(&self) -> Vec<Aircraft> {
        let now = now_unix();
        let mut by_hex = HashMap::new();
        if self
            .global_loaded
            .is_some_and(|t| t.elapsed().as_secs_f64() <= GLOBAL_MAX_AGE_SECS)
        {
            for flight in &self.global {
                by_hex.insert(flight.hex.clone(), flight.clone());
            }
        }
        if self
            .local_loaded
            .is_some_and(|t| t.elapsed().as_secs_f64() <= LOCAL_MAX_AGE_SECS)
        {
            for flight in &self.local {
                if now - flight.observed_unix <= MAX_POSITION_AGE_SECS {
                    let replace = by_hex
                        .get(&flight.hex)
                        .is_none_or(|old: &Aircraft| flight.observed_unix >= old.observed_unix);
                    if replace {
                        by_hex.insert(flight.hex.clone(), flight.clone());
                    }
                }
            }
        }
        let mut flights: Vec<Aircraft> = by_hex.into_values().collect();
        flights.sort_unstable_by(|a, b| a.hex.cmp(&b.hex));
        flights
    }

    pub fn status(&self, pt: bool) -> String {
        let count = self.snapshot().len();
        let latest = self.global_loaded.map(|t| t.elapsed().as_secs() / 60);
        let local = self.local_loaded.map(|t| t.elapsed().as_secs() / 60);
        let mut msg = if pt {
            format!(
                "Aviões reais rastreados: {count} | OpenSky: {} | ADSB.lol: {}",
                latest.map_or("carregando".into(), |m| format!("há {m} min")),
                local.map_or(
                    if self.local_attempt.is_some() {
                        "carregando".into()
                    } else {
                        "aproxime o mapa".into()
                    },
                    |m| format!("há {m} min")
                )
            )
        } else {
            format!(
                "Real tracked aircraft: {count} | OpenSky: {} | ADSB.lol: {}",
                latest.map_or("loading".into(), |m| format!("{m} min ago")),
                local.map_or(
                    if self.local_attempt.is_some() {
                        "loading".into()
                    } else {
                        "zoom in".into()
                    },
                    |m| format!("{m} min ago")
                )
            )
        };
        if let Some(err) = &self.global_error {
            msg.push_str(&format!("\nOpenSky: {err}"));
        }
        if let Some(err) = &self.local_error {
            msg.push_str(&format!("\nADSB.lol: {err}"));
        }
        msg
    }
}

fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
fn due(last: Option<Instant>, success: bool, interval: Duration) -> bool {
    last.is_none_or(|time| time.elapsed() >= if success { interval } else { FAILURE_RETRY })
}
fn geographic_distance_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dlat = (b.0 - a.0).to_radians();
    let dlon = (b.1 - a.1).to_radians();
    let h = (dlat / 2.0).sin().powi(2)
        + a.0.to_radians().cos() * b.0.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    12_742.0 * h.clamp(0.0, 1.0).sqrt().asin()
}
fn valid_coords(lat: f64, lon: f64) -> bool {
    lat.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && lon.is_finite()
        && (-180.0..=180.0).contains(&lon)
}
fn number(v: Option<&serde_json::Value>) -> Option<f64> {
    v.and_then(|v| v.as_f64()).filter(|v| v.is_finite())
}
fn safe_heading(v: Option<f64>) -> f64 {
    v.unwrap_or(0.0).rem_euclid(360.0)
}

async fn download_global() -> Result<Vec<Aircraft>, crate::Error> {
    let client = crate::http::client(crate::http::Policy::Aircraft)?;
    let bytes = crate::http::body(client.get(OPENSKY_STATES_URL), GLOBAL_RESPONSE_LIMIT).await?;
    tokio::task::spawn_blocking(move || -> Result<_, crate::Error> {
        let json: serde_json::Value = serde_json::from_slice(&bytes)?;
        if !json.get("states").is_some_and(|states| states.is_array()) {
            return Err("OpenSky response lacks state vectors".into());
        }
        Ok(parse_opensky(&json, now_unix()))
    })
    .await?
}
async fn download_local(lat: f64, lon: f64) -> Result<Vec<Aircraft>, crate::Error> {
    let client = crate::http::client(crate::http::Policy::Aircraft)?;
    let url =
        format!("https://api.adsb.lol/v2/lat/{lat:.4}/lon/{lon:.4}/dist/{ADSB_LOCAL_RADIUS_NM}");
    let bytes = crate::http::body(client.get(url), LOCAL_RESPONSE_LIMIT).await?;
    tokio::task::spawn_blocking(move || -> Result<_, crate::Error> {
        let json: serde_json::Value = serde_json::from_slice(&bytes)?;
        if !json.get("ac").is_some_and(|aircraft| aircraft.is_array()) {
            return Err("ADSB.lol response lacks aircraft".into());
        }
        Ok(parse_adsb_lol(&json, now_unix()))
    })
    .await?
}

fn parse_opensky(json: &serde_json::Value, now: f64) -> Vec<Aircraft> {
    json.get("states")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let f = v.as_array()?;
            let hex = f.first()?.as_str()?.trim().to_ascii_lowercase();
            if hex.is_empty() || f.get(8)?.as_bool().unwrap_or(false) {
                return None;
            }
            let lat = number(f.get(6))?;
            let lon = number(f.get(5))?;
            let observed = number(f.get(3)).or_else(|| number(f.get(4)))?;
            if !valid_coords(lat, lon) || !(0.0..=MAX_POSITION_AGE_SECS).contains(&(now - observed))
            {
                return None;
            }
            let callsign = f
                .get(1)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_owned();
            let altitude_feet = number(f.get(7))
                .or_else(|| number(f.get(13)))
                .map(|m| m * 3.28084);
            let speed_knots = number(f.get(9)).map(|mps| mps * 1.94384449);
            Some(Aircraft {
                hex,
                callsign,
                latitude: lat,
                longitude: lon,
                heading_deg: safe_heading(number(f.get(10))),
                speed_knots,
                altitude_feet,
                observed_unix: observed,
            })
        })
        .collect()
}
fn parse_adsb_lol(json: &serde_json::Value, now: f64) -> Vec<Aircraft> {
    json.get("ac")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let hex = v.get("hex")?.as_str()?.trim().to_ascii_lowercase();
            if hex.is_empty() || v.get("alt_baro").and_then(|v| v.as_str()) == Some("ground") {
                return None;
            }
            let lat = number(v.get("lat"))?;
            let lon = number(v.get("lon"))?;
            let age = number(v.get("seen_pos"))?;
            if !valid_coords(lat, lon) || !(0.0..=MAX_POSITION_AGE_SECS).contains(&age) {
                return None;
            }
            Some(Aircraft {
                hex,
                callsign: v
                    .get("flight")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_owned(),
                latitude: lat,
                longitude: lon,
                heading_deg: safe_heading(number(v.get("track"))),
                speed_knots: number(v.get("gs")),
                altitude_feet: number(v.get("alt_baro")).or_else(|| number(v.get("alt_geom"))),
                observed_unix: now - age,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_opensky_positions_not_ground_vehicles_or_invalid_locations() {
        let now = 1000.0;
        let data = serde_json::json!({"states": [
            ["e490aa", "AZU123 ", "Brazil", 990, 995, -43.2, -19.7, 10000.0, false, 250.0, 45.0, null, null, null],
            ["x", "GROUND", "Brazil", 990, 995, -43.2, -19.7, 0, true, 0, 0],
            ["z", "OLD", "Brazil", 800, 800, -43.2, -19.7, 1000, false, 100, 0]
        ]});
        let flights = parse_opensky(&data, now);
        assert_eq!(flights.len(), 1);
        assert_eq!(flights[0].callsign, "AZU123");
        assert!((flights[0].altitude_feet.unwrap() - 32808.4).abs() < 0.1);
    }
    #[test]
    fn parses_adsb_position_and_rejects_ground_and_stale_records() {
        let data = serde_json::json!({"ac": [
            {"hex":"ab1234", "flight":"GLO123 ", "lat":-23.4, "lon":-46.4,
             "seen_pos":4.2,"gs":440.0,"track":110.0,"alt_baro":36000},
            {"hex":"ab1235","lat":-23.4,"lon":-46.4,"seen_pos":1,"alt_baro":"ground"},
            {"hex":"ab1236","lat":-23.4,"lon":-46.4,"seen_pos":400}
        ]});
        let flights = parse_adsb_lol(&data, 1000.0);
        assert_eq!(flights.len(), 1);
        assert_eq!(flights[0].callsign, "GLO123");
        assert!((flights[0].observed_unix - 995.8).abs() < 0.01);
    }
    #[test]
    fn great_circle_distance_handles_dateline() {
        assert!(geographic_distance_km((0.0, 179.8), (0.0, -179.8)) < 50.0);
    }
}
