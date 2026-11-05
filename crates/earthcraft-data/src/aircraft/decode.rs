//! Provider-specific decoding; coordinates share core validation and CPU work is bounded.
use super::{Aircraft, MAX_POSITION_AGE_SECS};
use earthcraft_core::math::now_unix;
use std::sync::Arc;
const GLOBAL_RESPONSE_LIMIT: u64 = 20 * 1024 * 1024;
const LOCAL_RESPONSE_LIMIT: u64 = 8 * 1024 * 1024;
const ADSB_LOCAL_RADIUS_NM: u32 = 250;
const FEET_PER_METER: f64 = 3.28084;
const KNOTS_PER_METER_PER_SECOND: f64 = 1.94384449;
async fn response(request: reqwest::RequestBuilder, limit: u64) -> Result<Vec<u8>, crate::Error> {
    crate::http::body(request, limit).await
}
fn number(v: Option<&serde_json::Value>) -> Option<f64> {
    v.and_then(|v| v.as_f64()).filter(|v| v.is_finite())
}
fn safe_heading(v: Option<f64>) -> f64 {
    v.unwrap_or(0.0).rem_euclid(360.0)
}

pub(super) async fn download_global(url: String) -> Result<Arc<[Aircraft]>, crate::Error> {
    let client = crate::http::client(crate::http::Policy::Aircraft)?;
    let bytes = response(client.get(url), GLOBAL_RESPONSE_LIMIT).await?;
    crate::cpu::run(
        crate::cpu::Service::Features,
        move || -> Result<_, crate::Error> {
            let json: serde_json::Value = serde_json::from_slice(&bytes)?;
            if !json.get("states").is_some_and(|states| states.is_array()) {
                return Err("OpenSky response lacks state vectors".into());
            }
            Ok(parse_opensky(&json, now_unix()).into())
        },
    )
    .await
}
pub(super) async fn download_local(
    base_url: String,
    lat: f64,
    lon: f64,
) -> Result<Arc<[Aircraft]>, crate::Error> {
    let client = crate::http::client(crate::http::Policy::Aircraft)?;
    let url = format!("{base_url}/lat/{lat:.4}/lon/{lon:.4}/dist/{ADSB_LOCAL_RADIUS_NM}");
    let bytes = response(client.get(url), LOCAL_RESPONSE_LIMIT).await?;
    crate::cpu::run(
        crate::cpu::Service::Features,
        move || -> Result<_, crate::Error> {
            let json: serde_json::Value = serde_json::from_slice(&bytes)?;
            if !json.get("ac").is_some_and(|aircraft| aircraft.is_array()) {
                return Err("ADSB.lol response lacks aircraft".into());
            }
            Ok(parse_adsb_lol(&json, now_unix()).into())
        },
    )
    .await
}

fn parse_opensky(json: &serde_json::Value, now: f64) -> Vec<Aircraft> {
    json.get("states")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let f = v.as_array()?;
            let hex = f.first()?.as_str()?.trim().to_ascii_lowercase();
            if hex.is_empty() || f.get(8)?.as_bool().unwrap_or(false) {
                return None;
            }
            let (lat, lon) = earthcraft_core::coordinates::validate_coordinates(
                number(f.get(6))?,
                number(f.get(5))?,
            )
            .ok()?;
            let observed = number(f.get(3)).or_else(|| number(f.get(4)))?;
            if !(0.0..=MAX_POSITION_AGE_SECS).contains(&(now - observed)) {
                return None;
            }
            let callsign = f
                .get(1)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_owned();
            let altitude_feet = number(f.get(7))
                .or_else(|| number(f.get(13)))
                .map(|m| m * FEET_PER_METER);
            let speed_knots = number(f.get(9)).map(|mps| mps * KNOTS_PER_METER_PER_SECOND);
            Some(Aircraft {
                hex: hex.into(),
                callsign: callsign.into(),
                latitude: lat,
                longitude: lon,
                heading_deg: safe_heading(number(f.get(10))),
                speed_knots,
                altitude_feet,
                observed_unix: observed,
            })
        })
        .collect()
}
fn parse_adsb_lol(json: &serde_json::Value, now: f64) -> Vec<Aircraft> {
    json.get("ac")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let hex = v.get("hex")?.as_str()?.trim().to_ascii_lowercase();
            if hex.is_empty() || v.get("alt_baro").and_then(|v| v.as_str()) == Some("ground") {
                return None;
            }
            let (lat, lon) = earthcraft_core::coordinates::validate_coordinates(
                number(v.get("lat"))?,
                number(v.get("lon"))?,
            )
            .ok()?;
            let age = number(v.get("seen_pos"))?;
            if !(0.0..=MAX_POSITION_AGE_SECS).contains(&age) {
                return None;
            }
            Some(Aircraft {
                hex: hex.into(),
                callsign: v
                    .get("flight")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .into(),
                latitude: lat,
                longitude: lon,
                heading_deg: safe_heading(number(v.get("track"))),
                speed_knots: number(v.get("gs")),
                altitude_feet: number(v.get("alt_baro")).or_else(|| number(v.get("alt_geom"))),
                observed_unix: now - age,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_oversized_response_is_rejected_instead_of_accepting_a_truncated_prefix() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n0123456789",
                )
                .unwrap();
        });
        let runtime = crate::runtime::ServiceRuntime::new().unwrap();
        let result = runtime.handle().block_on(async {
            response(
                crate::http::client(crate::http::Policy::Aircraft)
                    .unwrap()
                    .get(format!("http://{address}")),
                8,
            )
            .await
        });
        assert!(result.unwrap_err().to_string().contains("exceeds 8 bytes"));
        server.join().unwrap();
    }
    #[test]
    fn parses_opensky_positions_not_ground_vehicles_or_invalid_locations() {
        let now = 1000.0;
        let data = serde_json::json!({"states": [
            ["e490aa", "AZU123 ", "Brazil", 990, 995, -43.2, -19.7, 10000.0, false, 250.0, 45.0, null, null, null],
            ["x", "GROUND", "Brazil", 990, 995, -43.2, -19.7, 0, true, 0, 0],
            ["z", "OLD", "Brazil", 800, 800, -43.2, -19.7, 1000, false, 100, 0]
        ]});
        let flights = parse_opensky(&data, now);
        assert_eq!(flights.len(), 1);
        assert_eq!(flights[0].callsign.as_ref(), "AZU123");
        assert!((flights[0].altitude_feet.unwrap() - 32808.4).abs() < 0.1);
    }
    #[test]
    fn parses_adsb_position_and_rejects_ground_and_stale_records() {
        let data = serde_json::json!({"ac": [
            {"hex":"ab1234", "flight":"GLO123 ", "lat":-23.4, "lon":-46.4,
             "seen_pos":4.2,"gs":440.0,"track":110.0,"alt_baro":36000},
            {"hex":"ab1235","lat":-23.4,"lon":-46.4,"seen_pos":1,"alt_baro":"ground"},
            {"hex":"ab1236","lat":-23.4,"lon":-46.4,"seen_pos":400}
        ]});
        let flights = parse_adsb_lol(&data, 1000.0);
        assert_eq!(flights.len(), 1);
        assert_eq!(flights[0].callsign.as_ref(), "GLO123");
        assert!((flights[0].observed_unix - 995.8).abs() < 0.01);
    }
}
