//! Portable .earthplanet document I/O; native dialogs and textures stay in frontends.
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, sync::Arc};
#[derive(Clone)]
pub struct PlanetAssets {
    pub texture: Arc<Vec<u8>>,
    pub sky: Arc<Vec<u8>>,
}
pub struct LoadedPlanet {
    pub name: String,
    pub planet: image::RgbaImage,
    pub sky: Option<image::RgbaImage>,
    pub assets: PlanetAssets,
}
#[derive(Serialize, Deserialize)]
struct PlanetFile {
    version: u32,
    name: String,
    texture: String,
    #[serde(default)]
    texture_bytes: Vec<u8>,
    #[serde(default)]
    sky_bytes: Vec<u8>,
}

pub fn save_planet(
    path: &Path,
    name: &str,
    source: &str,
    sky: &str,
) -> Result<String, crate::Error> {
    let texture_bytes = fs::read(source).map_err(|e| format!("Planet image: {e}"))?;
    let sky_bytes = if sky.trim().is_empty() {
        Vec::new()
    } else {
        fs::read(sky).map_err(|e| format!("Sky: {e}"))?
    };
    fs::write(path, encode_planet(name, texture_bytes, sky_bytes)?)?;
    Ok(format!("Planet and sky embedded in {}", path.display()))
}
/// Encode a self-contained version-2 document, also usable with browser file bytes.
pub fn encode_planet(
    name: &str,
    texture_bytes: Vec<u8>,
    sky_bytes: Vec<u8>,
) -> Result<Vec<u8>, crate::Error> {
    image::load_from_memory(&texture_bytes)?;
    if !sky_bytes.is_empty() {
        image::load_from_memory(&sky_bytes)?;
    }
    Ok(serde_json::to_vec(&PlanetFile {
        version: 2,
        name: name.into(),
        texture: String::new(),
        texture_bytes,
        sky_bytes,
    })?)
}

pub fn load_planet(path: &Path) -> Result<LoadedPlanet, crate::Error> {
    decode_planet(&fs::read(path)?, |texture| {
        Ok(fs::read(
            path.parent().unwrap_or(Path::new(".")).join(texture),
        )?)
    })
}
/// Decode either document version. A frontend resolves legacy sidecar image names.
pub fn decode_planet(
    bytes: &[u8],
    read_texture: impl FnOnce(&str) -> Result<Vec<u8>, crate::Error>,
) -> Result<LoadedPlanet, crate::Error> {
    let doc: PlanetFile = serde_json::from_slice(bytes)?;
    let data = if doc.version == 1 {
        let rel = Path::new(&doc.texture);
        if rel.components().count() != 1 {
            return Err("Invalid path".into());
        }
        read_texture(&doc.texture)?
    } else if doc.version == 2 {
        doc.texture_bytes
    } else {
        return Err("Unsupported version".into());
    };
    let planet = image::load_from_memory(&data)?.to_rgba8();
    let sky = if doc.sky_bytes.is_empty() {
        None
    } else {
        Some(image::load_from_memory(&doc.sky_bytes)?.to_rgba8())
    };
    Ok(LoadedPlanet {
        name: doc.name,
        planet,
        sky,
        assets: PlanetAssets {
            texture: Arc::new(data),
            sky: Arc::new(doc.sky_bytes),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn png() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([123, 45, 67, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }
    #[test]
    fn embedded_document_round_trips_without_a_filesystem() {
        let bytes = encode_planet("My planet", png(), png()).unwrap();
        let loaded = decode_planet(&bytes, |_| {
            panic!("embedded document must not read sidecars")
        })
        .unwrap();
        assert_eq!(loaded.name, "My planet");
        assert_eq!(loaded.planet.dimensions(), (2, 1));
        assert_eq!(loaded.sky.unwrap().get_pixel(0, 0).0, [123, 45, 67, 255]);
    }
    #[test]
    fn legacy_sidecar_is_resolved_by_frontend_and_missing_sky_defaults() {
        let bytes = br#"{"version":1,"name":"Legacy","texture":"planet.png"}"#;
        let loaded = decode_planet(bytes, |name| {
            assert_eq!(name, "planet.png");
            Ok(png())
        })
        .unwrap();
        assert!(loaded.sky.is_none());
        assert_eq!(loaded.name, "Legacy");
    }
    #[test]
    fn malformed_and_unsupported_documents_fail_without_sidecar_reads() {
        for bytes in [
            br#"{"version":1,"name":"x","texture":"../escape.png"}"#.as_slice(),
            br#"{"version":99,"name":"x","texture":""}"#,
            b"not json",
        ] {
            assert!(
                decode_planet(bytes, |_| panic!("invalid document must not read sidecars"))
                    .is_err()
            );
        }
        assert!(encode_planet("x", vec![1, 2, 3], Vec::new()).is_err());
    }
}
