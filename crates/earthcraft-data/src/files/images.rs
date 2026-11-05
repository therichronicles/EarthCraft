//! Image inputs are captured at submission; decoding runs on the file worker.
use std::{path::PathBuf, sync::Arc};

pub enum ImageSource {
    Path(PathBuf),
    Encoded(Arc<Vec<u8>>),
    Embedded(&'static [u8]),
}
impl ImageSource {
    pub(super) fn read(self) -> Result<Arc<Vec<u8>>, crate::Error> {
        match self {
            Self::Path(path) => Ok(Arc::new(std::fs::read(path)?)),
            Self::Encoded(bytes) => Ok(bytes),
            Self::Embedded(bytes) => Ok(Arc::new(bytes.to_vec())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageTarget {
    Planet,
    Sky,
    Moon,
    FlatEarth,
}

pub struct LoadedImage {
    pub pixels: image::RgbaImage,
    pub encoded: Arc<Vec<u8>>,
}

pub struct PlanetSave {
    pub path: PathBuf,
    pub name: String,
    pub texture: ImageSource,
    pub sky: Option<ImageSource>,
}

/// Bound frontend textures and decoder allocations independently. Originals remain encoded.
pub(crate) const MAX_TEXTURE_SIDE_PX: u32 = 4096;
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;

pub(crate) fn decode_texture(
    bytes: &[u8],
    max_side: u32,
) -> Result<image::RgbaImage, crate::Error> {
    use image::{ImageDecoder, ImageReader, Limits, imageops::FilterType};
    let max_side = max_side.min(MAX_TEXTURE_SIDE_PX);
    if max_side == 0 {
        return Err("Image texture limit must be positive".into());
    }
    let mut reader = ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) * 4 > MAX_DECODE_BYTES {
        return Err("Image exceeds the 256 MiB decoded pixel limit".into());
    }
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("Image decode failed (256 MiB allocation limit): {error}"))?;
    if width == 0 || height == 0 {
        return Err("Image has no pixels".into());
    }
    // Conversion to RGBA may need more memory than the original decoder layout.
    if u64::from(width) * u64::from(height) * 4 > MAX_DECODE_BYTES {
        return Err("Image exceeds the 256 MiB decoded pixel limit".into());
    }
    Ok(if width > max_side || height > max_side {
        image
            .resize(max_side, max_side, FilterType::Triangle)
            .to_rgba8()
    } else {
        image.to_rgba8()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_texture_preserves_aspect_and_rejects_zero_limit() {
        let image = image::RgbaImage::from_pixel(80, 40, image::Rgba([5, 10, 15, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let result = decode_texture(bytes.get_ref(), 16).unwrap();
        assert_eq!(result.dimensions(), (16, 8));
        assert_eq!(result.get_pixel(0, 0).0, [5, 10, 15, 255]);
        assert!(decode_texture(bytes.get_ref(), 0).is_err());
    }
}
