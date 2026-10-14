//! Raster resolution and compatibility keys; event timing stays in the frontend.
use earthcraft_core::camera::Cam;
use earthcraft_data::tiles::Provider;
use emath::Vec2;
use std::hash::{Hash, Hasher};
const IDLE_RENDER_SCALE: f32 = 1.0;
const ACTIVE_RENDER_SCALE: f32 = 0.4;
const MIN_RENDER_WIDTH_PX: usize = 64;
const MAX_RENDER_WIDTH_PX: usize = 2000;
const MIN_RENDER_HEIGHT_PX: usize = 32;
const SUN_UPDATE_INTERVAL_SECS: f64 = 60.0;
pub fn raster_size(size_points: Vec2, pixels_per_point: f32, idle: bool) -> (usize, usize) {
    let scale = if idle {
        IDLE_RENDER_SCALE
    } else {
        ACTIVE_RENDER_SCALE
    };
    let width = ((size_points.x * pixels_per_point * scale) as usize)
        .clamp(MIN_RENDER_WIDTH_PX, MAX_RENDER_WIDTH_PX);
    let height =
        ((width as f32 * size_points.y / size_points.x) as usize).max(MIN_RENDER_HEIGHT_PX);
    (width, height)
}
pub struct RasterView {
    pub camera: Cam,
    pub model: earthcraft_core::earth_model::EarthModel,
    pub width: usize,
    pub height: usize,
    pub aspect: f64,
    pub provider: Provider,
    pub release_index: usize,
    pub grid: bool,
    pub atmosphere: bool,
    pub sunlight: bool,
}
pub struct ViewKeys {
    pub state: u64,
    pub presentation: u64,
}
impl RasterView {
    pub fn keys(&self, unix_secs: f64) -> ViewKeys {
        let sun_key = if self.sunlight {
            (unix_secs / SUN_UPDATE_INTERVAL_SECS) as u64
        } else {
            0
        };
        let mut state = std::collections::hash_map::DefaultHasher::new();
        (
            (
                self.camera.lat.to_bits(),
                self.camera.lon.to_bits(),
                self.camera.heading.to_bits(),
                self.camera.alt.to_bits(),
                self.width,
                self.height,
                self.provider.id(),
                self.grid,
            ),
            (self.atmosphere, sun_key, self.release_index),
            (self.aspect.to_bits(), self.model.code()),
        )
            .hash(&mut state);
        let mut presentation = std::collections::hash_map::DefaultHasher::new();
        (
            self.width,
            self.height,
            self.aspect.to_bits(),
            self.provider.id(),
            self.release_index,
            self.grid,
            self.atmosphere,
            sun_key,
            self.model.code(),
        )
            .hash(&mut presentation);
        ViewKeys {
            state: state.finish(),
            presentation: presentation.finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn motion_changes_state_but_preserves_presentation_compatibility() {
        let mut view = RasterView {
            model: earthcraft_core::earth_model::EarthModel::PerfectSphere,
            camera: Cam {
                lat: 0.0,
                lon: 0.0,
                alt: 14_000.0,
                heading: 0.0,
            },
            width: 800,
            height: 600,
            aspect: 4.0 / 3.0,
            provider: Provider::Satellite,
            release_index: 0,
            grid: false,
            atmosphere: true,
            sunlight: true,
        };
        let original = view.keys(120.0);
        view.model = earthcraft_core::earth_model::EarthModel::Wgs84;
        assert_ne!(view.keys(120.0).presentation, original.presentation);
        assert_ne!(view.keys(120.0).state, original.state);
        view.model = earthcraft_core::earth_model::EarthModel::PerfectSphere;
        view.camera.lon = 90.0;
        let moved = view.keys(120.0);
        assert_ne!(moved.state, original.state);
        assert_eq!(moved.presentation, original.presentation);
        assert_ne!(view.keys(180.0).presentation, moved.presentation);
        view.grid = true;
        assert_ne!(view.keys(120.0).presentation, moved.presentation);
        let before_resize = view.keys(120.0);
        // Raster dimensions can round to the same pixels even as the viewport changes.
        view.aspect += 0.0001;
        assert_ne!(view.keys(120.0).state, before_resize.state);
        assert_ne!(view.keys(120.0).presentation, before_resize.presentation);
    }
    #[test]
    fn interaction_resolution_and_size_limits_are_preserved() {
        assert_eq!(
            raster_size(Vec2::new(1280.0, 800.0), 1.0, true),
            (1280, 800)
        );
        assert_eq!(
            raster_size(Vec2::new(1280.0, 800.0), 1.0, false),
            (512, 320)
        );
        assert_eq!(
            raster_size(Vec2::new(4000.0, 2000.0), 2.0, true),
            (2000, 1000)
        );
    }
}
