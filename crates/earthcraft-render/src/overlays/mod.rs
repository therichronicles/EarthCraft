//! Geographic overlay projection and occupancy, independent of a widget toolkit.
use earthcraft_core::{camera::Basis, math::wrap_lon};
use earthcraft_data::boundaries::{
    BoundaryData, LabelKind, MapLabel, OTHER_STATES_MAX_ALTITUDE_KM,
};
use emath::{Pos2, Rect};
pub mod grid;
mod labels;
mod spatial;
mod worker;
pub use labels::LabelIndex;
pub use spatial::{BoundaryIndex, VisibleBoundaries};
pub use worker::{
    OverlayGeometry, OverlayInput, OverlayPreparer, OverlayView, OverlayWorker, PreparedLabel,
};
const SIMPLIFY_BORDER_ALTITUDE_KM: f64 = 1500.0;
const MIN_BORDER_SEGMENT_POINTS: f32 = 1.4;
const MAX_BORDER_SUBDIVISIONS: usize = 96;
const CLOSE_BORDER_ALTITUDE_KM: f64 = 500.0;
const MEDIUM_BORDER_ALTITUDE_KM: f64 = 5000.0;
pub const MAX_VISIBLE_LABELS: usize = 500;
#[derive(Clone, Copy, PartialEq)]
pub struct OverlayLayers {
    pub countries: bool,
    pub states: bool,
    pub cities: bool,
    pub metro: bool,
    pub labels: bool,
}
pub struct OverlayProjection<'a> {
    pub basis: &'a Basis,
    pub rect: Rect,
    pub altitude_km: f64,
    pub layers: OverlayLayers,
}
#[derive(Clone, Copy)]
pub enum BorderKind {
    Country,
    State,
    City,
    Metro,
}

pub fn project_borders(
    view: &OverlayProjection<'_>,
    boundaries: &BoundaryData,
    visible: Option<&VisibleBoundaries>,
    mut emit: impl FnMut(BorderKind, [Pos2; 2]),
) {
    let (basis, rect) = (view.basis, view.rect);
    let min_sq: f32 = if view.altitude_km > SIMPLIFY_BORDER_ALTITUDE_KM {
        MIN_BORDER_SEGMENT_POINTS * MIN_BORDER_SEGMENT_POINTS
    } else {
        0.0
    };
    let mut draw_lines =
        |lines: &[earthcraft_data::boundaries::Line], kind: BorderKind, max_step_deg: f64| {
            for line in lines {
                if visible.is_some_and(|set| !set.contains_line(line)) {
                    continue;
                }
                let mut anchor: Option<Pos2> = None;
                for pair in line.array_windows::<2>() {
                    let (lat1, lon1) = pair[0];
                    let (lat2, lon2) = pair[1];
                    let dlat = lat2 - lat1;
                    let mut dlon = lon2 - lon1;
                    if dlon > 180.0 {
                        dlon -= 360.0;
                    }
                    if dlon < -180.0 {
                        dlon += 360.0;
                    }
                    let start = basis.project(basis.surface_position(lat1, lon1), rect);
                    let end = basis.project(basis.surface_position(lat2, lon2), rect);
                    if let (Some(a), Some(b)) = (start, end) {
                        // Common case: both endpoints are visible, so no extra sampling needed.
                        let from = anchor.unwrap_or(a);
                        if (b - from).length_sq() >= min_sq {
                            emit(kind, [from, b]);
                            anchor = Some(b);
                        } else if anchor.is_none() {
                            anchor = Some(a);
                        }
                        continue;
                    }
                    anchor = None;
                    let span = dlat.abs().max(dlon.abs());
                    if span < max_step_deg {
                        continue;
                    }
                    let steps =
                        ((span / max_step_deg).ceil() as usize).clamp(1, MAX_BORDER_SUBDIVISIONS);
                    let mut prev = start;
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        let lon = wrap_lon(lon1 + dlon * t);
                        let lat = lat1 + dlat * t;
                        let cur = basis.project(basis.surface_position(lat, lon), rect);
                        if let (Some(a), Some(b)) = (prev, cur) {
                            emit(kind, [a, b]);
                        }
                        prev = cur;
                    }
                }
            }
        };
    let close_zoom = view.altitude_km <= CLOSE_BORDER_ALTITUDE_KM;
    if view.layers.countries {
        draw_lines(
            &boundaries.countries,
            BorderKind::Country,
            if close_zoom { 0.25 } else { 2.0 },
        );
    }
    if view.layers.states {
        let step = if close_zoom {
            0.12
        } else if view.altitude_km <= MEDIUM_BORDER_ALTITUDE_KM {
            0.5
        } else {
            2.0
        };

        // States of the largest countries: visible at every zoom level (whole Earth in view).
        draw_lines(&boundaries.states_big, BorderKind::State, step);
        // States/provinces of the other countries: only from medium zoom on.
        if view.altitude_km < OTHER_STATES_MAX_ALTITUDE_KM {
            draw_lines(&boundaries.states, BorderKind::State, step);
        }
    }
    if view.layers.cities {
        draw_lines(
            &boundaries.cities,
            BorderKind::City,
            if close_zoom { 0.12 } else { 1.0 },
        );
        // OSM municipalities are fetched on demand outside Brazil and
        // projected with the same visibility/culling rules as IBGE outlines.
        draw_lines(
            &boundaries.world_cities,
            BorderKind::City,
            if close_zoom { 0.12 } else { 1.0 },
        );
    }
    if view.layers.metro {
        draw_lines(
            &boundaries.metro,
            BorderKind::Metro,
            if close_zoom { 0.12 } else { 1.0 },
        );
    }
}

pub struct ProjectedLabel<'a> {
    pub position: Pos2,
    pub label: &'a MapLabel,
    pub source_index: usize,
}
pub fn project_labels<'a>(
    view: &OverlayProjection<'_>,
    labels: &'a [MapLabel],
) -> Vec<ProjectedLabel<'a>> {
    if !view.layers.labels {
        return Vec::new();
    }
    let (basis, rect) = (view.basis, view.rect);
    let mut cands: Vec<(u8, u8, Pos2, &MapLabel, usize)> = Vec::new();
    for (source_index, label) in labels.iter().enumerate() {
        let enabled = label.visible_at(view.altitude_km)
            && match label.kind {
                LabelKind::Country | LabelKind::City => true,
                LabelKind::State => view.layers.states,
                LabelKind::Metro => view.layers.metro,
            };
        if !enabled {
            continue;
        }
        if let Some(pos) = basis.project(basis.surface_position(label.lat, label.lon), rect)
            && rect.contains(pos)
        {
            let prio = match label.kind {
                LabelKind::Country => 0,
                LabelKind::State => 1,
                LabelKind::Metro => 2,
                LabelKind::City => 3,
            };
            cands.push((prio, label.rank, pos, label, source_index));
        }
    }
    cands.sort_by_key(|c| (c.0, c.1));
    cands
        .into_iter()
        .map(|(_, _, position, label, source_index)| ProjectedLabel {
            position,
            label,
            source_index,
        })
        .collect()
}

pub mod ruler;
