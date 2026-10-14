use super::*;
use earthcraft_core::{camera::Cam, math::ll_to_v3};
use earthcraft_data::tiles::{Lookup, Provider, make_key};

#[test]
#[ignore = "requires a native GPU; run explicitly with --ignored --nocapture"]
fn gpu_overview_matches_cpu() {
    let mut gpu = GpuRenderer::new().expect("native GPU adapter and validated shader");
    let image = image::RgbaImage::from_fn(256, 256, |x, y| {
        image::Rgba([x as u8, y as u8, (x ^ y) as u8, 255])
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let tile = Arc::new(Tile::decode(bytes.get_ref()).unwrap());
    let (w, h) = (65, 49);
    for provider in Provider::ALL {
        for (lat, lon, alt) in [
            (-15.0, -50.0, 14000.0),
            (89.9, 179.9, 1000.0),
            (-89.9, -179.9, 14000.0),
            (0.0, 180.0, 30000.0),
        ] {
            for enabled in [false, true] {
                let cam = Cam {
                    lat,
                    lon,
                    alt,
                    heading: 25.0,
                };
                let request = RenderRequest {
                    width: w,
                    height: h,
                    cam,
                    basis: cam.basis(w as f64 / h as f64),
                    cfg: crate::RenderCfg {
                        provider,
                        rel: 7,
                        grid: false,
                        grid_step: 10.0,
                        atmo: enabled,
                        sun: enabled.then(|| ll_to_v3(20.0, 45.0)),
                    },
                    overlays: None,
                    aircraft: None,
                    presentation: 0,
                };
                for (loaded, pending) in [(false, false), (false, true), (true, false)] {
                    let key = make_key(provider, 0, 0, 0, 7);
                    let slots = gpu
                        .upload(&if loaded {
                            vec![(key, tile.clone())]
                        } else {
                            Vec::new()
                        })
                        .unwrap();
                    let lookup = |k| {
                        if loaded && k == key {
                            Lookup::Ready(tile.clone())
                        } else if pending && k == key {
                            Lookup::Pending
                        } else {
                            Lookup::Absent
                        }
                    };
                    let (cpu, cpu_misses, cpu_waiting) =
                        crate::render_pixels_tracked(w, h, &request.basis, &request.cfg, &lookup);
                    let frame = gpu
                        .overview
                        .render(
                            overview::Context {
                                device: &gpu.device,
                                queue: &gpu.queue,
                                atlas: &gpu.view,
                            },
                            &request,
                            &slots,
                            &lookup,
                            Vec::new(),
                        )
                        .unwrap_or_else(|(error, _, _)| panic!("{error}"))
                        .view;
                    for (i, (actual, expected)) in frame.pixels.iter().zip(&cpu).enumerate() {
                        assert!(
                            actual.abs_diff(*expected) <= 1,
                            "{provider:?} alt={alt} enabled={enabled}, channel {i}: {actual} != {expected}"
                        );
                    }
                    assert_eq!(
                        frame
                            .waiting
                            .iter()
                            .copied()
                            .collect::<std::collections::HashSet<_>>(),
                        cpu_waiting.into_iter().collect(),
                        "pending and missing dependencies must match CPU"
                    );
                    let actual: std::collections::HashSet<_> = frame.requests.into_iter().collect();
                    let expected = cpu_misses.into_iter().map(|entry| entry.0).collect();
                    assert_eq!(actual, expected, "requested tile coverage must match CPU");
                }
            }
        }
    }
    assert!(
        gpu.upload(&[(
            0,
            Arc::new(Tile::decode(include_bytes!("../../tests/fixtures/tile.png")).unwrap())
        )])
        .is_err()
    );
    assert!(
        gpu.upload(
            &(0..MAX_ATLAS_LAYERS as u64 + 1)
                .map(|key| (key, tile.clone()))
                .collect::<Vec<_>>()
        )
        .is_err()
    );
    gpu.device.destroy();
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    assert!(
        gpu.failure.lock().unwrap().is_some(),
        "device loss must be reported for CPU fallback"
    );
}

#[test]
fn unsupported_views_choose_precision_instead_of_truncating_gpu_work() {
    let cam = Cam {
        lat: -15.0,
        lon: -50.0,
        heading: 25.0,
        alt: 14000.0,
    };
    let mut request = RenderRequest {
        width: 2000,
        height: 1488,
        cam,
        basis: cam.basis(2000.0 / 1488.0),
        cfg: crate::RenderCfg {
            provider: Provider::Satellite,
            rel: 0,
            grid: false,
            grid_step: 10.0,
            atmo: true,
            sun: None,
        },
        overlays: None,
        aircraft: None,
        presentation: 0,
    };
    assert!(GpuRenderer::supports(&request));
    request.basis = request.cam.basis_for_model(
        request.width as f64 / request.height as f64,
        earthcraft_core::earth_model::EarthModel::Wgs84,
    );
    assert!(!GpuRenderer::supports(&request));
    request.basis = request
        .cam
        .basis(request.width as f64 / request.height as f64);
    request.cam.alt = 0.15;
    assert!(!GpuRenderer::supports(&request));
    request.cam.alt = 14000.0;
    request.cfg.grid = true;
    assert!(!GpuRenderer::supports(&request));
    request.cfg.grid = false;
    request.width = usize::MAX;
    assert!(!GpuRenderer::supports(&request));
    request.width = 0;
    assert!(!GpuRenderer::supports(&request));
}
