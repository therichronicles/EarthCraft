//! TomTom Orbis live congestion tiles. Explicit API key required; no fake roads.
use image::{RgbaImage, imageops};
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
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
    pub failed_tiles: usize,
}
impl TrafficTiles {
    pub fn sample(&self, lat: f64, lon: f64) -> Option<[u8; 4]> {
        if lat.abs() >= 85.0 {
            return None;
        }
        let scale = (1u64 << self.zoom) as f64;
        let tx = ((lon + 180.0) / 360.0 * scale).rem_euclid(scale);
        let lat_r = lat.to_radians();
        let ty = (1.0 - lat_r.tan().asinh() / std::f64::consts::PI) * 0.5 * scale;
        let mut dx = tx - self.left_x as f64;
        if dx < 0.0 {
            dx += scale;
        }
        if dx >= 3.0 || !(0.0..3.0).contains(&(ty - self.top_y as f64)) {
            return None;
        }
        let x = (dx * 256.0) as u32;
        let y = ((ty - self.top_y as f64) * 256.0) as u32;
        Some(self.image.get_pixel(x.min(767), y.min(767)).0)
    }
}
pub struct TrafficQuery<'a> {
    pub enabled: bool,
    pub key: &'a str,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_km: f64,
}

#[derive(Default)]
pub struct LiveTraffic {
    tiles: Option<Arc<TrafficTiles>>,
    rx: Option<mpsc::Receiver<Result<TrafficTiles, crate::Error>>>,
    attempt: Option<Instant>,
    error: Option<crate::Error>,
    last_key: String,
    pending_key: Option<String>,
    requested: Option<(u32, i64, i64)>,
    generation: u64,
}
impl LiveTraffic {
    pub fn tiles(&self) -> Option<&TrafficTiles> {
        self.tiles
            .as_deref()
            .filter(|t| t.loaded.elapsed() <= Duration::from_secs(15 * 60))
    }
    pub fn shared_tiles(&self) -> Option<Arc<TrafficTiles>> {
        self.tiles().and_then(|_| self.tiles.clone())
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn status(
        &self,
        key: &str,
        altitude: f64,
        lang: earthcraft_core::language::Lang,
    ) -> String {
        if key.trim().is_empty() {
            return lang.text("traffic-key-help").into();
        }
        if altitude > MAX_TRAFFIC_ALTITUDE_KM {
            return lang.text("traffic-zoom-help").into();
        }
        if let Some(error) = &self.error {
            return format!("TomTom: {error}");
        }
        if let Some(tiles) = self.tiles().filter(|tiles| tiles.failed_tiles > 0) {
            return earthcraft_core::language::Lang::En.format(
                "traffic-partial-error",
                &[("count", tiles.failed_tiles.into())],
            );
        }
        match self.tiles() {
            Some(tiles) => lang.format(
                "traffic-loaded-status",
                &[("minutes", (tiles.loaded.elapsed().as_secs() / 60).into())],
            ),
            None => lang.text("traffic-loading-status").into(),
        }
    }
    pub fn update(
        &mut self,
        runtime: &crate::runtime::ServiceRuntime,
        query: TrafficQuery<'_>,
        repaint: impl Fn() + Send + Sync + 'static,
    ) {
        let TrafficQuery {
            enabled,
            key,
            latitude: lat,
            longitude: lon,
            altitude_km: alt,
        } = query;
        let key = key.trim();
        self.set_key(key);
        self.poll(key);
        if !enabled || key.is_empty() || alt > MAX_TRAFFIC_ALTITUDE_KM || lat.abs() >= 85.0 {
            return;
        }
        let zoom = zoom_for_altitude(alt);
        let (x, y) = center_tile(lat, lon, zoom);
        let center = (zoom, x, y);
        let moving = self.requested != Some(center);
        let due = self.attempt.is_none_or(|a| {
            a.elapsed()
                >= if moving {
                    MOVE_INTERVAL
                } else if self.error.is_some() {
                    RETRY_INTERVAL
                } else {
                    UPDATE_INTERVAL
                }
        });
        if self.rx.is_none() && due {
            self.requested = Some(center);
            self.attempt = Some(Instant::now());
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            self.pending_key = Some(key.to_owned());
            let key = key.to_owned();
            runtime.spawn(async move {
                let _ = tx.send(download(zoom, x, y, key).await);
                repaint();
            });
        }
    }
    fn set_key(&mut self, key: &str) {
        if self.last_key != key {
            self.tiles = None;
            self.last_key = key.to_owned();
            self.requested = None;
            self.attempt = None;
            self.error = None;
            self.generation += 1;
        }
    }
    fn poll(&mut self, key: &str) {
        let Some(receiver) = &self.rx else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("TomTom disconnected".into()),
        };
        self.rx = None;
        if self.pending_key.take().as_deref() != Some(key) {
            return;
        }
        match result {
            Ok(tiles) => {
                self.tiles = Some(Arc::new(tiles));
                self.error = None;
                self.generation += 1;
            }
            Err(error) => self.error = Some(error),
        }
    }
}
fn zoom_for_altitude(alt: f64) -> u32 {
    (15_000.0 / alt.max(0.1)).log2().round().clamp(1.0, 15.0) as u32
}
fn center_tile(lat: f64, lon: f64, zoom: u32) -> (i64, i64) {
    let n = (1u64 << zoom) as f64;
    let x = ((lon + 180.0) / 360.0 * n).floor().rem_euclid(n) as i64;
    let y = ((1.0 - lat.to_radians().tan().asinh() / std::f64::consts::PI) * 0.5 * n)
        .floor()
        .clamp(0.0, n - 1.0) as i64;
    (x, y)
}
async fn download(zoom: u32, x: i64, y: i64, key: String) -> Result<TrafficTiles, crate::Error> {
    let client = crate::http::client(crate::http::Policy::Radar)?;
    let n = 1i64 << zoom;
    let left = x - 1;
    let top = (y - 1).clamp(0, n - 1);
    let mut decoded = Vec::with_capacity(9);
    let mut tasks = tokio::task::JoinSet::new();
    let mut expected_tiles = 0;
    for iy in 0..3 {
        for ix in 0..3 {
            let ty = top + iy;
            if ty >= n {
                continue;
            }
            let tx = (left + ix).rem_euclid(n);
            let url = format!(
                "https://api.tomtom.com/maps/orbis/traffic/flow/raster/tile/{zoom}/{tx}/{ty}?apiVersion=2&style=light&tileSize=256"
            );
            let client = client.clone();
            let key = key.clone();
            expected_tiles += 1;
            tasks.spawn(async move {
                let bytes =
                    crate::http::body(client.get(url).header("TomTom-Api-Key", key), 350_000)
                        .await
                        .ok()?;
                let tile = crate::cpu::run(crate::cpu::Service::Tiles, move || {
                    Ok(image::load_from_memory(&bytes)?.to_rgba8())
                })
                .await
                .ok()?;
                Some((ix, iy, tile))
            });
        }
    }
    while let Some(item) = tasks.join_next().await {
        if let Ok(Some((ix, iy, img))) = item
            && img.width() == 256
            && img.height() == 256
        {
            decoded.push((ix, iy, img));
        }
    }
    crate::cpu::run(crate::cpu::Service::Features, move || {
        compose_tiles(zoom, left, top, decoded, expected_tiles)
    })
    .await
}
fn compose_tiles(
    zoom: u32,
    left: i64,
    top: i64,
    decoded: Vec<(i64, i64, RgbaImage)>,
    expected_tiles: usize,
) -> Result<TrafficTiles, crate::Error> {
    if decoded.is_empty() {
        return Err(
            "No live traffic tiles (invalid key, rate limit or provider unavailable)".into(),
        );
    }
    let failed_tiles = expected_tiles.saturating_sub(decoded.len());
    let mut image = RgbaImage::new(768, 768);
    for (ix, iy, tile) in decoded {
        imageops::overlay(&mut image, &tile, ix * 256, iy * 256);
    }
    Ok(TrafficTiles {
        image,
        zoom,
        left_x: left,
        top_y: top,
        loaded: Instant::now(),
        failed_tiles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_composites_preserve_pixels_and_report_failures_in_both_languages() {
        let tile = RgbaImage::from_pixel(256, 256, image::Rgba([5, 10, 15, 255]));
        let tiles = compose_tiles(2, 0, 0, vec![(0, 0, tile)], 2).unwrap();
        assert_eq!(tiles.failed_tiles, 1);
        assert_eq!(tiles.image.get_pixel(0, 0).0, [5, 10, 15, 255]);
        assert_eq!(tiles.image.get_pixel(256, 0).0, [0, 0, 0, 0]);
        let live = LiveTraffic {
            tiles: Some(Arc::new(tiles)),
            ..Default::default()
        };
        for lang in earthcraft_core::language::Lang::ALL {
            assert_eq!(
                live.status("key", 1.0, lang),
                "Traffic incomplete: 1 tiles failed to load"
            );
        }
        assert!(compose_tiles(2, 0, 0, Vec::new(), 2).is_err());
    }
    #[test]
    fn mercator_center_and_wrap() {
        assert_eq!(center_tile(0.0, 0.0, 2), (2, 2));
        assert_eq!(center_tile(0.0, 180.0, 2).0, 0);
        let t = TrafficTiles {
            image: RgbaImage::new(768, 768),
            zoom: 2,
            left_x: 1,
            top_y: 1,
            loaded: Instant::now(),
            failed_tiles: 0,
        };
        assert!(t.sample(0.0, 0.0).is_some());
        // At zoom 2, rows 1..4 include the southern hemisphere down to -85 degrees.
        assert!(t.sample(-80.0, 0.0).is_some());
        assert!(t.sample(80.0, 0.0).is_none());
        assert!(t.sample(-85.0, 0.0).is_none());
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;
    fn tiles() -> TrafficTiles {
        TrafficTiles {
            image: RgbaImage::new(768, 768),
            zoom: 2,
            left_x: 1,
            top_y: 1,
            loaded: Instant::now(),
            failed_tiles: 0,
        }
    }
    #[test]
    fn changed_or_cleared_keys_discard_old_completions_and_keep_one_pending_request() {
        let mut traffic = LiveTraffic::default();
        traffic.set_key("first");
        let (send, receive) = mpsc::channel();
        traffic.rx = Some(receive);
        traffic.pending_key = Some("first".into());
        traffic.set_key("second");
        assert!(traffic.rx.is_some());
        send.send(Ok(tiles())).unwrap();
        traffic.poll("second");
        assert!(traffic.tiles().is_none());
        assert!(traffic.rx.is_none());
        let (send, receive) = mpsc::channel();
        traffic.rx = Some(receive);
        traffic.pending_key = Some("second".into());
        send.send(Ok(tiles())).unwrap();
        traffic.poll("second");
        let shared = traffic.shared_tiles().unwrap();
        assert!(Arc::ptr_eq(&shared, &traffic.shared_tiles().unwrap()));
        traffic.set_key("");
        assert!(traffic.tiles().is_none());
        assert_eq!(shared.image.dimensions(), (768, 768));
    }
    #[test]
    fn current_errors_keep_typed_causes_and_stale_errors_are_ignored() {
        for (response_key, accept) in [("current", true), ("old", false)] {
            let mut traffic = LiveTraffic::default();
            let (send, receive) = mpsc::channel();
            traffic.rx = Some(receive);
            traffic.pending_key = Some(response_key.into());
            send.send(Err(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            )
            .into()))
                .unwrap();
            traffic.poll("current");
            assert_eq!(traffic.error.is_some(), accept);
            if accept {
                assert!(matches!(traffic.error, Some(crate::Error::Io(_))));
            }
        }
    }
}
