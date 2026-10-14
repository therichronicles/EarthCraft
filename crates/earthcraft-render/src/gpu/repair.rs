//! Precise shared CPU sky, horizon and tile-seam repair after GPU readback.
use crate::{tuning, worker::RenderRequest};
use earthcraft_data::tiles::{Lookup, TileKey};
use rayon::prelude::*;
use std::collections::HashMap;

pub(super) fn pixels(
    request: &RenderRequest,
    pixels: &mut [u8],
    lookup: &(impl Fn(TileKey) -> Lookup + Sync),
) -> Vec<(TileKey, Option<f32>)> {
    let rays = crate::rays::RayCaster::new(request.width, request.height, &request.basis);
    let batches: Vec<_> = pixels
        .par_chunks_mut(request.width * 4 * tuning::ROWS_PER_TILE_BATCH)
        .enumerate()
        .map(|(batch, rows)| {
            let mut local = crate::Local {
                lookup,
                map: HashMap::default(),
            };
            for (offset, row) in rows.chunks_mut(request.width * 4).enumerate() {
                let y = batch * tuning::ROWS_PER_TILE_BATCH + offset;
                let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(row);
                for (pack, group) in pixels.chunks_mut(crate::rays::RAY_LANES).enumerate() {
                    // Most surface packs need no CPU work. Sky/horizon packs retain
                    // f64 classification and the deterministic 64-bit stars.
                    if group.iter().all(|pixel| pixel[3] == 255) {
                        continue;
                    }
                    for (lane, (pixel, ray)) in group
                        .iter_mut()
                        .zip(rays.cast(pack * crate::rays::RAY_LANES, y))
                        .enumerate()
                    {
                        if pixel[3] == 255 {
                            continue;
                        }
                        let color = if ray.discriminant >= 0.0 && ray.camera_dot < 0.0 {
                            let surface = crate::surface::<false>(
                                pack * crate::rays::RAY_LANES + lane,
                                y,
                                request.width,
                                request.height,
                                &ray,
                                &request.cfg,
                                request.basis.model.geometry(),
                            );
                            crate::shade_surface(surface, &request.cfg, &mut local)
                        } else {
                            crate::sky(&ray, rays.camera_len_squared, &request.cfg)
                        };
                        let color = color.clamp(glam::Vec4::ZERO, glam::Vec4::ONE) * 255.0;
                        *pixel = [color.x as u8, color.y as u8, color.z as u8, 255];
                    }
                }
            }
            local
                .map
                .into_iter()
                .filter_map(|(key, tile)| match tile {
                    crate::LocalTile::Absent(distance) => Some((key, Some(distance))),
                    crate::LocalTile::Pending => Some((key, None)),
                    crate::LocalTile::Ready(_) => None,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    batches.into_iter().flatten().collect()
}
