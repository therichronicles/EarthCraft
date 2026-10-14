//! Experimental GPU sampling and shading, without egui dependencies.
mod overview;
mod repair;
mod shader;
#[cfg(test)]
mod tests;
use crate::{RenderedView, worker::RenderRequest};
use earthcraft_data::tiles::{Tile, TileCache, TileKey};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

const TILE_EDGE_PX: u32 = 256;
const GPU_TILE_BUDGET_BYTES: u32 = 128 * 1024 * 1024;
const TILE_BYTES: u32 = TILE_EDGE_PX * TILE_EDGE_PX * 4;
const MAX_ATLAS_LAYERS: u32 = GPU_TILE_BUDGET_BYTES / TILE_BYTES;
const GLOBAL_RESIDENT_ZOOM: u8 = 3;
const MAX_RESIDENCY_PASSES: usize = 2;
const WORKGROUP_SIZE: u32 = 256;

struct Slot {
    key: TileKey,
    source: Weak<Tile>,
}
pub(crate) struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    overview: overview::Overview,
    atlas: wgpu::Texture,
    view: wgpu::TextureView,
    slots: Vec<Option<Slot>>,
    preferred: Vec<TileKey>,
    failure: Arc<Mutex<Option<String>>>,
}
impl GpuRenderer {
    #[cfg(test)]
    pub(crate) fn destroy_for_test(&self) {
        self.device.destroy();
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll destroyed device");
    }
    pub(crate) fn supports(request: &RenderRequest) -> bool {
        use earthcraft_core::{EARTH_RADIUS_KM, earth_model::SurfaceGeometry};
        match request.basis.model.geometry() {
            SurfaceGeometry::Sphere { radius_km } if radius_km == EARTH_RADIUS_KM => {}
            SurfaceGeometry::Sphere { .. } | SurfaceGeometry::Ellipsoid { .. } => return false,
        }

        overview::supported_zoom(request).is_some()
    }
    pub(crate) fn new() -> Result<Self, String> {
        pollster::block_on(Self::initialize())
    }
    async fn initialize() -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .map_err(|error| format!("No compatible GPU adapter: {error}"))?;
        if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
            return Err("Only a software GPU adapter is available".into());
        }
        let available = adapter.limits();
        let layers = MAX_ATLAS_LAYERS.min(available.max_texture_array_layers);
        let limits = wgpu::Limits {
            max_texture_array_layers: layers,
            ..Default::default()
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("EarthCraft compute renderer"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|error| error.to_string())?;
        let failure = Arc::new(Mutex::new(None));
        let errors = failure.clone();
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            *errors.lock().unwrap() = Some(error.to_string());
        }));
        let lost = failure.clone();
        device.set_device_lost_callback(move |reason, message| {
            *lost.lock().unwrap() = Some(format!("GPU device lost ({reason:?}): {message}"));
        });
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("EarthCraft bounded tile atlas"),
            size: wgpu::Extent3d {
                width: TILE_EDGE_PX,
                height: TILE_EDGE_PX,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
        let overview = overview::Overview::new(&device);
        if let Some(error) = scope.pop().await {
            return Err(error.to_string());
        }
        if let Some(error) = failure.lock().unwrap().take() {
            return Err(error);
        }
        eprintln!(
            "Experimental GPU globe rendering enabled: {} ({:?}), {} MiB tile atlas",
            adapter.get_info().name,
            adapter.get_info().backend,
            layers * TILE_BYTES / (1024 * 1024)
        );
        Ok(Self {
            device,
            queue,
            overview,
            atlas,
            view,
            slots: (0..layers).map(|_| None).collect(),
            preferred: Vec::new(),
            failure,
        })
    }

    fn upload(&mut self, tiles: &[(TileKey, Arc<Tile>)]) -> Result<HashMap<TileKey, u32>, String> {
        if tiles.len() > self.slots.len() {
            return Err("Decoded provider working set exceeds the bounded GPU atlas".into());
        }
        if tiles.iter().any(|(_, tile)| {
            let (w, h, _) = tile.rgba();
            w != TILE_EDGE_PX as usize || h != TILE_EDGE_PX as usize
        }) {
            return Err("Tile dimensions require the general CPU sampler".into());
        }
        let wanted: std::collections::HashSet<_> = tiles.iter().map(|(key, _)| *key).collect();
        let mut map: HashMap<_, _> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| slot.as_ref().map(|slot| (slot.key, i as u32)))
            .collect();
        for (key, tile) in tiles {
            let layer = if let Some(&layer) = map.get(key) {
                layer
            } else {
                let layer = self
                    .slots
                    .iter()
                    .position(Option::is_none)
                    .or_else(|| {
                        self.slots.iter().position(|slot| {
                            slot.as_ref()
                                .is_some_and(|slot| !wanted.contains(&slot.key))
                        })
                    })
                    .ok_or("Visible band exceeds GPU residency capacity")?;
                if let Some(old) = &self.slots[layer] {
                    map.remove(&old.key);
                }
                map.insert(*key, layer as u32);
                layer as u32
            };
            let slot = &mut self.slots[layer as usize];
            if slot
                .as_ref()
                .and_then(|slot| slot.source.upgrade())
                .is_some_and(|source| Arc::ptr_eq(&source, tile))
            {
                continue;
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                tile.rgba().2,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(TILE_EDGE_PX * 4),
                    rows_per_image: Some(TILE_EDGE_PX),
                },
                wgpu::Extent3d {
                    width: TILE_EDGE_PX,
                    height: TILE_EDGE_PX,
                    depth_or_array_layers: 1,
                },
            );
            *slot = Some(Slot {
                key: *key,
                source: Arc::downgrade(tile),
            });
        }
        map.retain(|key, _| wanted.contains(key));
        Ok(map)
    }
    pub(crate) fn render(
        &mut self,
        request: &RenderRequest,
        cache: &TileCache,
        buffer: Vec<u8>,
    ) -> Result<RenderedView, (String, Vec<u8>, bool)> {
        if let Some(error) = self.failure.lock().unwrap().take() {
            return Err((error, buffer, true));
        }
        let Some(max_zoom) = overview::supported_zoom(request) else {
            return Err(("View requires precise CPU rendering".into(), buffer, false));
        };
        let tiles: Vec<_> = cache
            .ready_tiles(request.cfg.provider, request.cfg.rel)
            .into_iter()
            .filter(|(key, _)| earthcraft_data::tiles::unkey(*key).1 <= max_zoom)
            .collect();
        let ready: HashMap<_, _> = tiles.iter().map(|(key, _)| (*key, ())).collect();
        let mut selected: std::collections::HashSet<_> = self
            .preferred
            .iter()
            .copied()
            .filter(|key| ready.contains_key(key))
            .collect();
        let mut buffer = buffer;
        for pass in 0..MAX_RESIDENCY_PASSES {
            if selected.len() > self.slots.len() {
                return Err((
                    "Visible tile working set exceeds the bounded GPU atlas".into(),
                    buffer,
                    false,
                ));
            }
            // Coarse global imagery is cheap and avoids a cold opposite hemisphere.
            // Current-view tiles always take precedence over this optional set.
            for (key, _) in &tiles {
                if selected.len() >= self.slots.len() {
                    break;
                }
                if earthcraft_data::tiles::unkey(*key).1 <= GLOBAL_RESIDENT_ZOOM {
                    selected.insert(*key);
                }
            }
            let referenced: Vec<_> = tiles
                .iter()
                .filter(|(key, _)| selected.contains(key))
                .cloned()
                .collect();
            let slots = match self.upload(&referenced) {
                Ok(slots) => slots,
                Err(error) => return Err((error, buffer, false)),
            };
            let frame = self.overview.render(
                overview::Context {
                    device: &self.device,
                    queue: &self.queue,
                    atlas: &self.view,
                },
                request,
                &slots,
                &|key| cache.lookup(key),
                buffer,
            )?;
            if let Some(error) = self.failure.lock().unwrap().take() {
                return Err((error, frame.view.pixels, true));
            }
            let wanted: std::collections::HashSet<_> = frame
                .sampled
                .iter()
                .copied()
                .filter(|key| ready.contains_key(key))
                .collect();
            let missing_ready = wanted.iter().any(|key| !slots.contains_key(key));
            self.preferred = frame.sampled;
            if !missing_ready {
                return Ok(frame.view);
            }
            buffer = frame.view.pixels;
            if pass + 1 == MAX_RESIDENCY_PASSES {
                return Err((
                    "GPU residency could not resolve this view within its bounded passes".into(),
                    buffer,
                    false,
                ));
            }
            selected = wanted;
        }
        unreachable!("nonzero bounded GPU residency passes")
    }
}
fn binding(buffer: &wgpu::Buffer, size: u64) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        buffer,
        offset: 0,
        size: std::num::NonZeroU64::new(size),
    })
}
