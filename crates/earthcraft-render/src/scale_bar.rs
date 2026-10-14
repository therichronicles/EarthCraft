//! Scale-bar distance selection, independent of drawing.
use earthcraft_core::{METERS_PER_KM, VERTICAL_FOV_HALF_TAN, units::DistanceUnits};
const MAX_ALTITUDE_KM: f64 = 3000.0;
const TARGET_WIDTH_POINTS: f64 = 120.0;
#[derive(Clone, Copy)]
pub struct ScaleBar {
    pub distance_m: f64,
    pub width_points: f32,
}
impl ScaleBar {
    pub fn label(&self) -> String {
        if self.distance_m >= METERS_PER_KM {
            format!("{} km", self.distance_m / METERS_PER_KM)
        } else {
            format!("{} m", self.distance_m)
        }
    }
}

/// The experimental ellipsoid bar measures the actual surface under its screen
/// endpoints. The default sphere retains its original altitude-based policy.
fn scale_bar_for_view(
    basis: &earthcraft_core::camera::Basis,
    rect: emath::Rect,
    altitude_km: f64,
    units: DistanceUnits,
) -> Option<ScaleBar> {
    if basis.model == earthcraft_core::earth_model::EarthModel::PerfectSphere {
        return scale_bar_with_units(altitude_km, rect.height(), units);
    }
    if altitude_km >= MAX_ALTITUDE_KM {
        return None;
    }
    let anchor = emath::pos2(
        rect.left() + SCALE_LEFT_INSET_POINTS,
        rect.bottom() - SCALE_BOTTOM_INSET_POINTS,
    );
    let start = basis.pick(rect, anchor)?;
    let end = basis.pick(rect, anchor + emath::vec2(TARGET_WIDTH_POINTS as f32, 0.0))?;
    let maximum_m = basis.model.distance_km(start, end) * METERS_PER_KM;
    if !maximum_m.is_finite() || maximum_m <= 0.0 {
        return None;
    }
    let unit_m = units.scale_unit_meters(maximum_m);
    let target = maximum_m / unit_m;
    let pow = 10f64.powf(target.log10().floor());
    let distance_m = [5.0, 2.0, 1.0]
        .into_iter()
        .map(|multiple| multiple * pow)
        .find(|value| *value <= target)?
        * unit_m;
    let (mut low, mut high) = (0.0, TARGET_WIDTH_POINTS as f32);
    const WIDTH_REFINEMENT_STEPS: usize = 24;
    for _ in 0..WIDTH_REFINEMENT_STEPS {
        let middle = (low + high) * 0.5;
        let end = basis.pick(rect, anchor + emath::vec2(middle, 0.0))?;
        if basis.model.distance_km(start, end) * METERS_PER_KM < distance_m {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some(ScaleBar {
        distance_m,
        width_points: (low + high) * 0.5,
    })
}
pub const SCALE_LEFT_INSET_POINTS: f32 = 18.0;
pub const SCALE_BOTTOM_INSET_POINTS: f32 = 28.0;

#[derive(Clone, Copy, PartialEq)]
struct ScaleView {
    basis: earthcraft_core::camera::Basis,
    rect: emath::Rect,
    altitude_km: f64,
    units: DistanceUnits,
}
#[derive(Default)]
pub struct ScaleBarCache {
    latest: Option<(ScaleView, Option<ScaleBar>)>,
}
impl ScaleBarCache {
    pub fn for_view(
        &mut self,
        basis: &earthcraft_core::camera::Basis,
        rect: emath::Rect,
        altitude_km: f64,
        units: DistanceUnits,
    ) -> Option<ScaleBar> {
        let view = ScaleView {
            basis: *basis,
            rect,
            altitude_km,
            units,
        };
        if let Some((old, result)) = &self.latest
            && *old == view
        {
            return *result;
        }
        let result = scale_bar_for_view(basis, rect, altitude_km, units);
        self.latest = Some((view, result));
        result
    }
}
pub fn scale_bar(altitude_km: f64, height_points: f32) -> Option<ScaleBar> {
    scale_bar_with_units(altitude_km, height_points, DistanceUnits::Metric)
}
/// Select a round displayed distance in the requested unit system.
pub fn scale_bar_with_units(
    altitude_km: f64,
    height_points: f32,
    units: DistanceUnits,
) -> Option<ScaleBar> {
    if altitude_km >= MAX_ALTITUDE_KM {
        return None;
    }
    let mpp = altitude_km * METERS_PER_KM * 2.0 * VERTICAL_FOV_HALF_TAN / height_points as f64;
    let unit_m = units.scale_unit_meters(mpp * TARGET_WIDTH_POINTS);
    let target = mpp * TARGET_WIDTH_POINTS / unit_m;
    let pow = 10f64.powf(target.log10().floor());
    let distance_m = [5.0, 2.0, 1.0]
        .iter()
        .map(|m| m * pow)
        .find(|v| *v <= target)
        .unwrap_or(pow);
    Some(ScaleBar {
        distance_m: distance_m * unit_m,
        width_points: (distance_m * unit_m / mpp) as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ellipsoid_bar_measures_its_actual_screen_endpoints() {
        use earthcraft_core::{camera::Cam, earth_model::EarthModel};
        let rect = emath::Rect::from_min_size(emath::Pos2::ZERO, emath::vec2(800.0, 600.0));
        let mut cache = ScaleBarCache::default();
        for units in [DistanceUnits::Metric, DistanceUnits::Imperial] {
            for altitude in [1.0, 100.0, 1000.0] {
                let basis = Cam {
                    lat: 45.0,
                    lon: 179.9,
                    heading: 75.0,
                    alt: altitude,
                }
                .basis_for_model(4.0 / 3.0, EarthModel::Wgs84);
                let bar = cache.for_view(&basis, rect, altitude, units).unwrap();
                let anchor = emath::pos2(
                    SCALE_LEFT_INSET_POINTS,
                    rect.bottom() - SCALE_BOTTOM_INSET_POINTS,
                );
                let a = basis.pick(rect, anchor).unwrap();
                let b = basis
                    .pick(rect, anchor + emath::vec2(bar.width_points, 0.0))
                    .unwrap();
                let actual_m = basis.model.distance_km(a, b) * METERS_PER_KM;
                assert!(
                    (actual_m - bar.distance_m).abs() < bar.distance_m * 1e-6,
                    "{actual_m} vs {}",
                    bar.distance_m
                );
                assert_eq!(
                    cache
                        .for_view(&basis, rect, altitude, units)
                        .unwrap()
                        .width_points,
                    bar.width_points
                );
            }
        }
    }
    #[test]
    fn imperial_scales_use_round_distances_at_bounded_screen_widths() {
        for altitude in [0.15, 1.0, 10.0, 100.0, 1000.0, 2999.0] {
            let scale = scale_bar_with_units(altitude, 800.0, DistanceUnits::Imperial).unwrap();
            assert!(scale.width_points > 0.0 && scale.width_points <= TARGET_WIDTH_POINTS as f32);
            let label = DistanceUnits::Imperial.scale_label(scale.distance_m);
            assert!(label.ends_with(" ft") || label.ends_with(" mi"), "{label}");
        }
    }
    #[test]
    fn familiar_distance_and_visibility_cutoff() {
        let scale = scale_bar(1.0, 800.0).unwrap();
        assert_eq!(scale.distance_m, 100.0);
        assert_eq!(scale.label(), "100 m");
        assert!(scale.width_points > 0.0 && scale.width_points <= 120.0);
        assert!(scale_bar(3000.0, 800.0).is_none());
    }
}
