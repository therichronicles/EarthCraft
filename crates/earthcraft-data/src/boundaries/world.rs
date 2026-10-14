//! Municipal borders around the current camera anywhere in the world, from OSM.
//! An intentionally small, throttled bounding box avoids abusive Overpass calls.
use super::{Layer, Line, Patch};
use std::sync::Arc;

pub(super) async fn load_osm_cities(lat: f64, lon: f64) -> Result<Patch, crate::Error> {
    // On-demand bounded geography, not a false promise of a global exhaustive file.
    let lat0 = (lat - 0.30).clamp(-85.0, 85.0);
    let lat1 = (lat + 0.30).clamp(-85.0, 85.0);
    let lon0 = (lon - 0.30).max(-180.0);
    let lon1 = (lon + 0.30).min(180.0);
    let query = format!(
        "[out:json][timeout:20];(way[\"boundary\"=\"administrative\"][\"admin_level\"~\"^(8|9)$\"]({lat0:.4},{lon0:.4},{lat1:.4},{lon1:.4});relation[\"boundary\"=\"administrative\"][\"admin_level\"~\"^(8|9)$\"]({lat0:.4},{lon0:.4},{lat1:.4},{lon1:.4}););out geom;"
    );
    let client = crate::http::client(crate::http::Policy::Boundary)?;
    let bytes = crate::http::body(
        client
            .post("https://overpass.kumi.systems/api/interpreter")
            .form(&[("data", query)]),
        15_000_000,
    )
    .await?;
    let v = crate::http::parse_json(bytes).await?;
    let lines = parse_overpass(&v);
    Ok(Patch {
        layer: Layer::WorldCities,
        lines,
        lines_big: vec![],
        labels: vec![],
    })
}
fn parse_overpass(v: &serde_json::Value) -> Vec<Line> {
    let mut out = Vec::new();
    for object in v["elements"].as_array().into_iter().flatten().take(3000) {
        let kind = object["type"].as_str().unwrap_or("");
        if kind == "way" {
            push_geom(&object["geometry"], &mut out);
        } else if kind == "relation" {
            for member in object["members"].as_array().into_iter().flatten() {
                if member["type"] == "way"
                    && member["role"]
                        .as_str()
                        .is_some_and(|r| r == "outer" || r == "inner" || r.is_empty())
                {
                    push_geom(&member["geometry"], &mut out);
                }
            }
        }
    }
    out
}
fn push_geom(v: &serde_json::Value, out: &mut Vec<Line>) {
    if out.len() > 6000 {
        return;
    }
    let pts: Vec<(f64, f64)> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let lat = p["lat"].as_f64()?;
            let lon = p["lon"].as_f64()?;
            (lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0)
                .then_some((lat, lon))
        })
        .collect();
    if pts.len() >= 2 {
        out.push(Arc::new(pts));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn municipal_boundary_can_be_read_from_any_country() {
        let v = serde_json::json!({"elements":[{"type":"way","geometry":[{"lat":48.8,"lon":2.3},{"lat":48.9,"lon":2.4}]}]});
        assert_eq!(parse_overpass(&v)[0][0], (48.8, 2.3));
    }
}
