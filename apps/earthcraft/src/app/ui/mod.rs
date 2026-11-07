mod places;
use eframe::egui::{self, FontId};
pub(in crate::app) use places::attribute_grid;
pub(in crate::app) mod aircraft;
pub(in crate::app) mod altitude;
mod bars;
mod compass;
mod coordinate_display;
mod cursor;
mod navigation;
mod paths;
pub(in crate::app) mod ruler;
mod scale_slider;
mod search_bar;
mod selection;
pub(in crate::app) mod settings;
mod sidebar;
mod status;
mod suggestions;
mod toolbar;
mod updater;
pub(in crate::app) fn apply_ui_scale(ctx: &egui::Context, scale: f32) {
    use egui::{
        FontFamily::{Monospace, Proportional},
        TextStyle,
    };
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::new(10.0 * scale, Proportional)),
            (TextStyle::Body, FontId::new(14.0 * scale, Proportional)),
            (TextStyle::Button, FontId::new(14.0 * scale, Proportional)),
            (TextStyle::Heading, FontId::new(19.0 * scale, Proportional)),
            (TextStyle::Monospace, FontId::new(13.0 * scale, Monospace)),
        ]
        .into();
        style.spacing.interact_size.y = (20.0 * scale).max(18.0);
    });
}
