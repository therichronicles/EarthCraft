//! Manual CPU-frame comparison using isolated disk-cache fixtures and production APIs.
use super::*;
mod kernels;
use earthcraft_core::{camera::Cam, math::now_unix};
use std::{
    hint::black_box,
    path::PathBuf,
    time::{Duration, Instant},
};

const FRAME_WIDTH_PX: usize = 512;
const FRAME_HEIGHT_PX: usize = 384;
const BENCH_FRAMES: usize = 200;
const BENCH_ROUNDS: usize = 5;
const FIXTURE_TILE_SIZE_PX: u32 = 256;

struct FixtureCache {
    root: PathBuf,
}
impl FixtureCache {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "earthcraft-render-bench-{}-{}",
            std::process::id(),
            now_unix()
        ));
        Self { root }
    }
    fn prepare(&self) -> Vec<TileKey> {
        let mut keys = Vec::new();
        let image =
            image::RgbaImage::from_fn(FIXTURE_TILE_SIZE_PX, FIXTURE_TILE_SIZE_PX, |x, y| {
                let value =
                    (x.wrapping_mul(1664525) ^ y.wrapping_mul(1013904223)).wrapping_mul(2246822519);
                image::Rgba([value as u8, (value >> 8) as u8, (value >> 16) as u8, 255])
            });
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        for z in 0..=4 {
            for x in 0..1u32 << z {
                for y in 0..1u32 << z {
                    let path = self
                        .root
                        .join("earthcraft_tiles/esri_sat")
                        .join(z.to_string())
                        .join(x.to_string())
                        .join(format!("{y}.tile"));
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path, encoded.get_ref()).unwrap();
                    keys.push(make_key(Provider::Satellite, z, x, y, 0));
                }
            }
        }
        keys
    }
}
impl Drop for FixtureCache {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
#[ignore = "manual warm CPU-frame benchmark; run alone with --ignored --nocapture --test-threads=1"]
fn warm_cpu_frame_time() {
    warm_cpu_frame_for_model(earthcraft_core::earth_model::EarthModel::PerfectSphere);
}
#[test]
#[ignore = "manual ellipsoid CPU-frame benchmark; run alone with --ignored --nocapture --test-threads=1"]
fn warm_ellipsoid_cpu_frame_time() {
    warm_cpu_frame_for_model(earthcraft_core::earth_model::EarthModel::Wgs84);
}

fn warm_cpu_frame_for_model(model: earthcraft_core::earth_model::EarthModel) {
    let fixture = FixtureCache::new();
    let keys = fixture.prepare();
    let runtime = earthcraft_data::runtime::ServiceRuntime::new().unwrap();
    let cache = TileCache::with_cache_dir(
        || {},
        runtime.handle(),
        fixture.root.join("earthcraft_tiles"),
    );
    let started = Instant::now();
    let mut warmup = earthcraft_data::tiles::TileWarmup::startup_satellite();
    while warmup.pump(&cache) {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "fixture warmup must finish"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    while keys
        .iter()
        .any(|&key| !matches!(cache.lookup(key), Lookup::Ready(_)))
    {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "fixture tiles must load from isolated disk cache"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let basis = Cam {
        lat: -15.0,
        lon: -50.0,
        heading: 25.0,
        alt: 14000.0,
    }
    .basis_for_model(FRAME_WIDTH_PX as f64 / FRAME_HEIGHT_PX as f64, model);
    let config = RenderCfg {
        provider: Provider::Satellite,
        rel: 0,
        grid: true,
        grid_step: 10.0,
        atmo: true,
        sun: Some(earthcraft_core::math::ll_to_v3(20.0, 45.0)),
    };
    // Global overview warming must also cover parts of the globe that were
    // behind the camera at startup, including the dateline and poles.
    for lat in [-90.0, -15.0, 0.0, 90.0] {
        for lon in [-180.0, -90.0, 0.0, 90.0, 180.0] {
            let rotated = Cam {
                lat,
                lon,
                heading: 25.0,
                alt: 14_000.0,
            }
            .basis_for_model(FRAME_WIDTH_PX as f64 / FRAME_HEIGHT_PX as f64, model);
            black_box(render_frame(
                FRAME_WIDTH_PX,
                FRAME_HEIGHT_PX,
                &rotated,
                &config,
                &cache,
            ));
            assert_eq!(
                cache.inflight(),
                0,
                "overview rotations must stay in memory"
            );
        }
    }
    black_box(render_frame(
        FRAME_WIDTH_PX,
        FRAME_HEIGHT_PX,
        &basis,
        &config,
        &cache,
    ));
    let mut samples = Vec::new();
    for _ in 0..BENCH_ROUNDS {
        let started = Instant::now();
        for _ in 0..BENCH_FRAMES {
            black_box(render_frame(
                FRAME_WIDTH_PX,
                FRAME_HEIGHT_PX,
                &basis,
                &config,
                &cache,
            ));
        }
        samples.push(started.elapsed().as_secs_f64() * 1000.0 / BENCH_FRAMES as f64);
    }
    assert_eq!(
        cache.inflight(),
        0,
        "warm frame must not request network tiles"
    );
    samples.sort_by(f64::total_cmp);
    println!(
        "CPU-frame samples (ms): {samples:?}; Rayon threads: {}",
        rayon::current_num_threads()
    );
    println!(
        "Warm 512x384 CPU frame: median {:.3} ms; 341 256x256 RGBA fixture tiles (85.25 MiB decoded), 5 rounds",
        samples[BENCH_ROUNDS / 2]
    );
}

#[cfg(feature = "gpu")]
#[test]
#[ignore = "manual native GPU/CPU comparison; run alone with --ignored --nocapture --test-threads=1"]
fn warm_gpu_frame_time() {
    let fixture = FixtureCache::new();
    let keys = fixture.prepare();
    let runtime = earthcraft_data::runtime::ServiceRuntime::new().unwrap();
    let cache = TileCache::with_cache_dir(
        || {},
        runtime.handle(),
        fixture.root.join("earthcraft_tiles"),
    );
    let started = Instant::now();
    let mut warmup = earthcraft_data::tiles::TileWarmup::startup_satellite();
    while warmup.pump(&cache)
        || keys
            .iter()
            .any(|&key| !matches!(cache.lookup(key), Lookup::Ready(_)))
    {
        assert!(started.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut gpu = crate::gpu::GpuRenderer::new().unwrap();
    for (w, h) in [(512, 384), (2000, 1488)] {
        let cam = Cam {
            lat: -15.0,
            lon: -50.0,
            heading: 25.0,
            alt: 14000.0,
        };
        let request = crate::worker::RenderRequest {
            width: w,
            height: h,
            cam,
            basis: cam.basis(w as f64 / h as f64),
            cfg: RenderCfg {
                provider: Provider::Satellite,
                rel: 0,
                grid: false,
                grid_step: 10.0,
                atmo: true,
                sun: Some(earthcraft_core::math::ll_to_v3(20.0, 45.0)),
            },
            overlays: None,
            aircraft: None,
            presentation: 0,
        };
        let mut cpu_times = Vec::new();
        let mut gpu_times = Vec::new();
        let mut cpu_buffer = Vec::new();
        let mut gpu_buffer = Vec::new();
        for round in 0..16 {
            let start = Instant::now();
            let cpu = render_view_into(w, h, &request.basis, &request.cfg, &cache, cpu_buffer);
            let cpu_time = start.elapsed();
            let start = Instant::now();
            let accelerated = gpu
                .render(&request, &cache, gpu_buffer)
                .unwrap_or_else(|(error, _, _)| panic!("{error}"));
            let gpu_time = start.elapsed();
            assert_eq!(
                cpu.waiting
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>(),
                accelerated
                    .waiting
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
            );
            if round == 0 {
                let mut histogram = [0usize; 256];
                for (a, b) in cpu.pixels.iter().zip(&accelerated.pixels) {
                    histogram[a.abs_diff(*b) as usize] += 1;
                }
                let maximum = histogram.iter().rposition(|count| *count > 0).unwrap();
                eprintln!(
                    "{w}x{h} channel difference max {maximum}; channels above one step {}",
                    histogram[2..].iter().sum::<usize>()
                );
                let worst = cpu
                    .pixels
                    .iter()
                    .zip(&accelerated.pixels)
                    .position(|(a, b)| a.abs_diff(*b) as usize == maximum)
                    .unwrap();
                eprintln!(
                    "worst pixel ({},{}), channel {}",
                    worst / 4 % w,
                    worst / 4 / w,
                    worst % 4
                );
            }
            assert!(
                cpu.pixels
                    .iter()
                    .zip(&accelerated.pixels)
                    .all(|(a, b)| a.abs_diff(*b) <= 1),
                "GPU pixels exceed the one-channel quantization tolerance"
            );
            cpu_buffer = cpu.pixels;
            gpu_buffer = accelerated.pixels;
            if round > 0 {
                cpu_times.push(cpu_time);
                gpu_times.push(gpu_time);
            }
        }
        cpu_times.sort();
        gpu_times.sort();
        eprintln!(
            "{w}x{h}: CPU median {:.3} ms; GPU including readback and sky repair median {:.3} ms (15 warm frames)",
            cpu_times[7].as_secs_f64() * 1000.0,
            gpu_times[7].as_secs_f64() * 1000.0
        );
    }
    // A decoded cache larger than the GPU atlas must not disable acceleration
    // when the actual view fits. Seed extra detail through the normal disk loader.
    let Lookup::Ready(root_tile) = cache.lookup(make_key(Provider::Satellite, 0, 0, 0, 0)) else {
        panic!("warm root tile");
    };
    let image = image::RgbaImage::from_raw(256, 256, root_tile.rgba().2.to_vec()).unwrap();
    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let mut detail = Vec::new();
    for x in 0..16 {
        for y in 0..16 {
            let path = fixture
                .root
                .join("earthcraft_tiles/esri_sat/5")
                .join(x.to_string())
                .join(format!("{y}.tile"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, encoded.get_ref()).unwrap();
            detail.push(make_key(Provider::Satellite, 5, x, y, 0));
        }
    }
    let started = Instant::now();
    while detail
        .iter()
        .any(|&key| !matches!(cache.lookup(key), Lookup::Ready(_)))
    {
        cache.prioritize_view(&detail);
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "isolated detail loading"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(cache.ready_tiles(Provider::Satellite, 0).len() > 512);
    for (lat, lon) in [(-15.0, -50.0), (40.0, 80.0), (-30.0, 170.0)] {
        let cam = Cam {
            lat,
            lon,
            heading: 25.0,
            alt: 14000.0,
        };
        let request = crate::worker::RenderRequest {
            width: 2000,
            height: 1488,
            cam,
            basis: cam.basis(2000.0 / 1488.0),
            cfg: RenderCfg {
                provider: Provider::Satellite,
                rel: 0,
                grid: false,
                grid_step: 10.0,
                atmo: true,
                sun: Some(earthcraft_core::math::ll_to_v3(20.0, 45.0)),
            },
            overlays: None,
            aircraft: None,
            presentation: 0,
        };
        let frame = gpu
            .render(&request, &cache, Vec::new())
            .unwrap_or_else(|(error, _, _)| {
                panic!("large decoded cache must remain GPU-renderable: {error}")
            });
        let cpu = render_view_into(
            request.width,
            request.height,
            &request.basis,
            &request.cfg,
            &cache,
            Vec::new(),
        );
        assert!(
            frame
                .pixels
                .iter()
                .zip(cpu.pixels)
                .all(|(a, b)| a.abs_diff(b) <= 1),
            "large-cache rotation pixel parity"
        );
        assert_eq!(
            frame
                .waiting
                .into_iter()
                .collect::<std::collections::HashSet<_>>(),
            cpu.waiting.into_iter().collect()
        );
    }
    eprintln!(
        "597 decoded tiles: GPU residency and pixel/dependency parity passed across three hemispheric views"
    );
    drop(cache);
    drop(runtime);
    drop(fixture);
}
