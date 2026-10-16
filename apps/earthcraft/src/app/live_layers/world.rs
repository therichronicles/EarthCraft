//! Cache styled AIS/orbital meshes without changing polygon/label ordering.
use earthcraft_render::live_layers::{LiveGeometry, ProjectedSatellite, ProjectedVessel};
use eframe::egui::{self, Align2, Color32, Pos2, Shape, Stroke, Vec2};
use std::sync::Arc;

const VESSEL_LABEL_ALTITUDE_KM: f64 = 75.0;
const VESSEL_LABEL_LIMIT: usize = 75;
const SATELLITE_LABEL_ALTITUDE_KM: f64 = 900.0;
const SATELLITE_LABEL_LIMIT: usize = 60;
const LABEL_FONT_POINTS: f32 = 10.0;
const LABEL_OFFSET_POINTS: f32 = 8.0;
const SATELLITE_ICON_POINTS: f32 = 5.0;
struct Cached<T> {
    geometry: Arc<Vec<T>>,
    shapes: Vec<Shape>,
}
#[derive(Default)]
pub(in crate::app) struct WorldShapes {
    vessels: Option<Cached<ProjectedVessel>>,
    satellites: Option<Cached<ProjectedSatellite>>,
}
impl WorldShapes {
    pub fn vessels(&mut self, painter: &egui::Painter, geometry: &LiveGeometry) -> &[Shape] {
        if self
            .vessels
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(&old.geometry, &geometry.vessels))
        {
            let shapes = build_vessels(painter, geometry);
            self.vessels = Some(Cached {
                geometry: geometry.vessels.clone(),
                shapes: super::super::live_geometry::mesh_shapes(painter, shapes),
            });
        }
        &self.vessels.as_ref().unwrap().shapes
    }
    pub fn satellites(&mut self, painter: &egui::Painter, geometry: &LiveGeometry) -> &[Shape] {
        if self
            .satellites
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(&old.geometry, &geometry.satellites))
        {
            let shapes = build_satellites(painter, geometry);
            self.satellites = Some(Cached {
                geometry: geometry.satellites.clone(),
                shapes: super::super::live_geometry::mesh_shapes(painter, shapes),
            });
        }
        &self.satellites.as_ref().unwrap().shapes
    }
}
fn label(painter: &egui::Painter, at: Pos2, text: &str, scale: f32, color: Color32) -> Shape {
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        crate::app::fonts::map_font(painter.ctx(), LABEL_FONT_POINTS * scale),
        color,
    );
    let position = Align2::LEFT_CENTER
        .anchor_size(
            at + Vec2::new(LABEL_OFFSET_POINTS * scale, 0.0),
            galley.size(),
        )
        .min;
    Shape::galley(position, galley, color)
}
fn build_vessels(painter: &egui::Painter, geometry: &LiveGeometry) -> Vec<Shape> {
    let Some(vessels) = &geometry.input.vessels else {
        return Vec::new();
    };
    let scale = geometry.input.view.map_scale;
    let mut shapes = Vec::with_capacity(geometry.vessels.len() + VESSEL_LABEL_LIMIT);
    for (index, projected) in geometry.vessels.iter().enumerate() {
        let vessel = &vessels[projected.source_index];
        shapes.push(Shape::convex_polygon(
            projected.icon.to_vec(),
            Color32::from_rgb(52, 224, 229),
            Stroke::new(1.0 * scale, Color32::from_rgb(5, 60, 65)),
        ));
        if geometry.input.view.altitude_km < VESSEL_LABEL_ALTITUDE_KM && index < VESSEL_LABEL_LIMIT
        {
            let text = if vessel.name.is_empty() {
                &vessel.mmsi
            } else {
                &vessel.name
            };
            shapes.push(label(
                painter,
                projected.position,
                text,
                scale,
                Color32::from_rgb(120, 245, 245),
            ));
        }
    }
    shapes
}
fn build_satellites(painter: &egui::Painter, geometry: &LiveGeometry) -> Vec<Shape> {
    let Some(satellites) = &geometry.input.satellites else {
        return Vec::new();
    };
    let scale = geometry.input.view.map_scale;
    let s = SATELLITE_ICON_POINTS * scale;
    let mut shapes = Vec::with_capacity(geometry.satellites.len() + SATELLITE_LABEL_LIMIT);
    for (index, projected) in geometry.satellites.iter().enumerate() {
        let at = projected.position;
        shapes.push(Shape::convex_polygon(
            vec![
                at + Vec2::new(0.0, -s),
                at + Vec2::new(s, 0.0),
                at + Vec2::new(0.0, s),
                at + Vec2::new(-s, 0.0),
            ],
            Color32::from_rgb(207, 159, 255),
            Stroke::new(scale, Color32::from_rgb(62, 22, 91)),
        ));
        if geometry.input.view.altitude_km < SATELLITE_LABEL_ALTITUDE_KM
            && index < SATELLITE_LABEL_LIMIT
        {
            shapes.push(label(
                painter,
                at,
                &satellites[projected.source_index].name,
                scale,
                Color32::from_rgb(219, 178, 252),
            ));
        }
    }
    shapes
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::camera::Cam;
    use earthcraft_render::live_layers::{LiveInput, LivePreparer, LiveView};
    #[test]
    fn vessel_label_order_limits_and_cached_meshes_survive_unchanged_frames() {
        let ctx = egui::Context::default();
        let ship = earthcraft_data::vessels::Vessel {
            mmsi: "123456789".into(),
            name: "Ship".into(),
            lat: 0.0,
            lon: 0.0,
            course: 45.0,
            speed_knots: None,
            observed_unix: 1_700_000_000.0,
        };
        let input = LiveInput {
            view: LiveView {
                basis: Cam {
                    lat: 0.0,
                    lon: 0.0,
                    heading: 0.0,
                    alt: 10.0,
                }
                .basis(1.0),
                rect: egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(300.0)),
                altitude_km: 10.0,
                map_scale: 1.0,
                pixels_per_point: 1.0,
                traffic_width_px: 16,
                traffic_height_px: 16,
                weather_width_px: 16,
                weather_height_px: 16,
            },
            roads: None,
            vessels: Some(vec![ship; 100].into()),
            satellites: None,
            traffic: None,
            cloud: None,
            radar: None,
            unix_secs: 1_700_000_000.0,
        };
        let geometry = LivePreparer::default().prepare(input);
        crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
            let painter = ui.painter();
            let raw = build_vessels(painter, &geometry);
            assert_eq!(raw.len(), 100 + VESSEL_LABEL_LIMIT);
            assert!(matches!(&raw[0], Shape::Path(_)));
            assert!(matches!(&raw[1], Shape::Text(_)));
            assert!(matches!(&raw[2], Shape::Path(_)));
            let mut cache = WorldShapes::default();
            let first = cache.vessels(painter, &geometry)[0].clone();
            let repeated = &cache.vessels(painter, &geometry)[0];
            assert!(
                matches!((&first,repeated),(Shape::Mesh(a),Shape::Mesh(b)) if Arc::ptr_eq(a,b))
            );
        });
    }
}
