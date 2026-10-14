//! Native UI presentation configuration; globe computation stays in the renderer.
use egui_wgpu::WgpuConfiguration;
use wgpu::PresentMode;

pub(crate) fn configuration(uncap_fps: bool) -> WgpuConfiguration {
    let mut configuration = eframe::NativeOptions::default().wgpu_options;
    if uncap_fps {
        configuration.surface.present_mode = PresentMode::AutoNoVsync;
    }
    configuration
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_mode_preserves_the_saved_vsync_preference() {
        let defaults = eframe::NativeOptions::default();
        assert_eq!(defaults.renderer, eframe::Renderer::Wgpu);
        let capped = configuration(false);
        let uncapped = configuration(true);
        assert_eq!(
            capped.surface.present_mode,
            defaults.wgpu_options.surface.present_mode
        );
        assert_eq!(uncapped.surface.present_mode, PresentMode::AutoNoVsync);
        for configuration in [capped, uncapped] {
            assert_eq!(
                configuration.surface.desired_maximum_frame_latency,
                defaults.wgpu_options.surface.desired_maximum_frame_latency
            );
        }
    }
}
