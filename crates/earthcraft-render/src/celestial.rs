//! Shared CPU/GPU-repair sampling behind the globe; no texture/backend ownership.
use earthcraft_core::{
    camera::Cam,
    celestial::{CelestialFrame, CelestialTimeError, equatorial_uv},
    math::ll_to_v3,
};
use earthcraft_data::sky::SkyImage;
use glam::{DVec3, Vec4};
use std::sync::Arc;
const TWILIGHT_DARK_ALTITUDE_DEG: f64 = -18.0;
const TWILIGHT_LIGHT_ALTITUDE_DEG: f64 = -6.0;
const FULL_ORBIT_VISIBILITY_ALTITUDE_KM: f64 = 100.0;
#[derive(Clone)]
pub struct SkyBackdrop {
    image: Arc<SkyImage>,
    frame: CelestialFrame,
    visibility: f32,
}
impl SkyBackdrop {
    pub fn new(
        image: Arc<SkyImage>,
        unix_secs: f64,
        camera: Cam,
        sun: DVec3,
        cinematic: bool,
    ) -> Result<Self, CelestialTimeError> {
        Ok(Self {
            image,
            frame: CelestialFrame::at_unix(unix_secs)?,
            visibility: visibility(camera, sun, cinematic),
        })
    }
    pub(super) fn sample(&self, earth_fixed: DVec3) -> Vec4 {
        let uv = equatorial_uv(self.frame.direction(earth_fixed));
        bilinear(self.image.pixels(), uv.x, uv.y) * self.visibility
    }
}
fn visibility(camera: Cam, sun: DVec3, cinematic: bool) -> f32 {
    if cinematic {
        return 1.0;
    }
    let sun_altitude_deg = ll_to_v3(camera.lat, camera.lon)
        .dot(sun)
        .clamp(-1.0, 1.0)
        .asin()
        .to_degrees();
    let twilight = ((TWILIGHT_LIGHT_ALTITUDE_DEG - sun_altitude_deg)
        / (TWILIGHT_LIGHT_ALTITUDE_DEG - TWILIGHT_DARK_ALTITUDE_DEG))
        .clamp(0.0, 1.0);
    let orbit = (camera.alt / FULL_ORBIT_VISIBILITY_ALTITUDE_KM).clamp(0.0, 1.0);
    (twilight + (1.0 - twilight) * orbit) as f32
}
fn bilinear(image: &image::RgbImage, u: f64, v: f64) -> Vec4 {
    let (width, height) = image.dimensions();
    let x = u * f64::from(width) - 0.5;
    let y = (v * f64::from(height) - 0.5).clamp(0.0, f64::from(height - 1));
    let x0 = x.floor() as i64;
    let y0 = y.floor() as u32;
    let pixel = |x: i64, y: u32| {
        let [r, g, b] = image
            .get_pixel(x.rem_euclid(i64::from(width)) as u32, y.min(height - 1))
            .0;
        Vec4::new(f32::from(r), f32::from(g), f32::from(b), 0.0) / 255.0
    };
    let top = pixel(x0, y0).lerp(pixel(x0 + 1, y0), x.fract().rem_euclid(1.0) as f32);
    let bottom = pixel(x0, y0 + 1).lerp(pixel(x0 + 1, y0 + 1), x.fract().rem_euclid(1.0) as f32);
    top.lerp(bottom, y.fract() as f32)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampling_wraps_without_a_dark_seam_and_clamps_poles() {
        let image =
            image::RgbImage::from_fn(4, 2, |x, y| image::Rgb([x as u8 * 50, y as u8 * 100, 0]));
        assert!(bilinear(&image, 0.0, 0.5).abs_diff_eq(bilinear(&image, 1.0, 0.5), 1e-7));
        assert!(bilinear(&image, 1e-10, 0.5).abs_diff_eq(bilinear(&image, 1.0 - 1e-10, 0.5), 1e-7));
        assert_eq!(bilinear(&image, 0.5, 0.0).y, 0.0);
        assert_eq!(bilinear(&image, 0.5, 1.0).y, 100.0 / 255.0);
        assert!((bilinear(&image, 0.5, 0.5).x - 75.0 / 255.0).abs() < 1e-7);
    }
    #[test]
    fn realistic_daylight_fades_at_the_surface_and_cinematic_is_explicit() {
        let mut camera = Cam {
            tilt: 0.0,
            lat: 0.0,
            lon: 0.0,
            alt: 0.15,
            heading: 0.0,
        };
        assert!(visibility(camera, DVec3::X, false) < 0.002);
        assert_eq!(visibility(camera, -DVec3::X, false), 1.0);
        assert_eq!(visibility(camera, DVec3::X, true), 1.0);
        camera.alt = 100.0;
        assert_eq!(visibility(camera, DVec3::X, false), 1.0);
    }
}
