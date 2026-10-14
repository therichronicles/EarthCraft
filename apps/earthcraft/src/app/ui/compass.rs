//! Desktop compass widgets; geographic bearings live in core.
use earthcraft_core::{compass::heading_text, language::Lang};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Sense, Vec2};
const CARDINAL_FONT_POINTS: f32 = 10.5;
const NORTH_COLOR: Color32 = Color32::from_rgb(240, 90, 90);

pub(super) fn heading_readout(ui: &mut egui::Ui, heading: f64, lang: Lang) -> bool {
    // Match the status bar's horizontal row height and baseline.
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(egui::RichText::new(heading_text(heading, lang)).monospace())
                .sense(Sense::click()),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(lang.text("compass-heading-tooltip"))
        .clicked()
    })
    .inner
}

/// Decorate the existing rotation ring without replacing its input or buttons.
pub(super) fn draw_cardinals(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    scale: f32,
    heading: f64,
    lang: Lang,
) {
    for (offset, id) in [
        (0.0_f64, "compass-north"),
        (90.0, "compass-east"),
        (180.0, "compass-south"),
        (270.0, "compass-west"),
    ] {
        let angle = (offset - heading).to_radians();
        let direction = Vec2::new(angle.sin() as f32, -angle.cos() as f32);
        painter.text(
            center + direction * radius,
            Align2::CENTER_CENTER,
            lang.text(id),
            FontId::proportional(CARDINAL_FONT_POINTS * scale),
            if offset == 0.0 {
                NORTH_COLOR
            } else {
                Color32::WHITE
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compass_uses_displayed_heading_and_localized_cardinals() {
        for lang in Lang::ALL {
            let ctx = egui::Context::default();
            let output = crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
                let ctx: &egui::Context = ui;
                draw_cardinals(
                    &ctx.layer_painter(egui::LayerId::background()),
                    Pos2::new(100.0, 100.0),
                    33.5,
                    1.0,
                    90.0,
                    lang,
                );
            });
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) => Some(t),
                    _ => None,
                })
                .collect();
            assert_eq!(labels.len(), 4);
            let north = labels.iter().find(|t| t.galley.job.text == "N").unwrap();
            assert!(north.pos.x + north.galley.size().x < 100.0);
            assert!(
                labels
                    .iter()
                    .any(|t| t.galley.job.text == lang.text("compass-west"))
            );
        }
    }
    #[test]
    fn heading_readout_resets_only_when_clicked_and_matches_status_baseline() {
        let ctx = egui::Context::default();
        let frame = |events| {
            let mut clicked = false;
            let output = crate::app::test_ui::run(
                &ctx,
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        let origin = ui.next_widget_position();
                        ui.scope_builder(
                            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                                origin,
                                egui::vec2(180.0, 30.0),
                            )),
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.monospace("Center");
                                });
                            },
                        );
                        ui.scope_builder(
                            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                                origin + egui::vec2(200.0, 0.0),
                                egui::vec2(180.0, 30.0),
                            )),
                            |ui| {
                                clicked = heading_readout(ui, 0.0, Lang::En);
                            },
                        );
                    });
                },
            );
            (clicked, output)
        };
        let (_, output) = frame(vec![]);
        let text = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "0° N" => Some(t),
                _ => None,
            })
            .unwrap();
        let center_y = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Center" => Some(t.pos.y),
                _ => None,
            })
            .unwrap();
        assert_eq!(text.pos.y, center_y);
        let pos = text.pos + text.galley.size() * 0.5;
        assert!(!frame(vec![egui::Event::PointerMoved(pos)]).0);
        let button = |pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        assert!(!frame(vec![button(true)]).0);
        assert!(frame(vec![button(false)]).0);
    }
}
