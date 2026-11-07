//! Line and polygon-outline geometry attached to a placemark (for example a
//! fiber route). Vertices are stored once, shared on clone, with the bounds the
//! renderer needs to cull and decimate without scanning every vertex.
use crate::math::{ll_to_v3, v3_to_ll};
use std::sync::Arc;

/// One polyline or closed ring in geographic degrees.
#[derive(Clone, Debug)]
pub struct GeoPath {
    closed: bool,
    /// `(lat, lon)` degrees; every vertex is finite and within range.
    points: Arc<[(f64, f64)]>,
    center: (f64, f64),
    radius: f64,
    length: f64,
}
impl PartialEq for GeoPath {
    fn eq(&self, other: &Self) -> bool {
        self.closed == other.closed && self.points == other.points
    }
}

impl GeoPath {
    /// Build a path, dropping vertices that are not valid coordinates. Returns
    /// `None` when fewer than two valid vertices remain.
    pub fn new(closed: bool, vertices: impl IntoIterator<Item = (f64, f64)>) -> Option<Self> {
        let points: Vec<(f64, f64)> = vertices
            .into_iter()
            .filter(|&(lat, lon)| {
                lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0
            })
            .collect();
        if points.len() < 2 {
            return None;
        }
        let units: Vec<_> = points
            .iter()
            .map(|&(lat, lon)| ll_to_v3(lat, lon))
            .collect();
        let sum = units
            .iter()
            .fold(glam::DVec3::ZERO, |sum, &unit| sum + unit);
        let mean = if sum.length_squared() > 1e-18 {
            sum.normalize()
        } else {
            units[0]
        };
        let radius = units
            .iter()
            .map(|unit| mean.dot(*unit).clamp(-1.0, 1.0).acos())
            .fold(0.0_f64, f64::max);
        let length = units
            .windows(2)
            .map(|pair| pair[0].dot(pair[1]).clamp(-1.0, 1.0).acos())
            .sum();
        Some(Self {
            closed,
            center: v3_to_ll(mean),
            radius,
            length,
            points: points.into(),
        })
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }
    /// Mean position `(lat, lon)` of the vertices.
    pub fn center(&self) -> (f64, f64) {
        self.center
    }
    /// Largest angle in radians from `center` to any vertex.
    pub fn radius(&self) -> f64 {
        self.radius
    }
    /// Total great-circle length in radians (closing segment excluded).
    pub fn length(&self) -> f64 {
        self.length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_vertices_are_dropped_and_short_paths_rejected() {
        assert!(GeoPath::new(false, [(0.0, 0.0)]).is_none());
        assert!(GeoPath::new(false, [(0.0, 0.0), (95.0, 0.0), (f64::NAN, 1.0)]).is_none());
        let path = GeoPath::new(false, [(0.0, 0.0), (95.0, 0.0), (0.0, 1.0)]).unwrap();
        assert_eq!(path.points().len(), 2);
    }

    #[test]
    fn bounds_cover_the_vertices() {
        let path = GeoPath::new(false, [(0.0, 0.0), (0.0, 2.0)]).unwrap();
        assert!((path.length() - 2.0_f64.to_radians()).abs() < 1e-9);
        assert!((path.radius() - 1.0_f64.to_radians()).abs() < 1e-6);
        let (lat, lon) = path.center();
        assert!(lat.abs() < 1e-6 && (lon - 1.0).abs() < 1e-6);
    }

    #[test]
    fn clones_share_vertices_and_compare_by_content() {
        let a = GeoPath::new(true, [(1.0, 1.0), (2.0, 2.0), (3.0, 1.0)]).unwrap();
        let b = a.clone();
        assert!(Arc::ptr_eq(&a.points, &b.points));
        assert_eq!(a, b);
        assert_ne!(a, GeoPath::new(false, a.points().iter().copied()).unwrap());
    }
}
