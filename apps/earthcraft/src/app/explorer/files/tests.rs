use super::*;
use earthcraft_data::{
    files::FileOutcome,
    planet::{decode_planet, encode_planet},
};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Output(PathBuf);
impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn planet(sky: bool) -> LoadedPlanet {
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let png = png.into_inner();
    let bytes = encode_planet("Loaded", png.clone(), if sky { png } else { Vec::new() }).unwrap();
    decode_planet(&bytes, |_| panic!("self-contained document")).unwrap()
}

#[test]
fn saving_a_loaded_planet_uses_its_assets_instead_of_stale_paths_and_snapshots_its_name() {
    let runtime = earthcraft_data::runtime::ServiceRuntime::new().unwrap();
    let (jobs, results) = FileJobs::new(runtime.handle(), || {});
    let ctx = egui::Context::default();
    let loaded = planet(true);
    let assets = loaded.assets.clone();
    let mut explorer = Explorer {
        image_path: "missing-old-texture.png".into(),
        sky_path: "missing-old-sky.png".into(),
        pending: true,
        activate_on_completion: true,
        ..Default::default()
    };
    explorer.planet_loaded(loaded, &ctx, Lang::En);
    assert!(explorer.active == Body::Custom);
    assert!(explorer.image_path.is_empty() && explorer.sky_path.is_empty());
    assert!(Arc::ptr_eq(
        explorer.image_bytes.as_ref().unwrap(),
        &assets.texture
    ));
    assert!(Arc::ptr_eq(
        explorer.sky_bytes.as_ref().unwrap(),
        &assets.sky
    ));
    let output = Output(std::env::temp_dir().join(format!(
        "earthcraft-explorer-resave-{}-{}.earthplanet",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    explorer.request_save(output.0.clone(), &jobs);
    explorer.name = "Edited after submission".into();
    explorer.image_bytes = None;
    explorer.sky_bytes = None;
    let FileOutcome::PlanetSaved { path } = results
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap()
    else {
        panic!("saved planet");
    };
    let saved = earthcraft_data::planet::load_planet(&path).unwrap();
    assert_eq!(saved.name, "Loaded");
    assert_eq!(*saved.assets.texture, *assets.texture);
    assert_eq!(*saved.assets.sky, *assets.sky);
}

#[test]
fn switching_bodies_during_a_load_is_preserved_and_a_planet_without_sky_clears_the_old_sky() {
    let runtime = earthcraft_data::runtime::ServiceRuntime::new().unwrap();
    let (jobs, _) = FileJobs::new(runtime.handle(), || {});
    let ctx = egui::Context::default();
    let mut explorer = Explorer::default();
    explorer.planet_loaded(planet(true), &ctx, Lang::En);
    assert!(explorer.sky.is_some());
    explorer.pending = true;
    explorer.activate_on_completion = true;
    explorer.choose(Body::Earth, &jobs);
    explorer.planet_loaded(planet(false), &ctx, Lang::PtBr);
    assert!(explorer.active == Body::Earth);
    assert!(explorer.sky.is_none() && explorer.sky_bytes.is_none());
    assert!(!explorer.pending);
}

#[test]
fn failed_image_load_releases_busy_state_without_discarding_existing_assets() {
    let ctx = egui::Context::default();
    let mut explorer = Explorer::default();
    explorer.planet_loaded(planet(true), &ctx, Lang::En);
    let previous_texture = explorer.viewer.as_ref().unwrap().id();
    explorer.pending = true;
    explorer.activate_on_completion = true;
    explorer.file_failed(
        FileKind::Image(ImageTarget::Planet),
        std::io::Error::other("fixture failure").into(),
    );
    assert!(!explorer.pending && !explorer.activate_on_completion);
    assert_eq!(explorer.viewer.as_ref().unwrap().id(), previous_texture);
    assert!(explorer.image_bytes.is_some() && explorer.sky_bytes.is_some());
    assert!(explorer.message.contains("fixture failure"));
}

#[test]
fn moon_is_decoded_by_the_file_service_and_does_not_override_a_later_body_selection() {
    let runtime = earthcraft_data::runtime::ServiceRuntime::new().unwrap();
    let (jobs, results) = FileJobs::new(runtime.handle(), || {});
    let ctx = egui::Context::default();
    let mut explorer = Explorer::default();
    explorer.choose(Body::Moon, &jobs);
    assert!(explorer.moon_loading && explorer.moon.is_none());
    explorer.choose(Body::Earth, &jobs);
    let FileOutcome::ImageLoaded { target, image } = results
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap()
    else {
        panic!("decoded moon");
    };
    assert_eq!(target, ImageTarget::Moon);
    explorer.image_loaded(target, image, &ctx, Lang::En);
    assert!(explorer.active == Body::Earth);
    assert!(!explorer.moon_loading && explorer.moon.is_some());
}
