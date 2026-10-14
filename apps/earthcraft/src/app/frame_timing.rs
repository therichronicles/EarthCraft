//! Time-based movement must advance once per frame, even with multi-pass layout.
use eframe::egui;

// Bound animation steps after slow frames to prevent sudden camera jumps.
const MAX_FRAME_DELTA_SECS: f32 = 0.1;

pub(super) fn animation_delta_secs(ctx: &egui::Context) -> f64 {
    frame_delta_secs(ctx, ctx.input(|input| input.unstable_dt))
}

pub(super) fn navigation_delta_secs(ctx: &egui::Context) -> f64 {
    frame_delta_secs(ctx, ctx.input(|input| input.stable_dt))
}

fn frame_delta_secs(ctx: &egui::Context, delta_secs: f32) -> f64 {
    if ctx.current_pass_index() == 0 {
        delta_secs.min(MAX_FRAME_DELTA_SECS) as f64
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_layout_passes_do_not_advance_movement_twice() {
        let ctx = egui::Context::default();
        ctx.options_mut(|options| options.max_passes = std::num::NonZeroUsize::new(2).unwrap());
        for time in [1.0, 2.0] {
            let mut deltas = Vec::new();
            let output = crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    deltas.push((animation_delta_secs(ui), navigation_delta_secs(ui)));
                    if ui.current_pass_index() == 0 {
                        ui.request_discard("exercise repeated layout");
                    }
                },
            );
            assert_eq!(output.platform_output.num_completed_passes, 2);
            assert!(deltas[0].0 > 0.0 && deltas[0].0 <= f64::from(MAX_FRAME_DELTA_SECS));
            assert!(deltas[0].1 > 0.0 && deltas[0].1 <= f64::from(MAX_FRAME_DELTA_SECS));
            assert_eq!(deltas[1], (0.0, 0.0));
        }
    }
}
