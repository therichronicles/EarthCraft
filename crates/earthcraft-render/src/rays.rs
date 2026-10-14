//! Independent f64 ray lanes; geographic/trigonometric sampling stays scalar.
use earthcraft_core::{
    EARTH_RADIUS_KM, VERTICAL_FOV_HALF_TAN, camera::Basis, earth_model::SurfaceGeometry,
};
use glam::{DMat3, DVec3};
use wide::f64x4;

pub(super) const RAY_LANES: usize = 4;

pub(super) struct RayCaster {
    rotation: DMat3,
    aspect: f64,
    camera: DVec3,
    geometry: SurfaceGeometry,
    inverse_axes_squared: DVec3,
    pub(super) camera_len_squared: f64,
    width: f64,
    height: f64,
}

pub(super) struct RaySample {
    pub(super) direction: DVec3,
    pub(super) camera_dot: f64,
    pub(super) discriminant: f64,
    pub(super) distance: f64,
    pub(super) point: DVec3,
    pub(super) normal: DVec3,
    pub(super) incidence: f64,
}

impl RayCaster {
    pub(super) fn model_geometry(&self) -> SurfaceGeometry {
        self.geometry
    }
    pub(super) fn uses_unit_sphere(&self) -> bool {
        // Keep this exhaustive: a new surface family needs an explicit kernel
        // decision rather than falling through to the ellipsoid equations.
        match self.geometry {
            SurfaceGeometry::Sphere { radius_km } => radius_km == EARTH_RADIUS_KM,
            SurfaceGeometry::Ellipsoid { .. } => false,
        }
    }
    pub(super) fn new(width: usize, height: usize, basis: &Basis) -> Self {
        let geometry = basis.model.geometry();
        let inverse_axes_squared = geometry.inverse_axes_squared();
        Self {
            rotation: basis.rotation_from_view(),
            aspect: basis.aspect_ratio(),
            camera: basis.c,
            geometry,
            inverse_axes_squared,
            camera_len_squared: if geometry
                == (SurfaceGeometry::Sphere {
                    radius_km: EARTH_RADIUS_KM,
                }) {
                basis.c.length_squared()
            } else {
                basis.c.dot(basis.c * inverse_axes_squared)
            },
            width: width as f64,
            height: height as f64,
        }
    }

    #[cfg(any(test, feature = "gpu"))]
    pub(super) fn cast(&self, first_x: usize, y: usize) -> [RaySample; RAY_LANES] {
        if self.uses_unit_sphere() {
            self.cast_model::<false>(first_x, y)
        } else {
            self.cast_model::<true>(first_x, y)
        }
    }
    #[inline]
    pub(super) fn cast_model<const ELLIPSOID: bool>(
        &self,
        first_x: usize,
        y: usize,
    ) -> [RaySample; RAY_LANES] {
        let nx = f64x4::new(std::array::from_fn(|lane| (first_x + lane) as f64));
        let nx = (nx + 0.5) / self.width * 2.0 - 1.0;
        let ny = 1.0 - (y as f64 + 0.5) / self.height * 2.0;
        let vx = nx * self.aspect * VERTICAL_FOV_HALF_TAN;
        let vy = ny * VERTICAL_FOV_HALF_TAN;
        let m = self.rotation;
        // Preserve glam's column accumulation and scalar normalize order. No FMA
        // helpers, reciprocal approximations or reduced geographic precision.
        let dx = vx * m.x_axis.x + f64x4::splat(m.y_axis.x * vy) - m.z_axis.x;
        let dy = vx * m.x_axis.y + f64x4::splat(m.y_axis.y * vy) - m.z_axis.y;
        let dz = vx * m.x_axis.z + f64x4::splat(m.y_axis.z * vy) - m.z_axis.z;
        let reciprocal_length = f64x4::ONE / (dx * dx + dy * dy + dz * dz).sqrt();
        let (dx, dy, dz) = (
            dx * reciprocal_length,
            dy * reciprocal_length,
            dz * reciprocal_length,
        );
        if ELLIPSOID {
            return self.cast_ellipsoid(dx, dy, dz);
        }
        let c = self.camera;
        let dot = dx * c.x + dy * c.y + dz * c.z;
        let disc = dot * dot - (self.camera_len_squared - 1.0);
        let surface_lanes = disc.simd_ge(f64x4::ZERO) & dot.simd_lt(f64x4::ZERO);
        let (distance, px, py, pz, incidence) = if surface_lanes.none() {
            (
                f64x4::ZERO,
                f64x4::ZERO,
                f64x4::ZERO,
                f64x4::ZERO,
                f64x4::ZERO,
            )
        } else {
            // Miss lanes may contain NaN here; their hit fields are never used.
            let distance = -dot - disc.sqrt();
            let px = f64x4::splat(c.x) + dx * distance;
            let py = f64x4::splat(c.y) + dy * distance;
            let pz = f64x4::splat(c.z) + dz * distance;
            let incidence = (-(dx * px + dy * py + dz * pz)).max(f64x4::ZERO);
            (distance, px, py, pz, incidence)
        };
        let (dx, dy, dz, dot, disc) = (
            dx.to_array(),
            dy.to_array(),
            dz.to_array(),
            dot.to_array(),
            disc.to_array(),
        );
        let (distance, px, py, pz, incidence) = (
            distance.to_array(),
            px.to_array(),
            py.to_array(),
            pz.to_array(),
            incidence.to_array(),
        );
        std::array::from_fn(|lane| RaySample {
            direction: DVec3::new(dx[lane], dy[lane], dz[lane]),
            camera_dot: dot[lane],
            discriminant: disc[lane],
            distance: distance[lane],
            point: DVec3::new(px[lane], py[lane], pz[lane]),
            normal: DVec3::new(px[lane], py[lane], pz[lane]),
            incidence: incidence[lane],
        })
    }
    fn cast_ellipsoid(&self, dx: f64x4, dy: f64x4, dz: f64x4) -> [RaySample; RAY_LANES] {
        let q = self.inverse_axes_squared;
        let c = self.camera;
        let a = dx * dx * q.x + dy * dy * q.y + dz * dz * q.z;
        let b = dx * (c.x * q.x) + dy * (c.y * q.y) + dz * (c.z * q.z);
        let disc = b * b - a * (self.camera_len_squared - 1.0);
        let camera_dot = (b / a.sqrt()).to_array();
        let discriminant = disc.to_array();
        let direction_x = dx.to_array();
        let direction_y = dy.to_array();
        let direction_z = dz.to_array();
        // Overview sky packs need only directions and the scaled impact
        // parameter for atmosphere. Skip surface positions/normals entirely.
        let surface_lanes = disc.simd_ge(f64x4::ZERO) & b.simd_lt(f64x4::ZERO);
        if surface_lanes.none() {
            return std::array::from_fn(|lane| RaySample {
                direction: DVec3::new(direction_x[lane], direction_y[lane], direction_z[lane]),
                camera_dot: camera_dot[lane],
                discriminant: discriminant[lane],
                distance: 0.0,
                point: DVec3::ZERO,
                normal: DVec3::ZERO,
                incidence: 0.0,
            });
        }
        let distance = (-b - disc.sqrt()) / a;
        let px = f64x4::splat(c.x) + dx * distance;
        let py = f64x4::splat(c.y) + dy * distance;
        let pz = f64x4::splat(c.z) + dz * distance;
        let (nx, ny, nz) = (px * q.x, py * q.y, pz * q.z);
        let reciprocal = f64x4::ONE / (nx * nx + ny * ny + nz * nz).sqrt();
        let (nx, ny, nz) = (nx * reciprocal, ny * reciprocal, nz * reciprocal);
        let incidence = (-(dx * nx + dy * ny + dz * nz)).max(f64x4::ZERO).to_array();
        let distance = distance.to_array();
        let (px, py, pz, nx, ny, nz) = (
            px.to_array(),
            py.to_array(),
            pz.to_array(),
            nx.to_array(),
            ny.to_array(),
            nz.to_array(),
        );
        std::array::from_fn(|lane| RaySample {
            direction: DVec3::new(direction_x[lane], direction_y[lane], direction_z[lane]),
            camera_dot: camera_dot[lane],
            discriminant: discriminant[lane],
            distance: distance[lane],
            point: DVec3::new(px[lane], py[lane], pz[lane]),
            normal: DVec3::new(nx[lane], ny[lane], nz[lane]),
            incidence: incidence[lane],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::earth_model::EarthModel;
    use earthcraft_core::{MAX_CAMERA_ALTITUDE_KM, MIN_CAMERA_ALTITUDE_KM, camera::Cam};

    #[test]
    fn ellipsoid_packed_rays_match_scalar_picking_and_surface_normals() {
        for (lat, lon, heading, alt) in [
            (45.0, 179.9, 40.0, 0.15),
            (89.9, -179.9, 120.0, 500.0),
            (-40.0, 35.0, -100.0, 14000.0),
            (0.0, 0.0, 0.0, 40000.0),
        ] {
            let basis = Cam {
                lat,
                lon,
                heading,
                alt,
            }
            .basis_for_model(65.0 / 49.0, EarthModel::Wgs84);
            let caster = RayCaster::new(65, 49, &basis);
            let geometry = basis.model.geometry();
            for y in 0..49 {
                for x in (0..65).step_by(RAY_LANES) {
                    for (lane, ray) in caster.cast(x, y).into_iter().enumerate().take(65 - x) {
                        let direction = basis.dir(
                            (x + lane) as f64 / 65.0 * 2.0 + 1.0 / 65.0 - 1.0,
                            1.0 - (y as f64 + 0.5) / 49.0 * 2.0,
                        );
                        let hit = geometry.intersect(basis.c, direction);
                        assert_eq!(
                            hit.is_some(),
                            ray.discriminant >= 0.0 && ray.camera_dot < 0.0
                        );
                        if let Some(hit) = hit {
                            assert!(ray.point.abs_diff_eq(hit, 1e-10));
                            assert!(ray.normal.abs_diff_eq(geometry.normal(hit), 1e-10));
                            assert!((ray.incidence + direction.dot(ray.normal)).abs() < 1e-12);
                            assert!(
                                (ray.point.dot(ray.point * geometry.inverse_axes_squared()) - 1.0)
                                    .abs()
                                    < 1e-10
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn packed_rays_match_scalar_camera_directions_and_sphere_geometry() {
        for (width, height) in [(1, 7), (3, 9), (65, 49), (127, 95)] {
            for (lat, lon, heading) in [
                (-15.0, -50.0, 25.0),
                (89.9, 179.9, -130.0),
                (-89.9, -179.9, 180.0),
                (0.0, 180.0, 0.0),
            ] {
                for alt in [
                    MIN_CAMERA_ALTITUDE_KM,
                    1000.0,
                    14_000.0,
                    MAX_CAMERA_ALTITUDE_KM,
                ] {
                    let basis = Cam {
                        lat,
                        lon,
                        heading,
                        alt,
                    }
                    .basis(width as f64 / height as f64);
                    let caster = RayCaster::new(width, height, &basis);
                    for y in 0..height {
                        for first_x in (0..width).step_by(RAY_LANES) {
                            for (lane, ray) in caster
                                .cast(first_x, y)
                                .into_iter()
                                .enumerate()
                                .take(width - first_x)
                            {
                                let x = first_x + lane;
                                let nx = (x as f64 + 0.5) / width as f64 * 2.0 - 1.0;
                                let ny = 1.0 - (y as f64 + 0.5) / height as f64 * 2.0;
                                let scalar = basis.dir(nx, ny);
                                assert!(
                                    ray.direction.abs_diff_eq(scalar, 1e-14),
                                    "{width}x{height} ({x},{y}), alt {alt}"
                                );
                                assert!((ray.direction.length_squared() - 1.0).abs() < 1e-14);
                                let dot = basis.c.dot(scalar);
                                let disc = dot * dot - (basis.c.length_squared() - 1.0);
                                assert_eq!(
                                    ray.discriminant >= 0.0 && ray.camera_dot < 0.0,
                                    disc >= 0.0 && dot < 0.0,
                                    "hit classification ({x},{y}), alt {alt}"
                                );
                                if ray.discriminant >= 0.0 && ray.camera_dot < 0.0 {
                                    assert!(ray.distance >= 0.0);
                                    assert!((ray.point.length_squared() - 1.0).abs() < 1e-10);
                                    assert!(
                                        (ray.point - basis.c).normalize().abs_diff_eq(scalar, 1e-9)
                                    );
                                    assert!((ray.incidence + scalar.dot(ray.point)).abs() < 1e-12);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
