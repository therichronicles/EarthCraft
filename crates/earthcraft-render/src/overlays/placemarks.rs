//! Level-of-detail selection for placemark markers and labels.
//!
//! Large KML/KMZ imports can put tens of thousands of placemarks on screen when
//! the globe is zoomed out. Drawing every marker and two text layouts per label
//! each frame dominates frame time, and markers closer than a few pixels are
//! indistinguishable. Small sets are returned untouched; above
//! [`DENSE_MARKER_LIMIT`] markers are thinned to one per screen cell and labels
//! are limited and kept from overlapping.
use emath::{Pos2, Rect, Vec2};

/// At or below this many on-screen markers every marker and label is drawn.
pub const DENSE_MARKER_LIMIT: usize = 2_000;
/// Marker thinning cell in points; roughly the drawn marker diameter.
pub const MARKER_CELL_POINTS: f32 = 10.0;
/// Label occupancy cell in points; roughly one short label.
pub const LABEL_CELL_POINTS: Vec2 = Vec2::new(140.0, 18.0);
/// Most labels drawn in a thinned frame.
pub const MAX_DENSE_LABELS: usize = 250;

/// One marker to draw: its index in the input slice and whether to label it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkerDraw {
    pub index: usize,
    pub label: bool,
}

struct Grid {
    origin: Pos2,
    cell: Vec2,
    columns: usize,
    rows: usize,
    taken: Vec<bool>,
}
impl Grid {
    fn new(bounds: Rect, cell: Vec2) -> Self {
        let columns = (bounds.width() / cell.x).ceil().max(1.0) as usize + 1;
        let rows = (bounds.height() / cell.y).ceil().max(1.0) as usize + 1;
        Self {
            origin: bounds.min,
            cell,
            columns,
            rows,
            taken: vec![false; columns * rows],
        }
    }
    /// Claim the cell holding `point`; false when it was already claimed.
    fn claim(&mut self, point: Pos2) -> bool {
        let column = (((point.x - self.origin.x) / self.cell.x).max(0.0) as usize)
            .min(self.columns - 1);
        let row = (((point.y - self.origin.y) / self.cell.y).max(0.0) as usize)
            .min(self.rows - 1);
        !std::mem::replace(&mut self.taken[row * self.columns + column], true)
    }
}

/// Keeps labels from piling up: one claim per label cell.
pub struct LabelGrid(Grid);
impl LabelGrid {
    pub fn new(bounds: Rect) -> Self {
        Self(Grid::new(bounds, LABEL_CELL_POINTS))
    }
    /// True when no earlier label took the cell containing `point`.
    pub fn claim(&mut self, point: Pos2) -> bool {
        self.0.claim(point)
    }
}

/// Choose which projected markers to draw. `points` are in input (tree) order,
/// which keeps the choice stable between frames; `bounds` contains them.
pub fn select_markers(points: &[Pos2], bounds: Rect) -> Vec<MarkerDraw> {
    if points.len() <= DENSE_MARKER_LIMIT {
        return (0..points.len())
            .map(|index| MarkerDraw { index, label: true })
            .collect();
    }
    let mut markers = Grid::new(bounds, Vec2::splat(MARKER_CELL_POINTS));
    let mut labels = Grid::new(bounds, LABEL_CELL_POINTS);
    let mut labelled = 0;
    let mut selected = Vec::new();
    for (index, &point) in points.iter().enumerate() {
        if !markers.claim(point) {
            continue;
        }
        let label = labelled < MAX_DENSE_LABELS && labels.claim(point);
        labelled += usize::from(label);
        selected.push(MarkerDraw { index, label });
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use emath::pos2;

    fn bounds() -> Rect {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1920.0, 1080.0))
    }

    #[test]
    fn small_sets_draw_every_marker_and_label() {
        let points: Vec<Pos2> = (0..DENSE_MARKER_LIMIT)
            .map(|i| pos2(100.0, 100.0 + (i % 3) as f32)) // heavily overlapping
            .collect();
        let selected = select_markers(&points, bounds());
        assert_eq!(selected.len(), points.len());
        assert!(selected.iter().all(|m| m.label));
    }

    #[test]
    fn dense_sets_are_thinned_to_one_marker_per_cell_and_limited_labels() {
        let points: Vec<Pos2> = (0..60_000)
            .map(|i| pos2(((i * 7919) % 1900) as f32 + 5.0, ((i * 104_729) % 1070) as f32 + 5.0))
            .collect();
        let selected = select_markers(&points, bounds());
        let max_cells = (1920 / 10 + 2) * (1080 / 10 + 2);
        assert!(selected.len() <= max_cells);
        assert!(selected.len() < points.len() / 2);
        assert!(selected.iter().filter(|m| m.label).count() <= MAX_DENSE_LABELS);
        assert!(selected.windows(2).all(|w| w[0].index < w[1].index));
    }

    #[test]
    fn labels_do_not_share_a_label_cell_and_choice_is_deterministic() {
        let points: Vec<Pos2> = (0..5_000)
            .map(|i| pos2(500.0 + (i % 50) as f32 * 0.1, 500.0 + (i / 50) as f32 * 0.1))
            .collect();
        let first = select_markers(&points, bounds());
        assert_eq!(first, select_markers(&points, bounds()));
        // Every point is within one 10-point cell, so exactly one marker remains.
        assert_eq!(first.len(), 1);
        assert!(first[0].label);
    }

    #[test]
    fn edge_points_stay_inside_the_grid() {
        let mut points = vec![pos2(1920.0, 1080.0), pos2(0.0, 0.0)];
        points.extend((0..DENSE_MARKER_LIMIT).map(|i| pos2((i % 1900) as f32, 500.0)));
        assert!(!select_markers(&points, bounds()).is_empty());
    }
}
