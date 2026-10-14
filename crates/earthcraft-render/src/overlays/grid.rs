//! Bounded label candidates for the raster's latitude/longitude graticule.
use earthcraft_core::{camera::Basis, math::wrap_lon};
use emath::{Pos2, Rect};
use glam::DVec3;
const VIEW_SAMPLES: usize = 16;
const LINE_SAMPLES: usize = 96;
const MAX_LINES_PER_AXIS: usize = 64;
const LABEL_INSET_POINTS: f32 = 18.0;
const GUIDE_INSET_RATIO: f32 = 0.15;
const MAX_GUIDE_INSET_POINTS: f32 = 48.0;
const CROSSING_REFINEMENT_STEPS: usize = 20;
const ORIENTATION_TIE_EPSILON: f64 = 1e-9;
const DUPLICATE_STEP_RATIO: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridAxis {
    Latitude,
    Longitude,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridLabelEdge {
    Left,
    Bottom,
}
#[derive(Clone, Copy, Debug)]
pub struct GridLabel {
    pub position: Pos2,
    pub degrees: f64,
    pub axis: GridAxis,
    pub edge: GridLabelEdge,
}

/// Uses the displayed raster's exact grid step. Longitudes are unwrapped about
/// its center so a dateline view cannot expand into a full-world scan.
pub fn project_grid_labels(basis: &Basis, rect: Rect, step_deg: f64) -> Vec<GridLabel> {
    if !step_deg.is_finite()
        || !(crate::MIN_GRID_STEP_DEG..=180.0).contains(&step_deg)
        || rect.width() <= LABEL_INSET_POINTS * 2.0
        || rect.height() <= LABEL_INSET_POINTS * 2.0
    {
        return vec![];
    }
    let (center_lat, center_lon) = basis.geographic_center();
    let (lat, lon) = (center_lat.to_radians(), center_lon.to_radians());
    let east = DVec3::new(-lon.sin(), lon.cos(), 0.0);
    let north = DVec3::new(-lat.sin() * lon.cos(), -lat.sin() * lon.sin(), lat.cos());
    let screen_up = basis.rotation_from_view().y_axis;
    let latitude_on_left =
        north.dot(screen_up).abs() + ORIENTATION_TIE_EPSILON >= east.dot(screen_up).abs();
    let mut visible = Rect::NOTHING;
    let mut lat_min = f64::INFINITY;
    let mut lat_max = f64::NEG_INFINITY;
    let mut lon_min = f64::INFINITY;
    let mut lon_max = f64::NEG_INFINITY;
    for row in 0..=VIEW_SAMPLES {
        for col in 0..=VIEW_SAMPLES {
            let pos = Pos2::new(
                rect.left() + rect.width() * col as f32 / VIEW_SAMPLES as f32,
                rect.top() + rect.height() * row as f32 / VIEW_SAMPLES as f32,
            );
            if let Some((lat, lon)) = basis.pick(rect, pos) {
                visible.extend_with(pos);
                let lon = center_lon + wrap_lon(lon - center_lon);
                lat_min = lat_min.min(lat);
                lat_max = lat_max.max(lat);
                lon_min = lon_min.min(lon);
                lon_max = lon_max.max(lon);
            }
        }
    }
    if !lat_min.is_finite() {
        return vec![];
    }
    lat_min = (lat_min - step_deg).max(-90.0);
    lat_max = (lat_max + step_deg).min(90.0);
    lon_min = (lon_min - step_deg).max(center_lon - 180.0);
    lon_max = (lon_max + step_deg).min(center_lon + 180.0);
    let inside = rect.shrink(LABEL_INSET_POINTS);
    let left_guide =
        visible.left() + (visible.width() * GUIDE_INSET_RATIO).min(MAX_GUIDE_INSET_POINTS);
    let bottom_guide =
        visible.bottom() - (visible.height() * GUIDE_INSET_RATIO).min(MAX_GUIDE_INSET_POINTS);
    let mut labels = Vec::new();
    for (axis, min, max) in [
        (GridAxis::Latitude, lat_min, lat_max),
        (GridAxis::Longitude, lon_min, lon_max),
    ] {
        let first = (min / step_deg).ceil() as i64;
        let last = (max / step_deg).floor() as i64;
        let count = (last - first + 1).max(0) as usize;
        // If a polar view spans many meridians, retain a stable subset of grid lines.
        let stride = count.div_ceil(MAX_LINES_PER_AXIS).max(1);
        for index in (first..=last).step_by(stride) {
            let degrees = index as f64 * step_deg;
            if axis == GridAxis::Latitude && degrees.abs() >= 90.0 {
                continue;
            }
            let edge = if (axis == GridAxis::Latitude) == latitude_on_left {
                GridLabelEdge::Left
            } else {
                GridLabelEdge::Bottom
            };
            let point_at = |t: f64| {
                let (lat, lon) = match axis {
                    GridAxis::Latitude => (degrees, lon_min + (lon_max - lon_min) * t),
                    GridAxis::Longitude => (lat_min + (lat_max - lat_min) * t, degrees),
                };
                basis.project(basis.surface_position(lat, wrap_lon(lon)), rect)
            };
            let signed_distance = |pos: Pos2| match edge {
                GridLabelEdge::Left => pos.x - left_guide,
                GridLabelEdge::Bottom => pos.y - bottom_guide,
            };
            let mut best: Option<(f32, Pos2)> = None;
            let mut previous = point_at(0.0).map(|pos| (0.0, pos));
            for sample in 1..=LINE_SAMPLES {
                let t = sample as f64 / LINE_SAMPLES as f64;
                let current = point_at(t);
                if let (Some((previous_t, previous_pos)), Some(pos)) = (previous, current) {
                    let from_distance = signed_distance(previous_pos);
                    let to_distance = signed_distance(pos);
                    if (from_distance <= 0.0 && to_distance >= 0.0)
                        || (from_distance >= 0.0 && to_distance <= 0.0)
                    {
                        let (mut low, mut high) = (previous_t, t);
                        for _ in 0..CROSSING_REFINEMENT_STEPS {
                            let mid = (low + high) * 0.5;
                            let Some(mid_pos) = point_at(mid) else {
                                break;
                            };
                            if (signed_distance(mid_pos) < 0.0) == (from_distance < 0.0) {
                                low = mid;
                            } else {
                                high = mid;
                            }
                        }
                        if let Some(position) =
                            point_at((low + high) * 0.5).filter(|pos| inside.contains(*pos))
                        {
                            let distance = position.distance_sq(rect.center());
                            if best.is_none_or(|(old, _)| distance < old) {
                                best = Some((distance, position));
                            }
                        }
                    }
                }
                previous = current.map(|pos| (t, pos));
            }
            if let Some((_, position)) = best {
                let degrees = if axis == GridAxis::Longitude {
                    wrap_lon(degrees)
                } else {
                    degrees
                };
                if !labels.iter().any(|label: &GridLabel| {
                    label.axis == axis
                        && (label.degrees - degrees).abs() < step_deg * DUPLICATE_STEP_RATIO
                }) {
                    labels.push(GridLabel {
                        position,
                        degrees,
                        axis,
                        edge,
                    });
                }
            }
        }
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::camera::Cam;
    #[test]
    fn labels_follow_visible_grid_lines_across_headings_dateline_and_poles() {
        let rect = Rect::from_min_size(Pos2::ZERO, emath::vec2(800.0, 600.0));
        for (lat, lon, heading, alt) in [
            (0.0, 0.0, 0.0, 14000.0),
            (0.0, 179.0, 90.0, 500.0),
            (89.0, 30.0, 135.0, 100.0),
            (37.0, -122.0, 45.0, 0.15),
        ] {
            let basis = Cam {
                lat,
                lon,
                heading,
                alt,
            }
            .basis(800.0 / 600.0);
            let step = crate::grid_step(alt, 800, 600);
            let labels = project_grid_labels(&basis, rect, step);
            assert!(!labels.is_empty(), "{lat} {lon} {alt}");
            assert!(labels.len() <= MAX_LINES_PER_AXIS * 2);
            for edge in [GridLabelEdge::Left, GridLabelEdge::Bottom] {
                let positions: Vec<_> = labels
                    .iter()
                    .filter(|label| label.edge == edge)
                    .map(|label| match edge {
                        GridLabelEdge::Left => label.position.x,
                        GridLabelEdge::Bottom => label.position.y,
                    })
                    .collect();
                if let Some(first) = positions.first() {
                    assert!(
                        positions.iter().all(|value| (value - first).abs() < 0.01),
                        "labels share a guide: {positions:?}"
                    );
                }
            }
            for label in labels {
                assert!(rect.contains(label.position));
                let picked = basis.pick(rect, label.position).unwrap();
                let error = match label.axis {
                    GridAxis::Latitude => (picked.0 - label.degrees).abs(),
                    GridAxis::Longitude => wrap_lon(picked.1 - label.degrees).abs(),
                };
                // Projection stores screen positions as f32; polar longitude amplifies rounding.
                assert!(error < step * 0.02, "{label:?} error {error} step {step}");
            }
        }
        for step in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                project_grid_labels(
                    &Cam {
                        lat: 0.0,
                        lon: 0.0,
                        heading: 0.0,
                        alt: 100.0
                    }
                    .basis(1.0),
                    rect,
                    step
                )
                .is_empty()
            );
        }
    }
}
