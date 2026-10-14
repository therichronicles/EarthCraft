//! Worldwide vessel positions from OpenSeaFeed's public full-fleet AIS snapshot.
//! The network's real AIS reception is incomplete (especially far offshore).
//! A custom licensed GeoJSON URL overrides this source; Finnish AIS is a fallback
//! only when the public worldwide provider cannot be reached.
use std::{sync::mpsc, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
const WORLD_URL: &str = "https://api.openseafeed.com/v1/snapshot";
const FINNISH_URL: &str = "https://meri.digitraffic.fi/api/ais/v1/locations";
const REFRESH: Duration = Duration::from_secs(120);
const RETRY: Duration = Duration::from_secs(90);
const MAX_CACHE_AGE: Duration = Duration::from_secs(15 * 60);
const MAX_REPORT_AGE_SECS: f64 = 90.0 * 60.0;

#[derive(Clone, Debug)]
pub struct Vessel {
    pub mmsi: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub course: f64,
    pub speed_knots: Option<f64>,
    pub observed_unix: f64,
}
#[derive(Clone, Copy, Debug, Default)]
enum Source { #[default] World, Custom, FinlandFallback }
struct VesselBatch { vessels: Vec<Vessel>, source: Source }
#[derive(Default)]
pub struct LiveVessels {
    current: Vec<Vessel>,
    source: Source,
    loaded: Option<Instant>,
    attempt: Option<Instant>,
    rx: Option<mpsc::Receiver<Result<VesselBatch, crate::Error>>>,
    error: Option<String>,
}
impl LiveVessels {
    pub fn update(&mut self, runtime: &crate::runtime::ServiceRuntime, enabled: bool,
                  repaint: impl Fn() + Send + Sync + 'static) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Ok(batch)) => {
                    self.current = batch.vessels;
                    self.source = batch.source;
                    self.loaded = Some(Instant::now());
                    self.error = None; self.rx = None;
                },
                Ok(Err(e)) => { self.error = Some(e.to_string()); self.rx = None; },
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("AIS disconnected".into()); self.rx = None;
                },
                Err(mpsc::TryRecvError::Empty) => {},
            }
        }
        if enabled && self.rx.is_none() && self.attempt.is_none_or(|t| t.elapsed() >=
            if self.error.is_some() { RETRY } else { REFRESH }) {
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            self.attempt = Some(Instant::now());
            runtime.spawn(async move {
                let _ = tx.send(download().await);
                repaint();
            });
        }
    }
    pub fn snapshot(&self) -> &[Vessel] {
        if self.loaded.is_some_and(|t| t.elapsed() <= MAX_CACHE_AGE) { &self.current } else { &[] }
    }
    pub fn status(&self, pt: bool) -> String {
        if let Some(err) = &self.error { return format!("AIS: {err}"); }
        if self.loaded.is_none() {
            return if pt {"Carregando navios AIS mundiais…"} else {"Loading worldwide AIS ships…"}.into();
        }
        let count = self.snapshot().len();
        match (self.source, pt) {
            (Source::World, true) => format!("{count} navios • OpenSeaFeed mundial • cobertura parcial, observações até 90 min"),
            (Source::World, false) => format!("{count} vessels • worldwide OpenSeaFeed • partial reception, reports up to 90 min old"),
            (Source::Custom, true) => format!("{count} navios • GeoJSON AIS configurado • cobertura do fornecedor"),
            (Source::Custom, false) => format!("{count} vessels • custom AIS GeoJSON • provider coverage"),
            (Source::FinlandFallback, true) => format!("{count} navios • contingência Fintraffic (só Finlândia); fonte mundial indisponível"),
            (Source::FinlandFallback, false) => format!("{count} vessels • Fintraffic fallback (Finland only); global feed unavailable"),
        }
    }
    pub fn source_label(&self) -> &'static str {
        match self.source {
            Source::World => "OpenSeaFeed (mundial)",
            Source::Custom => "GeoJSON AIS",
            Source::FinlandFallback => "Digitraffic/Fintraffic (Finlândia)",
        }
    }
}
async fn download() -> Result<VesselBatch, crate::Error> {
    if let Ok(url) = std::env::var("EARTHCRAFT_AIS_GEOJSON_URL") {
        if !url.starts_with("https://") {return Err("AIS endpoint must use HTTPS".into());}
        let json = request_json(&url).await?;
        if !json["features"].is_array() {return Err("Custom AIS endpoint must return a GeoJSON FeatureCollection".into());}
        return Ok(VesselBatch {vessels:parse_ais_geojson(&json,now_unix()),source:Source::Custom});
    }
    if let Ok(json) = request_json(WORLD_URL).await {
        if json["vessels"].is_array() {
            return Ok(VesselBatch {vessels:parse_world_snapshot(&json,now_unix()),source:Source::World});
        }
    }
    let json = request_json(FINNISH_URL).await?;
    if !json["features"].is_array() {return Err("No compatible worldwide or Finnish AIS feed".into());}
    Ok(VesselBatch {vessels:parse_ais_geojson(&json,now_unix()),source:Source::FinlandFallback})
}
async fn request_json(url:&str) -> Result<serde_json::Value,crate::Error> {
    let req = crate::http::client(crate::http::Policy::Aircraft)?.get(url)
        .header("Digitraffic-User", "EarthCraft/EducationalGlobe")
        .header("Accept", "application/json")
        .header("Accept-Encoding", "gzip");
    // Public fleet snapshots may be several MB; hard cap protects the application.
    crate::http::parse_json(crate::http::body(req, 40_000_000).await?).await
}
fn now_unix() -> f64 {SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64()}
fn number(v: Option<&serde_json::Value>) -> Option<f64> {
    v.and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok())))
        .filter(|v: &f64| v.is_finite())
}
fn valid_position(lat:f64, lon:f64, time:f64, now:f64) -> bool {
    (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)
        && now-time <= MAX_REPORT_AGE_SECS && time-now <= 300.0
}
fn time_seconds(raw: f64) -> f64 {if raw > 1e11 {raw/1000.0} else {raw}}
fn parse_world_snapshot(json: &serde_json::Value, now:f64) -> Vec<Vessel> {
    json["vessels"].as_array().into_iter().flatten().take(200_000).filter_map(|p| {
        let lat = number(p.get("lat"))?;
        let lon = number(p.get("lon"))?;
        // OpenSeaFeed: "ts" is last observation in Unix milliseconds, not snapshot time.
        let observed = time_seconds(number(p.get("ts"))?);
        if !valid_position(lat,lon,observed,now) {return None;}
        let mmsi = p.get("mmsi")?.to_string().trim_matches('"').to_owned();
        let name = p["name"].as_str().unwrap_or("").trim().to_owned();
        Some(Vessel {mmsi,name,lat,lon,
            course:number(p.get("cog")).unwrap_or(0.0).rem_euclid(360.0),
            speed_knots:number(p.get("sog")).filter(|s| (0.0..=102.2).contains(s)),
            observed_unix:observed})
    }).collect()
}
fn parse_ais_geojson(json: &serde_json::Value, now:f64) -> Vec<Vessel> {
    json["features"].as_array().into_iter().flatten().take(30_000).filter_map(|f| {
        let coords = f.pointer("/geometry/coordinates")?.as_array()?;
        let lon = number(coords.first())?;
        let lat = number(coords.get(1))?;
        let p = &f["properties"];
        let mmsi = p.get("mmsi").or_else(||p.get("MMSI"))
            .or_else(||f.get("mmsi")).or_else(||f.get("id"))?
            .to_string().trim_matches('"').to_owned();
        // Fintraffic timestamp is only second-of-minute; use timestampExternal.
        let observed = number(p.get("timestampExternal").or_else(||p.get("time")))
            .or_else(||number(p.get("observed_unix")))
            .or_else(||number(p.get("timestamp")).filter(|v| *v > 1e9))?;
        let observed = time_seconds(observed);
        if !valid_position(lat,lon,observed,now) {return None;}
        Some(Vessel {mmsi,name:p.get("name").or_else(||p.get("shipname"))
                .and_then(|v|v.as_str()).unwrap_or("").trim().to_owned(),lat,lon,
            course:number(p.get("cog").or_else(||p.get("course"))).unwrap_or(0.0).rem_euclid(360.0),
            speed_knots:number(p.get("sog").or_else(||p.get("speed"))),observed_unix:observed})
    }).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worldwide_snapshot_reads_milliseconds_and_multiple_regions() {
        let now=1_800_000_000.0;
        let input=serde_json::json!({"generated_at":1_800_000_000_000u64,"vessels":[
            {"mmsi":123456789,"lat":-23.95,"lon":-46.3,"name":"SANTOS","ts":1_799_999_930_000u64,"sog":9.1,"cog":110.0},
            {"mmsi":987654321,"lat":35.7,"lon":139.8,"name":"TOKYO","ts":1_799_999_940_000u64},
            {"mmsi":111111111,"lat":60.1,"lon":25.0,"ts":1_799_900_000_000u64}
        ]});
        let ships=parse_world_snapshot(&input,now);
        assert_eq!(ships.len(),2);
        assert_eq!(ships[0].name,"SANTOS");
        assert_eq!(ships[1].lat,35.7);
        assert_eq!(ships[0].speed_knots,Some(9.1));
    }
    #[test]
    fn geojson_preserves_external_timestamp() {
        let input=serde_json::json!({"features":[{"geometry":{"coordinates":[24.93,60.15]},
            "properties":{"mmsi":123456789,"sog":12.5,"cog":32,"timestampExternal":1_000_000}}]});
        let v=parse_ais_geojson(&input,1002.0);
        assert_eq!(v.len(),1);
        assert_eq!(v[0].speed_knots,Some(12.5));
    }
}
