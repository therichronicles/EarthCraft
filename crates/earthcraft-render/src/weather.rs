//! Georeferenced cloud/radar sampling and compositing without GUI types.
use earthcraft_core::camera::Basis;
use emath::{Pos2, Rect};
use image::RgbaImage;
use rayon::prelude::*;
pub const SETTLE_DELAY: std::time::Duration = std::time::Duration::from_millis(250);
const MIN_PROJECTION_WIDTH_PX: usize = 160;
const MOVING_WEATHER_WIDTH_PX: usize = 480;
const MAX_WEATHER_SIDE_PX: usize = 960;
const MAX_TRAFFIC_SIDE_PX: usize = 800;
const TRAFFIC_OPACITY: f32 = 0.90;

pub fn weather_size(rect: Rect, pixels_per_point: f32, moving: bool) -> (usize, usize) {
    projection_size(
        rect,
        pixels_per_point,
        if moving {
            MOVING_WEATHER_WIDTH_PX
        } else {
            MAX_WEATHER_SIDE_PX
        },
        MAX_WEATHER_SIDE_PX,
    )
}
pub fn traffic_size(rect: Rect, pixels_per_point: f32) -> (usize, usize) {
    projection_size(
        rect,
        pixels_per_point,
        MAX_TRAFFIC_SIDE_PX,
        MAX_TRAFFIC_SIDE_PX,
    )
}
fn projection_size(
    rect: Rect,
    pixels_per_point: f32,
    max_width: usize,
    max_height: usize,
) -> (usize, usize) {
    let width =
        ((rect.width() * pixels_per_point) as usize).clamp(MIN_PROJECTION_WIDTH_PX, max_width);
    let height =
        ((width as f32 * rect.height() / rect.width().max(1.0)) as usize).clamp(1, max_height);
    (width, height)
}
const CLOUD_OPACITY: f32 = 0.78;
const RADAR_OPACITY: f32 = 0.88;
const RADAR_LATITUDE_LIMIT_DEG: f64 = 85.0;
pub fn project_live_maps(
    basis: &Basis,
    rect: Rect,
    width: usize,
    height: usize,
    cloud: Option<&RgbaImage>,
    radar: Option<&RgbaImage>,
) -> Vec<u8> {
    let mut rgba = vec![0u8; width * height * 4];
    let rsize = rect.size();
    if width == 0 || height == 0 {
        return rgba;
    }
    rgba.par_chunks_mut(width * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let sy = rect.top() + (y as f32 + 0.5) / height as f32 * rsize.y;
            for x in 0..width {
                let sx = rect.left() + (x as f32 + 0.5) / width as f32 * rsize.x;
                let Some((lat, lon)) = basis.pick(rect, Pos2::new(sx, sy)) else {
                    continue;
                };
                let i = x * 4;
                if cloud.is_some()
                    && let Some(img) = cloud
                {
                    let c = equirect_pixel(img, lat, lon);
                    over(
                        &mut row[i..i + 4],
                        [c[0], c[1], c[2], (c[3] as f32 * CLOUD_OPACITY) as u8],
                    );
                }
                if radar.is_some()
                    && lat.abs() < RADAR_LATITUDE_LIMIT_DEG
                    && let Some(img) = radar
                {
                    let r = mercator_pixel(img, lat, lon);
                    over(
                        &mut row[i..i + 4],
                        [r[0], r[1], r[2], (r[3] as f32 * RADAR_OPACITY) as u8],
                    );
                }
            }
        });
    rgba
}
fn equirect_pixel(img: &RgbaImage, lat: f64, lon: f64) -> [u8; 4] {
    let x = (((lon + 180.0) / 360.0 * img.width() as f64) as u32).min(img.width() - 1);
    let y = (((90.0 - lat) / 180.0 * img.height() as f64) as u32).min(img.height() - 1);
    img.get_pixel(x, y).0
}
fn mercator_pixel(img: &RgbaImage, lat: f64, lon: f64) -> [u8; 4] {
    let x = (((lon + 180.0) / 360.0 * img.width() as f64) as u32).min(img.width() - 1);
    let radians = lat.to_radians();
    let y = ((1.0 - radians.tan().asinh() / std::f64::consts::PI) / 2.0 * img.height() as f64)
        .clamp(0.0, (img.height() - 1) as f64) as u32;
    img.get_pixel(x, y).0
}
fn over(dst: &mut [u8], src: [u8; 4]) {
    let a = src[3] as f32 / 255.0;
    if a == 0.0 {
        return;
    }
    let previous = dst[3] as f32 / 255.0;
    let total = a + previous * (1.0 - a);
    if total == 0.0 {
        return;
    }
    for i in 0..3 {
        dst[i] = ((src[i] as f32 * a + dst[i] as f32 * previous * (1.0 - a)) / total) as u8;
    }
    dst[3] = (total * 255.0) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_coords() {
        let mut img = RgbaImage::new(360, 180);
        img.put_pixel(180, 90, image::Rgba([123, 45, 67, 255]));
        assert_eq!(equirect_pixel(&img, 0.0, 0.0), [123, 45, 67, 255]);
    }
    #[test]
    fn alpha_blend() {
        let mut pixel = [0u8; 4];
        over(&mut pixel, [0, 200, 0, 128]);
        assert!(pixel[1] > 190 && pixel[3] > 120);
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use earthcraft_core::camera::Cam;
    use emath::Vec2;
    #[test]
    fn cloud_raster_preserves_opacity_and_leaves_space_clear() {
        let rect = Rect::from_min_size(Pos2::new(30.0, 50.0), Vec2::splat(800.0));
        let basis = Cam {
            lat: 0.0,
            lon: 0.0,
            alt: 14_000.0,
            heading: 0.0,
        }
        .basis(1.0);
        let cloud = RgbaImage::from_pixel(4, 2, image::Rgba([123, 45, 67, 255]));
        let pixels = project_live_maps(&basis, rect, 9, 9, Some(&cloud), None);
        assert_eq!(
            &pixels[(4 * 9 + 4) * 4..(4 * 9 + 5) * 4],
            &[123, 45, 67, 198]
        );
        assert_eq!(&pixels[..4], &[0, 0, 0, 0]);
    }
}

/// Project provider's transparent 3x3 congestion raster onto the curved globe.
pub fn project_traffic_tiles(
    basis: &Basis,
    rect: Rect,
    width: usize,
    height: usize,
    tiles: &earthcraft_data::traffic::TrafficTiles,
) -> Vec<u8> {
    let mut out = vec![0; width * height * 4];
    if width == 0 || height == 0 {
        return out;
    }
    out.par_chunks_mut(width * 4)
        .enumerate()
        .for_each(|(py, row)| {
            let sy = rect.top() + (py as f32 + 0.5) * rect.height() / height as f32;
            for px in 0..width {
                let sx = rect.left() + (px as f32 + 0.5) * rect.width() / width as f32;
                if let Some((lat, lon)) = basis.pick(rect, Pos2::new(sx, sy))
                    && let Some(mut rgba) = tiles.sample(lat, lon)
                {
                    rgba[3] = (rgba[3] as f32 * TRAFFIC_OPACITY) as u8;
                    row[px * 4..px * 4 + 4].copy_from_slice(&rgba);
                }
            }
        });
    out
}

#[cfg(test)]
mod parallel_tests {
    use super::*;
    #[test]
    fn parallel_rows_match_single_thread_pixels_and_sizing_is_bounded() {
        let rect = Rect::from_min_size(Pos2::ZERO, emath::vec2(2000.0, 1000.0));
        let basis = earthcraft_core::camera::Cam {
            lat: 20.0,
            lon: 30.0,
            heading: 45.0,
            alt: 14000.0,
        }
        .basis(2.0);
        let cloud = RgbaImage::from_pixel(4, 2, image::Rgba([100, 120, 140, 255]));
        let radar = RgbaImage::from_pixel(4, 4, image::Rgba([50, 80, 60, 128]));
        let project = || project_live_maps(&basis, rect, 65, 33, Some(&cloud), Some(&radar));
        let single = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(project);
        let parallel = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(project);
        assert_eq!(single, parallel);
        assert_eq!(weather_size(rect, 2.0, true), (480, 240));
        assert_eq!(weather_size(rect, 2.0, false), (960, 480));
        assert_eq!(traffic_size(rect, 2.0), (800, 400));
    }
}
