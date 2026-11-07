//! The area of the globe a place or folder covers, for flying the camera to it.
use super::{PlaceItem, PlaceNode};
use crate::{
    math::{ll_to_v3, v3_to_ll},
    EARTH_RADIUS_KM, MAX_CAMERA_ALTITUDE_KM, MIN_CAMERA_ALTITUDE_KM, VERTICAL_FOV_HALF_TAN,
};
use glam::DVec3;

/// How much of the half-screen height the content may fill; the rest is margin.
const FILL: f64 = 0.75;
/// Altitude used for a single point, which has no size to fit.
const POINT_ALTITUDE_KM: f64 = 3.0;

/// A spherical cap: its center in degrees and angular radius in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    pub lat: f64,
    pub lon: f64,
    pub radius: f64,
}

impl Extent {
    /// Camera altitude in km at which the whole cap fits on screen.
    pub fn fit_altitude_km(&self) -> f64 {
        if self.radius <= 0.0 {
            return POINT_ALTITUDE_KM;
        }
        (self.radius / (VERTICAL_FOV_HALF_TAN * FILL) * EARTH_RADIUS_KM)
            .clamp(MIN_CAMERA_ALTITUDE_KM, MAX_CAMERA_ALTITUDE_KM)
    }
}

impl PlaceNode {
    /// The smallest cap found that holds everything under this node, or `None`
    /// for an empty folder. Hidden items count: flying to a folder shows what
    /// the user chose, and toggling visibility must not move the target.
    pub fn extent(&self) -> Option<Extent> {
        let mut parts: Vec<(DVec3, f64)> = Vec::new();
        collect(self, &mut parts);
        let (first, _) = *parts.first()?;
        let sum: DVec3 = parts.iter().map(|(v, _)| *v).sum();
        let center = if sum.length() > 1e-9 { sum.normalize() } else { first };
        let radius = parts
            .iter()
            .map(|(v, r)| center.dot(*v).clamp(-1.0, 1.0).acos() + r)
            .fold(0.0_f64, f64::max);
        let (lat, lon) = v3_to_ll(center);
        Some(Extent { lat, lon, radius })
    }
}

fn collect(node: &PlaceNode, parts: &mut Vec<(DVec3, f64)>) {
    match &node.item {
        PlaceItem::Marker(pm) if pm.paths.is_empty() => parts.push((ll_to_v3(pm.lat, pm.lon), 0.0)),
        PlaceItem::Marker(pm) => {
            for path in &pm.paths {
                let (lat, lon) = path.center();
                parts.push((ll_to_v3(lat, lon), path.radius()));
            }
        }
        PlaceItem::Folder { children, .. } => {
            for child in children {
                collect(child, parts);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placemark::{GeoPath, Placemark};

    fn route(points: &[(f64, f64)]) -> PlaceNode {
        PlaceNode::marker(Placemark {
            lat: points[0].0,
            lon: points[0].1,
            paths: vec![GeoPath::new(false, points.iter().copied()).unwrap()],
            ..Default::default()
        })
    }

    #[test]
    fn a_lone_point_flies_to_the_point_altitude() {
        let node = PlaceNode::marker(Placemark { lat: 44.0, lon: -93.0, ..Default::default() });
        let extent = node.extent().unwrap();
        assert!((extent.lat - 44.0).abs() < 1e-9 && (extent.lon + 93.0).abs() < 1e-9);
        assert_eq!(extent.fit_altitude_km(), POINT_ALTITUDE_KM);
    }

    #[test]
    fn a_route_is_centered_and_fits() {
        let extent = route(&[(44.0, -93.0), (44.0, -92.0)]).extent().unwrap();
        assert!((extent.lat - 44.0).abs() < 0.1 && (extent.lon + 92.5).abs() < 0.1);
        // Half a degree each way is about 40 km, so the camera sits above that.
        let altitude = extent.fit_altitude_km();
        assert!((60.0..250.0).contains(&altitude), "{altitude}");
    }

    #[test]
    fn a_folder_covers_every_child_including_hidden_ones() {
        let mut far = route(&[(10.0, 10.0), (10.0, 11.0)]);
        far.visible = false;
        let folder = PlaceNode::folder(
            "f".into(),
            String::new(),
            false,
            vec![route(&[(44.0, -93.0), (44.0, -92.0)]), far],
        );
        let near = route(&[(44.0, -93.0), (44.0, -92.0)]).extent().unwrap();
        let all = folder.extent().unwrap();
        assert!(all.radius > near.radius * 10.0);
        // Every child sits inside the cap.
        for (lat, lon) in [(44.0, -93.0), (10.0, 11.0)] {
            let angle = ll_to_v3(all.lat, all.lon).dot(ll_to_v3(lat, lon)).acos();
            assert!(angle <= all.radius + 1e-9);
        }
    }

    #[test]
    fn empty_folders_have_no_extent_and_huge_ones_clamp() {
        let empty = PlaceNode::folder("e".into(), String::new(), false, vec![]);
        assert!(empty.extent().is_none());
        let world = Extent { lat: 0.0, lon: 0.0, radius: 3.0 };
        assert_eq!(world.fit_altitude_km(), MAX_CAMERA_ALTITUDE_KM);
    }
}
