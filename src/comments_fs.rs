//! Authoritative photo YAML references and their derived SQLite index.
use crate::domain::{
    Media,
    comments::{Comment, CommentCatalog},
    tags::LibraryFile,
};
use crate::library_db::{self, Database, Section};
use photosuite_application::{comments::*, diary::AudioSource, import::ImportAdapter};
use std::{
    fs,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
pub struct CommentFiles {
    pub audio: Arc<dyn AudioSource>,
    pub copier: Arc<dyn ImportAdapter>,
}
pub struct SqliteComments {
    pub priority: Arc<photosuite_application::work_priority::WorkPriority>,
    pub database: Arc<Database>,
}
mod photo_records;
fn directory(path: &Path) -> Result<(), String> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("Bibliothek/Ordner ist kein reguläres Verzeichnis".into());
    }
    Ok(())
}
fn add_photo(
    catalog: &mut CommentCatalog,
    photo: &str,
    entries: std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    let photo = LibraryFile::new(photo.into())?;
    Comment::metadata_path(&photo)?;
    for (id, audio) in entries {
        catalog.insert(Comment::new(id, LibraryFile::new(audio)?, photo.clone())?)?;
    }
    Ok(())
}
impl CommentRepository for SqliteComments {
    fn read_cache(&self, root: &Path) -> Result<CommentCache, String> {
        self.database.transaction(root, |tx| {
            let revision = library_db::revision(tx, Section::Comments)?;
            if revision.is_none() {
                return Ok(CommentCache::Missing);
            }
            let mut catalog = CommentCatalog::default();
            let mut stmt = tx.prepare(
                "SELECT id,photo_path,audio_path FROM audio_comments ORDER BY photo_path,id",
            )?;
            for row in stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })? {
                let (id, photo, audio) = row?;
                catalog
                    .insert(
                        Comment::new(
                            id,
                            LibraryFile::new(audio).map_err(library_db::invalid)?,
                            LibraryFile::new(photo).map_err(library_db::invalid)?,
                        )
                        .map_err(library_db::invalid)?,
                    )
                    .map_err(library_db::invalid)?;
            }
            Ok(CommentCache::Current(StoredComments { catalog, revision }))
        })
    }
    fn write_photo_record(&self, root: &Path, comment: &Comment) -> Result<CommentCatalog, String> {
        self.update_photo_record(root, comment)
    }
    fn rebuild_base(&self, root: &Path) -> Result<RebuildBase, String> {
        self.database.transaction(root, |tx| {
            Ok(RebuildBase {
                revision: library_db::revision(tx, Section::Comments)?,
                warning: String::new(),
            })
        })
    }
    fn scan_photo_records(
        &self,
        root: &Path,
        cancel: &AtomicBool,
        progress: &dyn Fn(RebuildProgress),
    ) -> Result<Vec<Comment>, String> {
        self.read_photo_records(root, cancel, progress)
    }
    fn replace_index(
        &self,
        root: &Path,
        catalog: &CommentCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        self.save(root, catalog, expected)
    }
    fn save(
        &self,
        root: &Path,
        catalog: &CommentCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        self.database.transaction(root, |tx| {
            let revision = library_db::publish(tx, Section::Comments, expected)?;
            tx.execute("DELETE FROM audio_comments", [])?;
            tx.execute("UPDATE media SET reference_at=recorded_at WHERE kind='audio'",[])?;
            let mut stmt = tx
                .prepare("INSERT INTO audio_comments(id,photo_path,audio_path) VALUES(?1,?2,?3)")?;
            for comment in catalog.entries().values() {
                library_db::media(tx, comment.photo())?;
                library_db::media(tx, comment.audio())?;
                    tx.execute("UPDATE media SET reference_at=(SELECT reference_at FROM media WHERE path=?1) WHERE path=?2",(comment.photo().as_str(),comment.audio().as_str()))?;
                stmt.execute((
                    comment.id(),
                    comment.photo().as_str(),
                    comment.audio().as_str(),
                ))?;
            }
            Ok(revision)
        })
    }
}
impl CommentMediaStore for CommentFiles {
    fn validate(&self, target: &PhotoTarget, take: &CapturedTake) -> Result<(), String> {
        directory(&target.root)?;
        let photo = target.root.join(target.file.as_str());
        let mut parent = photo.parent();
        while let Some(p) = parent {
            directory(p)?;
            if p == target.root {
                break;
            }
            parent = p.parent();
        }
        if !crate::metadata_fs::check_file(&photo)? {
            return Err("Zugeordnetes Foto fehlt; keine Aufnahme veröffentlicht".into());
        }
        if !crate::metadata_fs::check_file(&take.path)? {
            return Err("Aufnahmedatei fehlt".into());
        }
        let probe = self.audio.probe(&take.path)?;
        if !probe.audio_only || !probe.duration.is_finite() || probe.duration <= 0. {
            return Err("Keine gültige Audioaufnahme".into());
        }
        Ok(())
    }

    fn exists(&self, path: &Path) -> Result<bool, String> {
        self.copier.exists(path)
    }
    fn copy_verified(&self, source: &Path, target: &Path) -> Result<(), String> {
        self.copier
            .copy_verified(source, target, &AtomicBool::new(false), &|_| {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CaptureTime;
    struct ValidAudio;
    impl AudioSource for ValidAudio {
        fn probe(&self, _: &Path) -> Result<photosuite_application::diary::AudioProbe, String> {
            Ok(photosuite_application::diary::AudioProbe {
                duration: 1.,
                audio_only: true,
                file_created: None,
            })
        }
        fn capture_time(&self, _: &Path) -> Result<CaptureTime, String> {
            panic!("comment publication must not read photo dates")
        }
    }
    fn adapter() -> CommentFiles {
        let audio = Arc::new(ValidAudio);
        let repository = Arc::new(crate::infrastructure::FileSystemRepository {
            priority: Default::default(),
            audio: audio.clone(),
        });
        CommentFiles {
            audio: audio.clone(),
            copier: Arc::new(crate::import_fs::FileImportAdapter {
                audio,
                repository,
                tool: crate::exiftool::ExifTool::for_executable(Path::new("/tmp/test")),
            }),
        }
    }
    fn store() -> SqliteComments {
        SqliteComments {
            database: Arc::default(),
            priority: Default::default(),
        }
    }
    fn photo(root: &Path, relative: &str) -> PhotoTarget {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"original-photo").unwrap();
        PhotoTarget {
            root: root.into(),
            file: LibraryFile::new(relative.into()).unwrap(),
        }
    }
    fn take(root: &Path, n: u32, year: u32) -> CapturedTake {
        let path = root.join(format!("take-{n}.m4a"));
        fs::write(&path, format!("audio-{n}")).unwrap();
        CapturedTake {
            id: format!("12345678-1234-1234-1234-{n:012}"),
            path,
            recorded_at: format!("{year}-11-13T12:00:00+01:00"),
        }
    }
    struct NoCapture;
    impl CommentRecorder for NoCapture {
        fn devices(&self) -> Result<Vec<InputDevice>, String> {
            Ok(Vec::new())
        }
        fn start(&self, _: &str) -> Result<(), String> {
            panic!("rebuild must never start a microphone")
        }
        fn command(&self, _: CaptureCommand) -> Result<(), String> {
            Ok(())
        }
        fn snapshot(&self) -> CaptureSnapshot {
            Default::default()
        }
    }
    fn drain(session: &mut CommentSession) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while session.busy() {
            session.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_source_ancestors_abort_rebuild_without_erasing_cached_references() {
        for ancestor in ["yaml/2008", "yaml/2008/09"] {
            let root = tempfile::tempdir().unwrap();
            let source = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let target = photo(root.path(), "2008/09/photo.jpg");
            let files = Arc::new(adapter());
            let repo = Arc::new(store());
            let comment =
                publish_comment(files.as_ref(), &target, &take(source.path(), 1, 2026)).unwrap();
            let stored = commit_comment(
                repo.as_ref(),
                root.path(),
                &comment,
                &CommentCatalog::default(),
                None,
            )
            .unwrap();
            let before = stored.revision.unwrap();
            // Cached references remain viewable; rebuilding still rejects symlinked sources.
            let moved = outside.path().join("held");
            fs::rename(root.path().join(ancestor), &moved).unwrap();
            std::os::unix::fs::symlink(&moved, root.path().join(ancestor)).unwrap();
            let mut session = CommentSession::new(repo, Arc::new(NoCapture), files);
            session.load(root.path().into());
            drain(&mut session);
            assert_eq!(session.catalog().entries().len(), 1);
            session.rebuild().unwrap();
            drain(&mut session);
            assert!(!session.error().is_empty());
            assert_eq!(
                store().rebuild_base(root.path()).unwrap().revision.unwrap(),
                before
            );
            assert_eq!(session.catalog().entries().len(), 1);
        }
    }
    #[test]
    fn directory_with_media_extension_is_not_accepted_as_a_regular_reference() {
        let root = tempfile::tempdir().unwrap();
        let target = photo(root.path(), "2008/09/photo.jpg");
        let comment = Comment::new(
            "12345678-1234-1234-1234-123456789abc".into(),
            LibraryFile::new("2026/audio/comments/com_20261113_120000.m4a".into()).unwrap(),
            target.file,
        )
        .unwrap();
        fs::create_dir_all(root.path().join(comment.audio().as_str())).unwrap();
        assert!(store().write_photo_record(root.path(), &comment).is_err());
        assert!(!root.path().join(comment.sidecar().as_str()).exists());
    }
    #[test]
    fn commit_merges_authoritative_photo_records_and_never_resurrects_removed_references() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let target = photo(root.path(), "2008/09/photo.jpg");
        let repo = store();
        let files = adapter();
        let first = publish_comment(&files, &target, &take(source.path(), 1, 2025)).unwrap();
        repo.write_photo_record(root.path(), &first).unwrap();
        let second = publish_comment(&files, &target, &take(source.path(), 2, 2026)).unwrap();
        let mut stale = CommentCatalog::default();
        stale.insert(second.clone()).unwrap();
        let stored = commit_comment(&repo, root.path(), &second, &stale, None).unwrap();
        assert_eq!(stored.catalog.entries().len(), 2);
        let sidecar = root.path().join(second.sidecar().as_str());
        fs::remove_file(&sidecar).unwrap();
        let base = repo.rebuild_base(root.path()).unwrap();

        let empty = repo
            .scan_photo_records(root.path(), &AtomicBool::new(false), &|_| {})
            .unwrap();
        assert!(empty.is_empty());
        repo.replace_index(
            root.path(),
            &CommentCatalog::default(),
            base.revision.as_deref(),
        )
        .unwrap();
        assert!(
            load_catalog(&repo, root.path(), &AtomicBool::new(false))
                .unwrap()
                .catalog
                .entries()
                .is_empty()
        );
        assert!(!sidecar.exists());
        assert!(root.path().join(first.audio().as_str()).is_file());
        assert!(root.path().join(second.audio().as_str()).is_file());
    }
    #[test]
    fn one_photo_record_collects_multiple_recording_years_and_rebuilds_index() {
        let library = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let target = photo(library.path(), "2008/09/photo.jpg");
        let files = adapter();
        let repo = store();
        let mut catalog = CommentCatalog::default();
        let mut revision = None;
        for n in 1..=2 {
            let take = take(source.path(), n, 2024 + n);
            let comment = publish_comment(&files, &target, &take).unwrap();
            catalog.insert(comment.clone()).unwrap();
            let stored = commit_comment(
                &repo,
                library.path(),
                &comment,
                &catalog,
                revision.as_deref(),
            )
            .unwrap();
            catalog = stored.catalog;
            revision = stored.revision;
            assert_eq!(
                fs::read(&take.path).unwrap(),
                format!("audio-{n}").as_bytes()
            );
        }
        let sidecar = library.path().join("yaml/2008/09/photo.jpg.yaml");
        let text = fs::read_to_string(&sidecar).unwrap();
        assert!(
            text.contains("2025/audio/comments/com_") && text.contains("2026/audio/comments/com_")
        );
        assert!(!text.contains("recorded_at") && !text.contains("photo_taken_at"));
        assert_eq!(fs::read_dir(sidecar.parent().unwrap()).unwrap().count(), 1);
        fs::remove_file(library.path().join(crate::library_db::FILE)).unwrap();
        let rebuilt = load_catalog(&repo, library.path(), &AtomicBool::new(false)).unwrap();
        assert_eq!(rebuilt.catalog.entries().len(), 2);
        for comment in rebuilt.catalog.entries().values() {
            let m = Media::from_audio_asset(
                library.path().join(comment.audio().as_str()),
                Path::new(comment.audio().as_str())
                    .parent()
                    .unwrap()
                    .to_string_lossy()
                    .into(),
            )
            .unwrap()
            .with_comment_recording(Some(comment));
            assert!(m.group().starts_with(comment.recorded_at().time().year()));
            assert!(!m.group().starts_with("2008"));
        }
        assert_eq!(
            fs::read(library.path().join(target.file.as_str())).unwrap(),
            b"original-photo"
        );
    }
    #[test]
    fn photo_names_and_months_have_distinct_records_and_existing_photo_file_is_extended() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let repo = store();
        let files = adapter();
        let mut paths = std::collections::BTreeSet::new();
        for (n, name) in [
            "2008/09/photo.jpg",
            "2008/10/photo.jpg",
            "2008/09/photo.png",
        ]
        .iter()
        .enumerate()
        {
            let target = photo(root.path(), name);
            let c =
                publish_comment(&files, &target, &take(source.path(), n as u32 + 1, 2026)).unwrap();
            repo.write_photo_record(root.path(), &c).unwrap();
            repo.write_photo_record(root.path(), &c).unwrap();
            paths.insert(c.sidecar());
        }
        assert_eq!(paths.len(), 3);
        assert_eq!(
            repo.scan_photo_records(root.path(), &AtomicBool::new(false), &|_| {})
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn invalid_photo_metadata_and_external_index_changes_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let target = photo(root.path(), "2008/09/photo.jpg");
        let repo = store();
        let c = publish_comment(&adapter(), &target, &take(source.path(), 1, 2026)).unwrap();
        repo.write_photo_record(root.path(), &c).unwrap();
        let path = root.path().join(c.sidecar().as_str());
        fs::write(&path, b"version: 99\nphoto: x\ncomments: {}\n").unwrap();
        assert!(repo.write_photo_record(root.path(), &c).is_err());
        assert!(
            repo.scan_photo_records(root.path(), &AtomicBool::new(false), &|_| {})
                .is_err()
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            b"version: 99\nphoto: x\ncomments: {}\n"
        );
        let revision = repo
            .save(root.path(), &CommentCatalog::default(), None)
            .unwrap();
        fs::write(root.path().join(crate::library_db::FILE), b"external").unwrap();
        assert!(
            repo.replace_index(root.path(), &CommentCatalog::default(), Some(&revision))
                .is_err()
        );
        assert_eq!(
            fs::read(root.path().join(crate::library_db::FILE)).unwrap(),
            b"external"
        );
    }
    #[test]
    fn cancellation_and_symbolic_photo_metadata_never_replace_the_index() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let target = photo(root.path(), "2008/09/photo.jpg");
        let repo = store();
        let c = publish_comment(&adapter(), &target, &take(source.path(), 1, 2026)).unwrap();
        repo.write_photo_record(root.path(), &c).unwrap();
        assert!(
            repo.scan_photo_records(root.path(), &AtomicBool::new(true), &|_| {})
                .is_err()
        );
        #[cfg(unix)]
        {
            let path = root.path().join(c.sidecar().as_str());
            fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(source.path().join("take-1.m4a"), &path).unwrap();
            assert!(repo.write_photo_record(root.path(), &c).is_err());
            assert!(
                repo.scan_photo_records(root.path(), &AtomicBool::new(false), &|_| {})
                    .is_err()
            );
        }
    }
}
