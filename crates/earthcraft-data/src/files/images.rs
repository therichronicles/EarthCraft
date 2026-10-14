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
