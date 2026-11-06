use super::App;
use earthcraft_core::camera::Basis;
use earthcraft_data::boundaries::BoundarySnapshot;
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Stroke, Vec2};
#[cfg(test)]
mod benchmarks;
mod cache;
mod projected;
pub(super) use cache::OverlayCache;
use cache::{OverlayLayers, OverlayStyle, OverlayView};

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
        let path_stroke = Stroke::new(1.5_f32 * ms, Color32::from_rgb(255, 140, 0));
        let mut runs = Vec::new();
        for pm in self.placemarks.markers(true) {
            for path in &pm.paths {
                earthcraft_render::overlays::paths::project_path(basis, rect, path, &mut runs);
                for run in runs.drain(..) {
                    painter.add(egui::Shape::line(run, path_stroke));
                }
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
                painter.text(tp, Align2::LEFT_BOTTOM, &pm.name, font.clone(), Color32::WHITE);
            }
        }
    }
}
