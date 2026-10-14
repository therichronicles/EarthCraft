//! Read the main KML document without extracting archive paths or resources.
use std::{
    fs::File,
    io::{Cursor, Read, Write},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn read(path: &str) -> Result<String, crate::Error> {
    let mut archive = ZipArchive::new(File::open(path)?)?;
    // Prefer the conventional root document; otherwise preserve archive order.
    let mut selected = None;
    for (index, name) in archive.file_names().enumerate() {
        let name = name?;
        if name
            .rsplit('.')
            .next()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("kml"))
        {
            if name.eq_ignore_ascii_case("doc.kml") {
                selected = Some(index);
                break;
            }
            selected.get_or_insert(index);
        }
    }
    let index = selected.ok_or("KMZ archive contains no KML document")?;
    let document = archive.by_index(index)?;
    if document.size() > MAX_DOCUMENT_BYTES {
        return Err("KMZ KML document exceeds the 64 MiB limit".into());
    }
    let mut xml = String::new();
    document
        .take(MAX_DOCUMENT_BYTES + 1)
        .read_to_string(&mut xml)?;
    if xml.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("KMZ KML document exceeds the 64 MiB limit".into());
    }
    Ok(xml)
}

pub(super) fn encode(xml: &[u8]) -> Result<Vec<u8>, crate::Error> {
    if xml.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("KMZ KML document exceeds the 64 MiB limit".into());
    }
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    archive.start_file(
        "doc.kml",
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
    )?;
    archive.write_all(xml)?;
    Ok(archive.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kml::{load_kml, parse_tree, save_kml};
    use earthcraft_core::language::Lang;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const XML: &str = r#"<kml><Document><name>Places</name><Folder><name>São Paulo</name><visibility>0</visibility><Placemark><name>A &amp; B</name><description>Notes</description><Point><coordinates>-46.6,-23.5</coordinates></Point></Placemark></Folder></Document></kml>"#;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "earthcraft-kmz-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn archive(&self, entries: &[(&str, &[u8])], method: zip::CompressionMethod) -> PathBuf {
            let path = self.0.join("places.KMZ");
            let mut archive = ZipWriter::new(File::create(&path).unwrap());
            for (name, bytes) in entries {
                archive
                    .start_file(
                        *name,
                        SimpleFileOptions::default().compression_method(method),
                    )
                    .unwrap();
                archive.write_all(bytes).unwrap();
            }
            archive.finish().unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn stored_and_deflated_archives_prefer_root_doc_without_extracting_resources() {
        for method in [
            zip::CompressionMethod::Stored,
            zip::CompressionMethod::Deflated,
        ] {
            let fixture = Fixture::new();
            let path = fixture.archive(
                &[
                    ("first.kml", b"invalid KML"),
                    ("doc.kml", XML.as_bytes()),
                    ("../outside.txt", b"not extracted"),
                    ("images/pin.png", b"not decoded"),
                ],
                method,
            );
            let places = load_kml(path.to_str().unwrap(), Lang::En).unwrap();
            assert_eq!(places.len(), 1);
            assert_eq!(places.roots[0].name(), "Places");
            assert_eq!(places.first().unwrap().name, "A & B");
            assert_eq!(places.markers(true).count(), 0);
            assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
        }
    }

    #[test]
    fn nested_document_fallback_keeps_archive_order_and_imports_only_one_document() {
        let fixture = Fixture::new();
        let path = fixture.archive(
            &[
                ("nested/first.KML", XML.as_bytes()),
                ("other.kml", b"invalid KML"),
            ],
            zip::CompressionMethod::Deflated,
        );
        assert_eq!(load_kml(path.to_str().unwrap(), Lang::En).unwrap().len(), 1);
    }

    #[test]
    fn kmz_export_roundtrips_the_same_tree_as_plain_kml() {
        let fixture = Fixture::new();
        let original = parse_tree(XML, Lang::En).unwrap();
        let kmz = fixture.0.join("export.kmz");
        let kml = fixture.0.join("export.kml");
        save_kml(kmz.to_str().unwrap(), &original).unwrap();
        save_kml(kml.to_str().unwrap(), &original).unwrap();
        let mut archive = ZipArchive::new(File::open(&kmz).unwrap()).unwrap();
        assert_eq!(archive.len(), 1);
        assert_eq!(
            archive.by_index(0).unwrap().compression(),
            zip::CompressionMethod::Deflated
        );
        let mut xml = String::new();
        archive
            .by_name("doc.kml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert_eq!(xml, std::fs::read_to_string(kml).unwrap());
        let loaded = load_kml(kmz.to_str().unwrap(), Lang::En).unwrap();
        assert_eq!(loaded.first(), original.first());
        assert_eq!(loaded.markers(true).count(), 0);
    }

    #[test]
    fn invalid_archives_missing_documents_and_oversized_documents_report_errors() {
        let fixture = Fixture::new();
        let path = fixture.archive(&[("readme.txt", b"no KML")], zip::CompressionMethod::Stored);
        assert!(
            load_kml(path.to_str().unwrap(), Lang::En)
                .unwrap_err()
                .to_string()
                .contains("no KML")
        );
        std::fs::write(&path, b"not a ZIP archive").unwrap();
        assert!(matches!(
            load_kml(path.to_str().unwrap(), Lang::En),
            Err(crate::Error::Zip(_))
        ));
        let mut archive = ZipWriter::new(File::create(&path).unwrap());
        archive
            .start_file(
                "doc.kml",
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        std::io::copy(
            &mut std::io::repeat(b' ').take(MAX_DOCUMENT_BYTES + 1),
            &mut archive,
        )
        .unwrap();
        archive.finish().unwrap();
        assert!(
            load_kml(path.to_str().unwrap(), Lang::En)
                .unwrap_err()
                .to_string()
                .contains("64 MiB")
        );
    }

    #[test]
    fn archived_kml_keeps_empty_point_and_truncated_container_handling() {
        let fixture = Fixture::new();
        let xml = br#"<kml><Document><Folder><name>Imported</name><Placemark><name>Empty</name><Point/></Placemark><Placemark><name>Valid</name><Point><coordinates>1,2</coordinates></Point></Placemark>"#;
        let path = fixture.archive(&[("doc.kml", xml)], zip::CompressionMethod::Deflated);
        let places = load_kml(path.to_str().unwrap(), Lang::En).unwrap();
        assert_eq!(places.len(), 1);
        assert_eq!(places.first().unwrap().name, "Valid");
        assert_eq!(places.first().unwrap().lat, 2.0);
        assert_eq!(places.first().unwrap().lon, 1.0);
    }
}
