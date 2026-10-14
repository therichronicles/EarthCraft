//! Isolated production ray/surface costs; no window, services or cache workers.
use super::*;
use earthcraft_core::earth_model::EarthModel;

const WIDTH_PX: usize = 128;
const HEIGHT_PX: usize = 96;
const ROUNDS: usize = 7;
const REPEATS: usize = 32;

fn views(model: EarthModel) -> Vec<RayCaster> {
    [
        (45.0, 179.9, 0.15),
        (89.9, -170.0, 500.0),
        (-15.0, -50.0, 14000.0),
    ]
    .into_iter()
    .map(|(lat, lon, alt)| {
        RayCaster::new(
            WIDTH_PX,
            HEIGHT_PX,
            &Cam {
                lat,
                lon,
                alt,
                heading: 25.0,
            }
            .basis_for_model(WIDTH_PX as f64 / HEIGHT_PX as f64, model),
        )
    })
    .collect()
}

#[test]
#[ignore = "manual ray/surface microbenchmark; run alone with --ignored --nocapture"]
fn earth_model_kernel_time() {
    let cfg = RenderCfg {
        provider: Provider::Satellite,
        rel: 0,
        grid: true,
        grid_step: 10.0,
        atmo: true,
        sun: Some(earthcraft_core::math::ll_to_v3(20.0, 45.0)),
    };
    for model in EarthModel::ALL {
        let views = views(model);
        let mut surfaces = Vec::new();
        for caster in &views {
            for y in 0..HEIGHT_PX {
                for x in (0..WIDTH_PX).step_by(RAY_LANES) {
                    for (lane, ray) in caster.cast(x, y).into_iter().enumerate() {
                        if ray.discriminant >= 0.0 && ray.camera_dot < 0.0 {
                            surfaces.push((x + lane, y, ray));
                        }
                    }
                }
            }
        }
        let mut ray_times = Vec::new();
        let mut surface_times = Vec::new();
        for _ in 0..ROUNDS {
            let started = Instant::now();
            for _ in 0..REPEATS {
                for caster in &views {
                    for y in 0..HEIGHT_PX {
                        for x in (0..WIDTH_PX).step_by(RAY_LANES) {
                            black_box(caster.cast(black_box(x), black_box(y)));
                        }
                    }
                }
            }
            ray_times.push(
                started.elapsed().as_nanos() as f64
                    / (REPEATS * views.len() * WIDTH_PX * HEIGHT_PX / RAY_LANES) as f64,
            );
            let started = Instant::now();
            for _ in 0..REPEATS {
                for (x, y, ray) in &surfaces {
                    let ray = black_box(ray);
                    let result = match model {
                        EarthModel::PerfectSphere => surface::<false>(
                            *x,
                            *y,
                            WIDTH_PX,
                            HEIGHT_PX,
                            ray,
                            &cfg,
                            model.geometry(),
                        ),
                        EarthModel::Wgs84 => surface::<true>(
                            *x,
                            *y,
                            WIDTH_PX,
                            HEIGHT_PX,
                            ray,
                            &cfg,
                            model.geometry(),
                        ),
                    };
                    black_box(result);
                }
            }
            surface_times
                .push(started.elapsed().as_nanos() as f64 / (REPEATS * surfaces.len()) as f64);
        }
        ray_times.sort_by(f64::total_cmp);
        surface_times.sort_by(f64::total_cmp);
        println!(
            "{model:?}: ray ns/4 lanes {ray_times:?}; surface ns/hit {surface_times:?}; {} surface samples",
            surfaces.len()
        );
    }
}

#[test]
#[ignore = "manual differential capture: set EARTHCRAFT_BENCH_OUTPUT to an isolated directory"]
fn earth_model_raster_capture() {
    let root = PathBuf::from(
        std::env::var_os("EARTHCRAFT_BENCH_OUTPUT").expect("explicit capture directory"),
    );
    std::fs::create_dir_all(&root).unwrap();
    let tile = Arc::new(Tile::decode(include_bytes!("../../tests/fixtures/tile.png")).unwrap());
    for model in EarthModel::ALL {
        for (view, (lat, lon, alt)) in [
            (0.0, 179.9, 0.15),
            (89.9, -170.0, 500.0),
            (-15.0, -50.0, 14000.0),
            (-89.9, 179.9, 40000.0),
        ]
        .into_iter()
        .enumerate()
        {
            let basis = Cam {
                lat,
                lon,
                alt,
                heading: 25.0,
            }
            .basis_for_model(127.0 / 95.0, model);
            for (provider, cfg_provider) in Provider::ALL.into_iter().enumerate() {
                let cfg = RenderCfg {
                    provider: cfg_provider,
                    rel: 0,
                    grid: true,
                    grid_step: 10.0,
                    atmo: true,
                    sun: Some(earthcraft_core::math::ll_to_v3(20.0, 45.0)),
                };
                let (pixels, _) =
                    render_pixels(127, 95, &basis, &cfg, &|_| Lookup::Ready(tile.clone()));
                let (_, mut misses) = render_pixels(127, 95, &basis, &cfg, &|_| Lookup::Absent);
                misses.sort_by_key(|entry| entry.0);
                std::fs::write(
                    root.join(format!("{}-{view}-{provider}.rgba", model.code())),
                    pixels,
                )
                .unwrap();
                std::fs::write(
                    root.join(format!("{}-{view}-{provider}.txt", model.code())),
                    format!("{misses:?}"),
                )
                .unwrap();
            }
        }
    }
}
