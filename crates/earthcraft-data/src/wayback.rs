use crate::http::geo_cache_path;
use std::sync::Mutex;
const MAX_WAYBACK_RELEASES: usize = 60_000;
const MAX_WAYBACK_CONFIG_BYTES: u64 = 64 << 20;

// ───────────────────────────── Wayback (historical imagery) ─────────────────────────────

#[derive(Clone, Debug)]
pub struct WaybackRelease {
    pub(crate) id: u32,
    pub date: String,       // YYYY-MM-DD
    pub(crate) url: String, // template with {level}/{row}/{col}
}
/// Sorted newest first. A tile key stores the index into this list.
static WAYBACK: Mutex<Vec<WaybackRelease>> = Mutex::new(Vec::new());
const WAYBACK_CONFIG: &str =
    "https://s3-us-west-2.amazonaws.com/config.maptiles.arcgis.com/waybackconfig.json";

pub(crate) fn wayback_release(rel: u16) -> Option<WaybackRelease> {
    WAYBACK
        .lock()
        .ok()
        .and_then(|v| v.get(rel as usize).cloned())
}
pub(crate) fn wayback_tile_url(rel: u16, z: u8, x: u32, y: u32) -> String {
    match wayback_release(rel) {
        Some(r) => r
            .url
            .replace("{level}", &z.to_string())
            .replace("{row}", &y.to_string())
            .replace("{col}", &x.to_string())
            .replace("{M}", &r.id.to_string()),
        None => String::new(),
    }
}
const TRUSTED_WAYBACK_DOMAINS: &[&str] = &["arcgis.com", "arcgisonline.com", "esri.com"];

fn trusted_wayback_url(template: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(template) else {
        return false;
    };
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.host_str().is_some_and(|host| {
            TRUSTED_WAYBACK_DOMAINS.iter().any(|domain| {
                host == *domain
                    || host
                        .strip_suffix(domain)
                        .is_some_and(|prefix| prefix.ends_with('.'))
            })
        })
}

fn parse_wayback_config(v: &serde_json::Value) -> Vec<WaybackRelease> {
    let mut out = Vec::new();
    let Some(obj) = v.as_object() else { return out };
    for (k, item) in obj {
        let Ok(id) = k.trim().parse::<u32>() else {
            continue;
        };
        let title = item.get("itemTitle").and_then(|t| t.as_str()).unwrap_or("");
        // "World Imagery (Wayback 2014-02-20)"
        let date = title
            .split("Wayback")
            .nth(1)
            .map(|t| t.trim().trim_end_matches(')').trim().to_string())
            .filter(|d| d.len() >= 10)
            .unwrap_or_else(|| title.to_string());
        if date.is_empty() {
            continue;
        }
        let url = item
            .get("itemURL")
            .and_then(|u| u.as_str())
            .map(|u| u.to_string())
            .unwrap_or_else(|| {
                format!("https://wayback.maptiles.arcgis.com/arcgis/rest/services/World_Imagery/WMTS/1.0.0/default028mm/MapServer/tile/{id}/{{level}}/{{row}}/{{col}}")
            });
        if !trusted_wayback_url(&url) {
            continue;
        }
        out.push(WaybackRelease { id, date, url });
    }
    out.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
    out.truncate(MAX_WAYBACK_RELEASES);
    out
}
/// Fetches the release list (network first so that new releases show up; disk cache as fallback).
pub async fn load_wayback_releases() -> Result<Vec<WaybackRelease>, crate::Error> {
    let path = geo_cache_path(WAYBACK_CONFIG);
    let request = crate::http::client(crate::http::Policy::Wayback)?.get(WAYBACK_CONFIG);
    let net = async {
        let bytes = crate::http::body(request, MAX_WAYBACK_CONFIG_BYTES).await?;
        let parsed = tokio::task::spawn_blocking(move || {
            serde_json::from_slice::<serde_json::Value>(&bytes).map(|value| (value, bytes))
        })
        .await?;
        let (value, bytes) = parsed?;
        if let Some(dir) = path.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let _ = tokio::fs::write(&path, bytes).await;
        Ok::<_, crate::Error>(value)
    }
    .await;
    let json = match net {
        Ok(value) => value,
        Err(error) => match tokio::fs::read(&path).await {
            Ok(bytes) => match crate::http::parse_json(bytes).await {
                Ok(value) => value,
                Err(_) => return Err(error),
            },
            Err(_) => return Err(error),
        },
    };
    let list = parse_wayback_config(&json);
    if list.is_empty() {
        Err("empty Wayback release list".into())
    } else {
        Ok(list)
    }
}

/// Installs the release indices used by tile requests, preserving newest-first ordering.
pub fn set_wayback_releases(list: Vec<WaybackRelease>) {
    if let Ok(mut releases) = WAYBACK.lock() {
        *releases = list;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_templates_require_https_and_complete_trusted_hostnames() {
        for url in [
            "http://wayback.maptiles.arcgis.com/tile",
            "https://127.0.0.1/tile",
            "https://arcgis.com.evil.example/tile",
            "https://notarcgis.com/tile",
            "https://user@wayback.maptiles.arcgis.com/tile",
            "https://arcgis.com:444/tile",
            "file:///tmp/tiles",
        ] {
            assert!(!trusted_wayback_url(url), "{url}");
            let config = serde_json::json!({"1":{"itemTitle":"Wayback 2020-01-01","itemURL":url}});
            assert!(parse_wayback_config(&config).is_empty());
        }
        assert!(trusted_wayback_url(
            "https://wayback.maptiles.arcgis.com/tile/{level}/{row}/{col}"
        ));
        let config = serde_json::json!({"1":{"itemTitle":"Wayback 2020-01-01"}});
        assert_eq!(parse_wayback_config(&config).len(), 1);
    }
    #[test]
    fn wayback_config_parses() {
        let v: serde_json::Value = serde_json::json!({
            "10": {"itemTitle": "World Imagery (Wayback 2014-02-20)", "itemURL": "https://wayback.maptiles.arcgis.com/tile/10/{level}/{row}/{col}"},
            "5000": {"itemTitle": "World Imagery (Wayback 2020-01-01)"},
        });
        let l = parse_wayback_config(&v);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].date, "2020-01-01"); // newest first
        assert!(l[0].url.contains("/5000/{level}/{row}/{col}"));
        assert_eq!(l[1].date, "2014-02-20");
    }
}
