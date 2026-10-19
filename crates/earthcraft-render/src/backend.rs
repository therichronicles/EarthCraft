//! Backend selection owned by the background frame coordinator.
use crate::{RenderedView, worker::RenderRequest};
use earthcraft_data::tiles::TileCache;
// Complete 512x384 GPU/CPU frames were tied on the baseline adapter; avoid
// GPU synchronization overhead for small rasters while retaining forced tests.
#[cfg(feature = "gpu")]
const MIN_AUTO_GPU_PIXELS: usize = 256 * 1024;

// Optional GPU backend failures must not unwind the frame coordinator. A panic
// loses its in-progress buffer; CPU fallback allocates another only on failure.
#[cfg(feature = "gpu")]
fn gpu_call<T>(operation: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).map_err(|payload| {
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("non-string panic payload");
        format!("GPU backend panicked: {message}")
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendPreference {
    Auto,
    Cpu,
    Gpu,
}
impl BackendPreference {
    pub fn from_environment() -> Self {
        Self::from_setting(std::env::var("EARTHCRAFT_RENDERER").ok().as_deref())
    }
    fn from_setting(setting: Option<&str>) -> Self {
        match setting {
            Some("gpu") => Self::Gpu,
            Some("auto") => Self::Auto,
            Some("cpu") | None => Self::Cpu,
            Some(value) => {
                eprintln!("Unknown EARTHCRAFT_RENDERER value {value:?}; using CPU rendering");
                Self::Cpu
            }
        }
    }
}

/// Created cheaply on the UI thread; adapter/device initialization is deferred
/// until `render` runs on the frame coordinator. A failed device is not retried
/// every frame. Restarting the application permits another initialization attempt.
pub struct FrameRenderer {
    preference: BackendPreference,
    initialized: bool,
    #[cfg(feature = "gpu")]
    gpu: Option<crate::gpu::GpuRenderer>,
    #[cfg(feature = "gpu")]
    capacity_warning: bool,
}
impl FrameRenderer {
    pub fn new(preference: BackendPreference) -> Self {
        Self {
            preference,
            initialized: false,
            #[cfg(feature = "gpu")]
            gpu: None,
            #[cfg(feature = "gpu")]
            capacity_warning: false,
        }
    }
    #[cfg(feature = "gpu")]
    fn initialize_with(
        &mut self,
        initialize: impl FnOnce() -> Result<crate::gpu::GpuRenderer, String>,
    ) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        if self.preference != BackendPreference::Cpu {
            match gpu_call(initialize).and_then(|result| result) {
                Ok(gpu) => self.gpu = Some(gpu),
                Err(error) => eprintln!("GPU renderer unavailable; using CPU: {error}"),
            }
        }
    }
    pub fn render(
        &mut self,
        request: &RenderRequest,
        cache: &TileCache,
        buffer: Vec<u8>,
    ) -> RenderedView {
        #[cfg(feature = "gpu")]
        if self.preference == BackendPreference::Cpu
            || (self.preference == BackendPreference::Auto
                && request.width.saturating_mul(request.height) < MIN_AUTO_GPU_PIXELS)
            || !crate::gpu::GpuRenderer::supports(request)
        {
            return crate::render_view_into(
                request.width,
                request.height,
                &request.basis,
                &request.cfg,
                cache,
                buffer,
            );
        }
        #[cfg(feature = "gpu")]
        self.initialize_with(crate::gpu::GpuRenderer::new);
        #[cfg(not(feature = "gpu"))]
        {
            if !self.initialized && self.preference != BackendPreference::Cpu {
                eprintln!("GPU support is disabled in this build; using CPU renderer");
            }
            self.initialized = true;
        }
        #[cfg(feature = "gpu")]
        if let Some(gpu) = &mut self.gpu {
            let result = gpu_call(|| gpu.render(request, cache, buffer))
                .unwrap_or_else(|error| Err((error, Vec::new(), true)));
            match result {
                Ok(frame) => {
                    self.capacity_warning = false;
                    return frame;
                }
                Err((error, buffer, permanent)) => {
                    if permanent || !self.capacity_warning {
                        eprintln!("GPU renderer using CPU fallback: {error}");
                    }
                    self.capacity_warning = true;
                    if permanent {
                        self.gpu = None;
                    }
                    return crate::render_view_into(
                        request.width,
                        request.height,
                        &request.basis,
                        &request.cfg,
                        cache,
                        buffer,
                    );
                }
            }
        }
        crate::render_view_into(
            request.width,
            request.height,
            &request.basis,
            &request.cfg,
            cache,
            buffer,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpu_rendering_requires_explicit_opt_in() {
        assert_eq!(
            BackendPreference::from_setting(None),
            BackendPreference::Cpu
        );
        assert_eq!(
            BackendPreference::from_setting(Some("cpu")),
            BackendPreference::Cpu
        );
        assert_eq!(
            BackendPreference::from_setting(Some("invalid")),
            BackendPreference::Cpu
        );
        assert_eq!(
            BackendPreference::from_setting(Some("auto")),
            BackendPreference::Auto
        );
        assert_eq!(
            BackendPreference::from_setting(Some("gpu")),
            BackendPreference::Gpu
        );
    }
    #[test]
    fn forced_cpu_never_initializes_a_device() {
        let renderer = FrameRenderer::new(BackendPreference::Cpu);
        #[cfg(feature = "gpu")]
        let mut renderer = renderer;
        #[cfg(feature = "gpu")]
        renderer.initialize_with(|| panic!("forced CPU must skip initialization"));
        assert_eq!(renderer.preference, BackendPreference::Cpu);
        #[cfg(feature = "gpu")]
        assert!(renderer.initialized);
        #[cfg(feature = "gpu")]
        assert!(renderer.gpu.is_none());
    }
}

#[cfg(all(test, feature = "gpu"))]
mod fallback_tests {
    use super::*;
    #[test]
    fn gpu_calls_preserve_results_and_report_render_panics() {
        assert_eq!(gpu_call(|| 42), Ok(42));
        assert_eq!(
            gpu_call(|| panic!("simulated driver render panic")),
            Err::<(), _>("GPU backend panicked: simulated driver render panic".into())
        );
    }
    #[test]
    fn initialization_panic_disables_gpu_without_killing_the_frame_worker() {
        let mut renderer = FrameRenderer::new(BackendPreference::Gpu);
        renderer.initialize_with(|| panic!("simulated driver initialization panic"));
        assert!(renderer.initialized);
        assert!(renderer.gpu.is_none());
        renderer.initialize_with(|| panic!("failed initialization must not repeat"));
    }
    #[test]
    fn initialization_failure_disables_gpu_without_retrying_each_frame() {
        let mut renderer = FrameRenderer::new(BackendPreference::Auto);
        renderer.initialize_with(|| Err("test adapter is unavailable".into()));
        assert!(renderer.initialized);
        assert!(renderer.gpu.is_none());
        renderer.initialize_with(|| panic!("failed initialization must not repeat"));
    }
}

#[cfg(all(test, feature = "gpu"))]
mod native_fallback_tests {
    use super::*;
    #[test]
    #[ignore = "requires native GPU; run explicitly and alone with --test-threads=1"]
    fn gpu_device_loss_publishes_a_cpu_frame() {
        struct Fixture {
            root: std::path::PathBuf,
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.root).unwrap();
            }
        }
        let root = std::env::temp_dir().join(format!(
            "earthcraft-gpu-fallback-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let fixture = Fixture { root };
        let runtime = earthcraft_data::runtime::ServiceRuntime::new().unwrap();
        let cache = TileCache::with_cache_dir(
            || {},
            runtime.handle(),
            fixture.root.join("earthcraft_tiles"),
        );
        let gpu = crate::gpu::GpuRenderer::new().unwrap();
        gpu.destroy_for_test();
        let mut renderer = FrameRenderer {
            preference: BackendPreference::Gpu,
            initialized: true,
            gpu: Some(gpu),
            capacity_warning: false,
        };
        let cam = earthcraft_core::camera::Cam {
            tilt: 0.0,
            lat: -15.0,
            lon: -50.0,
            heading: 25.0,
            alt: 14000.0,
        };
        let request = RenderRequest {
            width: 65,
            height: 49,
            cam,
            basis: cam.basis(65.0 / 49.0),
            cfg: crate::RenderCfg {
                sky: None,
                provider: earthcraft_data::tiles::Provider::Satellite,
                rel: 0,
                grid: false,
                grid_step: 10.0,
                atmo: true,
                sun: None,
            },
            overlays: None,
            aircraft: None,
            live_layers: None,
            presentation: 0,
        };
        let fallback = renderer.render(&request, &cache, Vec::new());
        let expected = crate::render_view_into(
            request.width,
            request.height,
            &request.basis,
            &request.cfg,
            &cache,
            Vec::new(),
        );
        assert_eq!(fallback.pixels, expected.pixels);
        assert_eq!(
            fallback
                .required_tiles()
                .into_iter()
                .collect::<std::collections::HashSet<_>>(),
            expected.required_tiles().into_iter().collect()
        );
        assert!(renderer.gpu.is_none(), "failed device must remain disabled");
        assert_eq!(
            renderer.render(&request, &cache, Vec::new()).pixels,
            expected.pixels
        );
        drop(renderer);
        drop(cache);
        drop(runtime);
        drop(fixture);
    }
}
