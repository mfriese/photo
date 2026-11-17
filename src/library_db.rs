//! SQLite is a disposable index. All durable user metadata remains in individual YAML records.
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::{path::Path, sync::Mutex, time::Duration};
pub const FILE: &str = "yaml/library.sqlite";
const SCHEMA: i64 = 2;
// Covers all connections, including independent adapters and database replacement.
static ACCESS: Mutex<()> = Mutex::new(());
#[derive(Default)]
pub struct Database;
#[derive(Clone, Copy)]
pub enum Section {
    Tags,
    Comments,
    Transcription,
}
impl Section {
    fn column(self) -> &'static str {
        match self {
            Self::Tags => "tags_revision",
            Self::Comments => "comments_revision",
            Self::Transcription => "transcription_revision",
        }
    }
}
pub fn invalid(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(message.into())))
}
const DDL: &str = "
CREATE TABLE index_state (
 id INTEGER PRIMARY KEY CHECK(id=1), instance TEXT NOT NULL,
 tags_revision INTEGER NOT NULL DEFAULT 0,
 comments_revision INTEGER NOT NULL DEFAULT 0,
 transcription_revision INTEGER NOT NULL DEFAULT 0,
 change_sequence INTEGER NOT NULL DEFAULT 0
) STRICT;
INSERT INTO index_state(id,instance) VALUES(1,lower(hex(randomblob(16))));
CREATE TABLE media (
 path TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN ('photo','video','audio')),
 metadata_path TEXT NOT NULL, recorded_at TEXT, reference_at TEXT
) STRICT;
CREATE INDEX media_by_reference_time ON media(reference_at,path);
CREATE INDEX media_by_kind_time ON media(kind,reference_at,path);
CREATE TABLE media_tags (
 media_path TEXT NOT NULL REFERENCES media(path), tag_name TEXT NOT NULL CHECK(length(tag_name) BETWEEN 1 AND 100),
 PRIMARY KEY(media_path,tag_name)
) STRICT;
CREATE INDEX media_tags_by_name ON media_tags(tag_name,media_path);
CREATE VIEW tags AS SELECT tag_name AS name, count(*) AS media_count FROM media_tags GROUP BY tag_name;
CREATE TABLE media_geo (
 media_path TEXT PRIMARY KEY REFERENCES media(path),source_bytes INTEGER NOT NULL CHECK(source_bytes>=0),source_modified TEXT NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('missing','invalid','available')),latitude REAL,longitude REAL,error TEXT NOT NULL DEFAULT '',
 place_id INTEGER,place_name TEXT,place_ascii_name TEXT,place_latitude REAL,place_longitude REAL,country_code TEXT,admin1_code TEXT,distance_meters REAL,dataset_sha256 TEXT,
 CHECK((status='available' AND latitude BETWEEN -90 AND 90 AND longitude BETWEEN -180 AND 180 AND latitude IS NOT NULL AND longitude IS NOT NULL) OR (status!='available' AND latitude IS NULL AND longitude IS NULL)),
 CHECK(place_id IS NULL OR (place_id>0 AND status='available' AND place_name IS NOT NULL AND place_ascii_name IS NOT NULL AND place_latitude IS NOT NULL AND place_longitude IS NOT NULL AND country_code IS NOT NULL AND admin1_code IS NOT NULL AND distance_meters IS NOT NULL AND distance_meters>=0 AND dataset_sha256 IS NOT NULL AND length(dataset_sha256)=64))
) STRICT;
CREATE INDEX geo_by_country_region ON media_geo(country_code,admin1_code,media_path) WHERE status='available';
CREATE INDEX geo_by_place ON media_geo(place_id,media_path) WHERE place_id IS NOT NULL;
CREATE INDEX geo_by_coordinates ON media_geo(latitude,longitude) WHERE status='available';
CREATE VIEW geo_overview AS SELECT media_path,status,latitude,longitude,place_name,country_code,admin1_code,distance_meters FROM media_geo;
CREATE TABLE audio_comments (
 id TEXT PRIMARY KEY, photo_path TEXT NOT NULL REFERENCES media(path), audio_path TEXT NOT NULL REFERENCES media(path),
 UNIQUE(audio_path)
) STRICT;
CREATE INDEX comments_by_photo ON audio_comments(photo_path,audio_path);
CREATE TABLE audio_durations (
 media_path TEXT PRIMARY KEY REFERENCES media(path), year TEXT NOT NULL,
 seconds REAL NOT NULL CHECK(seconds>0), source_bytes INTEGER NOT NULL CHECK(source_bytes>=0), source_modified TEXT NOT NULL
) STRICT;
CREATE INDEX durations_by_year ON audio_durations(year,media_path);
CREATE TABLE audio_verifications (year TEXT PRIMARY KEY, last_full_verified TEXT) STRICT;
CREATE TABLE transcription_jobs (
 source_path TEXT PRIMARY KEY REFERENCES media(path), record_path TEXT NOT NULL REFERENCES media(path),
 source_bytes INTEGER NOT NULL CHECK(source_bytes>=0), source_modified TEXT NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('pending','complete','failed')), error TEXT NOT NULL DEFAULT ''
) STRICT;
CREATE INDEX jobs_by_status ON transcription_jobs(status,source_path);
CREATE INDEX jobs_by_record ON transcription_jobs(record_path,source_path);
CREATE VIEW media_overview AS SELECT m.path,m.kind,m.metadata_path,m.recorded_at,m.reference_at,
 (SELECT group_concat(tag_name, ', ') FROM (SELECT tag_name FROM media_tags WHERE media_path=m.path ORDER BY tag_name)) AS tags,
 (SELECT seconds FROM audio_durations WHERE media_path=m.path) AS audio_seconds
 FROM media m;
CREATE VIEW transcription_overview AS SELECT source_path,record_path,status,error,source_bytes,source_modified FROM transcription_jobs;
PRAGMA user_version=2;
PRAGMA application_id=1347634516;
";
impl Database {
    pub fn transaction<T>(
        &self,
        root: &Path,
        f: impl FnOnce(&Transaction<'_>) -> rusqlite::Result<T>,
    ) -> Result<T, String> {
        let _guard = ACCESS.lock().map_err(|_| "Datenbankzugriff unterbrochen")?;
        let path = crate::metadata_fs::library_path(root, FILE, true)?;
        crate::metadata_fs::check_file(&path)?;
        for suffix in ["-journal", "-wal", "-shm"] {
            crate::metadata_fs::check_file(
                &path.with_file_name(format!("library.sqlite{suffix}")),
            )?;
        }
        let canonical = path
            .parent()
            .ok_or("Datenbankordner fehlt")?
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("library.sqlite");
        let mut conn = Connection::open_with_flags(
            &canonical,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
                | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(|e| format!("Bibliotheksindex nicht lesbar; Neuaufbau erforderlich: {e}"))?;
        conn.busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(|e| e.to_string())?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(|e| e.to_string())?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|e| format!("Bibliotheksindex beschädigt; Neuaufbau erforderlich: {e}"))?;
        if version == 0 {
            let tables:i64=conn.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0)).map_err(|e|e.to_string())?;
            if tables != 0 {
                return Err("Unbekannte Datenbank; Datei bleibt unverändert".into());
            }
            conn.pragma_update(None, "journal_mode", "DELETE")
                .map_err(|e| e.to_string())?;
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            tx.execute_batch(DDL).map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        } else if version != SCHEMA {
            return Err(
                "Veraltetes Indexschema; aus Einzel-YAMLs neu aufbauen (keine Migration)".into(),
            );
        }
        let app: i64 = conn
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if app != 1347634516 {
            return Err("Keine PhotoSuite-Indexdatenbank".into());
        }
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let result = f(&tx).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }
}
pub fn revision(tx: &Transaction<'_>, section: Section) -> rusqlite::Result<Option<Vec<u8>>> {
    let (instance, value): (String, i64) = tx.query_row(
        &format!(
            "SELECT instance,{} FROM index_state WHERE id=1",
            section.column()
        ),
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok((value > 0).then(|| format!("{instance}:{value}").into_bytes()))
}
pub fn publish(
    tx: &Transaction<'_>,
    section: Section,
    expected: Option<&[u8]>,
) -> rusqlite::Result<Vec<u8>> {
    if revision(tx, section)?.as_deref() != expected {
        return Err(invalid(
            "Datenbankindex wurde gleichzeitig verändert; erneut laden",
        ));
    }
    tx.execute(
        &format!(
            "UPDATE index_state SET {0}={0}+1,change_sequence=change_sequence+1 WHERE id=1",
            section.column()
        ),
        [],
    )?;
    revision(tx, section)?.ok_or_else(|| invalid("Indexrevision fehlt"))
}
pub fn media(
    tx: &Transaction<'_>,
    file: &crate::domain::tags::LibraryFile,
) -> rusqlite::Result<()> {
    let group = std::path::Path::new(file.as_str())
        .parent()
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .into_owned();
    let m = crate::domain::Media::from_library_path(file.as_str().into(), group)
        .ok_or_else(|| invalid("Ungültiger Medienpfad im Index"))?;
    let kind = if m.is_audio() {
        "audio"
    } else if m.is_video() {
        "video"
    } else {
        "photo"
    };
    let captured = crate::domain::capture_from_filename(&m)
        .map(|t| t.exif_value().replacen(':', "-", 1).replacen(':', "-", 1));
    tx.prepare_cached("INSERT OR IGNORE INTO media(path,kind,metadata_path,recorded_at,reference_at) VALUES(?1,?2,?3,?4,?4)")?
        .execute((file.as_str(),kind,file.metadata_path().map_err(invalid)?,captured))?;
    Ok(())
}
pub fn optional<T>(value: rusqlite::Result<T>) -> rusqlite::Result<Option<T>> {
    value.optional()
}

impl Database {
    /// Build beside the existing index. Do not hold database locks while scanning YAML sources.
    pub fn replace(
        &self,
        root: &Path,
        cancel: &std::sync::atomic::AtomicBool,
        build: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        use std::{fs, io::Read, sync::atomic::Ordering};
        fn stamp(path: &Path) -> Result<Option<(u64, std::time::SystemTime, Vec<u8>)>, String> {
            if !crate::metadata_fs::check_file(path)? {
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
        let target = crate::metadata_fs::library_path(root, FILE, true)?;
        let before = {
            let _guard = ACCESS.lock().map_err(|_| "Datenbankzugriff unterbrochen")?;
            stamp(&target)?
        };
        let staging = tempfile::Builder::new()
            .prefix(".photosuite-index-")
            .tempdir_in(target.parent().ok_or("Datenbankordner fehlt")?)
            .map_err(|e| e.to_string())?;
        build(staging.path())?;
        if cancel.load(Ordering::Relaxed) {
            return Err("Datenbank-Neuaufbau abgebrochen; bisheriger Index bleibt erhalten".into());
        }
        let staged = staging.path().join(FILE);
        fs::File::open(&staged)
            .map_err(|e| e.to_string())?
            .sync_all()
            .map_err(|e| e.to_string())?;
        let _guard = ACCESS.lock().map_err(|_| "Datenbankzugriff unterbrochen")?;
        crate::metadata_fs::library_path(root, FILE, false)?;
        if stamp(&target)? != before {
            return Err("Datenbank während des Neuaufbaus geändert; erneut versuchen".into());
        }
        for suffix in ["-journal", "-wal", "-shm"] {
            let sidecar = target.with_file_name(format!("library.sqlite{suffix}"));
            if crate::metadata_fs::check_file(&sidecar)? {
                return Err("Datenbank ist noch geöffnet oder hat ein Wiederherstellungsjournal; Neuaufbau nicht veröffentlicht".into());
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Datenbank-Neuaufbau abgebrochen".into());
        }
        fs::rename(staged, &target).map_err(|e| e.to_string())?;
        if let Err(error) = fs::File::open(target.parent().ok_or("Datenbankordner fehlt")?)
            .and_then(|file| file.sync_all())
        {
            // exFAT may not support directory fsync. The staged SQLite file is already synced;
            // this derived index remains reconstructible after a power interruption.
            if error.kind() != std::io::ErrorKind::Unsupported
                && error.raw_os_error() != Some(45)
                && error.raw_os_error() != Some(22)
            {
                return Err(format!(
                    "Index veröffentlicht, Verzeichnissynchronisation fehlgeschlagen: {error}"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transaction_rollback_and_constraints_preserve_published_revision() {
        let root = tempfile::tempdir().unwrap();
        let db = Database;
        let before = db
            .transaction(root.path(), |tx| publish(tx, Section::Tags, None))
            .unwrap();
        assert!(
            db.transaction(root.path(), |tx| {
                publish(tx, Section::Tags, Some(&before))?;
                tx.execute(
                    "INSERT INTO media_tags(media_path,tag_name) VALUES('missing.jpg','scan')",
                    [],
                )?;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(
            db.transaction(root.path(), |tx| revision(tx, Section::Tags))
                .unwrap(),
            Some(before)
        );
    }
    #[cfg(unix)]
    #[test]
    fn database_and_journal_symlinks_cannot_modify_external_files() {
        for name in [
            "library.sqlite",
            "library.sqlite-journal",
            "library.sqlite-wal",
            "library.sqlite-shm",
        ] {
            let root = tempfile::tempdir().unwrap();
            std::fs::create_dir(root.path().join("yaml")).unwrap();
            let outside = root.path().join("outside");
            std::fs::write(&outside, b"untouched").unwrap();
            std::os::unix::fs::symlink(&outside, root.path().join("yaml").join(name)).unwrap();
            assert!(Database.transaction(root.path(), |_| Ok(())).is_err());
            assert_eq!(std::fs::read(outside).unwrap(), b"untouched");
        }
    }
}
