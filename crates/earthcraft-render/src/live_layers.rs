//! Geographic preparation for roads, AIS, predicted satellites and traffic.
mod worker;
use earthcraft_core::{camera::Basis, math::destination};
use earthcraft_data::{roads::Road, satellites::Satellite, traffic::TrafficTiles, vessels::Vessel};
use emath::{Pos2, Rect, Vec2};
use std::sync::Arc;
pub use worker::LiveWorker;

const VESSEL_HEADING_PROBE_KM: f64 = 2.0;
const VESSEL_ICON_POINTS: f32 = 8.0;
const VESSEL_WIDTH_RATIO: f32 = 0.55;
const VESSEL_REAR_RATIO: f32 = 0.8;
const VESSEL_TAIL_RATIO: f32 = 0.65;
const MIN_HEADING_LENGTH_SQUARED: f32 = 0.0001;
const MIN_ROAD_SEGMENT_LENGTH_SQUARED: f32 = 0.01;

#[derive(Clone, Copy, PartialEq)]
pub struct LiveView {
    pub basis: Basis,
    pub rect: Rect,
    pub altitude_km: f64,
    pub map_scale: f32,
    pub pixels_per_point: f32,
    pub traffic_width_px: usize,
    pub traffic_height_px: usize,
    pub weather_width_px: usize,
    pub weather_height_px: usize,
}
#[derive(Clone)]
pub struct LiveInput {
    pub view: LiveView,
    pub roads: Option<Arc<[Road]>>,
    pub vessels: Option<Arc<[Vessel]>>,
    pub satellites: Option<Arc<[Satellite]>>,
    pub traffic: Option<Arc<TrafficTiles>>,
    pub cloud: Option<Arc<image::RgbaImage>>,
    pub radar: Option<Arc<image::RgbaImage>>,
    pub unix_secs: f64,
}
fn same<T: ?Sized>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}
impl LiveInput {
    pub fn same_request(&self, other: &Self) -> bool {
        self.view == other.view
            && same(&self.roads, &other.roads)
            && same(&self.vessels, &other.vessels)
            && same(&self.satellites, &other.satellites)
            && same(&self.traffic, &other.traffic)
            && same(&self.cloud, &other.cloud)
            && same(&self.radar, &other.radar)
            && (self.satellites.is_none() || self.unix_secs.floor() == other.unix_secs.floor())
    }
}

pub struct ProjectedRoad {
    pub source_index: usize,
    pub segments: Vec<Vec<Pos2>>,
    bounds: Rect,
}
pub struct ProjectedVessel {
    pub source_index: usize,
    pub position: Pos2,
    pub icon: [Pos2; 4],
}
pub struct ProjectedSatellite {
    pub source_index: usize,
    pub position: Pos2,
}
pub struct LiveGeometry {
    pub input: LiveInput,
    pub roads: Arc<Vec<ProjectedRoad>>,
    pub vessels: Arc<Vec<ProjectedVessel>>,
    pub satellites: Arc<Vec<ProjectedSatellite>>,
    pub traffic: Option<Arc<Vec<u8>>>,
    pub weather: Option<Arc<Vec<u8>>>,
}
impl LiveGeometry {
    pub fn nearest_road(&self, mouse: Pos2, radius: f32) -> Option<(usize, Pos2)> {
        let mut nearest = None;
        let mut minimum = radius;
        for road in self
            .roads
            .iter()
            .filter(|road| road.bounds.expand(radius).contains(mouse))
        {
            for segment in road.segments.iter().flat_map(|line| line.windows(2)) {
                let a = segment[0];
                let ab = segment[1] - a;
                let factor = if ab.length_sq() > MIN_ROAD_SEGMENT_LENGTH_SQUARED {
                    ((mouse - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let point = a + ab * factor;
                let distance = (point - mouse).length();
                if distance < minimum {
                    minimum = distance;
                    nearest = Some((road.source_index, point));
                }
            }
        }
        nearest
    }
}
#[derive(Default)]
pub struct LivePreparer {
    latest: Option<Arc<LiveGeometry>>,
}
impl LivePreparer {
    pub fn prepare(&mut self, input: LiveInput) -> Arc<LiveGeometry> {
        if let Some(old) = &self.latest
            && old.input.same_request(&input)
        {
            return old.clone();
        }
        let previous = self
            .latest
            .as_ref()
            .filter(|old| old.input.view == input.view);
        let roads = previous
            .filter(|old| same(&old.input.roads, &input.roads))
            .map(|old| old.roads.clone())
            .unwrap_or_else(|| Arc::new(project_roads(&input)));
        let vessels = previous
            .filter(|old| same(&old.input.vessels, &input.vessels))
            .map(|old| old.vessels.clone())
            .unwrap_or_else(|| Arc::new(project_vessels(&input)));
        let satellites = previous
            .filter(|old| {
                same(&old.input.satellites, &input.satellites)
                    && old.input.unix_secs.floor() == input.unix_secs.floor()
            })
            .map(|old| old.satellites.clone())
            .unwrap_or_else(|| Arc::new(project_satellites(&input)));
        let traffic =
            if let Some(old) = previous.filter(|old| same(&old.input.traffic, &input.traffic)) {
                old.traffic.clone()
            } else {
                input.traffic.as_ref().map(|tiles| {
                    Arc::new(crate::weather::project_traffic_tiles(
                        &input.view.basis,
                        input.view.rect,
                        input.view.traffic_width_px,
                        input.view.traffic_height_px,
                        tiles,
                    ))
                })
            };
        let weather = if let Some(old) = previous.filter(|old| {
            same(&old.input.cloud, &input.cloud) && same(&old.input.radar, &input.radar)
        }) {
            old.weather.clone()
        } else if input.cloud.is_some() || input.radar.is_some() {
            Some(Arc::new(crate::weather::project_live_maps(
                &input.view.basis,
                input.view.rect,
                input.view.weather_width_px,
                input.view.weather_height_px,
                input.cloud.as_deref(),
                input.radar.as_deref(),
            )))
        } else {
            None
        };
        let geometry = Arc::new(LiveGeometry {
            input,
            roads,
            vessels,
            satellites,
            traffic,
            weather,
        });
        self.latest = Some(geometry.clone());
        geometry
    }
}
fn project_roads(input: &LiveInput) -> Vec<ProjectedRoad> {
    let Some(roads) = &input.roads else {
        return Vec::new();
    };
    roads
        .iter()
        .enumerate()
        .map(|(source_index, road)| {
            let mut segments = Vec::new();
            let mut visible = Vec::with_capacity(road.points.len());
            for &(lat, lon) in &road.points {
                if let Some(position) = input
                    .view
                    .basis
                    .project(input.view.basis.surface_position(lat, lon), input.view.rect)
                {
                    visible.push(position);
                } else if !visible.is_empty() {
                    segments.push(std::mem::take(&mut visible));
                }
            }
            if !visible.is_empty() {
                segments.push(visible);
            }
            let bounds = segments
                .iter()
                .flatten()
                .fold(Rect::NOTHING, |bounds, point| {
                    bounds.union(Rect::from_min_max(*point, *point))
                });
            ProjectedRoad {
                source_index,
                segments,
                bounds,
            }
        })
        .collect()
}
fn project_vessels(input: &LiveInput) -> Vec<ProjectedVessel> {
    let Some(vessels) = &input.vessels else {
        return Vec::new();
    };
    let view = input.view;
    vessels
        .iter()
        .enumerate()
        .filter_map(|(source_index, ship)| {
            let at = view
                .basis
                .project(view.basis.surface_position(ship.lat, ship.lon), view.rect)?;
            if !view.rect.contains(at) {
                return None;
            }
            // Preserve the original 2 km course probe; project it onto the selected model.
            let (lat, lon) = destination(ship.lat, ship.lon, ship.course, VESSEL_HEADING_PROBE_KM);
            let forward = view
                .basis
                .project(view.basis.surface_position(lat, lon), view.rect)
                .map(|p| p - at)
                .filter(|v| v.length_sq() > MIN_HEADING_LENGTH_SQUARED)
                .map(|v| v.normalized() * VESSEL_ICON_POINTS * view.map_scale)
                .unwrap_or(Vec2::new(0.0, -VESSEL_ICON_POINTS * view.map_scale));
            let right = Vec2::new(-forward.y, forward.x) * VESSEL_WIDTH_RATIO;
            Some(ProjectedVessel {
                source_index,
                position: at,
                icon: [
                    at + forward,
                    at + right - forward * VESSEL_REAR_RATIO,
                    at - forward * VESSEL_TAIL_RATIO,
                    at - right - forward * VESSEL_REAR_RATIO,
                ],
            })
        })
        .collect()
}
fn project_satellites(input: &LiveInput) -> Vec<ProjectedSatellite> {
    let Some(satellites) = &input.satellites else {
        return Vec::new();
    };
    satellites
        .iter()
        .enumerate()
        .filter_map(|(source_index, sat)| {
            let (lat, lon) = sat.ground_position(input.unix_secs)?;
            let position = input
                .view
                .basis
                .project(input.view.basis.surface_position(lat, lon), input.view.rect)?;
            input
                .view
                .rect
                .contains(position)
                .then_some(ProjectedSatellite {
                    source_index,
                    position,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests;
