//! The attribute box opened by clicking a line on the globe.
use crate::app::App;
use eframe::egui::{self, Align, Layout, Vec2};

const POPUP_WIDTH_POINTS: f32 = 340.0;
const POPUP_ATTRIBUTE_HEIGHT_POINTS: f32 = 260.0;
const POPUP_OFFSET_POINTS: f32 = 14.0;

impl App {
    pub(super) fn show_line_popup(&mut self, ctx: &egui::Context) {
        let Some((id, at)) = self.line_pick.open else {
            return;
        };
        let Some(pm) = self.placemarks.find_marker(id) else {
            self.line_pick.open = None;
            return;
        };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        egui::Area::new(egui::Id::new("line-popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(at + Vec2::splat(POPUP_OFFSET_POINTS))
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(POPUP_WIDTH_POINTS);
                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        close |= ui.small_button("✕").clicked();
                        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
                            ui.add(egui::Label::new(egui::RichText::new(&pm.name).strong()).wrap());
                        });
                    });
                    if pm.attributes.is_empty() {
                        if !pm.desc.is_empty() {
                            ui.add(egui::Label::new(&pm.desc).wrap());
                        }
                    } else {
                        ui.separator();
                        crate::app::ui::attribute_grid(
                            ui,
                            id,
                            &pm.attributes,
                            POPUP_ATTRIBUTE_HEIGHT_POINTS,
                        );
                    }
                });
            });
        if close {
            self.line_pick.open = None;
        }
    }
}
