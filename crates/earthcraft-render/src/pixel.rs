//! Shared geographic decisions and exact deterministic sky colors.
use super::*;

fn hash3(a: i64, b: i64, c: i64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (c as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^ (h >> 33)
}

#[derive(Clone, Copy)]
pub(super) struct Surface {
    pub(super) u: f64,
    pub(super) v: f64,
    pub(super) z0: i32,
    pub(super) frac: f32,
    pub(super) dist: f32,
    pub(super) grid: bool,
    pub(super) shade: f32,
    pub(super) night: f32,
    pub(super) rim: f32,
}
pub(super) fn surface<const ELLIPSOID: bool>(
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    ray: &rays::RaySample,
    cfg: &RenderCfg,
    geometry: earthcraft_core::earth_model::SurfaceGeometry,
) -> Surface {
    let pa = 2.0 * VERTICAL_FOV_HALF_TAN / h as f64;
    let maxz = cfg.provider.max_zoom() as f64;
    let t = ray.distance;
    let cosinc = ray.incidence;
    let normal = if ELLIPSOID { ray.normal } else { ray.point };
    let latitude_cosine = if ELLIPSOID {
        normal.x.hypot(normal.y)
    } else {
        0.0
    };
    let (lat, lon) = if ELLIPSOID {
        (
            normal.z.atan2(latitude_cosine).to_degrees(),
            normal.y.atan2(normal.x).to_degrees(),
        )
    } else {
        v3_to_ll(ray.point)
    };
    let coslat = if ELLIPSOID {
        latitude_cosine
    } else {
        lat.to_radians().cos()
    }
    .max(MIN_LOD_LATITUDE_COSINE);
    // Level of detail (LOD): softer grazing-angle limit and a sharpness bias,
    // which greatly enlarges the high-resolution area.
    let mpp = t * EARTH_RADIUS_M * pa / cosinc.max(MIN_LOD_INCIDENCE_COSINE).sqrt();
    let (meridional_km, prime_vertical_km) = if ELLIPSOID {
        geometry.curvature_km_from_latitude_sine(normal.z)
    } else {
        (0.0, 0.0)
    };
    let latitude_radius_m = if !ELLIPSOID {
        EARTH_RADIUS_M
    } else {
        prime_vertical_km * METERS_PER_KM
    };
    let zf = ((2.0 * PI * latitude_radius_m * coslat) / (TILE_SIZE_PX * mpp)).log2();
    let lod = zf + LOD_SHARPNESS_BIAS;
    // Satellite: smooth blend between two levels (no visible circles).
    // Text maps: single, slightly sharper level so labels are not duplicated.
    let (zl, frac) = if cfg.provider.is_imagery() {
        let f = lod.floor();
        (f, lod - f)
    } else {
        (lod.ceil(), 0.0)
    };
    let z0 = zl.clamp(0.0, maxz) as i32;
    let frac = if (z0 as f64) >= maxz {
        0.0
    } else {
        frac as f32
    };

    let latc = lat.clamp(
        -WEB_MERCATOR_MAX_LATITUDE_DEG,
        WEB_MERCATOR_MAX_LATITUDE_DEG,
    );
    let u = ((lon + 180.0) / 360.0).rem_euclid(1.0);
    let s = if ELLIPSOID && lat == latc {
        normal.z
    } else {
        latc.to_radians().sin()
    };
    let v = 0.5 - ((1.0 + s) / (1.0 - s)).ln() / (4.0 * PI);
    let offset = Vec2::new(x as f32 - w as f32 / 2.0, y as f32 - h as f32 / 2.0);
    let dist = offset.length_squared();

    let grid = if cfg.grid {
        let st = cfg.grid_step;
        let meridian_m_per_degree = if !ELLIPSOID {
            METERS_PER_LATITUDE_DEGREE
        } else {
            meridional_km * METERS_PER_KM * PI / 180.0
        };
        let thr = mpp / meridian_m_per_degree * GRID_LINE_WIDTH_PX;
        let dlat = (lat / st - (lat / st).round()).abs() * st;
        let grid_latitude_cosine = if ELLIPSOID {
            latitude_cosine
        } else {
            lat.to_radians().cos().abs()
        };
        let dlon = (lon / st - (lon / st).round()).abs() * st * grid_latitude_cosine;
        // Express east-west distance in meridional-degree units before comparing
        // against the shared threshold. The two curvature radii differ on an ellipsoid.
        let dlon = if ELLIPSOID {
            dlon * (prime_vertical_km / meridional_km)
        } else {
            dlon
        };
        dlat < thr || dlon < thr
    } else {
        false
    };
    let (shade, night) = if let Some(sun) = cfg.sun {
        let k = ((normal.dot(sun) + TWILIGHT_OFFSET) / TWILIGHT_WIDTH).clamp(0.0, 1.0) as f32;
        let k = k * k * (3.0 - 2.0 * k);
        (
            NIGHT_BRIGHTNESS + DAY_BRIGHTNESS_RANGE * k,
            NIGHT_BLUE_TINT * (1.0 - k),
        )
    } else {
        (1.0, 0.0)
    };
    let rim = if cfg.atmo {
        (1.0 - cosinc as f32).powi(RIM_FALLOFF_POWER) * RIM_BLEND
    } else {
        0.0
    };
    Surface {
        u,
        v,
        z0,
        frac,
        dist,
        grid,
        shade,
        night,
        rim,
    }
}
pub(super) fn sky(ray: &rays::RaySample, cc: f64, cfg: &RenderCfg) -> Vec4 {
    let d = ray.direction;
    let bb = ray.camera_dot;
    let mut col = SPACE_COLOR;
    let cell = (
        (d.x * STAR_GRID_RESOLUTION).floor() as i64,
        (d.y * STAR_GRID_RESOLUTION).floor() as i64,
        (d.z * STAR_GRID_RESOLUTION).floor() as i64,
    );
    let hs = hash3(cell.0, cell.1, cell.2);
    if hs.is_multiple_of(STAR_OCCURRENCE_MODULUS) {
        let br = STAR_MIN_BRIGHTNESS
            + ((hs >> STAR_BRIGHTNESS_SHIFT) & STAR_BRIGHTNESS_MASK) as f32
                / STAR_BRIGHTNESS_DIVISOR;
        col = Vec4::new(br, br, br, 0.0);
    }
    if cfg.atmo && bb < 0.0 {
        let rho = (cc - bb * bb).max(0.0).sqrt();
        if (1.0..ATMOSPHERE_RADIUS_RATIO).contains(&rho) {
            let g = (1.0 - (rho - 1.0) / ATMOSPHERE_HEIGHT_RATIO).powi(2) as f32;
            col = col.lerp_monotonic(ATMOSPHERE_COLOR, g * ATMOSPHERE_BLEND);
        }
    }
    col
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::{camera::Cam, earth_model::EarthModel};

    #[test]
    fn ellipsoid_texture_coordinates_match_geodetic_mercator_reference() {
        let model = EarthModel::Wgs84;
        let cfg = RenderCfg {
            provider: Provider::Satellite,
            rel: 0,
            grid: false,
            grid_step: 10.0,
            atmo: false,
            sun: None,
        };
        for latitude in [
            -90.0_f64,
            -85.05112878,
            -45.0,
            -0.0001,
            0.0,
            45.0,
            85.05112878,
            89.999,
            90.0,
        ] {
            for longitude in [-179.9, 0.0, 179.9] {
                let basis = Cam {
                    lat: latitude,
                    lon: longitude,
                    heading: 25.0,
                    alt: 0.15,
                }
                .basis_for_model(1.0, model);
                let rays = RayCaster::new(1, 1, &basis);
                let pixel =
                    surface::<true>(0, 0, 1, 1, &rays.cast(0, 0)[0], &cfg, model.geometry());
                let expected_u = (longitude + 180.0) / 360.0;
                // Equivalent independent form of the spherical Mercator ordinate.
                let latitude = latitude.clamp(
                    -WEB_MERCATOR_MAX_LATITUDE_DEG,
                    WEB_MERCATOR_MAX_LATITUDE_DEG,
                );
                let expected_v = 0.5 - latitude.to_radians().tan().asinh() / (2.0 * PI);
                assert!((pixel.u - expected_u).abs() < 1e-12);
                assert!((pixel.v - expected_v).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn ellipsoid_meridians_use_east_west_surface_distance_for_grid_width() {
        let model = EarthModel::Wgs84;
        let altitude_km = 1.0;
        let threshold_m =
            altitude_km * METERS_PER_KM * 2.0 * VERTICAL_FOV_HALF_TAN * GRID_LINE_WIDTH_PX;
        let cfg = RenderCfg {
            provider: Provider::Satellite,
            rel: 0,
            grid: true,
            grid_step: 1.0,
            atmo: false,
            sun: None,
        };
        // Stay midway between parallels so only the prime meridian can mark
        // this pixel. Distances straddle its physical line-width threshold.
        for (fraction, expected) in [(0.998, true), (1.002, false)] {
            let (lat, lon) =
                model.destination((0.5, 0.0), 90.0, threshold_m * fraction / METERS_PER_KM);
            let basis = Cam {
                lat,
                lon,
                heading: 0.0,
                alt: altitude_km,
            }
            .basis_for_model(1.0, model);
            let rays = RayCaster::new(1, 1, &basis);
            let ray = &rays.cast(0, 0)[0];
            let pixel = surface::<true>(0, 0, 1, 1, ray, &cfg, model.geometry());
            assert_eq!(
                pixel.grid, expected,
                "{fraction} of the line-width threshold"
            );
        }
    }
}
