use crate::paths::cache_dir;
use std::{path::PathBuf, time::Duration};
const MAX_CACHE_FILENAME_CHARS: usize = 120;
const JSON_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const JSON_READ_TIMEOUT: Duration = Duration::from_secs(90);
pub(crate) const JSON_REQUEST_ATTEMPTS: usize = 2;
pub(crate) const MAX_JSON_RESPONSE_BYTES: u64 = 1 << 30;

pub(crate) fn geo_cache_path(url: &str) -> PathBuf {
    let clean: String = url
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let start = clean.len().saturating_sub(MAX_CACHE_FILENAME_CHARS);
    cache_dir().join("geo").join(&clean[start..])
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Policy {
    Tile,
    Boundary,
    Wayback,
    Search,
    Suggest,
    Cloud,
    Radar,
    Aircraft,
}
pub(crate) fn client(policy: Policy) -> Result<reqwest::Client, crate::Error> {
    use std::{
        collections::HashMap,
        sync::{Mutex, OnceLock},
    };
    static CLIENTS: OnceLock<Mutex<HashMap<Policy, reqwest::Client>>> = OnceLock::new();
    let mut clients = CLIENTS
        .get_or_init(Default::default)
        .lock()
        .expect("HTTP client pool poisoned");
    if let Some(client) = clients.get(&policy) {
        return Ok(client.clone());
    }
    let mut builder = reqwest::Client::builder();
    let agent = match policy {
        Policy::Tile | Policy::Search => "EarthCraft/0.1 (visualizador educacional de globo 3D)",
        Policy::Boundary => "EarthCraft/0.1 (map boundary overlay)",
        Policy::Wayback => "EarthCraft/0.2 (wayback release list)",
        Policy::Suggest => "EarthCraft/0.2 (search suggestions)",
        Policy::Cloud => "EarthCraft/1.0 (attribution: Matt Eason, EUMETSAT)",
        Policy::Radar => "EarthCraft/1.0 radar viewer",
        Policy::Aircraft => "EarthCraft/1.0 (live aircraft viewer; OpenSky / ADSB.lol)",
    };
    builder = builder.user_agent(agent);
    match policy {
        Policy::Tile => builder = builder.timeout(Duration::from_secs(15)),
        Policy::Boundary => {
            builder = builder
                .connect_timeout(JSON_CONNECT_TIMEOUT)
                .read_timeout(JSON_READ_TIMEOUT)
        }
        Policy::Wayback => {
            builder = builder
                .connect_timeout(Duration::from_secs(15))
                .read_timeout(Duration::from_secs(60))
        }
        Policy::Suggest => builder = builder.timeout(Duration::from_secs(8)),
        Policy::Cloud => builder = builder.timeout(Duration::from_secs(65)),
        Policy::Radar => builder = builder.timeout(Duration::from_secs(12)),
        Policy::Aircraft => builder = builder.timeout(Duration::from_secs(25)),
        Policy::Search => {}
    }
    let client = builder.build()?;
    clients.insert(policy, client.clone());
    Ok(client)
}
pub(crate) async fn body(
    request: reqwest::RequestBuilder,
    limit: u64,
) -> Result<Vec<u8>, crate::Error> {
    let mut response = request.send().await?.error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let remaining = (limit as usize).saturating_sub(bytes.len());
        bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        if bytes.len() as u64 >= limit {
            break;
        }
    }
    Ok(bytes)
}
pub(crate) async fn parse_json(bytes: Vec<u8>) -> Result<serde_json::Value, crate::Error> {
    tokio::task::spawn_blocking(move || serde_json::from_slice(&bytes))
        .await?
        .map_err(Into::into)
}
/// Fetch boundary JSON with bounded retries and disk-cache fallback.
pub(crate) async fn fetch_json(url: &str) -> Result<serde_json::Value, crate::Error> {
    fetch_json_at(url, geo_cache_path(url)).await
}
async fn fetch_json_at(url: &str, path: PathBuf) -> Result<serde_json::Value, crate::Error> {
    if let Ok(bytes) = tokio::fs::read(&path).await
        && let Ok(value) = parse_json(bytes).await
    {
        return Ok(value);
    }
    let client = client(Policy::Boundary)?;
    let mut last = None;
    for _ in 0..JSON_REQUEST_ATTEMPTS {
        match body(client.get(url), MAX_JSON_RESPONSE_BYTES).await {
            Ok(bytes) => {
                let parsed = tokio::task::spawn_blocking(move || {
                    serde_json::from_slice::<serde_json::Value>(&bytes).map(|value| (value, bytes))
                })
                .await?;
                match parsed {
                    Ok((value, bytes)) => {
                        if let Some(dir) = path.parent() {
                            let _ = tokio::fs::create_dir_all(dir).await;
                        }
                        let _ = tokio::fs::write(&path, bytes).await;
                        return Ok(value);
                    }
                    Err(error) => last = Some(format!("Invalid JSON: {error}").into()),
                }
            }
            Err(error) => last = Some(error),
        }
    }
    Err(last.expect("at least one request attempt"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn server(
        responses: Vec<(&'static str, &'static str)>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let job = tokio::spawn(async move {
            for (status, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0u8; 2048];
                let _ = socket.read(&mut request).await.unwrap();
                socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        (url, job)
    }
    #[tokio::test]
    async fn body_caps_reads_and_rejects_http_errors() {
        let (url, job) = server(vec![("200 OK", "hello world")]).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        assert_eq!(body(client.get(url), 4).await.unwrap(), b"hell");
        job.await.unwrap();
        let (url, job) = server(vec![("500 Server Error", "bad")]).await;
        assert!(matches!(
            body(client.get(url), 100).await,
            Err(crate::Error::Http(_))
        ));
        job.await.unwrap();
    }
    #[tokio::test]
    async fn boundary_retry_and_disk_cache_avoid_a_second_network_fetch() {
        let (url, job) = server(vec![
            ("500 Server Error", "bad"),
            ("200 OK", r#"{"features":[]}"#),
        ])
        .await;
        let path = std::env::temp_dir().join(format!(
            "earthcraft-http-{}-{}.json",
            std::process::id(),
            earthcraft_core::math::now_unix()
        ));
        let first = fetch_json_at(&url, path.clone()).await.unwrap();
        job.await.unwrap();
        let cached = fetch_json_at(&url, path.clone()).await.unwrap();
        assert_eq!(first, cached);
        tokio::fs::remove_file(path).await.unwrap();
    }
}
