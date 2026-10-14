//! Earth surface geometry and geodesics. World coordinates retain mean-radius units.
//! Reference-frame metadata does not transform epochs or model crustal motion.
use crate::{EARTH_RADIUS_KM, METERS_PER_KM, compass, math};
use geographiclib_rs::{DirectGeodesic, Geodesic, GeodesicLine, InverseGeodesic, capability};
use glam::DVec3;
use std::sync::LazyLock;

const WGS84_SEMI_MAJOR_KM: f64 = 6378.137;
const WGS84_INVERSE_FLATTENING: f64 = 298.257223563;
// Roundoff in the dimensionless surface equation p·Qp = 1 (about 6 nm on Earth).
const SURFACE_RESIDUAL_EPSILON: f64 = 8.0 * f64::EPSILON;
static WGS84_GEODESIC: LazyLock<Geodesic> = LazyLock::new(Geodesic::wgs84);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EarthModel {
    PerfectSphere,
    #[default]
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
    pub const ALL: [Self; 2] = [Self::Wgs84, Self::PerfectSphere];
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
                meters / METERS_PER_KM
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
                WGS84_GEODESIC.direct(start.0, start.1, bearing_deg, distance_km * METERS_PER_KM)
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
    /// Equally spaced shortest-route samples, including both exact endpoints.
    /// Zero segments returns only the start; antipodal ties use a deterministic route.
    pub fn path(self, start: (f64, f64), end: (f64, f64), segments: usize) -> Vec<DVec3> {
        let geodesic = match self {
            Self::PerfectSphere => return math::great_circle(start, end, segments),
            Self::Wgs84 => &*WGS84_GEODESIC,
        };
        let geometry = self.geometry();
        if segments == 0 {
            return vec![geometry.position(start.0, start.1)];
        }
        let (meters, azimuth, _, _): (f64, f64, f64, f64) =
            geodesic.inverse(start.0, start.1, end.0, end.1);
        // Prepare the route coefficients once instead of rebuilding a direct
        // solver for every sample. _gen_position is the port's typed public API.
        let outputs = capability::LATITUDE | capability::LONGITUDE;
        let line = GeodesicLine::new(
            geodesic,
            start.0,
            start.1,
            azimuth,
            Some(outputs | capability::DISTANCE_IN),
            None,
            None,
        );
        (0..=segments)
            .map(|index| {
                if index == 0 {
                    geometry.position(start.0, start.1)
                } else if index == segments {
                    geometry.position(end.0, end.1)
                } else {
                    let (_, lat, lon, ..) =
                        line._gen_position(false, meters * index as f64 / segments as f64, outputs);
                    geometry.position(lat, lon)
                }
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
    /// Nearest forward ray intersection, including rays starting inside the surface.
    /// Direction need not be normalized; invalid or zero-length rays have no hit.
    pub fn intersect(self, origin: DVec3, direction: DVec3) -> Option<DVec3> {
        if !origin.is_finite() || !direction.is_finite() {
            return None;
        }
        let q = self.inverse_axes_squared();
        let a = direction.dot(q * direction);
        let b = origin.dot(q * direction);
        let c = origin.dot(q * origin) - 1.0;
        if a <= 0.0 || !a.is_finite() || !b.is_finite() || !c.is_finite() {
            return None;
        }
        // A surface point can round just inside/outside the quadric. Treat it
        // as a zero-distance hit instead of selecting the opposite hemisphere.
        if c.abs() <= SURFACE_RESIDUAL_EPSILON {
            return Some(origin);
        }
        let discriminant = b * b - a * c;
        if !discriminant.is_finite() || discriminant < 0.0 {
            return None;
        }
        // The product of the roots is c/a. Computing the small root from the
        // other root avoids subtracting nearly equal terms close to the surface.
        let root = -b - discriminant.sqrt().copysign(b);
        let distance = if root == 0.0 {
            0.0
        } else {
            let (first, second) = (root / a, c / root);
            let near = first.min(second);
            if near >= 0.0 { near } else { first.max(second) }
        };
        (distance >= 0.0 && distance.is_finite()).then(|| origin + direction * distance)
    }
    /// Local meridional and prime-vertical curvature radii, in kilometers.
    pub fn curvature_km(self, latitude_deg: f64) -> (f64, f64) {
        match self {
            Self::Sphere { radius_km } => (radius_km, radius_km),
            Self::Ellipsoid { .. } => {
                self.curvature_km_from_latitude_sine(latitude_deg.to_radians().sin())
            }
        }
    }
    /// Curvature when the sine of geodetic latitude is already available from a
    /// unit surface normal. Avoids converting that normal through angles again.
    pub fn curvature_km_from_latitude_sine(self, latitude_sine: f64) -> (f64, f64) {
        match self {
            Self::Sphere { radius_km } => (radius_km, radius_km),
            Self::Ellipsoid {
                semi_major_km,
                flattening,
            } => {
                let eccentricity_squared = flattening * (2.0 - flattening);
                let denominator = 1.0 - eccentricity_squared * latitude_sine.powi(2);
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
mod tests;
