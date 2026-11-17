//! Rebuildable appwide GeoNames index. rusqlite/ZIP stay outside the application core.
use crate::metadata_fs::check_file;
use photosuite_application::geodata::{
    Bounds, DatabaseInfo, Phase, Place, PlaceIndex, Progress, SOURCE,
};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
const VERSION: i64 = 1;
const APPLICATION_ID: i64 = 0x50534745;
const MAX_EXPANDED: u64 = 256 * 1024 * 1024;
const MAX_LINE: u64 = 64 * 1024;
static COMMIT: Mutex<()> = Mutex::new(());
pub struct SqlitePlaces;
fn cancelled(stop: &AtomicBool) -> Result<(), String> {
    if stop.load(Ordering::Relaxed) {
        Err("Geodatenvorbereitung abgebrochen".into())
    } else {
        Ok(())
    }
}
fn canonical(path: &Path) -> Result<std::path::PathBuf, String> {
    if !check_file(path)? {
        return Err("GeoNames-Datenbank fehlt".into());
    }
    Ok(path
        .parent()
        .ok_or("Geodatenverzeichnis fehlt")?
        .canonicalize()
        .map_err(|e| e.to_string())?
        .join(path.file_name().ok_or("Datenbankname fehlt")?))
}
fn open(path: &Path, write: bool) -> Result<Connection, String> {
    for suffix in ["-journal", "-wal", "-shm"] {
        check_file(&path.with_file_name(
            format!("{}{suffix}",path.file_name().ok_or("Datenbankname fehlt")?.to_string_lossy()),
        ))?;
    }
    let flags = if write {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let conn = Connection::open_with_flags(
        canonical(path)?,
        flags | OpenFlags::SQLITE_OPEN_NOFOLLOW | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    Ok(conn)
}
fn schema(conn: &Connection) -> Result<(), String> {
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let app: i64 = conn
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version != VERSION || app != APPLICATION_ID {
        return Err("GeoNames-Datenbank hat ein unbekanntes Schema; erneut vorbereiten".into());
    }
    Ok(())
}
fn stamp(path: &Path) -> Result<Option<(u64, std::time::SystemTime, Vec<u8>)>, String> {
    if !check_file(path)? {
        return Ok(None);
    }
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    let mut header = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(100)
        .read_to_end(&mut header)
        .map_err(|e| e.to_string())?;
    Ok(Some((
        m.len(),
        m.modified().map_err(|e| e.to_string())?,
        header,
    )))
}
const DDL:&str="
CREATE TABLE import_info(id INTEGER PRIMARY KEY CHECK(id=1),source_url TEXT NOT NULL,source_sha256 TEXT NOT NULL,place_count INTEGER NOT NULL CHECK(place_count>0),imported_at TEXT NOT NULL) STRICT;
CREATE TABLE places(
 geoname_id INTEGER PRIMARY KEY CHECK(geoname_id>0),name TEXT NOT NULL,ascii_name TEXT NOT NULL,alternative_names TEXT NOT NULL,
 latitude REAL NOT NULL CHECK(latitude BETWEEN -90 AND 90),longitude REAL NOT NULL CHECK(longitude BETWEEN -180 AND 180),
 feature_class TEXT NOT NULL,feature_code TEXT NOT NULL,country_code TEXT NOT NULL,alternate_country_codes TEXT NOT NULL,
 admin1_code TEXT NOT NULL,admin2_code TEXT NOT NULL,admin3_code TEXT NOT NULL,admin4_code TEXT NOT NULL,
 population INTEGER NOT NULL CHECK(population>=0),elevation INTEGER,dem INTEGER,timezone TEXT NOT NULL,modified_on TEXT NOT NULL
) STRICT;
CREATE INDEX places_by_name ON places(name,geoname_id);
CREATE INDEX places_by_ascii_name ON places(ascii_name,geoname_id);
CREATE INDEX places_by_country_admin1 ON places(country_code,admin1_code,geoname_id);
CREATE VIRTUAL TABLE place_coordinates USING rtree(geoname_id,min_longitude,max_longitude,min_latitude,max_latitude);
CREATE VIEW places_overview AS SELECT geoname_id,name,country_code,admin1_code,latitude,longitude,population,timezone FROM places;
PRAGMA user_version=1;
PRAGMA application_id=1347635013;
";
struct Row<'a> {
    columns: Vec<&'a str>,
    id: i64,
    latitude: f64,
    longitude: f64,
    population: i64,
    elevation: Option<i64>,
    dem: Option<i64>,
}
fn number(value: &str) -> Result<Option<i64>, String> {
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|_| "Ungültige GeoNames-Höhenangabe".into())
    }
}
fn parse(line: &str) -> Result<Row<'_>, String> {
    let columns: Vec<_> = line.trim_end_matches(['\n', '\r']).split('\t').collect();
    if columns.len() != 19 {
        return Err("GeoNames-Zeile benötigt genau 19 Spalten".into());
    }
    let id: i64 = columns[0].parse().map_err(|_| "Ungültige GeoNames-ID")?;
    let latitude: f64 = columns[4].parse().map_err(|_| "Ungültiger Breitengrad")?;
    let longitude: f64 = columns[5].parse().map_err(|_| "Ungültiger Längengrad")?;
    let population: i64 = columns[14].parse().map_err(|_| "Ungültige Einwohnerzahl")?;
    if id <= 0
        || columns[1].is_empty()
        || !latitude.is_finite()
        || !longitude.is_finite()
        || !(-90. ..=90.).contains(&latitude)
        || !(-180. ..=180.).contains(&longitude)
        || population < 0
        || columns[6].len() != 1
        || (!columns[8].is_empty()
            && (columns[8].len() != 2 || !columns[8].bytes().all(|b| b.is_ascii_uppercase())))
    {
        return Err("Ungültige GeoNames-Ortsdaten".into());
    }
    crate::domain::CaptureTime::parse(
        &format!("{} 00:00:00", columns[18].replace('-', ":")),
        "GeoNames Änderungsdatum",
    )?;
    Ok(Row {
        id,
        latitude,
        longitude,
        population,
        elevation: number(columns[15])?,
        dem: number(columns[16])?,
        columns,
    })
}
impl PlaceIndex for SqlitePlaces {
    fn inspect(
        &self,
        database: &Path,
        source_sha256: Option<&str>,
        stop: &AtomicBool,
        notify: &(dyn Fn(Progress) + Sync),
    ) -> Result<DatabaseInfo, String> {
        cancelled(stop)?;
        notify(Progress {
            phase: Phase::CheckingDatabase,
            current: 0,
            total: 0,
        });
        let before = stamp(database)?;
        let conn = open(database, false)?;
        schema(&conn)?;
        let integrity: String = conn
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if integrity != "ok" {
            return Err(format!("GeoNames-Datenbank beschädigt: {integrity}"));
        }
        cancelled(stop)?;
        let (url,hash,count,imported):(String,String,u64,String)=conn.query_row("SELECT source_url,source_sha256,place_count,imported_at FROM import_info WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|e|e.to_string())?;
        if url != SOURCE
            || count == 0
            || hash.len() != 64
            || !hash.bytes().all(|b| b.is_ascii_hexdigit())
            || source_sha256.is_some_and(|h| h != hash)
        {
            return Err("SQLite-Import gehört nicht zum geprüften ZIP; neu importieren".into());
        }
        let actual: u64 = conn
            .query_row("SELECT count(*) FROM places", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let indexed: u64 = conn
            .query_row("SELECT count(*) FROM place_coordinates", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let tree: String = conn
            .query_row("SELECT rtreecheck('place_coordinates')", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let missing:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM places p LEFT JOIN place_coordinates r ON p.geoname_id=r.geoname_id WHERE r.geoname_id IS NULL OR r.min_longitude>p.longitude OR r.max_longitude<p.longitude OR r.min_latitude>p.latitude OR r.max_latitude<p.latitude)",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        if actual != count || indexed != count || tree != "ok" || missing {
            return Err(
                "GeoNames-Import oder räumlicher Index unvollständig; neu importieren".into(),
            );
        }
        cancelled(stop)?;
        drop(conn);
        if stamp(database)? != before {
            return Err("GeoNames-Datenbank während der Prüfung verändert".into());
        }
        Ok(DatabaseInfo {
            places: count,
            source_sha256: hash,
            imported_at: imported,
        })
    }
    fn prepare(
        &self,
        archive: &Path,
        database: &Path,
        source_sha256: &str,
        stop: &AtomicBool,
        notify: &(dyn Fn(Progress) + Sync),
        verify_source: &(dyn Fn() -> Result<(), String> + Sync),
    ) -> Result<DatabaseInfo, String> {
        cancelled(stop)?;
        let previous = stamp(database)?;
        let parent = database.parent().ok_or("Geodatenverzeichnis fehlt")?;
        let temp = tempfile::Builder::new()
            .prefix(".photosuite-places-")
            .suffix(".tmp")
            .tempfile_in(parent)
            .map_err(|e| e.to_string())?;
        let mut conn = open(temp.path(), true)?;
        conn.pragma_update(None, "journal_mode", "DELETE")
            .map_err(|e| e.to_string())?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(DDL).map_err(|e| e.to_string())?;
        let input = fs::File::open(archive).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(input).map_err(|e| e.to_string())?;
        let entry = zip.by_name("cities500.txt").map_err(|e| e.to_string())?;
        if entry.size() > MAX_EXPANDED {
            return Err("GeoNames-Inhalt überschreitet 256 MiB".into());
        }
        let mut reader = BufReader::new(entry.take(MAX_EXPANDED + 1));
        let mut line = Vec::new();
        let mut consumed = 0;
        let mut count = 0u64;
        notify(Progress {
            phase: Phase::Importing,
            current: 0,
            total: 0,
        });
        {
            let mut insert=tx.prepare("INSERT INTO places VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)").map_err(|e|e.to_string())?;
            loop {
                cancelled(stop)?;
                line.clear();
                let read = reader
                    .by_ref()
                    .take(MAX_LINE + 1)
                    .read_until(b'\n', &mut line)
                    .map_err(|e| e.to_string())?;
                if read == 0 {
                    break;
                }
                consumed += read as u64;
                if read as u64 > MAX_LINE || consumed > MAX_EXPANDED {
                    return Err("GeoNames-Inhalt oder einzelne Zeile zu groß".into());
                }
                let text = std::str::from_utf8(&line).map_err(|e| e.to_string())?;
                let row = parse(text).map_err(|e| format!("GeoNames-Zeile {}: {e}", count + 1))?;
                let c = &row.columns;
                insert
                    .execute(rusqlite::params![
                        row.id,
                        c[1],
                        c[2],
                        c[3],
                        row.latitude,
                        row.longitude,
                        c[6],
                        c[7],
                        c[8],
                        c[9],
                        c[10],
                        c[11],
                        c[12],
                        c[13],
                        row.population,
                        row.elevation,
                        row.dem,
                        c[17],
                        c[18]
                    ])
                    .map_err(|e| format!("GeoNames-Zeile {}: {e}", count + 1))?;
                count += 1;
                if count.is_multiple_of(1024) {
                    notify(Progress {
                        phase: Phase::Importing,
                        current: count,
                        total: 0,
                    });
                }
            }
        }
        if count == 0 {
            return Err("GeoNames-Import enthält keine Orte".into());
        }
        cancelled(stop)?;
        notify(Progress {
            phase: Phase::Importing,
            current: count,
            total: count,
        });
        tx.execute("INSERT INTO place_coordinates SELECT geoname_id,longitude,longitude,latitude,latitude FROM places",[]).map_err(|e|e.to_string())?;
        tx.execute(
            "INSERT INTO import_info VALUES(1,?1,?2,?3,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            (SOURCE, source_sha256, count),
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        drop(conn);
        let info = self.inspect(temp.path(), Some(source_sha256), stop, notify)?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        cancelled(stop)?;
        let _guard = COMMIT
            .lock()
            .map_err(|_| "Geodaten-Schreibschutz unterbrochen")?;
        verify_source()?;
        cancelled(stop)?;
        if stamp(database)? != previous {
            return Err("GeoNames-Datenbank extern geändert; kein Überschreiben".into());
        }
        for suffix in ["-journal", "-wal", "-shm"] {
            let sidecar = database.with_file_name(format!(
                "{}{suffix}",
                database
                    .file_name()
                    .ok_or("Dateiname fehlt")?
                    .to_string_lossy()
            ));
            if check_file(&sidecar)? {
                return Err(
                    "GeoNames-Datenbank ist noch geöffnet; Import nicht veröffentlicht".into(),
                );
            }
        }
        cancelled(stop)?;
        if previous.is_none() {
            temp.persist_noclobber(database)
                .map_err(|e| e.to_string())?;
        } else {
            temp.persist(database).map_err(|e| e.to_string())?;
        }
        Ok(info)
    }
    fn candidates(
        &self,
        database: &Path,
        bounds: Bounds,
        limit: usize,
    ) -> Result<Vec<Place>, String> {
        if limit == 0 || limit > 1000 {
            return Err("GeoNames-Abfrage benötigt ein Limit von 1 bis 1000".into());
        }
        let conn = open(database, false)?;
        schema(&conn)?;
        let mut stmt=conn.prepare("SELECT p.geoname_id,p.name,p.ascii_name,p.latitude,p.longitude,p.country_code,p.admin1_code FROM place_coordinates r JOIN places p ON p.geoname_id=r.geoname_id WHERE r.min_longitude<=?1 AND r.max_longitude>=?2 AND r.min_latitude<=?3 AND r.max_latitude>=?4 AND p.longitude BETWEEN ?2 AND ?1 AND p.latitude BETWEEN ?4 AND ?3 ORDER BY p.geoname_id LIMIT ?5").map_err(|e|e.to_string())?;
        stmt.query_map(
            rusqlite::params![
                bounds.east(),
                bounds.west(),
                bounds.north(),
                bounds.south(),
                limit
            ],
            |r| {
                Ok(Place {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    ascii_name: r.get(2)?,
                    latitude: r.get(3)?,
                    longitude: r.get(4)?,
                    country_code: r.get(5)?,
                    admin1_code: r.get(6)?,
                })
            },
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn row(
        id: i64,
        name: &str,
        latitude: &str,
        longitude: &str,
        country: &str,
        admin1: &str,
    ) -> String {
        format!(
            "{id}\t{name}\t{name}\t\t{latitude}\t{longitude}\tP\tPPL\t{country}\t\t{admin1}\t\t\t\t1000\t\t0\tPacific/Fiji\t2026-11-17\n"
        )
    }
    fn archive(root: &Path, lines: &str) -> std::path::PathBuf {
        let path = root.join("cities500.zip");
        let file = fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("cities500.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(lines.as_bytes()).unwrap();
        zip.finish().unwrap();
        path
    }
    #[test]
    fn import_is_readable_complete_and_spatial_candidates_are_available_to_rust() {
        let root = tempfile::tempdir().unwrap();
        let lines = row(1, "Berlin", "52.52", "13.41", "DE", "16")
            + &row(2, "Los Angeles", "34.05", "-118.24", "US", "CA")
            + &row(3, "Suva", "-18.14", "178.44", "FJ", "01");
        let archive = archive(root.path(), &lines);
        let database = root.path().join("places.sqlite");
        let hash = "a".repeat(64);
        let stop = AtomicBool::new(false);
        let info = SqlitePlaces
            .prepare(&archive, &database, &hash, &stop, &|_| {}, &|| Ok(()))
            .unwrap();
        assert_eq!(info.places, 3);
        assert!(!info.imported_at.is_empty());
        assert_eq!(
            SqlitePlaces
                .inspect(&database, Some(&hash), &stop, &|_| {})
                .unwrap(),
            info
        );
        let results = SqlitePlaces
            .candidates(&database, Bounds::new(-119., 33., -117., 35.).unwrap(), 10)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Los Angeles");
        assert_eq!(results[0].country_code, "US");
        assert_eq!(results[0].admin1_code, "CA");
        assert_eq!(
            SqlitePlaces
                .candidates(&database, Bounds::new(178., -19., 179., -17.).unwrap(), 10)
                .unwrap()[0]
                .name,
            "Suva"
        );
        let conn = open(&database, false).unwrap();
        let plan:String=conn.query_row("EXPLAIN QUERY PLAN SELECT geoname_id FROM place_coordinates WHERE min_longitude<=14 AND max_longitude>=13",[],|r|r.get(3)).unwrap();
        assert!(plan.contains("VIRTUAL TABLE INDEX"));
        assert!(
            SqlitePlaces
                .inspect(&database, Some(&"b".repeat(64)), &stop, &|_| {})
                .is_err()
        );
        assert!(Bounds::new(f64::NAN, 0., 1., 1.).is_err());
        assert!(Bounds::new(1., 0., -1., 1.).is_err());
        assert!(
            SqlitePlaces
                .candidates(&database, Bounds::new(0., 0., 1., 1.).unwrap(), 1001)
                .is_err()
        );
    }
    #[test]
    fn cancelled_malformed_or_changed_import_never_replaces_the_previous_database() {
        let root = tempfile::tempdir().unwrap();
        let line = row(1, "Berlin", "52.52", "13.41", "DE", "16");
        let input = archive(root.path(), &line);
        let db = root.path().join("places.sqlite");
        let hash = "a".repeat(64);
        SqlitePlaces
            .prepare(
                &input,
                &db,
                &hash,
                &AtomicBool::new(false),
                &|_| {},
                &|| Ok(()),
            )
            .unwrap();
        let before = fs::read(&db).unwrap();
        let stop = AtomicBool::new(false);
        assert!(
            SqlitePlaces
                .prepare(
                    &input,
                    &db,
                    &hash,
                    &stop,
                    &|p| {
                        if p.phase == Phase::Importing {
                            stop.store(true, Ordering::Relaxed);
                        }
                    },
                    &|| Ok(())
                )
                .is_err()
        );
        assert_eq!(fs::read(&db).unwrap(), before);
        for lines in [
            "wrong\tcolumns\n".to_owned(),
            row(1, "Bad", "NaN", "0", "DE", "16"),
            line.clone() + &line,
        ] {
            let input = archive(root.path(), &lines);
            assert!(
                SqlitePlaces
                    .prepare(
                        &input,
                        &db,
                        &hash,
                        &AtomicBool::new(false),
                        &|_| {},
                        &|| Ok(())
                    )
                    .is_err()
            );
            assert_eq!(fs::read(&db).unwrap(), before);
        }
        let input = archive(root.path(), &line);
        assert!(
            SqlitePlaces
                .prepare(
                    &input,
                    &db,
                    &hash,
                    &AtomicBool::new(false),
                    &|_| {},
                    &|| Err("ZIP changed".into())
                )
                .is_err()
        );
        assert_eq!(fs::read(&db).unwrap(), before);
        assert!(
            SqlitePlaces
                .prepare(
                    &input,
                    &db,
                    &hash,
                    &AtomicBool::new(false),
                    &|_| {},
                    &|| {
                        fs::write(&db, b"external replacement").unwrap();
                        Ok(())
                    }
                )
                .is_err()
        );
        assert_eq!(fs::read(&db).unwrap(), b"external replacement");
        assert!(
            !fs::read_dir(root.path()).unwrap().any(|p| p
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp"))
        );
    }
    #[cfg(unix)]
    #[test]
    fn database_symlinks_are_rejected_without_touching_external_files() {
        let root = tempfile::tempdir().unwrap();
        let input = archive(root.path(), &row(1, "Berlin", "52.52", "13.41", "DE", "16"));
        let outside = root.path().join("external");
        fs::write(&outside, b"untouched").unwrap();
        let db = root.path().join("places.sqlite");
        std::os::unix::fs::symlink(&outside, &db).unwrap();
        assert!(
            SqlitePlaces
                .prepare(
                    &input,
                    &db,
                    &"a".repeat(64),
                    &AtomicBool::new(false),
                    &|_| {},
                    &|| Ok(())
                )
                .is_err()
        );
        assert_eq!(fs::read(outside).unwrap(), b"untouched");
    }
}
