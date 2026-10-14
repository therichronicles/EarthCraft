//! Reopen source files, retaining failures except genuinely missing paths.
use super::FileOutcome;
use earthcraft_core::{language::Lang, placemark::Places};
use std::path::{Path, PathBuf};

pub struct RestoredKml {
    pub path: PathBuf,
    pub result: Result<Places, crate::Error>,
}

pub(super) fn restore(path: &Path, lang: Lang) -> Result<FileOutcome, crate::Error> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileOutcome::KmlRestored { files: Vec::new() });
        }
        Err(error) => return Err(error.into()),
    };
    let paths: Vec<PathBuf> = serde_json::from_slice(&bytes)?;
    let mut seen = std::collections::HashSet::new();
    let mut files = Vec::new();
    for path in paths {
        if !seen.insert(path.clone()) {
            continue;
        }
        let result = crate::kml::load_kml(&path.to_string_lossy(), lang);
        if matches!(&result, Err(crate::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound)
        {
            continue;
        }
        files.push(RestoredKml { path, result });
    }
    Ok(FileOutcome::KmlRestored { files })
}
