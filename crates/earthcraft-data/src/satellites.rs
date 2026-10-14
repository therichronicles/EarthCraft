//! CelesTrak OMM element sets converted to approximate *predicted* ground tracks.
//! This is a Kepler two-body educational visualization, not SGP4 or live telemetry.
use std::{
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const URLS: [&str; 3] = [
    "https://celestrak.org/NORAD/elements/gp.php?GROUP=STATIONS&FORMAT=JSON",
    "https://celestrak.org/NORAD/elements/gp.php?GROUP=GPS-OPS&FORMAT=JSON",
    "https://celestrak.org/NORAD/elements/gp.php?GROUP=WEATHER&FORMAT=JSON",
];
const REFRESH: Duration = Duration::from_secs(6 * 3600);
const RETRY: Duration = Duration::from_secs(10 * 60);
const GRAVITY_MU: f64 = 398_600.441_8;
#[derive(Clone, Debug)]
pub struct Satellite {
    pub name: String,
    pub norad: String,
    pub inclination: f64,
    pub raan: f64,
    pub eccentricity: f64,
    pub argument_perigee: f64,
    pub mean_anomaly: f64,
    pub mean_motion: f64,
    pub epoch: f64,
}
impl Satellite {
    /// Predicted geodetic subpoint, low-precision two-body propagation (no J2/SGP4).
    pub fn ground_position(&self, unix: f64) -> Option<(f64, f64)> {
        if (unix - self.epoch).abs() > 14.0 * 86400.0 {
            return None;
        }
        let n = self.mean_motion * std::f64::consts::TAU / 86400.0;
        if n <= 0.0 {
            return None;
        }
        let a = (GRAVITY_MU / (n * n)).cbrt();
        let e = self.eccentricity;
        if !(0.0..1.0).contains(&e) {
            return None;
        }
        let m = (self.mean_anomaly.to_radians() + n * (unix - self.epoch))
            .rem_euclid(std::f64::consts::TAU);
        let mut anomaly = m;
        for _ in 0..8 {
            anomaly -= (anomaly - e * anomaly.sin() - m) / (1.0 - e * anomaly.cos());
        }
        let r = a * (1.0 - e * anomaly.cos());
        let true_anomaly = ((1.0 - e * e).sqrt() * anomaly.sin()).atan2(anomaly.cos() - e);
        let u = self.argument_perigee.to_radians() + true_anomaly;
        let (incl, raan) = (self.inclination.to_radians(), self.raan.to_radians());
        let (si, ci) = incl.sin_cos();
        let (su, cu) = u.sin_cos();
        let (so, co) = raan.sin_cos();
        let x = r * (co * cu - so * su * ci);
        let y = r * (so * cu + co * su * ci);
        let z = r * su * si;
        // Greenwich mean sidereal angle using Julian-date approximation.
        let jd = unix / 86400.0 + 2_440_587.5;
        let t = (jd - 2_451_545.0) / 36_525.0;
        let gmst =
            (280.460_618_37 + 360.985_647_366_29 * (jd - 2_451_545.0) + 0.000_387_933 * t * t
                - t * t * t / 38_710_000.0)
                .to_radians();
        let lat = z.atan2((x * x + y * y).sqrt()).to_degrees();
        let lon = ((y.atan2(x) - gmst).to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
        Some((lat, lon))
    }
}
#[derive(Default)]
pub struct LiveSatellites {
    satellites: Vec<Satellite>,
    loaded: Option<Instant>,
    attempted: Option<Instant>,
    error: Option<String>,
    rx: Option<mpsc::Receiver<Result<Vec<Satellite>, crate::Error>>>,
}
impl LiveSatellites {
    pub fn update(
        &mut self,
        runtime: &crate::runtime::ServiceRuntime,
        enabled: bool,
        repaint: impl Fn() + Send + Sync + 'static,
    ) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Ok(s)) => {
                    self.satellites = s;
                    self.loaded = Some(Instant::now());
                    self.error = None;
                    self.rx = None;
                }
                Ok(Err(e)) => {
                    self.error = Some(e.to_string());
                    self.rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("CelesTrak disconnected".into());
                    self.rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if enabled
            && self.rx.is_none()
            && self
                .attempted
                .is_none_or(|t| t.elapsed() >= if self.error.is_some() { RETRY } else { REFRESH })
        {
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            self.attempted = Some(Instant::now());
            runtime.spawn(async move {
                let _ = tx.send(download().await);
                repaint();
            });
        }
    }
    pub fn snapshot(&self) -> &[Satellite] {
        &self.satellites
    }
    pub fn status(&self, pt: bool) -> String {
        if let Some(err) = &self.error {
            return format!("CelesTrak: {err}");
        }
        if self.loaded.is_none() {
            return if pt {
                "Carregando órbitas…"
            } else {
                "Loading orbits…"
            }
            .into();
        }
        let n = self.satellites.len();
        if pt {
            format!("{n} satélites • posição orbital aproximada (não telemetria)")
        } else {
            format!("{n} satellites • approximate orbital position (not telemetry)")
        }
    }
}
fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
async fn download() -> Result<Vec<Satellite>, crate::Error> {
    let client = crate::http::client(crate::http::Policy::Aircraft)?;
    let mut all = Vec::new();
    let mut errors = Vec::new();
    for url in URLS {
        match crate::http::body(client.get(url), 4_000_000).await {
            Ok(bytes) => match crate::http::parse_json(bytes).await {
                Ok(v) => all.extend(parse_omm(&v)),
                Err(e) => errors.push(e.to_string()),
            },
            Err(e) => errors.push(e.to_string()),
        }
    }
    all.sort_unstable_by(|a, b: &Satellite| a.norad.cmp(&b.norad));
    all.dedup_by(|a, b| a.norad == b.norad);
    if all.is_empty() {
        return Err(format!("CelesTrak unavailable: {}", errors.join("; ")).into());
    }
    Ok(all)
}
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
    let m: i64 = date.next()?.parse().ok()?;
    let d: i64 = date.next()?.parse().ok()?;
    let mut time = time.trim_end_matches('Z').split(':');
    let hour: f64 = time.next()?.parse().ok()?;
    let minute: f64 = time.next()?.parse().ok()?;
    let second: f64 = time.next()?.parse().ok()?;
    Some(unix_day(y, m, d) as f64 * 86400.0 + hour * 3600.0 + minute * 60.0 + second)
}
fn parse_omm(value: &serde_json::Value) -> Vec<Satellite> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let s = Satellite {
                name: v["OBJECT_NAME"].as_str()?.to_owned(),
                norad: v["NORAD_CAT_ID"].to_string().trim_matches('"').to_owned(),
                inclination: num(v, "INCLINATION")?,
                raan: num(v, "RA_OF_ASC_NODE")?,
                eccentricity: num(v, "ECCENTRICITY")?,
                argument_perigee: num(v, "ARG_OF_PERICENTER")?,
                mean_anomaly: num(v, "MEAN_ANOMALY")?,
                mean_motion: num(v, "MEAN_MOTION")?,
                epoch: parse_epoch(v["EPOCH"].as_str()?)?,
            };
            if s.mean_motion > 0.0 && s.ground_position(now_unix()).is_some() {
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
            inclination: 90.0,
            raan: 0.0,
            eccentricity: 0.001,
            argument_perigee: 0.0,
            mean_anomaly: 0.0,
            mean_motion: 15.0,
            epoch: 1_700_000_000.0,
        };
        let p = sat.ground_position(sat.epoch).unwrap();
        assert!(p.0.abs() < 0.001);
        assert!(sat.ground_position(sat.epoch + 86400.0 * 15.0).is_none());
    }
}
