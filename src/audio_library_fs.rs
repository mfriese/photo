//! SQLite display projection; maintenance is called only by the serial processing workflow.
use crate::{
    domain::{Media, comments::Comment, tags::LibraryFile},
    library_db::{self, Database},
};
use photosuite_application::{
    audio_library::{Progress, Repository, Snapshot, Stage},
    diary::AudioSource,
    duration_cache::{DurationCache, Fingerprint, Snapshot as Durations},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
type Inventory = BTreeMap<PathBuf, Fingerprint>;
pub struct AudioLibrary {
    pub database: Arc<Database>,
    pub tags: Option<Arc<dyn photosuite_application::organization::CatalogRepository>>,
    pub comments: Option<Arc<dyn photosuite_application::comments::CommentRepository>>,
    pub durations: Arc<dyn DurationCache>,
    pub audio: Arc<dyn AudioSource>,
    pub priority: Arc<photosuite_application::work_priority::WorkPriority>,
    pub pending: Mutex<Option<(PathBuf, Inventory)>>,
    pub issues: Mutex<Option<(PathBuf, Vec<String>)>>,
}
fn check(stop: &AtomicBool) -> Result<(), String> {
    if stop.load(Ordering::Relaxed) {
        Err("Abgebrochen".into())
    } else {
        Ok(())
    }
}
struct CapturedInventory<'a> {
    inner: &'a dyn DurationCache,
    files: &'a Inventory,
}
impl DurationCache for CapturedInventory<'_> {
    fn groups(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        self.inner.groups(paths)
    }
    fn load(&self, group: &Path, stop: &AtomicBool) -> Result<Durations, String> {
        self.inner.load(group, stop)
    }
    fn inventory(&self, group: &Path, _: &AtomicBool) -> Result<Inventory, String> {
        Ok(self
            .files
            .iter()
            .filter(|(p, _)| p.starts_with(group))
            .map(|(p, f)| (p.clone(), f.clone()))
            .collect())
    }
    fn save(&self, group: &Path, snapshot: &Durations) -> Result<(), String> {
        self.inner.save(group, snapshot)
    }
}
impl AudioLibrary {
    fn scan(
        &self,
        root: &Path,
        stop: &AtomicBool,
        progress: &dyn Fn(Progress),
    ) -> Result<Inventory, String> {
        let mut years = Vec::new();
        for entry in std::fs::read_dir(root).map_err(|e| e.to_string())? {
            check(stop)?;
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.len() == 4
                && name.bytes().all(|b| b.is_ascii_digit())
                && entry.file_type().map_err(|e| e.to_string())?.is_dir()
            {
                years.push(entry.path());
            }
        }
        years.sort();
        let total = years.len();
        let mut files = BTreeMap::new();
        for (index, year) in years.into_iter().enumerate() {
            if !self.priority.wait_background(stop) {
                return Err("Abgebrochen".into());
            }
            files.extend(self.durations.inventory(&year, stop)?);
            progress(Progress {
                checked: index + 1,
                total,
                current: Some(year),
            });
        }
        Ok(files)
    }
}
impl Repository for AudioLibrary {
    fn read(&self, root: &Path) -> Result<Snapshot, String> {
        self.database.read(root, |db| {
            let warnings = self
                .issues
                .lock()
                .map_err(|_| library_db::invalid("Audiowarnungen unterbrochen"))?
                .as_ref()
                .filter(|(library, _)| library == root)
                .map(|(_, warnings)| warnings.clone())
                .unwrap_or_default();
            let mut result = Snapshot {
                warnings,
                ..Default::default()
            };
            let mut statement = db.prepare_cached(
                "SELECT d.media_path, d.seconds, c.id, c.photo_path, p.reference_at
                 FROM audio_durations d JOIN media m ON m.path = d.media_path
                 LEFT JOIN audio_comments c ON c.audio_path = d.media_path
                 LEFT JOIN media p ON p.path = c.photo_path
                 ORDER BY d.media_path",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?;
            for row in rows {
                let (path, seconds, id, photo, reference) = row?;
                let file = LibraryFile::new(path.clone()).map_err(library_db::invalid)?;
                if !seconds.is_finite() || seconds <= 0. {
                    return Err(library_db::invalid("Ungültige Audiodauer"));
                }
                let group = Path::new(&path)
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .into_owned();
                let mut media = Media::from_library_path(root.join(&path), group)
                    .ok_or_else(|| library_db::invalid("Ungültiger Audiopfad"))?;
                if !media.is_audio() {
                    return Err(library_db::invalid("Audiocache enthält fremden Medientyp"));
                }
                if let (Some(id), Some(photo)) = (id, photo) {
                    let comment = Comment::new(
                        id,
                        file,
                        LibraryFile::new(photo).map_err(library_db::invalid)?,
                    )
                    .map_err(library_db::invalid)?
                    .with_capture_correction(
                        reference
                            .as_deref()
                            .map(crate::date_correction_fs::parse_index)
                            .transpose()
                            .map_err(library_db::invalid)?,
                    );
                    media = media.with_comment_recording(Some(&comment));
                }
                result.durations.push((media.path().into(), seconds));
                result.media.push(media);
            }
            let has_years: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master
                               WHERE type = 'table' AND name = 'audio_year_blocks')",
                [],
                |row| row.get(0),
            )?;
            if has_years {
                let mut statement =
                    db.prepare_cached("SELECT year, seconds FROM audio_year_blocks ORDER BY year")?;
                for row in statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))? {
                    result.years.push(row?);
                }
            }
            Ok(result)
        })
    }
    fn rerun(
        &self,
        root: &Path,
        stage: Stage,
        stop: &AtomicBool,
        progress: &dyn Fn(Progress),
    ) -> Result<(), String> {
        check(stop)?;
        if stage == Stage::Durations {
            self.database.transaction(root, |tx| {
                tx.execute("DELETE FROM audio_verifications", [])?;
                Ok(())
            })?;
        }
        if stage == Stage::Metadata {
            if let Some(tags) = &self.tags {
                photosuite_application::organization::rebuild_projection(
                    tags.as_ref(),
                    root,
                    stop,
                )?;
            }
            check(stop)?;
            if let Some(comments) = &self.comments {
                photosuite_application::comments::rebuild_catalog(
                    comments.as_ref(),
                    root,
                    stop,
                    &|_| {},
                )?;
            }
            crate::date_correction_fs::reconcile(root, &self.database, &self.priority, stop)?;
            return Ok(());
        }
        self.run(root, stage, stop, progress)
    }
    fn run(
        &self,
        root: &Path,
        stage: Stage,
        stop: &AtomicBool,
        progress: &dyn Fn(Progress),
    ) -> Result<(), String> {
        match stage {
            Stage::Inventory => {
                let files = self.scan(root, stop, progress)?;
                check(stop)?;
                *self
                    .issues
                    .lock()
                    .map_err(|_| "Audiowarnungen unterbrochen")? = Some((root.into(), Vec::new()));
                self.database.transaction(root, |tx| {
                    tx.execute_batch("CREATE TEMP TABLE current_audio(path TEXT PRIMARY KEY) WITHOUT ROWID;")?;
                    for path in files.keys() {
                        let relative = path.strip_prefix(root)
                            .map_err(|error| library_db::invalid(error.to_string()))?
                            .to_str().ok_or_else(|| library_db::invalid("Ungültiger Pfad"))?;
                        let file = LibraryFile::new(relative.into()).map_err(library_db::invalid)?;
                        library_db::media(tx, &file)?;
                        tx.prepare_cached("INSERT INTO current_audio VALUES(?1)")?
                            .execute([file.as_str()])?;
                    }
                    check(stop).map_err(library_db::invalid)?;
                    tx.execute(
                        "DELETE FROM audio_durations WHERE media_path NOT IN (SELECT path FROM current_audio)", []
                    )?;
                    Ok(())
                })?;
                *self
                    .pending
                    .lock()
                    .map_err(|_| "Audioinventar unterbrochen")? = Some((root.into(), files));
            }
            Stage::Durations => {
                let cached = self
                    .pending
                    .lock()
                    .map_err(|_| "Audioinventar unterbrochen")?
                    .as_ref()
                    .filter(|(library, _)| library == root)
                    .map(|(_, files)| files.clone());
                let files = match cached {
                    Some(files) => files,
                    None => self.scan(root, stop, progress)?,
                };
                let cache = CapturedInventory {
                    inner: self.durations.as_ref(),
                    files: &files,
                };
                let paths = files.keys().cloned().collect::<Vec<_>>();
                let total = paths.len();
                let mut checked = 0;
                let mut errors = Vec::new();
                for group in cache.groups(&paths) {
                    check(stop)?;
                    let snapshot = cache.load(&group, stop)?;
                    let result = photosuite_application::duration_cache::reconcile_loaded_reporting(
                        &cache,
                        &group,
                        snapshot,
                        stop,
                        |path| {
                            if !self.priority.wait_background(stop) {
                                return Err("Abgebrochen".into());
                            }
                            checked += 1;
                            progress(Progress {
                                checked,
                                total,
                                current: Some(path.into()),
                            });
                            let probe = self.audio.probe(path)?;
                            if !probe.audio_only {
                                return Err("Keine reine Audiodatei".into());
                            }
                            Ok(probe.duration)
                        },
                    );
                    errors.extend(result?);
                }
                check(stop)?;
                progress(Progress {
                    checked: total,
                    total,
                    current: None,
                });
                // Individual measurement failures do not prevent GPS/transcription. Report them.
                *self
                    .issues
                    .lock()
                    .map_err(|_| "Audiowarnungen unterbrochen")? =
                    Some((root.into(), errors.into_iter().take(16).collect()));
            }
            Stage::Metadata => {
                if let Some(tags) = &self.tags {
                    photosuite_application::organization::restore_projection(
                        tags.as_ref(),
                        root,
                        stop,
                    )?;
                }
                check(stop)?;
                if let Some(comments) = &self.comments
                    && matches!(
                        comments.read_cache(root)?,
                        photosuite_application::comments::CommentCache::Missing
                    )
                {
                    photosuite_application::comments::rebuild_catalog(
                        comments.as_ref(),
                        root,
                        stop,
                        &|_| {},
                    )?;
                }
                crate::date_correction_fs::ensure_projection(
                    root,
                    &self.database,
                    &self.priority,
                    stop,
                )?;
            }
            Stage::Years => {
                let years = photosuite_application::audio_library::year_totals(&self.read(root)?);
                check(stop)?;
                self.database.transaction(root,|tx| {
                    tx.execute_batch("CREATE TABLE IF NOT EXISTS audio_year_blocks(year TEXT PRIMARY KEY,seconds REAL NOT NULL CHECK(seconds>0)) STRICT; DELETE FROM audio_year_blocks;")?;
                    for (year,seconds) in years { check(stop).map_err(library_db::invalid)?; tx.prepare_cached("INSERT INTO audio_year_blocks VALUES(?1,?2)")?.execute((year,seconds))?; }
                    Ok(())
                })?;
                *self
                    .pending
                    .lock()
                    .map_err(|_| "Audioinventar unterbrochen")? = None;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photosuite_application::duration_cache::{Length, Snapshot as Cache};
    struct Source(std::sync::atomic::AtomicUsize);
    impl AudioSource for Source {
        fn probe(&self, _: &Path) -> Result<photosuite_application::diary::AudioProbe, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(photosuite_application::diary::AudioProbe {
                duration: 12.,
                audio_only: true,
                file_created: None,
            })
        }
        fn capture_time(&self, _: &Path) -> Result<crate::domain::CaptureTime, String> {
            panic!("display must not read dates")
        }
    }
    #[test]
    fn database_alone_restores_audio_reference_times_lengths_and_year_blocks_then_rebuilds_from_sources()
     {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let database = Arc::new(Database);
        let durations = Arc::new(crate::duration_cache_fs::SqliteDurations {
            database: database.clone(),
        });
        let source = Arc::new(Source(std::sync::atomic::AtomicUsize::new(0)));
        let adapter = AudioLibrary {
            tags: Some(Arc::new(crate::tag_store::SqliteTags {
                database: database.clone(),
                priority: Arc::default(),
            })),
            comments: Some(Arc::new(crate::comments_fs::SqliteComments {
                database: database.clone(),
                priority: Arc::default(),
            })),
            database: database.clone(),
            durations: durations.clone(),
            audio: source.clone(),
            priority: Arc::default(),
            pending: Mutex::new(None),
            issues: Mutex::new(None),
        };
        let path = root.join("2026/audio/comments/com_20261121_120000.m4a");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"unchanged original").unwrap();
        let group = root.join("2026");
        let stop = AtomicBool::new(false);
        let files = durations.inventory(&group, &stop).unwrap();
        durations
            .save(
                &group,
                &Cache {
                    today: crate::file_dates_fs::today().unwrap(),
                    last_full_verified: Some(crate::file_dates_fs::today().unwrap()),
                    entries: [(
                        path.clone(),
                        Length {
                            seconds: 12.,
                            fingerprint: files[&path].clone(),
                        },
                    )]
                    .into(),
                    files,
                },
            )
            .unwrap();
        database
            .transaction(root, |tx| {
                let audio =
                    LibraryFile::new("2026/audio/comments/com_20261121_120000.m4a".into()).unwrap();
                let photo = LibraryFile::new("1980/scans/img_19800203_040506.jpg".into()).unwrap();
                library_db::media(tx, &photo)?;
                tx.execute(
                    "INSERT INTO audio_comments VALUES(?1,?2,?3)",
                    (
                        "893d3899-9d2d-4241-8a3c-9f4226353b67",
                        photo.as_str(),
                        audio.as_str(),
                    ),
                )?;
                Ok(())
            })
            .unwrap();
        let photo = LibraryFile::new("1980/scans/img_19800203_040506.jpg".into()).unwrap();
        std::fs::create_dir_all(root.join("1980/scans")).unwrap();
        std::fs::write(root.join(photo.as_str()), b"original photo").unwrap();
        let mut record = crate::media_records_fs::Record::new(&photo);
        record.tags = Some(["keep".to_owned()].into());
        record.comments.insert(
            "893d3899-9d2d-4241-8a3c-9f4226353b67".into(),
            "2026/audio/comments/com_20261121_120000.m4a".into(),
        );
        crate::media_records_fs::write(root, &photo, &record, None).unwrap();
        let sidecar = crate::media_records_fs::path(root, &photo, false).unwrap();
        let source_yaml = std::fs::read(&sidecar).unwrap();
        adapter.run(root, Stage::Years, &stop, &|_| {}).unwrap();
        let original = std::fs::read(&path).unwrap();
        let hidden = root.join("offline");
        std::fs::rename(root.join("2026"), &hidden).unwrap();
        let cached = adapter.read(root).unwrap();
        assert_eq!(cached.durations, vec![(path.clone(), 12.)]);
        assert_eq!(cached.years, vec![("1980".into(), 12.)]);
        assert_eq!(cached.media[0].reference_time().unwrap().year(), "1980");
        assert_eq!(source.0.load(Ordering::SeqCst), 0);
        std::fs::rename(hidden, root.join("2026")).unwrap();
        for stage in [
            Stage::Inventory,
            Stage::Durations,
            Stage::Metadata,
            Stage::Years,
        ] {
            adapter.run(root, stage, &stop, &|_| {}).unwrap();
        }
        assert_eq!(
            source.0.load(Ordering::SeqCst),
            0,
            "today's unchanged duration must be reused"
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        adapter
            .rerun(root, Stage::Durations, &stop, &|_| {})
            .unwrap();
        assert_eq!(
            source.0.load(Ordering::SeqCst),
            1,
            "explicit duration repeat must remeasure even a current cache"
        );
        std::fs::remove_file(root.join(crate::library_db::FILE)).unwrap();
        // Derived length/year caches recover from audio files; authoritative links are rebuilt separately from media YAMLs.
        for stage in [
            Stage::Inventory,
            Stage::Durations,
            Stage::Metadata,
            Stage::Years,
        ] {
            adapter.run(root, stage, &stop, &|_| {}).unwrap();
        }
        assert_eq!(source.0.load(Ordering::SeqCst), 2);
        assert_eq!(adapter.read(root).unwrap().durations.len(), 1);
        let recovered = adapter.read(root).unwrap();
        assert_eq!(recovered.years, vec![("1980".into(), 12.)]);
        assert_eq!(recovered.media[0].reference_time().unwrap().year(), "1980");
        use photosuite_application::organization::CatalogRepository;
        assert!(
            crate::tag_store::SqliteTags {
                database: database.clone(),
                priority: Arc::default()
            }
            .load(root)
            .unwrap()
            .catalog
            .tags()
            .contains_key("keep")
        );
        assert_eq!(std::fs::read(sidecar).unwrap(), source_yaml);
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}
