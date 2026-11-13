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
pub struct YamlComments;
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
    if doc.version != 1 {
        return Err("Unbekannte Kommentarversion; Datei bleibt unverändert".into());
    }
    let mut catalog = CommentCatalog::default();
    for (id, e) in doc.comments {
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
        version: 1,
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
                        photo_taken_at: c.photo_time().map(CaptureTime::exif_value),
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
        Ok(StoredComments { catalog, revision })
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
        let bytes = YamlComments.save(library.path(), &catalog, None).unwrap();
        let loaded = YamlComments.load(library.path()).unwrap();
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
            YamlComments
                .load(library.path())
                .unwrap()
                .catalog
                .entries()
                .len(),
            2
        );
        fs::write(library.path().join(FILE), b"version: 2\ncomments: {}\n").unwrap();
        assert!(
            YamlComments
                .save(library.path(), &catalog, Some(&bytes))
                .is_err()
        );
        assert!(YamlComments.load(library.path()).is_err());
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
            assert!(YamlComments.load(directory.path()).is_err());
            assert!(
                YamlComments
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
        assert!(YamlComments.load(root.path()).is_err());
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
}
