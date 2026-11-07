use super::App;
use earthcraft_core::camera::Basis;
use earthcraft_data::boundaries::BoundarySnapshot;
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Stroke, Vec2};
#[cfg(test)]
mod benchmarks;
mod cache;
mod popup;
mod projected;
pub(super) use cache::OverlayCache;
use cache::{OverlayLayers, OverlayStyle, OverlayView};

/// How close (points) a click or hover must be to a line to pick it.
const LINE_PICK_RADIUS: f32 = 6.0;

/// Line picking: the pointer to test this frame and the line whose box is open.
#[derive(Default)]
pub(super) struct LinePick {
    /// Pointer position over the globe this frame, and whether it clicked.
    pub(super) probe: Option<(Pos2, bool)>,
    /// Open attribute box: placemark id and where it was clicked.
    open: Option<(u64, Pos2)>,
}

/// A run must be at least this long (points) to carry a label.
const PATH_LABEL_MIN_LENGTH: f32 = 90.0;
/// Most labels considered per frame; each needs a free screen cell.
const PATH_LABEL_CANDIDATES: usize = 400;

impl App {
    pub(super) fn invalidate_font_layout(&mut self) {
        self.overlay_cache.invalidate();
        self.aircraft_overlay.invalidate_text();
        self.world_shapes = super::live_layers::WorldShapes::default();
        self.roads.invalidate_text();
    }
    pub(super) fn overlay_layers(&self) -> earthcraft_render::overlays::OverlayLayers {
        OverlayLayers {
            countries: self.show_country_boundaries,
            states: self.show_state_boundaries,
            cities: self.show_city_boundaries,
            metro: self.show_metro_boundaries,
            labels: self.show_place_labels,
        }
    }

    pub(super) fn merge_snapshot(&mut self, snapshot: BoundarySnapshot) {
        self.boundaries = snapshot.data;
        if let Err(error) = self.local_search.update_shared(snapshot.names) {
            self.status = format!("Local prefix index failed: {error}");
        }
        self.overlay_cache.invalidate();
    }

    pub(super) fn draw_overlays(
        &mut self,
        painter: &egui::Painter,
        basis: &Basis,
        rect: Rect,
        altitude_km: f64,
    ) {
        let style = OverlayStyle {
            layers: self.overlay_layers(),
            map_scale: self.settings.map_scale,
            lang: self.lang,
        };
        self.overlay_cache.draw(
            painter,
            OverlayView {
                basis: *basis,
                rect,
                altitude_km,
                grid_step_deg: if self.show_grid && self.settings.grid_numbers {
                    self.renderer.displayed_grid_step()
                } else {
                    None
                },
                pixels_per_point: painter.ctx().pixels_per_point(),
            },
            &self.boundaries,
            style,
        );
        let ms = self.settings.map_scale;
        // ruler
        let stroke = Stroke::new(2.5_f32, Color32::from_rgb(255, 80, 80));
        for leg in earthcraft_render::overlays::ruler::project_ruler(basis, rect, &self.ruler_paths)
        {
            for segment in leg.segments {
                painter.line_segment(segment, stroke);
            }
            if let Some(mid) = leg.label_position {
                let txt = self.settings.ruler_unit.format_with_angle(
                    leg.distance_km,
                    leg.angle_degrees,
                    self.distance_units(),
                    self.lang,
                );
                painter.text(
                    mid + Vec2::new(1.0, 1.0),
                    Align2::CENTER_BOTTOM,
                    &txt,
                    crate::app::fonts::map_font(painter.ctx(), 13.0 * ms),
                    Color32::BLACK,
                );
                painter.text(
                    mid,
                    Align2::CENTER_BOTTOM,
                    &txt,
                    crate::app::fonts::map_font(painter.ctx(), 13.0 * ms),
                    Color32::WHITE,
                );
            }
        }
        for p in self.ruler_paths.points() {
            if let Some(s) = basis.project(basis.surface_position(p.0, p.1), rect) {
                painter.circle_filled(s, 4.0, Color32::WHITE);
                painter.circle_stroke(s, 4.0, Stroke::new(1.5_f32, Color32::from_rgb(255, 80, 80)));
            }
        }
        // lines and outlines imported with placemarks (for example fiber routes)
        let default_style = earthcraft_core::placemark::LineStyle {
            rgba: [255, 140, 0, 255],
            width: 2.0,
        };
        let mut runs = Vec::new();
        let mut labels: Vec<(f32, &str, Pos2, f32)> = Vec::new();
        let probe = self.line_pick.probe.take();
        let open_id = self.line_pick.open.map(|(id, _)| id);
        let mut nearest: Option<(f32, u64)> = None;
        for (id, pm) in self.placemarks.markers_with_id(true) {
            let style = pm.line_style.unwrap_or(default_style);
            let [r, g, b, a] = style.rgba;
            let stroke = Stroke::new(
                0.75_f32 * style.width.clamp(1.0, 8.0) * ms,
                Color32::from_rgba_unmultiplied(r, g, b, a),
            );
            for path in &pm.paths {
                earthcraft_render::overlays::paths::project_path(basis, rect, path, &mut runs);
                for run in runs.drain(..) {
                    if !pm.name.is_empty()
                        && let Some((at, angle, length)) =
                            earthcraft_render::overlays::paths::label_anchor(
                                &run,
                                PATH_LABEL_MIN_LENGTH,
                            )
                    {
                        labels.push((length, pm.name.as_str(), at, angle));
                    }
                    if let Some((at, _)) = probe {
                        let reach = LINE_PICK_RADIUS + stroke.width * 0.5;
                        let distance =
                            earthcraft_render::overlays::paths::distance_to_run(&run, at);
                        if distance <= reach && nearest.is_none_or(|(best, _)| distance < best) {
                            nearest = Some((distance, id));
                        }
                    }
                    if open_id == Some(id) {
                        let halo = Stroke::new(stroke.width + 5.0, Color32::from_white_alpha(200));
                        painter.add(egui::Shape::line(run.clone(), halo));
                    }
                    painter.add(egui::Shape::line(run, stroke));
                }
            }
        }
        if let Some((at, clicked)) = probe {
            if nearest.is_some() {
                painter
                    .ctx()
                    .set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if clicked {
                // Clicking empty globe closes the box.
                self.line_pick.open = nearest.map(|(_, id)| (id, at));
                if let Some((_, id)) = nearest {
                    self.place_selection = Some(id);
                }
            }
        }
        // Label the longest visible lines first, one per screen cell.
        labels.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut occupied = earthcraft_render::overlays::placemarks::LabelGrid::new(rect);
        let label_font = crate::app::fonts::map_font(painter.ctx(), 12.0 * ms);
        for (_, name, at, angle) in labels.into_iter().take(PATH_LABEL_CANDIDATES) {
            if !occupied.claim(at) {
                continue;
            }
            for (shadow, color) in [
                (Vec2::new(1.0, 1.0), Color32::BLACK),
                (Vec2::ZERO, Color32::WHITE),
            ] {
                let galley = painter.layout_no_wrap(name.to_owned(), label_font.clone(), color);
                let size = galley.size();
                // Lift the text off the line along its normal.
                let lift = Vec2::new(angle.sin(), -angle.cos()) * (size.y * 0.5 + 2.0);
                let top_left = at + lift + shadow - size * 0.5;
                painter.add(
                    egui::epaint::TextShape::new(top_left, galley, color)
                        .with_angle_and_anchor(angle, Align2::CENTER_CENTER),
                );
            }
        }
        // placemarks: thin dense imports so tens of thousands of markers stay interactive
        let font = crate::app::fonts::map_font(painter.ctx(), 13.0 * ms);
        let visible: Vec<(Pos2, &earthcraft_core::placemark::Placemark)> = self
            .placemarks
            .markers(true)
            // Lines and outlines draw their own geometry; only plain places get a pin.
            .filter(|pm| pm.paths.is_empty())
            .filter_map(|pm| {
                basis
                    .project(basis.surface_position(pm.lat, pm.lon), rect)
                    .filter(|s| rect.contains(*s))
                    .map(|s| (s, pm))
            })
            .collect();
        let points: Vec<Pos2> = visible.iter().map(|(s, _)| *s).collect();
        for draw in earthcraft_render::overlays::placemarks::select_markers(&points, rect) {
            let (s, pm) = visible[draw.index];
            painter.circle_filled(s, 6.0, Color32::from_rgb(255, 200, 0));
            painter.circle_stroke(s, 6.0, Stroke::new(1.5_f32, Color32::BLACK));
            if draw.label {
                let tp = s + Vec2::new(9.0, -9.0);
                painter.text(
                    tp + Vec2::new(1.0, 1.0),
                    Align2::LEFT_BOTTOM,
                    &pm.name,
                    font.clone(),
                    Color32::BLACK,
                );
                painter.text(
                    tp,
                    Align2::LEFT_BOTTOM,
                    &pm.name,
                    font.clone(),
                    Color32::WHITE,
                );
            }
        }
        self.show_line_popup(painter.ctx());
    }
}
