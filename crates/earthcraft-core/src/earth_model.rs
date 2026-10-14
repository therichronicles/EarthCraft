//! Earth surface geometry and geodesics. World coordinates retain mean-radius units.
//! Reference-frame metadata does not transform epochs or model crustal motion.
use crate::{EARTH_RADIUS_KM, compass, math};
use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};
use glam::DVec3;
use std::sync::LazyLock;

const WGS84_SEMI_MAJOR_KM: f64 = 6378.137;
const WGS84_INVERSE_FLATTENING: f64 = 298.257223563;
static WGS84_GEODESIC: LazyLock<Geodesic> = LazyLock::new(Geodesic::wgs84);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EarthModel {
    #[default]
    PerfectSphere,
    Wgs84,
}

/// Geometry is separate from model identity: additional reference frames can
/// share a surface kernel. Different surface families must supply their own
/// intersection, coordinates, normals and geodesics instead of pretending to be ellipsoids.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceGeometry {
    Sphere { radius_km: f64 },
    Ellipsoid { semi_major_km: f64, flattening: f64 },
}

impl EarthModel {
    pub const ALL: [Self; 2] = [Self::PerfectSphere, Self::Wgs84];
    pub fn code(self) -> &'static str {
        match self {
            Self::PerfectSphere => "perfect_sphere",
            Self::Wgs84 => "wgs84_g2296",
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|model| model.code() == code)
    }
    pub fn name_id(self) -> &'static str {
        match self {
            Self::PerfectSphere => "earth-model-sphere",
            Self::Wgs84 => "earth-model-wgs84",
        }
    }
    pub fn help_id(self) -> &'static str {
        match self {
            Self::PerfectSphere => "earth-model-sphere-help",
            Self::Wgs84 => "earth-model-wgs84-help",
        }
    }
    #[inline]
    pub fn geometry(self) -> SurfaceGeometry {
        match self {
            Self::PerfectSphere => SurfaceGeometry::Sphere {
                radius_km: EARTH_RADIUS_KM,
            },
            Self::Wgs84 => SurfaceGeometry::Ellipsoid {
                semi_major_km: WGS84_SEMI_MAJOR_KM,
                flattening: 1.0 / WGS84_INVERSE_FLATTENING,
            },
        }
    }
    pub fn distance_km(self, start: (f64, f64), end: (f64, f64)) -> f64 {
        match self {
            Self::PerfectSphere => math::haversine_km(start, end),
            Self::Wgs84 => {
                let meters: f64 = WGS84_GEODESIC.inverse(start.0, start.1, end.0, end.1);
                meters / 1000.0
            }
        }
    }
    pub fn initial_bearing(self, start: (f64, f64), end: (f64, f64)) -> Option<f64> {
        // Preserve the UI's undefined-bearing convention at coincident/antipodal points and poles.
        compass::initial_bearing(start, end)?;
        match self {
            Self::PerfectSphere => compass::initial_bearing(start, end),
            Self::Wgs84 => {
                let (_, azimuth, _, _): (f64, f64, f64, f64) =
                    WGS84_GEODESIC.inverse(start.0, start.1, end.0, end.1);
                Some(compass::normalize_heading(azimuth))
            }
        }
    }
    pub(crate) fn route_bearing(self, start: (f64, f64), end: (f64, f64)) -> f64 {
        match self {
            Self::PerfectSphere => compass::initial_bearing(start, end).unwrap_or(0.0),
            Self::Wgs84 => {
                let (_, azimuth, _, _): (f64, f64, f64, f64) =
                    WGS84_GEODESIC.inverse(start.0, start.1, end.0, end.1);
                azimuth
            }
        }
    }
    #[inline]
    pub fn destination(self, start: (f64, f64), bearing_deg: f64, distance_km: f64) -> (f64, f64) {
        match self {
            Self::PerfectSphere => math::destination(start.0, start.1, bearing_deg, distance_km),
            Self::Wgs84 => {
                WGS84_GEODESIC.direct(start.0, start.1, bearing_deg, distance_km * 1000.0)
            }
        }
    }
    /// Central angular separation of geocentric surface directions.
    pub fn central_angle_degrees(self, start: (f64, f64), end: (f64, f64)) -> f64 {
        if self == Self::PerfectSphere {
            return (self.distance_km(start, end) / EARTH_RADIUS_KM).to_degrees();
        }
        let a = self.geometry().position(start.0, start.1).normalize();
        let b = self.geometry().position(end.0, end.1).normalize();
        a.cross(b).length().atan2(a.dot(b)).to_degrees()
    }
    pub fn path(self, start: (f64, f64), end: (f64, f64), segments: usize) -> Vec<DVec3> {
        if self == Self::PerfectSphere {
            return math::great_circle(start, end, segments);
        }
        let (meters, azimuth, _, _): (f64, f64, f64, f64) =
            WGS84_GEODESIC.inverse(start.0, start.1, end.0, end.1);
        (0..=segments)
            .map(|index| {
                let (lat, lon): (f64, f64) = WGS84_GEODESIC.direct(
                    start.0,
                    start.1,
                    azimuth,
                    meters * index as f64 / segments.max(1) as f64,
                );
                self.geometry().position(lat, lon)
            })
            .collect()
    }
}

impl SurfaceGeometry {
    pub fn axes(self) -> DVec3 {
        let (major, minor) = match self {
            Self::Sphere { radius_km } => (radius_km, radius_km),
            Self::Ellipsoid {
                semi_major_km,
                flattening,
            } => (semi_major_km, semi_major_km * (1.0 - flattening)),
        };
        DVec3::new(major, major, minor) / EARTH_RADIUS_KM
    }
    pub fn inverse_axes_squared(self) -> DVec3 {
        let axes = self.axes();
        DVec3::ONE / (axes * axes)
    }
    /// Conservative component displacement from the unit geodetic normal.
    pub fn displacement_bound(self) -> f64 {
        let axes = self.axes();
        let squared = axes * axes;
        (squared / axes.max_element() - DVec3::ONE)
            .abs()
            .max_element()
            .max(
                (squared / axes.min_element() - DVec3::ONE)
                    .abs()
                    .max_element(),
            )
    }
    /// Geodetic latitude/longitude on the surface, in the renderer's world units.
    #[inline]
    pub fn position(self, lat: f64, lon: f64) -> DVec3 {
        let normal = math::ll_to_v3(lat, lon);
        match self {
            Self::Sphere { radius_km } => normal * (radius_km / EARTH_RADIUS_KM),
            Self::Ellipsoid { .. } => {
                let axes_squared = self.axes() * self.axes();
                axes_squared * normal / (axes_squared * normal * normal).element_sum().sqrt()
            }
        }
    }
    /// Surface normal and geographic coordinates for a point on this surface.
    pub fn normal(self, point: DVec3) -> DVec3 {
        match self {
            Self::Sphere { radius_km } => point * (EARTH_RADIUS_KM / radius_km),
            Self::Ellipsoid { .. } => (point * self.inverse_axes_squared()).normalize(),
        }
    }
    pub fn coordinates(self, point: DVec3) -> (f64, f64) {
        match self {
            Self::Sphere { .. } => math::v3_to_ll(self.normal(point)),
            Self::Ellipsoid { .. } => {
                let normal = point * self.inverse_axes_squared();
                (
                    normal.z.atan2(normal.x.hypot(normal.y)).to_degrees(),
                    normal.y.atan2(normal.x).to_degrees(),
                )
            }
        }
    }
    pub fn intersect(self, origin: DVec3, direction: DVec3) -> Option<DVec3> {
        let q = self.inverse_axes_squared();
        let a = direction.dot(q * direction);
        let b = origin.dot(q * direction);
        let c = origin.dot(q * origin) - 1.0;
        let discriminant = b * b - a * c;
        if discriminant < 0.0 || b > 0.0 {
            return None;
        }
        Some(origin + direction * ((-b - discriminant.sqrt()) / a))
    }
    /// Local meridional and prime-vertical curvature radii, in kilometers.
    pub fn curvature_km(self, latitude_deg: f64) -> (f64, f64) {
        match self {
            Self::Sphere { radius_km } => (radius_km, radius_km),
            Self::Ellipsoid {
                semi_major_km,
                flattening,
            } => {
                let eccentricity_squared = flattening * (2.0 - flattening);
                let denominator =
                    1.0 - eccentricity_squared * latitude_deg.to_radians().sin().powi(2);
                let root = denominator.sqrt();
                (
                    semi_major_km * (1.0 - eccentricity_squared) / (denominator * root),
                    semi_major_km / root,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_geodesics_distinguish_models_and_cover_antipodes() {
        let model = EarthModel::Wgs84;
        assert!((model.distance_km((90.0, 0.0), (-90.0, 0.0)) - 20003.931458625).abs() < 1e-8);
        assert!((model.distance_km((0.0, 0.0), (0.0, 1.0)) - 111.319490793).abs() < 1e-8);
        assert!((model.distance_km((0.0, 0.0), (0.0, 180.0)) - 20003.931458625).abs() < 1e-8);
        for end in [(0.01, 179.99), (89.99, 20.0), (-35.0, -179.9)] {
            let start = (10.0, 1.0);
            let bearing = model.initial_bearing(start, end).unwrap();
            let actual = model.destination(start, bearing, model.distance_km(start, end));
            assert!((actual.0 - end.0).abs() < 1e-10 && (actual.1 - end.1).abs() < 1e-10);
        }
        assert_eq!(
            EarthModel::PerfectSphere.distance_km((0.0, 0.0), (0.0, 1.0)),
            math::haversine_km((0.0, 0.0), (0.0, 1.0))
        );
    }
    #[test]
    fn surface_coordinates_normals_and_intersections_agree() {
        for model in EarthModel::ALL {
            let geometry = model.geometry();
            for (lat, lon) in [(0.0, 0.0), (45.0, 179.9), (89.999, -179.9), (-90.0, 60.0)] {
                let point = geometry.position(lat, lon);
                let normal = math::ll_to_v3(lat, lon);
                let hit = geometry.intersect(point + normal * 0.1, -normal).unwrap();
                assert!(hit.abs_diff_eq(point, 1e-12));
                let (a, b) = geometry.coordinates(hit);
                assert!((a - lat).abs() < 1e-8 && (b - lon).abs() < 1e-8);
                assert!((point.dot(point * geometry.inverse_axes_squared()) - 1.0).abs() < 1e-14);
            }
        }
    }
}
