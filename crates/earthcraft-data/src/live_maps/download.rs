//! Service-specific downloads and radar palette decoding.
use super::RadarFrame;
use image::{RgbaImage, imageops::FilterType};
const CLOUD_SOURCE: &str = "https://clouds.matteason.co.uk/images/8192x4096/clouds-alpha.png";
const RADAR_API: &str = "https://api.rainviewer.com/public/weather-maps.json";
const MAX_CLOUD_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RADAR_METADATA_BYTES: u64 = 1024 * 1024;
const MAX_RADAR_TILE_BYTES: u64 = 4 * 1024 * 1024;
const TILE_ZOOM: usize = 2;
const TILE_SIZE: u32 = 512;
pub(super) async fn download_clouds() -> Result<RgbaImage, crate::Error> {
    let client = crate::http::client(crate::http::Policy::Cloud)?;
    // Cache bust by 3-hour slots; URL remains the requested source image.
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / (3 * 60 * 60);
    let url = format!("{CLOUD_SOURCE}?slot={epoch}");
    let bytes = crate::http::body(client.get(url), MAX_CLOUD_RESPONSE_BYTES).await?;
    // Decode off the UI thread and keep only the working resolution in memory.
    crate::cpu::run(crate::cpu::Service::Features, move || {
        let image = image::load_from_memory(&bytes)?;
        Ok(image
            .resize_exact(2048, 1024, FilterType::Triangle)
            .to_rgba8())
    })
    .await
}

pub(super) async fn download_radar() -> Result<RadarFrame, crate::Error> {
    let client = crate::http::client(crate::http::Policy::Radar)?;
    let bytes = crate::http::body(client.get(RADAR_API), MAX_RADAR_METADATA_BYTES).await?;
    let json = crate::http::parse_json(bytes).await?;
    let frame = json
        .pointer("/radar/past")
        .and_then(|v| v.as_array())
        .and_then(|v| v.last())
        .ok_or("No radar frames available")?;
    let path = frame
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Radar path missing")?;
    if !path.starts_with("/v2/radar/")
        || !path[10..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/')
    {
        return Err("Unexpected radar path".into());
    }
    let time = frame
        .get("time")
        .and_then(|v| v.as_i64())
        .ok_or("Radar time missing")?;
    let n = 1usize << TILE_ZOOM;
    let mut image = RgbaImage::new((n as u32) * TILE_SIZE, (n as u32) * TILE_SIZE);
    let mut accepted = 0;
    let limiter = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    let mut tasks = tokio::task::JoinSet::new();
    for y in 0..n {
        for x in 0..n {
            // RainViewer 2026 free API: ONLY color scheme 2 and max zoom 7.
            let url = format!(
                "https://tilecache.rainviewer.com{path}/{TILE_SIZE}/{TILE_ZOOM}/{x}/{y}/2/1_0.png"
            );
            let client = client.clone();
            let limiter = limiter.clone();
            tasks.spawn(async move {
                let _permit = limiter.acquire_owned().await.ok()?;
                let bytes = crate::http::body(client.get(url), MAX_RADAR_TILE_BYTES)
                    .await
                    .ok()?;
                let image = crate::cpu::run(crate::cpu::Service::Tiles, move || {
                    Ok(image::load_from_memory(&bytes)?.to_rgba8())
                })
                .await
                .ok()?;
                Some((x, y, image))
            });
        }
    }
    while let Some(result) = tasks.join_next().await {
        let Ok(Some((x, y, tile))) = result else {
            continue;
        };
        if tile.width() != TILE_SIZE || tile.height() != TILE_SIZE {
            continue;
        }
        accepted += 1;
        for (px, py, c) in tile.enumerate_pixels() {
            if px >= TILE_SIZE || py >= TILE_SIZE {
                continue;
            }
            let mapped = recolor_radar(c.0);
            image.put_pixel(
                x as u32 * TILE_SIZE + px,
                y as u32 * TILE_SIZE + py,
                image::Rgba(mapped),
            );
        }
    }
    if accepted == 0 {
        return Err("RainViewer radar tiles unavailable (check network/coverage)".into());
    }
    Ok(RadarFrame {
        image,
        time,
        failed_tiles: n * n - accepted,
    })
}

// The API only delivers Universal Blue as of 2026. Map its RGB palette to an
// approximate reflectivity (dBZ) and display a radar-style green-yellow-red ramp.
// This is an intensity visualization, not temperature or satellite infrared.
fn recolor_radar(src: [u8; 4]) -> [u8; 4] {
    if src[3] == 0 {
        return [0, 0, 0, 0];
    }
    const PALETTE: &[(f32, [u8; 3])] = &[
        (0.0, [0, 90, 0]),
        (5.0, [0, 126, 0]),
        (10.0, [8, 127, 219]),
        (15.0, [28, 71, 232]),
        (20.0, [110, 13, 198]),
        (25.0, [200, 15, 134]),
        (30.0, [192, 100, 135]),
        (35.0, [210, 136, 59]),
        (40.0, [250, 196, 49]),
        (45.0, [254, 251, 2]),
        (50.0, [254, 154, 88]),
        (55.0, [254, 95, 5]),
        (60.0, [253, 52, 28]),
        (65.0, [190, 190, 190]),
    ];
    let mut min_error = u32::MAX;
    let mut dbz = 0.0;
    for (value, rgb) in PALETTE {
        let err = (src[0] as i32 - rgb[0] as i32).pow(2)
            + (src[1] as i32 - rgb[1] as i32).pow(2)
            + (src[2] as i32 - rgb[2] as i32).pow(2);
        if (err as u32) < min_error {
            min_error = err as u32;
            dbz = *value;
        }
    }
    let (r, g, b) = if dbz < 18.0 {
        (42, 205, 60)
    } else if dbz < 28.0 {
        (155, 220, 35)
    } else if dbz < 38.0 {
        (247, 230, 20)
    } else if dbz < 47.0 {
        (255, 145, 0)
    } else {
        (240, 35, 30)
    };
    [r, g, b, src[3].min(200)]
}
