//! TomTom Orbis live congestion tiles. Explicit API key required; no fake roads.
use image::{RgbaImage, imageops};
use std::{sync::mpsc, time::{Duration, Instant}};
const UPDATE_INTERVAL: Duration = Duration::from_secs(120);
const RETRY_INTERVAL: Duration = Duration::from_secs(60);
const MOVE_INTERVAL: Duration = Duration::from_secs(25);
pub const MAX_TRAFFIC_ALTITUDE_KM: f64 = 80.0;
#[derive(Clone)]
pub struct TrafficTiles {
    pub image: RgbaImage,
    pub zoom: u32,
    pub left_x: i64,
    pub top_y: i64,
    pub loaded: Instant,
}
impl TrafficTiles {
    pub fn sample(&self, lat: f64, lon: f64) -> Option<[u8;4]> {
        if lat.abs() >= 85.0 {return None;}
        let scale = (1u64 << self.zoom) as f64;
        let tx = ((lon + 180.0) / 360.0 * scale).rem_euclid(scale);
        let lat_r = lat.to_radians();
        let ty = (1.0 - lat_r.tan().asinh()/std::f64::consts::PI) * 0.5 * scale;
        let mut dx = tx - self.left_x as f64;
        if dx < 0.0 {dx += scale;}
        if dx >= 3.0 || !(0.0..3.0).contains(&(ty - self.top_y as f64)) { return None; }
        let x = (dx * 256.0) as u32;
        let y = ((ty - self.top_y as f64) * 256.0) as u32;
        Some(self.image.get_pixel(x.min(767), y.min(767)).0)
    }
}
#[derive(Default)]
pub struct LiveTraffic {
    tiles: Option<TrafficTiles>,
    rx: Option<mpsc::Receiver<Result<TrafficTiles, crate::Error>>>,
    attempt: Option<Instant>,
    error: Option<String>,
    last_key: String,
    requested: Option<(u32, i64, i64)>,
    generation: u64,
}
impl LiveTraffic {
    pub fn tiles(&self) -> Option<&TrafficTiles> {
        self.tiles.as_ref().filter(|t| t.loaded.elapsed() <= Duration::from_secs(15*60))
    }
    pub fn generation(&self) -> u64 {self.generation}
    pub fn status(&self, key: &str, altitude: f64, pt: bool) -> String {
        if key.trim().is_empty() { return if pt {"Insira uma chave TomTom para tráfego real"} else {"Enter a TomTom key for real traffic"}.into(); }
        if altitude > MAX_TRAFFIC_ALTITUDE_KM { return if pt {"Aproxime o mapa para mostrar o trânsito"} else {"Zoom in to see traffic"}.into(); }
        if let Some(e) = &self.error {return format!("TomTom: {e}");}
        if let Some(t) = self.tiles() {
            if pt {format!("Tráfego atualizado há {} min (verde=livre, amarelo=lento, vermelho=congestionado)",t.loaded.elapsed().as_secs()/60)}
            else {format!("Traffic updated {} min ago (green=free, yellow=slow, red=jammed)",t.loaded.elapsed().as_secs()/60)}
        } else {if pt {"Carregando tráfego real…"} else {"Loading live traffic…"}.into()}
    }
    pub fn update(&mut self, runtime: &crate::runtime::ServiceRuntime, enabled: bool,
                  key: &str, lat: f64, lon: f64, alt: f64,
                  repaint: impl Fn() + Send + Sync + 'static) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Ok(t)) => { self.tiles = Some(t); self.error = None; self.generation += 1; self.rx = None; },
                Ok(Err(e)) => { self.error = Some(e.to_string()); self.rx = None; },
                Err(mpsc::TryRecvError::Disconnected) => { self.error = Some("TomTom disconnected".into()); self.rx = None; },
                Err(mpsc::TryRecvError::Empty) => {},
            }
        }
        let key = key.trim();
        if !enabled || key.is_empty() || alt > MAX_TRAFFIC_ALTITUDE_KM || lat.abs() >= 85.0 {return;}
        let zoom = zoom_for_altitude(alt);
        let (x,y) = center_tile(lat,lon,zoom);
        let center = (zoom, x, y);
        if self.last_key != key {
            self.tiles = None;
            self.last_key = key.to_owned();
            self.requested = None;
            self.attempt = None;
            self.error = None;
            self.generation += 1;
        }
        let moving = self.requested != Some(center);
        let due = self.attempt.is_none_or(|a| a.elapsed() >= if moving { MOVE_INTERVAL }
            else if self.error.is_some() { RETRY_INTERVAL } else {UPDATE_INTERVAL});
        if self.rx.is_none() && due {
            self.requested = Some(center);
            self.attempt = Some(Instant::now());
            let (tx,rx) = mpsc::channel(); self.rx = Some(rx);
            let key = key.to_owned();
            runtime.spawn(async move { let _ = tx.send(download(zoom,x,y,key).await); repaint(); });
        }
    }
}
fn zoom_for_altitude(alt: f64) -> u32 { (15_000.0 / alt.max(0.1)).log2().round().clamp(1.0, 15.0) as u32 }
fn center_tile(lat: f64, lon: f64, zoom: u32) -> (i64,i64) {
    let n = (1u64 << zoom) as f64;
    let x = ((lon+180.0)/360.0*n).floor().rem_euclid(n) as i64;
    let y = ((1.0-lat.to_radians().tan().asinh()/std::f64::consts::PI)*0.5*n)
        .floor().clamp(0.0,n-1.0) as i64;
    (x,y)
}
async fn download(zoom: u32,x:i64,y:i64,key:String) -> Result<TrafficTiles,crate::Error> {
    let client = crate::http::client(crate::http::Policy::Radar)?;
    let mut image = RgbaImage::new(768,768);
    let n = 1i64 << zoom;
    let left = x-1;
    let top = (y-1).clamp(0,n-1);
    let mut received = 0;
    let mut tasks = tokio::task::JoinSet::new();
    for iy in 0..3 {
        for ix in 0..3 {
            let ty = top+iy;
            if ty>=n {continue;}
            let tx = (left+ix).rem_euclid(n);
            let url = format!("https://api.tomtom.com/maps/orbis/traffic/flow/raster/tile/{zoom}/{tx}/{ty}?apiVersion=2&style=light&tileSize=256");
            let client = client.clone(); let key = key.clone();
            tasks.spawn(async move {
                let bytes = crate::http::body(client.get(url).header("TomTom-Api-Key",key), 350_000).await.ok()?;
                let tile = image::load_from_memory(&bytes).ok()?.to_rgba8();
                Some((ix,iy,tile))
            });
        }
    }
    while let Some(item) = tasks.join_next().await {
        if let Ok(Some((ix,iy,img))) = item {
            if img.width()==256 && img.height()==256 {
                imageops::overlay(&mut image, &img, ix*256, iy*256);
                received+=1;
            }
        }
    }
    if received==0 {return Err("No live traffic tiles (invalid key, rate limit or provider unavailable)".into());}
    Ok(TrafficTiles {image, zoom, left_x:left, top_y:top, loaded:Instant::now()})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mercator_center_and_wrap() {
        assert_eq!(center_tile(0.0,0.0,2),(2,2));
        assert_eq!(center_tile(0.0,180.0,2).0,0);
        let t=TrafficTiles {image:RgbaImage::new(768,768),zoom:2,left_x:1,top_y:1,loaded:Instant::now()};
        assert!(t.sample(0.0,0.0).is_some());
        assert!(t.sample(-80.0,0.0).is_none());
    }
}
