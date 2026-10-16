const MAX_SEARCH_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;
use earthcraft_core::coordinates::{parse_coordinates, validate_coordinates};
const SEARCH_KM_PER_DEGREE: f64 = 111.2;
const SEARCH_VIEW_PADDING_RATIO: f64 = 1.6;
const MIN_SEARCH_ALTITUDE_KM: f64 = 0.5;
const MAX_SEARCH_ALTITUDE_KM: f64 = 12_000.0;

pub const COUNTRY_SEARCH_ALTITUDE_KM: f64 = 6000.0;
pub const CITY_SEARCH_ALTITUDE_KM: f64 = 40.0;
const DEFAULT_SEARCH_ALTITUDE_KM: f64 = 5.0;
const STATE_SEARCH_ALTITUDE_KM: f64 = 1500.0;
const COUNTY_SEARCH_ALTITUDE_KM: f64 = 300.0;
const LOCALITY_SEARCH_ALTITUDE_KM: f64 = 12.0;
const STREET_SEARCH_ALTITUDE_KM: f64 = 2.0;
const HOUSE_SEARCH_ALTITUDE_KM: f64 = 0.6;

#[derive(Clone)]
pub struct GeoResult {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}

pub async fn geocode(q: &str, lang: &str) -> Result<Vec<GeoResult>, crate::Error> {
    let request = crate::http::client(crate::http::Policy::Search)?
        .get("https://nominatim.openstreetmap.org/search")
        .query(&[
            ("format", "json"),
            ("limit", "6"),
            ("accept-language", lang),
            ("q", q),
        ]);
    let v = crate::http::parse_json(crate::http::body(request, MAX_SEARCH_RESPONSE_BYTES).await?)
        .await?;
    parse_geocode_response(v)
}

#[derive(serde::Deserialize)]
struct NominatimResult {
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    lat: Option<String>,
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    lon: Option<String>,
    #[serde(default, deserialize_with = "crate::json_fields::string_numbers")]
    boundingbox: Vec<f64>,
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    display_name: Option<String>,
}
fn parse_geocode_response(value: serde_json::Value) -> Result<Vec<GeoResult>, crate::Error> {
    let mut out = Vec::new();
    for item in value.as_array().ok_or("invalid search response")? {
        let Ok(row) = serde_json::from_value::<NominatimResult>(item.clone()) else {
            continue;
        };
        let (Some(lat), Some(lon)) = (row.lat, row.lon) else {
            continue;
        };
        let Ok((lat, lon)) = parse_coordinates(&lat, &lon) else {
            continue;
        };
        let bb = row.boundingbox;
        let alt = if bb.len() == 4
            && validate_coordinates(bb[0], bb[2]).is_ok()
            && validate_coordinates(bb[1], bb[3]).is_ok()
        {
            let span = (bb[1] - bb[0]).max((bb[3] - bb[2]) * lat.to_radians().cos().abs());
            (span * SEARCH_KM_PER_DEGREE * SEARCH_VIEW_PADDING_RATIO)
                .clamp(MIN_SEARCH_ALTITUDE_KM, MAX_SEARCH_ALTITUDE_KM)
        } else {
            DEFAULT_SEARCH_ALTITUDE_KM
        };
        out.push(GeoResult {
            name: row.display_name.unwrap_or_else(|| "?".into()),
            lat,
            lon,
            alt,
        });
    }
    Ok(out)
}

#[derive(Clone)]
pub struct Suggestion {
    pub label: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}

/// Search-as-you-type suggestions. Nominatim forbids autocomplete, so Photon (Komoot, built on
/// OpenStreetMap data and meant for this use) is used instead.
pub async fn photon_suggest(q: &str, lang: &str) -> Result<Vec<Suggestion>, crate::Error> {
    let mut request = crate::http::client(crate::http::Policy::Suggest)?
        .get("https://photon.komoot.io/api/")
        .query(&[("q", q), ("limit", "8")]);
    if lang == "en" {
        request = request.query(&[("lang", "en")]);
    }
    let v = crate::http::parse_json(crate::http::body(request, MAX_SEARCH_RESPONSE_BYTES).await?)
        .await?;
    Ok(parse_photon_response(v))
}

fn parse_photon_response(v: serde_json::Value) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = Vec::new();
    for f in v["features"].as_array().into_iter().flatten() {
        let Some((lat, lon)) = f
            .pointer("/geometry/coordinates")
            .and_then(crate::json_fields::coordinates)
        else {
            continue;
        };
        let pr = &f["properties"];
        let g = |k: &str| {
            pr[k]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        let mut parts: Vec<String> = Vec::new();
        let first = g("name").or_else(|| {
            g("street").map(|st| match g("housenumber") {
                Some(n) => format!("{st}, {n}"),
                None => st,
            })
        });
        parts.extend(first);
        for k in ["district", "city", "state", "country"] {
            if let Some(x) = g(k)
                && !parts.contains(&x)
            {
                parts.push(x);
            }
        }
        if parts.is_empty() {
            continue;
        }
        let ex: Vec<f64> = pr["extent"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
            .unwrap_or_default();
        let alt = if ex.len() == 4
            && validate_coordinates(ex[1], ex[0]).is_ok()
            && validate_coordinates(ex[3], ex[2]).is_ok()
        {
            let span = (ex[1] - ex[3])
                .abs()
                .max((ex[2] - ex[0]).abs() * lat.to_radians().cos().abs());
            (span * SEARCH_KM_PER_DEGREE * SEARCH_VIEW_PADDING_RATIO)
                .clamp(MIN_SEARCH_ALTITUDE_KM, MAX_SEARCH_ALTITUDE_KM)
        } else {
            match pr["type"].as_str().unwrap_or("") {
                "country" => COUNTRY_SEARCH_ALTITUDE_KM,
                "state" => STATE_SEARCH_ALTITUDE_KM,
                "county" => COUNTY_SEARCH_ALTITUDE_KM,
                "city" => CITY_SEARCH_ALTITUDE_KM,
                "district" | "locality" => LOCALITY_SEARCH_ALTITUDE_KM,
                "street" => STREET_SEARCH_ALTITUDE_KM,
                "house" => HOUSE_SEARCH_ALTITUDE_KM,
                _ => DEFAULT_SEARCH_ALTITUDE_KM,
            }
        };
        let label = parts.join(", ");
        if !out.iter().any(|s| s.label == label) {
            out.push(Suggestion {
                label,
                lat,
                lon,
                alt,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn photon_coordinates_share_validation_and_keep_longitude_first_order() {
        let rows = parse_photon_response(serde_json::json!({"features": [
            {"geometry":{"coordinates":[-50, -15]},"properties":{"name":"Valid"}},
            {"geometry":{"coordinates":[181, 10]},"properties":{"name":"Bad longitude"}},
            {"geometry":{"coordinates":[10, 91]},"properties":{"name":"Bad latitude"}},
            {"geometry":{"coordinates":[10]},"properties":{"name":"Incomplete"}}
        ]}));
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].lat, rows[0].lon), (-15.0, -50.0));
    }
    #[test]
    fn typed_search_skips_invalid_rows_and_retains_defaults() {
        let rows = parse_geocode_response(serde_json::json!([
            {"lat":"-15","lon":"-50","display_name":"Brasília"},
            {"lat":"invalid","lon":"10"},
            {"lat":"91","lon":"10"},
            {"lat":"NaN","lon":"10"},
            {"lat":"10","lon":"181"},
            {"lat":"1","lon":"2","display_name":42,"boundingbox":["bad"]}
        ]))
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Brasília");
        assert_eq!(rows[1].name, "?");
        assert_eq!(rows[1].alt, DEFAULT_SEARCH_ALTITUDE_KM);
    }
}

mod local;
pub use local::merge_suggestions;

mod prefix;
pub use prefix::LocalSearch;
