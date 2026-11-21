//! Media YAML is authoritative; SQLite contains only its reconstructible date projection.
mod relocation;
use crate::{
    domain::{CaptureTime, tags::LibraryFile},
    library_db::{self, Database},
    media_records_fs,
};
use photosuite_application::date_correction::{Catalog, Repository};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
pub struct CaptureDates {
    pub database: Arc<Database>,
    pub priority: Arc<photosuite_application::work_priority::WorkPriority>,
}
pub(crate) fn project(
    tx: &rusqlite::Transaction<'_>,
    file: &LibraryFile,
    date: &CaptureTime,
    root: &Path,
) -> rusqlite::Result<()> {
    tx.execute_batch("CREATE TABLE IF NOT EXISTS capture_date_overrides(media_path TEXT PRIMARY KEY REFERENCES media(path) ON DELETE CASCADE) STRICT;")?;
    library_db::record_address(tx, file, root)?;
    tx.execute(
        "INSERT OR IGNORE INTO capture_date_overrides VALUES(?1)",
        [file.as_str()],
    )?;
    let value = date
        .exif_value()
        .replacen(':', "-", 1)
        .replacen(':', "-", 1);
    tx.execute(
        "UPDATE media SET reference_at=?2 WHERE path=?1",
        (file.as_str(), value),
    )?;
    tx.execute("UPDATE media SET reference_at=(SELECT reference_at FROM media WHERE path=?1) WHERE path IN (SELECT audio_path FROM audio_comments WHERE photo_path=?1)",[file.as_str()])?;
    // Year blocks are disposable sums; the queue reconstructs them after changed placement.
    let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='audio_year_blocks' AND type='table')",[],|r|r.get(0))?;
    if exists {
        tx.execute("DELETE FROM audio_year_blocks", [])?;
    }
    Ok(())
}
pub(crate) fn parse_index(value: &str) -> Result<CaptureTime, String> {
    CaptureTime::parse(
        &value.replacen('-', ":", 1).replacen('-', ":", 1),
        "Korrigiertes Mediendatum",
    )
}
impl Repository for CaptureDates {
    fn load(&self, root: &Path) -> Result<Catalog, String> {
        relocation::recover(self, root)?;
        self.database
            .read(root, |db| {
                let mut catalog = Catalog::new();
                let exists: bool = db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='capture_date_overrides')",
                    [],
                    |row| row.get(0),
                )?;
                if !exists {
                    return Ok(catalog);
                }
                let mut statement = db.prepare(
                    "SELECT m.path,m.reference_at FROM media m JOIN capture_date_overrides c ON c.media_path=m.path WHERE m.kind IN ('photo','video')",
                )?;
                let rows = statement.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (file, date) = row?;
                    catalog.insert(
                        LibraryFile::new(file).map_err(library_db::invalid)?,
                        parse_index(&date).map_err(library_db::invalid)?,
                    );
                }
                Ok(catalog)
            })
            .or_else(|_| {
                let mut catalog = Catalog::new();
                media_records_fs::visit(root, &AtomicBool::new(false), &self.priority, |file, record, _| {
                    if let Some(date) = record.corrected_date()? {
                        catalog.insert(file, date);
                    }
                    Ok(())
                })?;
                Ok(catalog)
            })
    }
    fn relocate(
        &self,
        root: &Path,
        file: &LibraryFile,
        date: &CaptureTime,
        expected: Option<&CaptureTime>,
    ) -> Result<LibraryFile, String> {
        relocation::relocate(self, root, file, date, expected)
    }
    fn save(
        &self,
        root: &Path,
        file: &LibraryFile,
        date: &CaptureTime,
        expected: Option<&CaptureTime>,
    ) -> Result<(), String> {
        relocation::recover(self, root)?;
        let group = Path::new(file.as_str())
            .parent()
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .into_owned();
        let media = crate::domain::Media::from_library_path(file.as_str().into(), group)
            .ok_or("Ungültiges Medium")?;
        if media.is_audio() {
            return Err("Datumskorrektur ist nur für Fotos und Videos verfügbar".into());
        }
        let original = crate::metadata_fs::library_path(root, file.as_str(), false)?;
        if !crate::metadata_fs::check_file(&original)? {
            return Err("Originaldatei fehlt".into());
        }
        let path = media_records_fs::path(root, file, false)?;
        let previous = media_records_fs::read(&path)?;
        let mut record = previous
            .as_deref()
            .map(media_records_fs::parse)
            .transpose()?
            .unwrap_or_else(|| media_records_fs::Record::new(file));
        if record.file != file.as_str() {
            return Err(format!(
                "Medien-YAML gehört zu {} statt {}: {}. Keine fremden Daten geändert.",
                record.file,
                file.as_str(),
                path.display()
            ));
        }
        if record
            .corrected_date()?
            .as_ref()
            .map(CaptureTime::exif_value)
            != expected.map(CaptureTime::exif_value)
        {
            return Err("Mediendatum wurde inzwischen geändert. Dialog erneut öffnen.".into());
        }
        record.capture_date = Some(date.exif_value());
        media_records_fs::write(root, file, &record, previous.as_deref())?;
        self.database
            .transaction(root, |tx| project(tx, file, date, root))
            .map_err(|e| {
                format!("Mediendatum in YAML gespeichert; Suchindex muss neu aufgebaut werden: {e}")
            })
    }
}

/// Source reconciliation is admitted only as part of the existing metadata queue stage.
pub(crate) fn reconcile(
    root: &Path,
    database: &Database,
    priority: &photosuite_application::work_priority::WorkPriority,
    stop: &AtomicBool,
) -> Result<(), String> {
    let mut dates = Catalog::new();
    let mut revisions = Vec::new();
    media_records_fs::visit(root, stop, priority, |file, record, _| {
        let path = media_records_fs::path(root, &file, false)?;
        let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        revisions.push((
            path,
            metadata.len(),
            metadata.modified().map_err(|e| e.to_string())?,
        ));
        if let Some(date) = record.corrected_date()? {
            dates.insert(file, date);
        }
        Ok(())
    })?;
    database.transaction(root, |tx| {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(library_db::invalid("Abgebrochen"));
        }
        for (path, len, modified) in &revisions {
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|error| library_db::invalid(error.to_string()))?;
            let current_modified = metadata.modified()
                .map_err(|error| library_db::invalid(error.to_string()))?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() != *len
                || current_modified != *modified
            {
                return Err(library_db::invalid(
                    "Medien-YAML während der Datumsprüfung geändert; erneut versuchen",
                ));
            }
        }
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS capture_date_overrides(media_path TEXT PRIMARY KEY REFERENCES media(path) ON DELETE CASCADE) STRICT; DELETE FROM capture_date_overrides;",
        )?;
        tx.execute("UPDATE media SET reference_at=recorded_at WHERE kind IN ('photo','video')", [])?;
        for (file, date) in &dates {
            project(tx, file, date, root)?;
        }
        tx.execute(
            "UPDATE media SET reference_at=(SELECT m.reference_at FROM audio_comments c JOIN media m ON m.path=c.photo_path WHERE c.audio_path=media.path) WHERE path IN (SELECT audio_path FROM audio_comments)",
            [],
        )?;
        Ok(())
    })
}

pub(crate) fn ensure_projection(
    root: &Path,
    database: &Database,
    priority: &photosuite_application::work_priority::WorkPriority,
    stop: &AtomicBool,
) -> Result<(), String> {
    let ready=database.read(root,|db|db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='capture_date_overrides' AND type='table')",[],|r|r.get::<_,bool>(0)))?;
    if ready {
        Ok(())
    } else {
        reconcile(root, database, priority, stop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        comments::Comment,
        transcript::{SourceVersion, Transcript, TranscriptStatus},
    };
    use photosuite_application::{comments::CommentRepository, library_index::Rebuilder};
    fn date(value: &str) -> CaptureTime {
        CaptureTime::parse(value, "Korrigiertes Mediendatum").unwrap()
    }
    #[test]
    fn correction_preserves_originals_tags_transcripts_and_moves_all_comment_references_after_rebuild()
     {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let database = Arc::new(Database);
        let adapter = CaptureDates {
            database: database.clone(),
            priority: Arc::default(),
        };
        let photo = LibraryFile::new("2026/scans/img_20261121_035811.jpg".into()).unwrap();
        let original = root.join(photo.as_str());
        std::fs::create_dir_all(original.parent().unwrap()).unwrap();
        std::fs::write(&original, b"original photo bytes").unwrap();
        let mut record = media_records_fs::Record::new(&photo);
        record.tags = Some(["scan".into(), "dia".into()].into());
        let audios = [
            "2026/audio/comments/com_20261121_120000.m4a",
            "2027/audio/comments/com_20270102_130000.m4a",
        ];
        for (i, audio) in audios.iter().enumerate() {
            let path = root.join(audio);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"unchanged audio").unwrap();
            record.comments.insert(
                format!("893d3899-9d2d-4241-8a3c-9f4226353b6{i}"),
                audio.to_string(),
            );
        }
        let transcript = Transcript::new(
            SourceVersion::new(15, "1.0".into()).unwrap(),
            TranscriptStatus::Complete,
            "tiny".into(),
            "de".into(),
            "Erinnerung\nmit Umlaut: schön".into(),
            vec![],
        )
        .unwrap();
        record.transcripts.insert(
            audios[0].into(),
            crate::transcript_codec::TranscriptDto::from_value(&transcript),
        );
        media_records_fs::write(root, &photo, &record, None).unwrap();
        let rebuilder = crate::library_index_fs::SqliteRebuilder {
            database: database.clone(),
            priority: Arc::default(),
        };
        rebuilder
            .rebuild(root, &AtomicBool::new(false), &|_| {})
            .unwrap();
        let corrected = date("1980:01:02 03:58:11");
        adapter.save(root, &photo, &corrected, None).unwrap();
        assert_eq!(adapter.load(root).unwrap()[&photo], corrected);
        let bytes = media_records_fs::read(&media_records_fs::path(root, &photo, false).unwrap())
            .unwrap()
            .unwrap();
        let saved = media_records_fs::parse(&bytes).unwrap();
        assert_eq!(saved.tags, record.tags);
        assert_eq!(saved.comments, record.comments);
        assert_eq!(saved.transcripts[audios[0]].value().unwrap(), transcript);
        assert_eq!(std::fs::read(&original).unwrap(), b"original photo bytes");
        for audio in audios {
            assert_eq!(std::fs::read(root.join(audio)).unwrap(), b"unchanged audio");
        }
        fn check(root: &Path, database: &Arc<Database>, photo: &LibraryFile) {
            let repository = crate::comments_fs::SqliteComments {
                database: database.clone(),
                priority: Arc::default(),
            };
            let photosuite_application::comments::CommentCache::Current(stored) =
                repository.read_cache(root).unwrap()
            else {
                panic!("cache missing")
            };
            assert_eq!(stored.catalog.entries().len(), 2);
            let mut replay = stored.catalog.clone();
            for comment in stored.catalog.entries().values() {
                replay
                    .insert(comment.clone().with_capture_correction(None))
                    .unwrap();
            }
            for comment in replay.entries().values() {
                assert_eq!(comment.reference_time().unwrap().year(), "1980");
            }
            for comment in stored.catalog.entries().values() {
                assert_eq!(comment.reference_time().unwrap().stamp(), "19800102_035811");
                assert!(matches!(
                    comment.recorded_at().time().year(),
                    "2026" | "2027"
                ));
                let media = crate::domain::Media::from_audio_asset(
                    root.join(comment.audio().as_str()),
                    "2026/audio/comments".into(),
                )
                .unwrap()
                .with_comment_recording(Some(comment));
                for key in ["all", "1980", "1980/01"] {
                    assert!(
                        crate::domain::diary::AudioViewScope::parse(key)
                            .unwrap()
                            .includes(&media)
                    );
                }
                for key in ["2026", "2027", "1980/02"] {
                    assert!(
                        !crate::domain::diary::AudioViewScope::parse(key)
                            .unwrap()
                            .includes(&media)
                    );
                }
                assert_eq!(comment.photo(), photo);
            }
            database.read(root,|db| {let count:i64=db.query_row("SELECT count(*) FROM media WHERE kind='audio' AND reference_at='1980-01-02 03:58:11'",[],|r|r.get(0))?;assert_eq!(count,2);Ok(())}).unwrap();
        }
        check(root, &database, &photo);
        // Source-only recovery: discard the complete disposable database.
        std::fs::remove_file(root.join(library_db::FILE)).unwrap();
        rebuilder
            .rebuild(root, &AtomicBool::new(false), &|_| {})
            .unwrap();
        check(root, &database, &photo);
        assert_eq!(adapter.load(root).unwrap()[&photo], corrected);
        assert_eq!(
            std::fs::read(media_records_fs::path(root, &photo, false).unwrap()).unwrap(),
            bytes
        );
        // A later new recording is confirmed by identity, even though its date projection differs.
        let third = Comment::new(
            "893d3899-9d2d-4241-8a3c-9f4226353b69".into(),
            LibraryFile::new("2028/audio/comments/com_20280203_040506.m4a".into()).unwrap(),
            photo.clone(),
        )
        .unwrap();
        let path = root.join(third.audio().as_str());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"new audio").unwrap();
        let repository = crate::comments_fs::SqliteComments {
            database: database.clone(),
            priority: Arc::default(),
        };
        let stored = photosuite_application::comments::load_catalog(
            &repository,
            root,
            &AtomicBool::new(false),
        )
        .unwrap();
        let committed = photosuite_application::comments::commit_comment(
            &repository,
            root,
            &third,
            &stored.catalog,
            stored.revision.as_deref(),
        )
        .unwrap();
        assert_eq!(
            committed
                .catalog
                .by_audio(third.audio())
                .unwrap()
                .reference_time()
                .unwrap()
                .year(),
            "1980"
        );
        // A correction equal to the filename is still an explicit user override, including for scans.
        let reset = date("2026:11:21 03:58:11");
        adapter
            .save(root, &photo, &reset, Some(&corrected))
            .unwrap();
        assert_eq!(adapter.load(root).unwrap()[&photo], reset);
    }
    #[test]
    fn stale_changes_foreign_records_and_symlinks_are_never_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let adapter = CaptureDates {
            database: Arc::new(Database),
            priority: Arc::default(),
        };
        let file = LibraryFile::new("2024/02/img_20240229_120000.jpg".into()).unwrap();
        let original = root.join(file.as_str());
        std::fs::create_dir_all(original.parent().unwrap()).unwrap();
        std::fs::write(&original, b"original").unwrap();
        let first = date("1980:01:01 12:00:00");
        adapter.save(root, &file, &first, None).unwrap();
        let path = media_records_fs::path(root, &file, false).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            adapter
                .save(root, &file, &date("1981:01:01 12:00:00"), None)
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let mut record = media_records_fs::parse(&bytes).unwrap();
        record.file = "2024/02/img_20240228_120000.jpg".into();
        std::fs::write(&path, serde_saphyr::to_string(&record).unwrap()).unwrap();
        let foreign = std::fs::read(&path).unwrap();
        assert!(adapter.save(root, &file, &first, Some(&first)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), foreign);
        #[cfg(unix)]
        {
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(&original, &path).unwrap();
            assert!(adapter.save(root, &file, &first, None).is_err());
            assert_eq!(std::fs::read(original).unwrap(), b"original");
        }
    }
}

#[cfg(test)]
mod video_tests {
    use super::*;
    use crate::domain::{
        Media,
        transcript::{SourceVersion, Transcript, TranscriptStatus},
    };
    use photosuite_application::library_index::Rebuilder;
    #[test]
    fn video_date_round_trip_and_index_rebuild_preserve_original_and_transcript() {
        for extension in ["mp4", "mov", "mkv"] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            let database = Arc::new(Database);
            let adapter = CaptureDates {
                database: database.clone(),
                priority: Arc::default(),
            };
            let file =
                LibraryFile::new(format!("2024/02/img_20240229_035811.{extension}")).unwrap();
            let path = root.join(file.as_str());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"unchanged video bytes").unwrap();
            let mut record = media_records_fs::Record::new(&file);
            record.tags = Some(["holiday".into()].into());
            let transcript = Transcript::new(
                SourceVersion::new(21, "1.0".into()).unwrap(),
                TranscriptStatus::Complete,
                "tiny".into(),
                "de".into(),
                "Video: schöner Tag\nZweite Zeile".into(),
                vec![],
            )
            .unwrap();
            record.transcripts.insert(
                file.as_str().into(),
                crate::transcript_codec::TranscriptDto::from_value(&transcript),
            );
            media_records_fs::write(root, &file, &record, None).unwrap();
            let date =
                CaptureTime::parse("1980:12:31 03:58:11", "Korrigiertes Mediendatum").unwrap();
            adapter.save(root, &file, &date, None).unwrap();
            let sidecar = media_records_fs::path(root, &file, false).unwrap();
            let bytes = std::fs::read(&sidecar).unwrap();
            let saved = media_records_fs::parse(&bytes).unwrap();
            assert_eq!(saved.tags, record.tags);
            assert_eq!(
                saved.transcripts[file.as_str()].value().unwrap(),
                transcript
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"unchanged video bytes");
            for rebuild in [false, true] {
                if rebuild {
                    std::fs::remove_file(root.join(library_db::FILE)).unwrap();
                    crate::library_index_fs::SqliteRebuilder {
                        database: database.clone(),
                        priority: Arc::default(),
                    }
                    .rebuild(root, &AtomicBool::new(false), &|_| {})
                    .unwrap();
                }
                let catalog = adapter.load(root).unwrap();
                assert_eq!(catalog[&file], date);
                let video = Media::from_library_path(path.clone(), "2024/02".into()).unwrap();
                let projected =
                    photosuite_application::date_correction::project(root, &catalog, video);
                assert_eq!(projected.group(), "1980/12");
                assert_eq!(projected.path(), path);
                assert_eq!(
                    crate::domain::capture_from_filename(&projected)
                        .unwrap()
                        .time_label(),
                    "03:58:11"
                );
                assert_eq!(
                    crate::application::visible(
                        std::slice::from_ref(&projected),
                        "",
                        Some("1980/12"),
                        true
                    ),
                    vec![0]
                );
                assert!(
                    crate::application::visible(&[projected], "", Some("2024"), true).is_empty()
                );
            }
            assert_eq!(std::fs::read(sidecar).unwrap(), bytes);
        }
    }
    #[test]
    fn audio_files_including_mp4_diary_containers_reject_date_corrections() {
        let temp = tempfile::tempdir().unwrap();
        let adapter = CaptureDates {
            database: Arc::new(Database),
            priority: Arc::default(),
        };
        let date = CaptureTime::parse("1980:01:01 12:00:00", "test").unwrap();
        for name in [
            "2024/audio/diary/aud_20240229_120000.mp4",
            "2024/audio/diary/aud_20240229_120000.wav",
            "2024/audio/comments/com_20240229_120000.m4a",
        ] {
            let file = LibraryFile::new(name.into()).unwrap();
            assert!(adapter.save(temp.path(), &file, &date, None).is_err());
            let mut record = media_records_fs::Record::new(&file);
            record.capture_date = Some(date.exif_value());
            assert!(
                media_records_fs::parse(serde_saphyr::to_string(&record).unwrap().as_bytes())
                    .is_err()
            );
        }
    }
}

#[cfg(test)]
mod broken_folder_tests {
    use super::*;
    use photosuite_application::library_index::Rebuilder;
    #[test]
    fn broken_example_gets_its_own_date_without_reassigning_an_existing_year_record() {
        for former_file_present in [true, false] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            let database = Arc::new(Database);
            let adapter = CaptureDates {
                database: database.clone(),
                priority: Arc::default(),
            };
            let former = LibraryFile::new("2011/11/img_20111111_124844.mp4".into()).unwrap();
            let broken = LibraryFile::new("broken/img_20111111_124844.mp4".into()).unwrap();
            assert_eq!(
                former.metadata_path().unwrap(),
                broken.metadata_path().unwrap()
            );
            let video = root.join(broken.as_str());
            std::fs::create_dir_all(video.parent().unwrap()).unwrap();
            std::fs::write(&video, b"broken example original").unwrap();
            if former_file_present {
                let path = root.join(former.as_str());
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, b"different original").unwrap();
            }
            let mut old = media_records_fs::Record::new(&former);
            old.tags = Some(["previous".into()].into());
            media_records_fs::write(root, &former, &old, None).unwrap();
            let old_path = media_records_fs::path(root, &former, false).unwrap();
            let old_bytes = std::fs::read(&old_path).unwrap();
            let date =
                CaptureTime::parse("1980:07:08 12:48:44", "Korrigiertes Mediendatum").unwrap();
            adapter.save(root, &broken, &date, None).unwrap();
            let path = media_records_fs::path(root, &broken, false).unwrap();
            assert_ne!(path, old_path);
            assert_eq!(path, root.join(broken.metadata_collision_path().unwrap()));
            let record = media_records_fs::parse(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(record.file, broken.as_str());
            assert!(record.tags.is_none());
            assert_eq!(record.corrected_date().unwrap(), Some(date.clone()));
            assert_eq!(std::fs::read(&old_path).unwrap(), old_bytes);
            assert_eq!(std::fs::read(&video).unwrap(), b"broken example original");
            let rebuild = crate::library_index_fs::SqliteRebuilder {
                database: database.clone(),
                priority: Arc::default(),
            };
            std::fs::remove_file(root.join(library_db::FILE)).unwrap();
            rebuild
                .rebuild(root, &AtomicBool::new(false), &|_| {})
                .unwrap();
            assert_eq!(adapter.load(root).unwrap()[&broken], date);
            database
                .read(root, |db| {
                    let actual: String = db.query_row(
                        "SELECT metadata_path FROM media WHERE path=?1",
                        [broken.as_str()],
                        |row| row.get(0),
                    )?;
                    assert_eq!(actual, broken.metadata_collision_path().unwrap());
                    Ok(())
                })
                .unwrap();
            let raw = crate::domain::Media::from_library_path(video, "broken".into()).unwrap();
            assert_eq!(
                crate::domain::capture_from_filename(&raw).unwrap().year(),
                "2011"
            );
            let projected = photosuite_application::date_correction::project(
                root,
                &adapter.load(root).unwrap(),
                raw,
            );
            assert_eq!(projected.group(), "1980/07");
            assert_eq!(
                crate::domain::capture_from_filename(&projected)
                    .unwrap()
                    .year(),
                "1980"
            );
            // Losing the other owner's YAML must never change the new record's address.
            std::fs::remove_file(&old_path).unwrap();
            assert_eq!(media_records_fs::path(root, &broken, false).unwrap(), path);
            adapter.save(root, &broken, &date, Some(&date)).unwrap();
            assert!(!old_path.exists());
        }
    }
}
