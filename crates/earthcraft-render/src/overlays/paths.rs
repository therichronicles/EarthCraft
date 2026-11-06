//! Screen projection of placemark lines and outlines (for example fiber routes).
//!
//! A network can hold tens of thousands of paths, so each one is culled with
//! its precomputed bounds and drawn with only as many vertices as its on-screen
//! length can show, instead of projecting every vertex every frame.
use earthcraft_core::{camera::Basis, math::ll_to_v3, placemark::GeoPath, VERTICAL_FOV_HALF_TAN};
use emath::{Pos2, Rect};

/// Paths shorter than this on screen (points) are not drawn.
const MIN_SCREEN_LENGTH: f64 = 0.75;
/// Target spacing between drawn vertices in points.
const VERTEX_SPACING: f64 = 2.0;
/// Extra angle in radians beyond the horizon that still counts as visible.
const HORIZON_MARGIN: f64 = 0.02;

/// Append the visible runs of `path` as screen polylines to `runs`. A run ends
/// where the path passes behind the horizon; closed paths repeat their first
/// point when the whole ring is visible.
pub fn project_path(basis: &Basis, rect: Rect, path: &GeoPath, runs: &mut Vec<Vec<Pos2>>) {
    let altitude = (basis.c.length() - 1.0).max(1e-6);
    let per_radian = f64::from(rect.height()) * 0.5 / (VERTICAL_FOV_HALF_TAN * altitude);
    let on_screen_length = path.length() * per_radian;
    if on_screen_length < MIN_SCREEN_LENGTH {
        return;
    }
    // Horizon cull: the camera sees a cap of this angular radius around its nadir.
    let (center_lat, center_lon) = basis.geographic_center();
    let nadir = ll_to_v3(center_lat, center_lon);
    let (lat, lon) = path.center();
    let angle = nadir.dot(ll_to_v3(lat, lon)).clamp(-1.0, 1.0).acos();
    let horizon = (1.0 / basis.c.length()).clamp(-1.0, 1.0).acos();
    if angle - path.radius() > horizon + HORIZON_MARGIN {
        return;
    }
    // Screen cull when the center is projectable. Foreshortening only shrinks
    // true pixel extents, so the nadir scale is a conservative radius.
    if let Some(center) = basis.project(basis.surface_position(lat, lon), rect) {
        let reach = f64::from(rect.size().length()) * 0.5 + path.radius() * per_radian;
        if f64::from((center - rect.center()).length()) > reach {
            return;
        }
    }
    let points = path.points();
    let wanted = (on_screen_length / VERTEX_SPACING).ceil().max(2.0);
    let stride = ((points.len() as f64 / wanted).ceil() as usize).max(1);
    let last = points.len() - 1;
    let mut run: Vec<Pos2> = Vec::new();
    let flush = |run: &mut Vec<Pos2>, runs: &mut Vec<Vec<Pos2>>| {
        if run.len() >= 2 {
            runs.push(std::mem::take(run));
        } else {
            run.clear();
        }
    };
    let mut all_visible = true;
    for index in (0..=last).step_by(stride).chain((last % stride != 0).then_some(last)) {
        let (lat, lon) = points[index];
        match basis.project(basis.surface_position(lat, lon), rect) {
            Some(point) => run.push(point),
            None => {
                all_visible = false;
                flush(&mut run, runs);
            }
        }
    }
    if path.is_closed() && all_visible && let Some(&first) = run.first() {
        run.push(first);
    }
    flush(&mut run, runs);
}

/// Where to put a label on a projected run: the point halfway along it and the
/// text angle in radians (clockwise on screen, kept within +-90 degrees so text
/// is never upside down). `None` when the run is shorter than `min_length`.
pub fn label_anchor(run: &[Pos2], min_length: f32) -> Option<(Pos2, f32, f32)> {
    let total: f32 = run.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    if total < min_length {
        return None;
    }
    let mut remaining = total * 0.5;
    for pair in run.windows(2) {
        let segment = pair[1] - pair[0];
        let length = segment.length();
        if length >= remaining && length > 0.0 {
            let point = pair[0] + segment * (remaining / length);
            let mut angle = segment.y.atan2(segment.x);
            if angle > std::f32::consts::FRAC_PI_2 {
                angle -= std::f32::consts::PI;
            } else if angle < -std::f32::consts::FRAC_PI_2 {
                angle += std::f32::consts::PI;
            }
            return Some((point, angle, total));
        }
        remaining -= length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthcraft_core::{camera::Cam, earth_model::EarthModel};
    use emath::pos2;

    fn view(lat: f64, lon: f64, altitude_km: f64) -> (Basis, Rect) {
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1920.0, 1080.0));
        let cam = Cam { lat, lon, heading: 0.0, alt: altitude_km };
        (cam.basis_for_model(1920.0 / 1080.0, EarthModel::PerfectSphere), rect)
    }
    fn line(vertices: usize, step: f64) -> GeoPath {
        GeoPath::new(false, (0..vertices).map(|i| (0.0, i as f64 * step))).unwrap()
    }
    fn project(basis: &Basis, rect: Rect, path: &GeoPath) -> Vec<Vec<Pos2>> {
        let mut runs = Vec::new();
        project_path(basis, rect, path, &mut runs);
        runs
    }

    #[test]
    fn a_visible_line_is_projected_in_order() {
        let (basis, rect) = view(0.0, 1.0, 1500.0);
        let runs = project(&basis, rect, &line(20, 0.1));
        assert_eq!(runs.len(), 1);
        assert!(runs[0].windows(2).all(|w| w[0].x < w[1].x));
        assert!(rect.contains(runs[0][0]));
    }

    #[test]
    fn lines_behind_the_horizon_or_off_screen_are_culled() {
        let (basis, rect) = view(0.0, 0.0, 1500.0);
        let far_side = GeoPath::new(false, [(0.0, 170.0), (0.0, 171.0)]).unwrap();
        assert!(project(&basis, rect, &far_side).is_empty());
        let (zoomed, rect) = view(0.0, 0.0, 50.0);
        let elsewhere = GeoPath::new(false, [(0.0, 20.0), (0.0, 21.0)]).unwrap();
        assert!(project(&zoomed, rect, &elsewhere).is_empty());
    }

    #[test]
    fn sub_pixel_paths_are_skipped() {
        let (basis, rect) = view(0.0, 0.0, 30_000.0);
        let tiny = GeoPath::new(false, [(0.0, 0.0), (0.0, 0.001)]).unwrap();
        assert!(project(&basis, rect, &tiny).is_empty());
    }

    #[test]
    fn long_paths_are_decimated_to_their_screen_length() {
        let (basis, rect) = view(0.0, 5.0, 3000.0);
        let dense = line(50_000, 0.0002);
        let runs = project(&basis, rect, &dense);
        let drawn: usize = runs.iter().map(Vec::len).sum();
        assert!(drawn >= 2 && drawn < 2_000, "{drawn}");
        // The final vertex is always kept.
        let (lat, lon) = *dense.points().last().unwrap();
        let end = basis.project(basis.surface_position(lat, lon), rect).unwrap();
        assert_eq!(*runs[0].last().unwrap(), end);
    }

    #[test]
    fn label_anchor_is_the_midpoint_with_upright_text() {
        let run = [pos2(0.0, 0.0), pos2(100.0, 0.0), pos2(100.0, 100.0)];
        let (point, angle, length) = label_anchor(&run, 50.0).unwrap();
        assert_eq!(length, 200.0);
        assert_eq!(point, pos2(100.0, 0.0));
        assert!(angle.abs() <= std::f32::consts::FRAC_PI_2);
        assert!(label_anchor(&run, 250.0).is_none());
        // A line running right-to-left is flipped so text still reads left to right.
        let backwards = [pos2(100.0, 0.0), pos2(0.0, 0.0)];
        let (_, angle, _) = label_anchor(&backwards, 10.0).unwrap();
        assert!(angle.abs() < 1e-6);
    }

    #[test]
    fn closed_rings_repeat_their_first_point() {
        let (basis, rect) = view(0.0, 0.0, 1500.0);
        let ring = GeoPath::new(true, [(1.0, 1.0), (1.0, -1.0), (-1.0, -1.0), (-1.0, 1.0)]).unwrap();
        let runs = project(&basis, rect, &ring);
        assert_eq!(runs[0].first(), runs[0].last());
    }
}
