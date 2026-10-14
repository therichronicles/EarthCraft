//! Count presented globe frames, independently of UI-only repaints.
use eframe::egui;
use std::time::{Duration, Instant};

const FPS_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

pub(in crate::app) struct FrameRate {
    started: Instant,
    frames: usize,
    rate: Option<f64>,
}
impl Default for FrameRate {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            frames: 0,
            rate: None,
        }
    }
}
impl FrameRate {
    pub(in crate::app) fn presented(&mut self) {
        self.frames += 1;
    }
    pub(super) fn value(&self) -> Option<f64> {
        self.rate
    }
    pub(super) fn refresh(&mut self, ctx: &egui::Context) {
        let elapsed = self.started.elapsed();
        if elapsed >= FPS_SAMPLE_INTERVAL {
            // An idle map produces no new frames; retain the latest active sample.
            if self.frames > 0 {
                self.rate = Some(self.frames as f64 / elapsed.as_secs_f64());
            }
            self.frames = 0;
            self.started = Instant::now();
        }
        // Refresh the readout at rest, without scheduling another globe raster.
        ctx.request_repaint_after(FPS_SAMPLE_INTERVAL.saturating_sub(self.started.elapsed()));
    }
}
