//! Aggregate file-byte accounting and age-based eviction, including legacy tiles.
use super::{Address, FORMATS, with_database};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
const EVICTION_BATCH_TILES: usize = 512;
const EVICTION_HEADROOM_BYTES: u64 = 4 * 1024 * 1024;

pub(super) struct Budget {
    files: HashMap<PathBuf, u64>,
}
#[derive(Clone)]
struct Victim {
    path: PathBuf,
    coordinates: Option<(u32, u32, u32)>,
    overview: bool,
    age: i64,
    bytes: u64,
}
impl Budget {
    pub(super) fn scan(root: &Path) -> Result<Self, crate::Error> {
        let mut pending: Vec<_> = super::super::Provider::ALL
            .iter()
            .map(|p| root.join(p.dir()))
            .collect();
        let mut files = HashMap::new();
        while let Some(path) = pending.pop() {
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                for entry in std::fs::read_dir(&path)? {
                    pending.push(entry?.path());
                }
            } else if metadata.is_file() {
                files.insert(path, metadata.len());
            }
        }
        Ok(Self { files })
    }
    pub(super) fn refresh(&mut self, path: &Path) -> Result<(), crate::Error> {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.file_type().is_symlink() => {
                self.files.insert(path.to_path_buf(), metadata.len());
            }
            Ok(_) => {
                self.files.remove(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.files.remove(path);
            }
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }
    fn bytes(&self) -> u64 {
        self.files.values().copied().sum()
    }
    pub(super) fn trim(
        &mut self,
        connection: &mut rusqlite::Connection,
        root: &Path,
        limit: u64,
    ) -> Result<(), crate::Error> {
        if self.bytes() <= limit {
            return Ok(());
        }
        let target = limit.saturating_sub((limit / 16).min(EVICTION_HEADROOM_BYTES));
        // Legacy metadata is collected once; removed entries are consumed in order.
        let mut legacy = Vec::new();
        let mut databases = Vec::new();
        for (path, bytes) in &self.files {
            if path.extension().is_some_and(|e| e == "tile") {
                let Ok(address) = Address::parse(root, path) else {
                    continue;
                };
                let age = std::fs::metadata(path)?
                    .modified()?
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
                    .min(i64::MAX as u128) as i64;
                legacy.push(Victim {
                    path: path.clone(),
                    coordinates: None,
                    overview: address.overview,
                    age,
                    bytes: *bytes,
                });
            } else if let Some(format) = database_format(path) {
                databases.push((path.clone(), format));
            }
        }
        legacy.sort_by(|a, b| priority(a).cmp(&priority(b)));
        let mut legacy: VecDeque<_> = legacy.into();
        while self.bytes() > target {
            let mut victims: Vec<_> = legacy.iter().take(EVICTION_BATCH_TILES).cloned().collect();
            let mut empty = Vec::new();
            for (path, format) in &databases {
                if !self.files.contains_key(path) {
                    continue;
                }
                let candidates = with_database(connection, root, path, false, format, |db| {
                    let mut query = db.prepare("SELECT t.zoom_level,t.tile_column,t.tile_row,a.overview,a.last_used,LENGTH(t.tile_data) FROM tile_cache.tiles t JOIN tile_cache.earthcraft_access a USING(zoom_level,tile_column,tile_row) ORDER BY a.overview,a.last_used,t.zoom_level,t.tile_column,t.tile_row LIMIT ?1")?;
                    let rows = query.query_map([EVICTION_BATCH_TILES as u32], |row| {
                        Ok(Victim {
                            path: path.clone(),
                            coordinates: Some((row.get(0)?, row.get(1)?, row.get(2)?)),
                            overview: row.get(3)?,
                            age: row.get(4)?,
                            bytes: row.get::<_, i64>(5)? as u64,
                        })
                    })?;
                    Ok(rows.collect::<Result<Vec<_>, _>>()?)
                })?;
                if candidates.is_empty() {
                    empty.push(path.clone());
                }
                victims.extend(candidates);
            }
            for path in empty {
                std::fs::remove_file(&path)?;
                self.refresh(&path)?;
            }
            if self.bytes() <= target {
                break;
            }
            victims.sort_by(|a, b| priority(a).cmp(&priority(b)));
            let needed = self.bytes() - target;
            let mut estimated = 0u64;
            let mut selected = Vec::new();
            for victim in victims.into_iter().take(EVICTION_BATCH_TILES) {
                estimated = estimated.saturating_add(victim.bytes);
                selected.push(victim);
                if estimated >= needed {
                    break;
                }
            }
            if selected.is_empty() {
                return Err("disk cache budget is occupied by unmanaged files".into());
            }
            let mut grouped: HashMap<PathBuf, Vec<(u32, u32, u32)>> = HashMap::new();
            for victim in selected {
                if let Some(coordinates) = victim.coordinates {
                    grouped.entry(victim.path).or_default().push(coordinates);
                } else {
                    std::fs::remove_file(&victim.path)?;
                    self.refresh(&victim.path)?;
                    let removed = legacy.pop_front().expect("selected legacy prefix");
                    debug_assert_eq!(removed.path, victim.path);
                }
            }
            for (path, coordinates) in grouped {
                let format = database_format(&path).ok_or("unknown MBTiles format")?;
                with_database(connection, root, &path, false, format, |db| {
                    let transaction = db.transaction()?;
                    {
                        let mut delete = transaction.prepare("DELETE FROM tile_cache.tiles WHERE zoom_level=?1 AND tile_column=?2 AND tile_row=?3")?;
                        for (zoom, x, y) in coordinates {
                            delete.execute(rusqlite::params![zoom, x, y])?;
                        }
                    }
                    // FULL auto-vacuum reclaims pages once per batch commit.
                    transaction.commit()?;
                    Ok(())
                })?;
                self.refresh(&path)?;
            }
        }
        Ok(())
    }
}
fn priority(victim: &Victim) -> (bool, i64, &Path, Option<(u32, u32, u32)>) {
    (
        victim.overview,
        victim.age,
        &victim.path,
        victim.coordinates,
    )
}
fn database_format(path: &Path) -> Option<&'static str> {
    FORMATS.iter().find_map(|(suffix, format)| {
        path.file_name()
            .is_some_and(|name| name == format!("tiles-{suffix}.mbtiles").as_str())
            .then_some(*format)
    })
}
