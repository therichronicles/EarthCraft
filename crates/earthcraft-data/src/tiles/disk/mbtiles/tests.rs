use super::super::tests::Fixture;
use super::*;
#[tokio::test]
async fn schema_uses_tms_rows_and_preserves_encoded_payload_across_restarts() {
    let fixture = Fixture::new();
    let store = Mbtiles::new(fixture.0.clone());
    let path = fixture.0.join("osm/3/2/1.tile");
    let bytes = include_bytes!("../../../../../earthcraft-render/tests/fixtures/tile.png");
    store.write(&path, bytes).await.unwrap();
    assert!(!path.exists());
    let db = rusqlite::Connection::open(fixture.0.join("osm/tiles-png.mbtiles")).unwrap();
    let row: (u32, Vec<u8>) = db
        .query_row(
            "SELECT tile_row,tile_data FROM tiles WHERE zoom_level=3 AND tile_column=2",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(row.0, 6);
    assert_eq!(row.1, bytes);
    assert_eq!(
        db.query_row("SELECT value FROM metadata WHERE name='format'", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "png"
    );
    assert_eq!(
        db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    drop(store);
    let reopened = Mbtiles::new(fixture.0.clone());
    assert_eq!(reopened.read(&path).await.unwrap().unwrap(), bytes);
}
#[tokio::test]
async fn aggregate_file_budget_evicts_detail_before_overview_and_reclaims_disk() {
    let fixture = Fixture::new();
    let store = Mbtiles::with_limit(fixture.0.clone(), 160 * 1024);
    let overview = fixture.0.join("esri_sat/0/0/0.tile");
    store.write(&overview, &vec![1; 24 * 1024]).await.unwrap();
    for x in 0..20 {
        store
            .write(
                &fixture.0.join(format!("osm/6/{x}/0.tile")),
                &vec![2; 24 * 1024],
            )
            .await
            .unwrap();
    }
    assert!(store.usage().await.unwrap() <= 160 * 1024);
    assert!(store.read(&overview).await.unwrap().is_some());
    assert!(
        store
            .read(&fixture.0.join("osm/6/0/0.tile"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .read(&fixture.0.join("osm/6/19/0.tile"))
            .await
            .unwrap()
            .is_some()
    );
    store.clear().await.unwrap();
    assert_eq!(store.usage().await.unwrap(), 0);
}
#[tokio::test]
async fn legacy_tiles_migrate_without_a_network_fetch_and_invalid_tiles_can_be_removed() {
    let fixture = Fixture::new();
    let store = Mbtiles::new(fixture.0.clone());
    let path = fixture.0.join("esri_sat/2/1/1.tile");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"legacy").unwrap();
    assert_eq!(store.read(&path).await.unwrap().unwrap(), b"legacy");
    assert!(!path.exists());
    store.invalidate(&path).await.unwrap();
    assert!(store.read(&path).await.unwrap().is_none());
}
#[tokio::test]
async fn providers_releases_and_payload_formats_have_distinct_tilesets() {
    let fixture = Fixture::new();
    let store = Mbtiles::new(fixture.0.clone());
    for scope in ["esri_sat", "osm", "opentopo", "wayback/42", "wayback/99"] {
        let path = fixture.0.join(format!("{scope}/0/0/0.tile"));
        store.write(&path, scope.as_bytes()).await.unwrap();
    }
    for scope in ["esri_sat", "osm", "opentopo", "wayback/42", "wayback/99"] {
        let path = fixture.0.join(format!("{scope}/0/0/0.tile"));
        assert_eq!(store.read(&path).await.unwrap().unwrap(), scope.as_bytes());
    }
}
#[test]
fn bundled_sqlite_supports_fts5_and_rtree_without_loading_extensions() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE VIRTUAL TABLE text_index USING fts5(name); CREATE VIRTUAL TABLE spatial_index USING rtree(id,min_x,max_x,min_y,max_y);").unwrap();
}

#[test]
fn batch_trim_reclaims_many_tiles_and_retains_newest_and_overview() {
    let fixture = Fixture::new();
    let path = fixture.0.join("osm/tiles-png.mbtiles");
    let mut connection = rusqlite::Connection::open_in_memory().unwrap();
    with_database(&mut connection, &fixture.0, &path, true, "png", |db| {
        let transaction = db.transaction()?;
        for x in 0..700u32 {
            transaction.execute(
                "INSERT INTO tile_cache.tiles VALUES (10,?1,0,?2)",
                rusqlite::params![x, vec![7u8; 4096]],
            )?;
            transaction.execute(
                "INSERT INTO tile_cache.earthcraft_access VALUES (10,?1,0,?1,?2)",
                rusqlite::params![x, x == 0],
            )?;
        }
        transaction.commit()?;
        Ok(())
    })
    .unwrap();
    let mut budget = budget::Budget::scan(&fixture.0).unwrap();
    let limit = 256 * 1024;
    budget.trim(&mut connection, &fixture.0, limit).unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() <= limit);
    let db = rusqlite::Connection::open(path).unwrap();
    let retained: Vec<u32> = db
        .prepare("SELECT tile_column FROM tiles ORDER BY tile_column")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(retained.first(), Some(&0));
    assert_eq!(retained.last(), Some(&699));
    assert!(retained.len() < 100);
    assert!(!retained.contains(&1));
    assert_eq!(
        db.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}
