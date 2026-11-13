//! Verified audio publication and revision-guarded photo references; never edits a photo.
use crate::domain::{
    CaptureTime, Media,
    comments::{Comment, CommentCatalog, RecordedAt},
    tags::LibraryFile,
};
use photosuite_application::{
    comments::*, diary::AudioSource, import::ImportAdapter, media_dates::MediaDateSource,
};
use std::{
    fs,
    io::Read,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
const FILE: &str = "photosuite-comments.yaml";
const MAX: u64 = 32 * 1024 * 1024;
pub struct CommentFiles {
    pub audio: Arc<dyn AudioSource>,
    pub copier: Arc<dyn ImportAdapter>,
    pub dates: Arc<dyn MediaDateSource>,
}
pub struct YamlComments {
    pub dates: Arc<dyn MediaDateSource>,
    pub copier: Arc<dyn ImportAdapter>,
    pub priority: Arc<photosuite_application::work_priority::WorkPriority>,
}
mod sidecars;
fn directory(path: &Path) -> Result<(), String> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("Bibliothek/Ordner ist kein reguläres Verzeichnis".into());
    }
    Ok(())
}
fn read(root: &Path) -> Result<Option<Vec<u8>>, String> {
    directory(root)?;
    let path = root.join(FILE);
    if !crate::metadata_fs::check_file(&path)? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX {
        return Err("Kommentarverwaltung überschreitet 32 MiB".into());
    }
    Ok(Some(bytes))
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    comments: std::collections::BTreeMap<String, Entry>,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    audio: String,
    photo: String,
    recorded_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    photo_taken_at: Option<String>,
}
fn decode(bytes: &[u8]) -> Result<CommentCatalog, String> {
    let doc: Document =
        serde_saphyr::from_str(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if ![1, 2].contains(&doc.version) {
        return Err("Unbekannte Kommentarversion; Datei bleibt unverändert".into());
    }
    let mut catalog = CommentCatalog::default();
    for (id, mut e) in doc.comments {
        if e.photo_taken_at.is_none() {
            e.photo_taken_at = Media::from_path(e.photo.clone().into(), String::new())
                .as_ref()
                .and_then(crate::domain::capture_from_filename)
                .map(|date| date.exif_value());
        }
        catalog.insert(Comment::new(
            id,
            LibraryFile::new(e.audio)?,
            LibraryFile::new(e.photo)?,
            RecordedAt::parse(&e.recorded_at)?,
            e.photo_taken_at
                .map(|d| CaptureTime::parse(&d, "Fotozuordnung"))
                .transpose()?,
        )?)?;
    }
    Ok(catalog)
}
fn encode(catalog: &CommentCatalog) -> Result<Vec<u8>, String> {
    let doc = Document {
        version: 2,
        comments: catalog
            .entries()
            .iter()
            .map(|(id, c)| {
                (
                    id.clone(),
                    Entry {
                        audio: c.audio().as_str().into(),
                        photo: c.photo().as_str().into(),
                        recorded_at: c.recorded_at().iso().into(),
                        photo_taken_at: if Media::from_path(
                            c.photo().as_str().into(),
                            String::new(),
                        )
                        .as_ref()
                        .and_then(crate::domain::capture_from_filename)
                        .is_some()
                        {
                            None
                        } else {
                            c.photo_time().map(CaptureTime::exif_value)
                        },
                    },
                )
            })
            .collect(),
    };
    let bytes = format!(
        "# PhotoSuite – Audiokommentare mit relativen Fotoverweisen\n{}",
        serde_saphyr::to_string(&doc).map_err(|e| e.to_string())?
    )
    .into_bytes();
    if bytes.len() as u64 > MAX {
        return Err("Kommentarverwaltung überschreitet 32 MiB".into());
    }
    Ok(bytes)
}
impl CommentRepository for YamlComments {
    fn load(&self, root: &Path) -> Result<StoredComments, String> {
        let revision = read(root)?;
        let catalog = revision
            .as_deref()
            .map(decode)
            .transpose()?
            .unwrap_or_default();
        let catalog = if revision.is_none() {
            let mut catalog = catalog;
            for comment in self.scan_sidecars(root, &AtomicBool::new(false), &|_| {})? {
                catalog.insert(comment)?;
            }
            catalog
        } else {
            catalog
        };
        Ok(StoredComments { catalog, revision })
    }
    fn write_sidecar(&self, root: &Path, comment: &Comment) -> Result<(), String> {
        self.publish_sidecar(root, comment)
    }
    fn commit(
        &self,
        root: &Path,
        comment: &Comment,
        catalog: &CommentCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        self.publish_sidecar(root, comment)?;
        self.save(root, catalog, expected)
    }
    fn recovery_base(&self, root: &Path) -> Result<RecoveryBase, String> {
        self.read_recovery_base(root)
    }
    fn scan_sidecars(
        &self,
        root: &Path,
        cancel: &AtomicBool,
        progress: &dyn Fn(RebuildProgress),
    ) -> Result<Vec<Comment>, String> {
        self.scan_comment_sidecars(root, cancel, progress)
    }
    fn replace_index(
        &self,
        root: &Path,
        catalog: &CommentCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        directory(root)?;
        let bytes = encode(catalog)?;
        crate::metadata_fs::atomic_write_if_current(&root.join(FILE), &bytes, expected)?;
        Ok(bytes)
    }
    fn save(
        &self,
        root: &Path,
        catalog: &CommentCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        let current = read(root)?;
        if current.as_deref() != expected {
            return Err(
                "photosuite-comments.yaml wurde extern geändert; bitte Speichern erneut versuchen"
                    .into(),
            );
        }
        if let Some(bytes) = current {
            decode(&bytes)?;
        }
        let bytes = encode(catalog)?;
        crate::metadata_fs::atomic_write_if_current(&root.join(FILE), &bytes, expected)?;
        Ok(bytes)
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
    fn photo_time(&self, target: &PhotoTarget) -> Result<CaptureTime, String> {
        let photo = Media::from_path(target.root.join(target.file.as_str()), String::new())
            .ok_or("Kommentarziel muss ein Foto sein")?;
        self.dates.captured_at(&photo)
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
            Err("unused".into())
        }
    }
    struct NoDate;
    impl MediaDateSource for NoDate {
        fn captured_at(&self, _: &Media) -> Result<CaptureTime, String> {
            Err("no metadata".into())
        }
    }
    fn adapter() -> CommentFiles {
        let audio = Arc::new(ValidAudio);
        let repository = Arc::new(crate::infrastructure::FileSystemRepository {
            priority: Default::default(),
            audio: audio.clone(),
        });
        let copier = Arc::new(crate::import_fs::FileImportAdapter {
            audio: audio.clone(),
            repository,
            tool: crate::exiftool::ExifTool::for_executable(Path::new("/tmp/test")),
        });
        CommentFiles {
            audio,
            copier,
            dates: Arc::new(NoDate),
        }
    }
    fn store() -> YamlComments {
        YamlComments {
            dates: Arc::new(NoDate),
            copier: adapter().copier,
            priority: Default::default(),
        }
    }
    fn target(root: &Path) -> PhotoTarget {
        fs::create_dir_all(root.join("2008/09")).unwrap();
        fs::write(root.join("2008/09/photo.jpg"), b"unchanged photo").unwrap();
        PhotoTarget {
            root: root.into(),
            file: LibraryFile::new("2008/09/photo.jpg".into()).unwrap(),
            time: Some(CaptureTime::parse("2008:09:16 03:58:11", "EXIF").unwrap()),
        }
    }
    #[test]
    fn publishing_preserves_take_and_photo_uses_suffix_and_roundtrips_references() {
        let library = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("recording.m4a");
        fs::write(&path, b"verified test payload").unwrap();
        let take = CapturedTake {
            id: "12345678-1234-1234-1234-123456789abc".into(),
            path: path.clone(),
            recorded_at: "2026-11-12T15:30:00+01:00".into(),
        };
        let target = target(library.path());
        let adapter = adapter();
        let first = publish_comment(&adapter, &target, &take).unwrap();
        let mut second = take.clone();
        second.id = "22345678-1234-1234-1234-123456789abc".into();
        let second = publish_comment(&adapter, &target, &second).unwrap();
        assert!(second.audio().as_str().ends_with("_001.m4a"));
        assert_eq!(fs::read(&path).unwrap(), b"verified test payload");
        assert_eq!(
            fs::read(library.path().join(first.audio().as_str())).unwrap(),
            b"verified test payload"
        );
        assert_eq!(
            fs::read(library.path().join(target.file.as_str())).unwrap(),
            b"unchanged photo"
        );
        let mut catalog = CommentCatalog::default();
        catalog.insert(first.clone()).unwrap();
        catalog.insert(second).unwrap();
        let bytes = store().save(library.path(), &catalog, None).unwrap();
        let loaded = store().load(library.path()).unwrap();
        assert_eq!(loaded.catalog.entries(), catalog.entries());
        assert_eq!(loaded.revision, Some(bytes.clone()));
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(!text.contains(library.path().to_str().unwrap()));
        assert!(text.contains("photo_taken_at:"));
        // Moving a library retains all relative associations, missing media remain visible references.
        fs::rename(
            library.path().join("2008/09/photo.jpg"),
            library.path().join("2008/09/renamed.jpg"),
        )
        .unwrap();
        assert_eq!(
            store()
                .load(library.path())
                .unwrap()
                .catalog
                .entries()
                .len(),
            2
        );
        fs::write(library.path().join(FILE), b"version: 9\ncomments: {}\n").unwrap();
        assert!(
            store()
                .save(library.path(), &catalog, Some(&bytes))
                .is_err()
        );
        assert!(store().load(library.path()).is_err());
        assert!(library.path().join(first.audio().as_str()).is_file());
    }
    #[test]
    fn unknown_malformed_and_unsafe_metadata_remain_untouched() {
        let directory = tempfile::tempdir().unwrap();
        for bytes in [
            "version: 1\ncomments: [",
            "version: 1\ncomments: {}\ncustom: true\n",
            "version: 8\ncomments: {}\n",
        ] {
            fs::write(directory.path().join(FILE), bytes).unwrap();
            assert!(store().load(directory.path()).is_err());
            assert!(
                store()
                    .save(directory.path(), &CommentCatalog::default(), None)
                    .is_err()
            );
            assert_eq!(
                fs::read_to_string(directory.path().join(FILE)).unwrap(),
                bytes
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn symbolic_photo_folders_and_metadata_are_rejected() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        fs::write(external.path().join("file"), b"outside").unwrap();
        symlink(external.path().join("file"), root.path().join(FILE)).unwrap();
        assert!(store().load(root.path()).is_err());
        assert_eq!(fs::read(external.path().join("file")).unwrap(), b"outside");
        fs::remove_file(root.path().join(FILE)).unwrap();
        symlink(external.path(), root.path().join("2008")).unwrap();
        let target = PhotoTarget {
            root: root.path().into(),
            file: LibraryFile::new("2008/file.jpg".into()).unwrap(),
            time: None,
        };
        let take = CapturedTake {
            id: "12345678-1234-1234-1234-123456789abc".into(),
            path: external.path().join("file"),
            recorded_at: "2026-11-12T15:30:00+01:00".into(),
        };
        assert!(publish_comment(&adapter(), &target, &take).is_err());
        assert!(!root.path().join("2026").exists());
    }
    struct Recorder;
    impl CommentRecorder for Recorder {
        fn devices(&self) -> Result<Vec<InputDevice>, String> {
            Ok(Vec::new())
        }
        fn start(&self, _: &str) -> Result<(), String> {
            Err("no microphone in tests".into())
        }
        fn command(&self, _: CaptureCommand) -> Result<(), String> {
            Ok(())
        }
        fn snapshot(&self) -> CaptureSnapshot {
            Default::default()
        }
    }
    fn drain(session: &mut CommentSession) {
        for _ in 0..3000 {
            session.poll();
            if !session.busy() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("comment job did not finish");
    }
    #[test]
    fn individual_yamls_recover_deleted_or_corrupt_index_and_keep_media_unchanged() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let mut target = target(root.path());
        fs::rename(
            root.path().join(target.file.as_str()),
            root.path().join("2008/09/img_20080916_035811.jpg"),
        )
        .unwrap();
        target.file = LibraryFile::new("2008/09/img_20080916_035811.jpg".into()).unwrap();
        let take = CapturedTake {
            id: "12345678-1234-1234-1234-123456789abc".into(),
            path: source.path().join("take.m4a"),
            recorded_at: "2026-11-13T13:00:00+01:00".into(),
        };
        fs::write(&take.path, b"test-audio-unchanged").unwrap();
        let media = adapter();
        let comment = publish_comment(&media, &target, &take).unwrap();
        assert_eq!(
            comment.audio().as_str(),
            "2026/audio/c/com_20261113_130000.m4a"
        );
        assert_eq!(
            comment.sidecar().as_str(),
            "2026/audio/y/com_20261113_130000.yaml"
        );
        let repository = Arc::new(store());
        let mut catalog = CommentCatalog::default();
        catalog.insert(comment.clone()).unwrap();
        repository
            .commit(root.path(), &comment, &catalog, None)
            .unwrap();
        let yaml = fs::read_to_string(root.path().join(comment.sidecar().as_str())).unwrap();
        assert!(yaml.contains("recorded_at:"));
        assert!(yaml.contains(&take.id));
        assert!(!yaml.contains("photo_taken_at"));
        assert!(!yaml.contains(root.path().to_str().unwrap()));
        fs::remove_file(root.path().join(FILE)).unwrap();
        let recovered = repository.load(root.path()).unwrap();
        assert_eq!(
            recovered.catalog.entries()[comment.id()].photo(),
            comment.photo()
        );
        assert_eq!(
            recovered.catalog.entries()[comment.id()]
                .photo_time()
                .unwrap()
                .stamp(),
            "20080916_035811"
        );
        fs::write(root.path().join(FILE), b"broken: [").unwrap();
        let mut session =
            CommentSession::new(repository.clone(), Arc::new(Recorder), Arc::new(media));
        session.load(root.path().into());
        drain(&mut session);
        assert!(!session.is_loaded());
        // Same deferred recovery requested by File → Refresh, even with a damaged cache.
        session.refresh_index();
        drain(&mut session);
        assert!(session.is_loaded());
        assert_eq!(session.catalog().entries().len(), 1);
        assert!(session.message().contains("neu aufgebaut"));
        assert!(fs::read_dir(root.path()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("photosuite-comments-recovery-")
        }));
        assert_eq!(
            fs::read(root.path().join(comment.audio().as_str())).unwrap(),
            b"test-audio-unchanged"
        );
        assert_eq!(fs::read(&take.path).unwrap(), b"test-audio-unchanged");
        assert_eq!(
            fs::read(root.path().join(target.file.as_str())).unwrap(),
            b"unchanged photo"
        );
        // Existing individual metadata overrides an outdated cache; it is never rewritten.
        let sidecar_path = root.path().join(comment.sidecar().as_str());
        let mut value: serde_json::Value =
            serde_saphyr::from_str(&fs::read_to_string(&sidecar_path).unwrap()).unwrap();
        value["photo"] = serde_json::json!("2010/01/img_20100102_030405.jpg");
        let edited = serde_saphyr::to_string(&value).unwrap();
        fs::write(&sidecar_path, &edited).unwrap();
        fs::write(
            root.path().join("2026/audio/y/._com_20261113_130000.yaml"),
            b"AppleDouble binary",
        )
        .unwrap();
        session.rebuild().unwrap();
        drain(&mut session);
        assert!(session.error().is_empty(), "{}", session.error());
        assert_eq!(
            session.catalog().entries()[comment.id()].photo().as_str(),
            "2010/01/img_20100102_030405.jpg"
        );
        assert_eq!(fs::read_to_string(&sidecar_path).unwrap(), edited);
        let previous = fs::read(root.path().join(FILE)).unwrap();
        fs::write(
            root.path().join(comment.sidecar().as_str()),
            b"version: 1\nunknown: true",
        )
        .unwrap();
        session.rebuild().unwrap();
        drain(&mut session);
        assert!(!session.error().is_empty());
        assert_eq!(fs::read(root.path().join(FILE)).unwrap(), previous);
    }
    #[test]
    fn existing_index_only_fills_missing_sidecars_during_explicit_rebuild() {
        let root = tempfile::tempdir().unwrap();
        let c = Comment::new(
            "12345678-1234-1234-1234-123456789abc".into(),
            LibraryFile::new("2023/audio/comment_20230101_120000.m4a".into()).unwrap(),
            LibraryFile::new("2008/09/img_20080916_035811.jpg".into()).unwrap(),
            RecordedAt::parse("2023-01-01T12:00:00+01:00").unwrap(),
            None,
        )
        .unwrap();
        let mut catalog = CommentCatalog::default();
        catalog.insert(c.clone()).unwrap();
        let bytes = encode(&catalog).unwrap();
        fs::write(
            root.path().join(FILE),
            String::from_utf8(bytes)
                .unwrap()
                .replace("version: 2", "version: 1"),
        )
        .unwrap();
        let repository = Arc::new(store());
        let mut session =
            CommentSession::new(repository.clone(), Arc::new(Recorder), Arc::new(adapter()));
        session.load(root.path().into());
        drain(&mut session);
        session.rebuild().unwrap();
        drain(&mut session);
        assert!(session.error().is_empty(), "{}", session.error());
        assert!(root.path().join(c.sidecar().as_str()).is_file());
        assert_eq!(session.catalog().entries().len(), 1);
        let previous = fs::read(root.path().join(FILE)).unwrap();
        session.rebuild().unwrap();
        session.cancel_rebuild();
        drain(&mut session);
        assert_eq!(fs::read(root.path().join(FILE)).unwrap(), previous);
        // Missing individual records may be recovered from a readable index (also version 2).
        fs::remove_file(root.path().join(c.sidecar().as_str())).unwrap();
        session.rebuild().unwrap();
        drain(&mut session);
        assert!(session.error().is_empty());
        assert_eq!(session.catalog().entries().len(), 1);
        assert!(root.path().join(c.sidecar().as_str()).is_file());
        fs::write(root.path().join(FILE), b"external edit").unwrap();
        assert!(
            repository
                .replace_index(root.path(), &catalog, Some(&previous))
                .is_err()
        );
        assert_eq!(fs::read(root.path().join(FILE)).unwrap(), b"external edit");
    }
    #[cfg(unix)]
    #[test]
    fn sidecar_collisions_symlinked_metadata_folders_and_external_updates_are_preserved() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let c = Comment::new(
            "12345678-1234-1234-1234-123456789abc".into(),
            LibraryFile::new("2026/audio/c/com_20261113_130000.m4a".into()).unwrap(),
            LibraryFile::new("2008/09/a.jpg".into()).unwrap(),
            RecordedAt::parse("2026-11-13T13:00:00+01:00").unwrap(),
            None,
        )
        .unwrap();
        fs::create_dir_all(root.path().join("2026/audio")).unwrap();
        symlink(external.path(), root.path().join("2026/audio/y")).unwrap();
        assert!(store().write_sidecar(root.path(), &c).is_err());
        assert!(
            store()
                .scan_sidecars(root.path(), &AtomicBool::new(false), &|_| {})
                .is_err()
        );
        assert_eq!(fs::read_dir(external.path()).unwrap().count(), 0);
        fs::remove_file(root.path().join("2026/audio/y")).unwrap();
        store().write_sidecar(root.path(), &c).unwrap();
        let path = root.path().join(c.sidecar().as_str());
        let bytes = fs::read(&path).unwrap();
        store().write_sidecar(root.path(), &c).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::write(&path, b"foreign data").unwrap();
        assert!(store().write_sidecar(root.path(), &c).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"foreign data");
    }
}
