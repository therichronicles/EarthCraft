//! Static geographic bounds; screen-space label admission remains in `labels`.
use earthcraft_core::{VERTICAL_FOV_HALF_TAN, camera::Basis};
use earthcraft_data::boundaries::{BoundaryData, Line};
use emath::Rect;
use geo_index::rtree::{RTree, RTreeBuilder, RTreeIndex, sort::HilbertSort};
use glam::DVec3;
use std::{collections::HashSet, sync::Arc};

const BOUNDS_EPSILON: f64 = 1e-12;
const VIEW_MARGIN_POINTS: f64 = 8.0;

#[derive(Clone, Copy)]
struct Bounds {
    min: DVec3,
    max: DVec3,
}
impl Bounds {
    fn line(line: &Line) -> Self {
        let mut bounds = Self {
            min: DVec3::splat(1.0),
            max: DVec3::splat(-1.0),
        };
        for pair in line.array_windows::<2>() {
            let ((lat1, lon1), (lat2, lon2)) = (pair[0], pair[1]);
            if ![lat1, lon1, lat2, lon2].iter().all(|v| v.is_finite()) {
                return Self {
                    min: DVec3::splat(-1.0),
                    max: DVec3::ONE,
                };
            }
            let mut dlon = lon2 - lon1;
            if dlon > 180.0 {
                dlon -= 360.0;
            }
            if dlon < -180.0 {
                dlon += 360.0;
            }
            let latitude = (lat1.min(lat2), lat1.max(lat2));
            let longitude = (lon1.min(lon1 + dlon), lon1.max(lon1 + dlon));
            // Bound the entire latitude/longitude interpolation, including extrema
            // between vertices. Products deliberately overestimate correlated terms.
            let cos_lat = trig_range(latitude, true);
            let x = product(cos_lat, trig_range(longitude, true));
            let y = product(cos_lat, trig_range(longitude, false));
            let z = trig_range(latitude, false);
            bounds.min = bounds.min.min(DVec3::new(x.0, y.0, z.0));
            bounds.max = bounds.max.max(DVec3::new(x.1, y.1, z.1));
        }
        bounds.min -= DVec3::splat(BOUNDS_EPSILON);
        bounds.max += DVec3::splat(BOUNDS_EPSILON);
        bounds
    }
    fn reaches(self, normal: DVec3, threshold: f64) -> bool {
        let farthest = DVec3::new(
            if normal.x >= 0.0 {
                self.max.x
            } else {
                self.min.x
            },
            if normal.y >= 0.0 {
                self.max.y
            } else {
                self.min.y
            },
            if normal.z >= 0.0 {
                self.max.z
            } else {
                self.min.z
            },
        );
        farthest.dot(normal) + BOUNDS_EPSILON >= threshold
    }
    fn visible(self, basis: &Basis, rect: Rect) -> bool {
        let geometry = basis.model.geometry();
        let margin = geometry.displacement_bound();
        let bounds = Self {
            min: self.min - DVec3::splat(margin),
            max: self.max + DVec3::splat(margin),
        };
        let horizon_camera = basis.c * geometry.inverse_axes_squared();
        if !bounds.reaches(horizon_camera, 1.0) {
            return false;
        }
        let rotation = basis.rotation_from_view();
        let forward = -rotation.z_axis;
        let tx = VERTICAL_FOV_HALF_TAN
            * basis.aspect_ratio()
            * (1.0 + 2.0 * VIEW_MARGIN_POINTS / rect.width().max(1.0) as f64);
        let ty = VERTICAL_FOV_HALF_TAN
            * (1.0 + 2.0 * VIEW_MARGIN_POINTS / rect.height().max(1.0) as f64);
        [
            forward * tx + rotation.x_axis,
            forward * tx - rotation.x_axis,
            forward * ty + rotation.y_axis,
            forward * ty - rotation.y_axis,
        ]
        .iter()
        .all(|normal| bounds.reaches(*normal, basis.c.dot(*normal)))
    }
}
fn product(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let values = [a.0 * b.0, a.0 * b.1, a.1 * b.0, a.1 * b.1];
    (
        values.into_iter().fold(f64::INFINITY, f64::min),
        values.into_iter().fold(f64::NEG_INFINITY, f64::max),
    )
}
fn trig_range((low, high): (f64, f64), cosine: bool) -> (f64, f64) {
    if high - low >= 360.0 {
        return (-1.0, 1.0);
    }
    let f = |v: f64| {
        if cosine {
            v.to_radians().cos()
        } else {
            v.to_radians().sin()
        }
    };
    let (mut min, mut max) = (f(low).min(f(high)), f(low).max(f(high)));
    let offset = if cosine { 0.0 } else { 90.0 };
    for extremum in
        ((low - offset) / 180.0).ceil() as i64..=((high - offset) / 180.0).floor() as i64
    {
        let value = if extremum % 2 == 0 { 1.0 } else { -1.0 };
        min = min.min(value);
        max = max.max(value);
    }
    (min, max)
}

#[derive(Debug, PartialEq, Eq)]
pub struct VisibleBoundaries {
    ids: HashSet<usize>,
}
impl VisibleBoundaries {
    pub(super) fn contains_line(&self, line: &Line) -> bool {
        self.ids.contains(&(Arc::as_ptr(line) as usize))
    }
}

pub struct BoundaryIndex {
    tree: Option<RTree<f64>>,
    entries: Vec<(usize, Bounds)>,
}
impl BoundaryIndex {
    pub fn new(data: &BoundaryData) -> Self {
        let entries: Vec<_> = [
            &data.countries,
            &data.states,
            &data.states_big,
            &data.cities,
            &data.world_cities,
            &data.metro,
        ]
        .into_iter()
        .flatten()
        .filter(|line| line.len() >= 2)
        .map(|line| (Arc::as_ptr(line) as usize, Bounds::line(line)))
        .collect();
        let tree = if entries.is_empty() {
            None
        } else {
            let mut builder = RTreeBuilder::new(
                u32::try_from(entries.len()).expect("boundary count fits in u32"),
            );
            for (_, bounds) in &entries {
                builder.add(bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y);
            }
            Some(builder.finish::<HilbertSort>())
        };
        Self { tree, entries }
    }
    pub fn visible(&self, basis: &Basis, rect: Rect) -> VisibleBoundaries {
        // The horizon's spherical cap lies inside this chord-radius box. The
        // packed 2D tree supplies candidates; 3D plane tests tighten the result.
        let geometry = basis.model.geometry();
        let horizon_camera = basis.c * geometry.inverse_axes_squared();
        let center = horizon_camera.normalize();
        let threshold =
            (1.0 / horizon_camera.length() - geometry.displacement_bound()).clamp(-1.0, 1.0);
        let radius = (2.0 - 2.0 * threshold).max(0.0).sqrt();
        let Some(tree) = &self.tree else {
            return VisibleBoundaries {
                ids: HashSet::new(),
            };
        };
        let ids = tree
            .search(
                center.x - radius,
                center.y - radius,
                center.x + radius,
                center.y + radius,
            )
            .into_iter()
            .filter_map(|id| {
                let (pointer, bounds) = self.entries[id as usize];
                bounds.visible(basis, rect).then_some(pointer)
            })
            .collect();
        VisibleBoundaries { ids }
    }
}

#[cfg(test)]
mod tests;
