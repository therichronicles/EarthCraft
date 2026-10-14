use super::super::App;
use crate::app::PAN_SPEED_POINTS_PER_SECOND;
use earthcraft_core::math::wrap_lon;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
// Navigation palette and geometry in egui points before UI scaling.
const MIN_CONTROL_SCALE: f32 = 0.8;
const MAX_CONTROL_SCALE: f32 = 1.8;
const CONTROL_FILL_RGBA: [u8; 4] = [40, 70, 140, 215];
const CONTROL_HOVER_FILL_RGBA: [u8; 4] = [75, 115, 205, 235];
const CONTROL_BORDER_ALPHA: u8 = 200;
const CONTROL_ARROW_COLOR: Color32 = Color32::from_rgb(205, 222, 255);
const CONTROL_RIGHT_MARGIN_POINTS: f32 = 56.0;
const CONTROL_TOP_MARGIN_POINTS: f32 = 52.0;
const ROTATION_RING_RADIUS_POINTS: f32 = 40.0;
const CONTROL_DISC_RADIUS_POINTS: f32 = 27.0;
const CONTROL_GAP_POINTS: f32 = 12.0;
const ZOOM_TRACK_LENGTH_POINTS: f32 = 110.0;
const ARROW_BASE_RATIO: f32 = 0.6;
const ARROW_HALF_WIDTH_RATIO: f32 = 0.9;
const EYE_OUTLINE_SEGMENTS: usize = 24;
const HEADING_DEG_PER_SECOND: f64 = 60.0;

impl App {
    /// Google Earth style control cluster: rotation ring, move pad and zoom slider.
    /// Returns true while the user is operating a control.
    pub(in crate::app) fn nav_controls(&mut self, ui: &mut egui::Ui, rect: Rect, dt: f64) -> bool {
        if self.settings.hide_navigation_controls {
            self.ring_drag = false;
            return false;
        }
        let lg = self.lang;
        let k = self
            .settings
            .ui_scale
            .clamp(MIN_CONTROL_SCALE, MAX_CONTROL_SCALE);
        let painter = ui.painter_at(rect);
        let base = ui.id().with("nav");
        let mut active = false;

        let rgba = |[r, g, b, a]: [u8; 4]| Color32::from_rgba_unmultiplied(r, g, b, a);
        let fill = rgba(CONTROL_FILL_RGBA);
        let fill_hot = rgba(CONTROL_HOVER_FILL_RGBA);
        let edge = Color32::from_white_alpha(CONTROL_BORDER_ALPHA);
        let arrow_col = CONTROL_ARROW_COLOR;

        let cx = rect.right() - CONTROL_RIGHT_MARGIN_POINTS * k;
        let c1 = Pos2::new(cx, rect.top() + CONTROL_TOP_MARGIN_POINTS * k);
        let (r_ring, r_disc) = (
            ROTATION_RING_RADIUS_POINTS * k,
            CONTROL_DISC_RADIUS_POINTS * k,
        );
        let c2 = Pos2::new(cx, c1.y + r_ring + CONTROL_GAP_POINTS * k + r_disc);
        let dirs = [
            Vec2::new(0.0, -1.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
        ]; // up, down, left, right

        let tri = |center: Pos2, d: Vec2, size: f32, col: Color32| {
            let n = Vec2::new(-d.y, d.x);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + d * size,
                    center - d * size * ARROW_BASE_RATIO + n * size * ARROW_HALF_WIDTH_RATIO,
                    center - d * size * ARROW_BASE_RATIO - n * size * ARROW_HALF_WIDTH_RATIO,
                ],
                col,
                Stroke::NONE,
            ));
        };

        // ── rotation ring + "look" disc ──
        painter.circle_filled(c1, r_ring, Color32::from_black_alpha(110));
        painter.circle_stroke(
            c1,
            r_ring,
            Stroke::new(1.5 * k, Color32::from_white_alpha(170)),
        );
        painter.circle_filled(c1, r_disc, fill);
        painter.circle_stroke(c1, r_disc, Stroke::new(1.0_f32, edge));
        let displayed_heading = self
            .renderer
            .displayed_view()
            .map_or(self.cam.heading, |(cam, _)| cam.heading);
        super::compass::draw_cardinals(&painter, c1, r_ring - 6.5 * k, k, displayed_heading, lg);
        let hr = displayed_heading.to_radians() as f32;
        let nd = Vec2::new(-hr.sin(), -hr.cos()); // screen direction of north
        let n_pos = c1 + nd * (r_ring - 6.5 * k);
        // eye icon
        let eye: Vec<Pos2> = (0..=EYE_OUTLINE_SEGMENTS)
            .map(|i| {
                let t = i as f32 / EYE_OUTLINE_SEGMENTS as f32 * std::f32::consts::TAU;
                c1 + Vec2::new(t.cos() * 9.5 * k, t.sin() * 5.5 * k)
            })
            .collect();
        painter.add(egui::Shape::line(eye, Stroke::new(1.6_f32, Color32::WHITE)));
        painter.circle_filled(c1, 3.2 * k, Color32::WHITE);

        let ring = ui.interact(
            Rect::from_center_size(c1, Vec2::splat(2.0 * r_ring)),
            base.with("ring"),
            Sense::drag(),
        );
        if ring.drag_started() {
            self.ring_drag = ring
                .interact_pointer_pos()
                .is_some_and(|p| (p - c1).length() >= r_disc - 1.0);
        }
        if ring.dragged()
            && self.ring_drag
            && let Some(cur) = ring.interact_pointer_pos()
        {
            let prev = cur - ui.input(|i| i.pointer.delta());
            let ang = |v: Vec2| v.y.atan2(v.x);
            let mut d = ang(cur - c1) - ang(prev - c1);
            if d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            if d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            self.cam.heading = wrap_lon(self.cam.heading - d.to_degrees() as f64);
            self.fly = None;
            active = true;
        }
        if ring.drag_stopped() {
            self.ring_drag = false;
        }
        let nb = ui.interact(
            Rect::from_center_size(n_pos, Vec2::splat(16.0 * k)),
            base.with("north"),
            Sense::click(),
        );
        if nb
            .on_hover_text(lg.text("navigation-compass-tooltip"))
            .clicked()
        {
            self.cam.heading = 0.0;
            self.fly = None;
            active = true;
        }
        // arrows: left/right rotate the view, up = north up, down = flip 180°
        let names = [
            lg.text("navigation-north-tooltip"),
            lg.text("navigation-reverse-tooltip"),
            lg.text("navigation-rotate-left-tooltip"),
            lg.text("navigation-rotate-right-tooltip"),
        ];
        for (i, d) in dirs.iter().enumerate() {
            let c = c1 + *d * 17.0 * k;
            let arrow_rect = Rect::from_center_size(c, Vec2::splat(15.0 * k));
            let id = base.with(("eye", i));
            let r = if i < 2 {
                ui.interact(arrow_rect, id, Sense::click())
            } else {
                hold_control(ui, arrow_rect, id)
            };
            let held = r.is_pointer_button_down_on();
            tri(
                c,
                *d,
                4.8 * k,
                if held || r.hovered() {
                    Color32::WHITE
                } else {
                    arrow_col
                },
            );
            let r = r.on_hover_text(names[i]);
            match i {
                0 if r.clicked() => {
                    self.cam.heading = 0.0;
                    self.fly = None;
                    active = true;
                }
                1 if r.clicked() => {
                    self.cam.heading = wrap_lon(self.cam.heading + 180.0);
                    self.fly = None;
                    active = true;
                }
                2 if held => {
                    self.cam.heading = wrap_lon(self.cam.heading + HEADING_DEG_PER_SECOND * dt);
                    active = true;
                }
                3 if held => {
                    self.cam.heading = wrap_lon(self.cam.heading - HEADING_DEG_PER_SECOND * dt);
                    active = true;
                }
                _ => {}
            }
        }

        // ── move (pan) disc ──
        painter.circle_filled(c2, r_disc, fill);
        painter.circle_stroke(c2, r_disc, Stroke::new(1.0_f32, edge));
        // hand icon: palm + fingers
        painter.circle_filled(c2 + Vec2::new(0.0, 2.5 * k), 5.0 * k, Color32::WHITE);
        for fx in [-4.5f32, -1.5, 1.5, 4.5] {
            painter.line_segment(
                [
                    c2 + Vec2::new(fx * k, 0.0),
                    c2 + Vec2::new(fx * k, -6.5 * k),
                ],
                Stroke::new(2.2 * k, Color32::WHITE),
            );
        }
        let center = ui.interact(
            Rect::from_center_size(c2, Vec2::splat(18.0 * k)),
            base.with("pad"),
            Sense::drag(),
        );
        if center.dragged() {
            let d = ui.input(|i| i.pointer.delta());
            self.fly = None;
            self.pan(d.x as f64, d.y as f64, rect.height());
            active = true;
        }
        let pan_names = [
            lg.text("navigation-pan-north-tooltip"),
            lg.text("navigation-pan-south-tooltip"),
            lg.text("navigation-pan-west-tooltip"),
            lg.text("navigation-pan-east-tooltip"),
        ];
        for (i, d) in dirs.iter().enumerate() {
            let c = c2 + *d * 17.0 * k;
            let r = hold_control(
                ui,
                Rect::from_center_size(c, Vec2::splat(15.0 * k)),
                base.with(("pan", i)),
            );
            let held = r.is_pointer_button_down_on();
            tri(
                c,
                *d,
                4.8 * k,
                if held || r.hovered() {
                    Color32::WHITE
                } else {
                    arrow_col
                },
            );
            r.on_hover_text(pan_names[i]);
            if held {
                let sp = PAN_SPEED_POINTS_PER_SECOND * dt;
                // same convention as the arrow keys
                let (dx, dy) = match i {
                    0 => (0.0, sp),
                    1 => (0.0, -sp),
                    2 => (sp, 0.0),
                    _ => (-sp, 0.0),
                };
                self.fly = None;
                self.pan(dx, dy, rect.height());
                active = true;
            }
        }

        // ── zoom slider ──
        let y0 = c2.y + r_disc + 16.0 * k;
        let r_btn = 11.0 * k;
        let track_len = ZOOM_TRACK_LENGTH_POINTS * k;
        let (t_top, t_bot) = (y0 + r_btn + 8.0 * k, y0 + r_btn + 8.0 * k + track_len);
        let y_minus = t_bot + 8.0 * k + r_btn;
        let cplus = Pos2::new(cx, y0);
        let cminus = Pos2::new(cx, y_minus);
        let t = self.cam.zoom_fraction();

        painter.line_segment(
            [Pos2::new(cx, t_top), Pos2::new(cx, t_bot)],
            Stroke::new(7.0 * k, Color32::from_black_alpha(120)),
        );
        painter.line_segment(
            [Pos2::new(cx, t_top), Pos2::new(cx, t_bot)],
            Stroke::new(4.0 * k, fill),
        );
        for (c, plus) in [(cplus, true), (cminus, false)] {
            let r = hold_control(
                ui,
                Rect::from_center_size(c, Vec2::splat(2.0 * r_btn)),
                base.with(("zoombtn", plus)),
            );
            let held = r.is_pointer_button_down_on();
            painter.circle_filled(c, r_btn, if held || r.hovered() { fill_hot } else { fill });
            painter.circle_stroke(c, r_btn, Stroke::new(1.0_f32, edge));
            let l = 5.0 * k;
            painter.line_segment(
                [c - Vec2::new(l, 0.0), c + Vec2::new(l, 0.0)],
                Stroke::new(2.2 * k, Color32::WHITE),
            );
            if plus {
                painter.line_segment(
                    [c - Vec2::new(0.0, l), c + Vec2::new(0.0, l)],
                    Stroke::new(2.2 * k, Color32::WHITE),
                );
            }
            r.on_hover_text(if plus {
                lg.text("navigation-zoom-in-tooltip")
            } else {
                lg.text("navigation-zoom-out-tooltip")
            });
            if held {
                self.fly = None;
                self.zoom((if plus { -1.6 * dt } else { 1.6 * dt }).exp());
                active = true;
            }
        }
        let track = ui.interact(
            Rect::from_min_max(
                Pos2::new(cx - 11.0 * k, t_top - 4.0 * k),
                Pos2::new(cx + 11.0 * k, t_bot + 4.0 * k),
            ),
            base.with("track"),
            Sense::click_and_drag(),
        );
        if (track.dragged() || track.clicked() || track.is_pointer_button_down_on())
            && let Some(p) = track.interact_pointer_pos()
        {
            let tt = ((p.y - t_top) / track_len).clamp(0.0, 1.0) as f64;
            self.cam.set_zoom_fraction(tt);
            self.fly = None;
            active = true;
        }
        let hy = t_top + t * track_len;
        let hot = track.hovered() || track.dragged();
        painter.rect_filled(
            Rect::from_center_size(Pos2::new(cx, hy), Vec2::new(20.0 * k, 11.0 * k)),
            4.0 * k,
            if hot { fill_hot } else { fill },
        );
        painter.rect_stroke(
            Rect::from_center_size(Pos2::new(cx, hy), Vec2::new(20.0 * k, 11.0 * k)),
            4.0 * k,
            Stroke::new(1.0_f32, edge),
            egui::StrokeKind::Middle,
        );
        painter.line_segment(
            [Pos2::new(cx - 4.5 * k, hy), Pos2::new(cx + 4.5 * k, hy)],
            Stroke::new(2.0 * k, Color32::WHITE),
        );
        track.on_hover_text(lg.text("navigation-zoom-slider-tooltip"));

        active
    }
}

fn hold_control(ui: &mut egui::Ui, rect: Rect, id: egui::Id) -> egui::Response {
    // Click-only ownership expires during a long press. Retain drag ownership
    // until release so continuous controls also tolerate small pointer movements.
    ui.interact(rect, id, Sense::click_and_drag())
}

#[cfg(test)]
mod tests;
