//! Shared technical codec for per-media tags and photo comments. No workflow decisions.
use crate::domain::tags::{LibraryFile, normalize_tag_name};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub version: u32,
    #[serde(rename = "file", alias = "photo")]
    pub photo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<BTreeSet<String>>,
    #[serde(default)]
    pub comments: BTreeMap<String, String>,
}
impl Record {
    pub fn new(file: &LibraryFile) -> Self {
        Self {
            version: 3,
            photo: file.as_str().into(),
            tags: None,
            comments: Default::default(),
        }
    }
}
pub(crate) fn parse(bytes: &[u8]) -> Result<Record, String> {
    let mut record: Record =
        serde_saphyr::from_str(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if ![2, 3].contains(&record.version) {
        return Err("Unbekannte Version der Medien-YAML".into());
    }
    let file = LibraryFile::new(record.photo.clone())?;
    file.metadata_path()?;
    if let Some(tags) = record.tags.take() {
        record.tags = Some(
            tags.iter()
                .map(|t| normalize_tag_name(t))
                .collect::<Result<_, _>>()?,
        );
    }
    let mut comments = crate::domain::comments::CommentCatalog::default();
    for (id, audio) in &record.comments {
        comments.insert(crate::domain::comments::Comment::new(
            id.clone(),
            LibraryFile::new(audio.clone())?,
            file.clone(),
        )?)?;
    }
    record.comments = comments
        .entries()
        .values()
        .map(|c| (c.id().to_owned(), c.audio().as_str().to_owned()))
        .collect();
    Ok(record)
}
pub(crate) fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    if !crate::metadata_fs::check_file(path)? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Medien-YAML überschreitet 1 MiB".into());
    }
    Ok(Some(bytes))
}
pub(crate) fn path(
    root: &Path,
    file: &LibraryFile,
    create: bool,
) -> Result<std::path::PathBuf, String> {
    crate::metadata_fs::library_path(root, &file.metadata_path()?, create)
}
pub(crate) fn write(
    root: &Path,
    file: &LibraryFile,
    record: &Record,
    expected: Option<&[u8]>,
) -> Result<(), String> {
    if record.photo != file.as_str() {
        return Err("Medien-YAML gehört zu einer anderen Datei".into());
    }
    let mut record = parse(
        &serde_saphyr::to_string(record)
            .map_err(|e| e.to_string())?
            .into_bytes(),
    )?;
    record.version = 3;
    let bytes = serde_saphyr::to_string(&record)
        .map_err(|e| e.to_string())?
        .into_bytes();
    if bytes.len() > 1024 * 1024 {
        return Err("Medien-YAML überschreitet 1 MiB".into());
    }
    crate::metadata_fs::atomic_write_if_current(&path(root, file, true)?, &bytes, expected)
}
pub(crate) fn visit(
    root: &Path,
    cancel: &AtomicBool,
    priority: &photosuite_application::work_priority::WorkPriority,
    mut visitor: impl FnMut(LibraryFile, Record, Vec<u8>) -> Result<(), String>,
) -> Result<(), String> {
    let marker = crate::metadata_fs::library_path(root, "yaml/.scan", false)?;
    let base = marker.parent().ok_or("YAML-Verzeichnis fehlt")?;
    if !base.try_exists().map_err(|e| e.to_string())? {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(base)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with("._"))
    {
        if cancel.load(Ordering::Relaxed) || !priority.wait_background(cancel) {
            return Err("Neuaufbau abgebrochen; Index unverändert".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.depth() == 1
            && entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.len() == 4 && n.bytes().all(|b| b.is_ascii_digit()))
            && !entry.file_type().is_dir()
        {
            return Err("Jahreszweig unter yaml ist kein reguläres Verzeichnis".into());
        }
        if entry.file_type().is_symlink() {
            return Err(format!(
                "Symbolischer Metadatenpfad: {}",
                entry.path().display()
            ));
        }
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "yaml") {
            continue;
        }
        let Some(name) = entry.path().file_stem().and_then(|n| n.to_str()) else {
            continue;
        };
        if crate::domain::Media::from_path(name.into(), String::new()).is_none() {
            continue;
        }
        let bytes = read(entry.path())?.ok_or("Medien-YAML während Neuaufbau entfernt")?;
        let record = parse(&bytes)?;
        let file = LibraryFile::new(record.photo.clone())?;
        if path(root, &file, false)? != entry.path() {
            return Err(format!(
                "Medien-YAML liegt am falschen Ort: {}",
                entry.path().display()
            ));
        }
        visitor(file, record, bytes)?;
    }
    Ok(())
}

/// Technical, resumable relocation of existing application metadata. References stay unchanged.
pub(crate) fn prepare_layout(
    root: &Path,
    cancel: &AtomicBool,
    priority: &photosuite_application::work_priority::WorkPriority,
) -> Result<(), String> {
    static MIGRATION: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let marker = crate::metadata_fs::library_path(root, "yaml/layout.yaml", false)?;
    let base = marker.parent().ok_or("YAML-Verzeichnis fehlt")?;
    if !base.try_exists().map_err(|e| e.to_string())? {
        return Ok(());
    }
    const LAYOUT: &[u8] = b"version: 1\nlayout: year-month\n";
    let inspect_marker = || -> Result<bool, String> {
        match read(&marker)? {
            None => Ok(false),
            Some(bytes) if bytes == LAYOUT => Ok(true),
            Some(_) => Err(
                "Unbekannte YAML-Verzeichnisstruktur in yaml/layout.yaml; Dateien bleiben erhalten"
                    .into(),
            ),
        }
    };
    if inspect_marker()? {
        return Ok(());
    }
    let _guard = loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("YAML-Umsortierung abgebrochen; Dateien bleiben erhalten".into());
        }
        match MIGRATION.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            Err(_) => return Err("YAML-Umsortierung wurde unterbrochen".into()),
        }
    };
    if inspect_marker()? {
        return Ok(());
    }
    let mut directories = std::collections::BTreeSet::new();
    let mut moved = false;
    for entry in walkdir::WalkDir::new(base)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with("._"))
    {
        if cancel.load(Ordering::Relaxed) || !priority.wait_background(cancel) {
            return Err(
                "YAML-Umsortierung abgebrochen; bereits umsortierte Einzeldateien bleiben erhalten"
                    .into(),
            );
        }
        let entry = entry.map_err(|e| e.to_string())?;
        let source = entry.path();
        if entry.depth() == 1
            && entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.len() == 4 && n.bytes().all(|b| b.is_ascii_digit()))
            && !entry.file_type().is_dir()
        {
            return Err("Jahreszweig unter yaml ist kein reguläres Verzeichnis".into());
        }
        if entry.file_type().is_symlink() {
            return Err(format!("Symbolischer Metadatenpfad: {}", source.display()));
        }
        if entry.file_type().is_dir() {
            if source != base {
                directories.insert(source.to_path_buf());
            }
            continue;
        }
        if !entry.file_type().is_file() || source.extension().is_none_or(|e| e != "yaml") {
            continue;
        }
        let Some(name) = source.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if crate::domain::Media::from_path(name.into(), String::new()).is_none() {
            continue;
        }
        let bytes = read(source)?.ok_or("Medien-YAML wurde während Umsortierung entfernt")?;
        let record = parse(&bytes)?;
        let file = LibraryFile::new(record.photo.clone())?;
        let destination = path(root, &file, false)?;
        if source == destination {
            continue;
        }
        let old = root.join(format!("yaml/{}.yaml", file.as_str()));
        if source != old {
            return Err(format!(
                "Unbekannter Ort einer Medien-YAML: {}",
                source.display()
            ));
        }
        let equivalent = |other: &Record| {
            other.photo == record.photo
                && other.tags == record.tags
                && other.comments == record.comments
        };
        match read(&destination)? {
            Some(existing) if equivalent(&parse(&existing)?) => {}
            Some(_) => {
                return Err(format!(
                    "YAML-Namenskollision: {} gehört zu anderen Daten; beide Dateien bleiben erhalten",
                    destination.display()
                ));
            }
            None => {
                let destination = path(root, &file, true)?;
                crate::metadata_fs::atomic_write_if_current(&destination, &bytes, None)?;
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(
                "YAML-Umsortierung abgebrochen; Quell- und Zieldatei bleiben erhalten".into(),
            );
        }
        let verified = read(&destination)?.ok_or("Neue Medien-YAML fehlt nach Umsortierung")?;
        if !equivalent(&parse(&verified)?) || read(source)?.as_deref() != Some(bytes.as_slice()) {
            return Err("YAML wurde gleichzeitig verändert; beide Dateien bleiben erhalten".into());
        }
        fs::remove_file(source).map_err(|e| e.to_string())?;
        moved = true;
    }
    // Remove only empty application metadata directories; never remove media or foreign files.
    let mut directories: Vec<_> = directories.into_iter().collect();
    directories.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for directory in directories {
        if cancel.load(Ordering::Relaxed) {
            return Err("YAML-Umsortierung abgebrochen".into());
        }
        match fs::remove_dir(&directory) {
            Ok(()) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
                ) => {}
            Err(e) if !moved && e.kind() == std::io::ErrorKind::PermissionDenied => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("YAML-Umsortierung abgebrochen; Einzeldateien bleiben erhalten".into());
    }
    match crate::metadata_fs::atomic_write_if_current(&marker, LAYOUT, None) {
        Ok(()) => Ok(()),
        Err(_) if !moved => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use photosuite_application::{comments::CommentRepository, organization::CatalogRepository};
    fn old_record(root: &Path, file: &LibraryFile, tags: &[&str]) -> Vec<u8> {
        let mut record = Record::new(file);
        record.tags = Some(tags.iter().map(|s| s.to_string()).collect());
        let bytes = serde_saphyr::to_string(&record).unwrap().into_bytes();
        let old = root.join(format!("yaml/{}.yaml", file.as_str()));
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(old, &bytes).unwrap();
        bytes
    }
    #[test]
    fn migration_flattens_audio_yaml_resumes_completed_copies_and_preserves_media_and_indexes() {
        let root = tempfile::tempdir().unwrap();
        let diary = LibraryFile::new("2026/audio/diary/aud_20261114_120000.wav".into()).unwrap();
        let audio = LibraryFile::new("2026/audio/comments/com_20261114_130000.m4a".into()).unwrap();
        let photo = LibraryFile::new("1980/09/img_19800916_035811.jpg".into()).unwrap();
        for file in [&diary, &audio, &photo] {
            let path = root.path().join(file.as_str());
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"original media bytes").unwrap();
        }
        let diary_bytes = old_record(root.path(), &diary, &["diary"]);
        let audio_bytes = old_record(root.path(), &audio, &["comment"]);
        let destination = path(root.path(), &diary, true).unwrap();
        fs::write(&destination, &diary_bytes).unwrap();
        let mut record = Record::new(&photo);
        record.tags = Some(["scan".into()].into());
        record.comments.insert(
            "12345678-1234-1234-1234-123456789abc".into(),
            audio.as_str().into(),
        );
        write(root.path(), &photo, &record, None).unwrap();
        let repo = crate::tag_store::YamlTags::default();
        let tags =
            photosuite_application::organization::index::load_with_backfill(&repo, root.path())
                .unwrap();
        assert!(tags.catalog.contains("comment", audio.as_str()));
        assert!(tags.catalog.contains("scan", photo.as_str()));
        assert_eq!(
            fs::read(path(root.path(), &audio, false).unwrap()).unwrap(),
            audio_bytes
        );
        assert_eq!(fs::read(destination).unwrap(), diary_bytes);
        assert!(!root.path().join("yaml/2026/audio").exists());
        for file in [&diary, &audio, &photo] {
            assert_eq!(
                fs::read(root.path().join(file.as_str())).unwrap(),
                b"original media bytes"
            );
        }
        let comments = crate::comments_fs::YamlComments {
            priority: Default::default(),
        };
        let catalog = photosuite_application::comments::load_catalog(
            &comments,
            root.path(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(catalog.catalog.entries().len(), 1);
        assert_eq!(
            comments
                .scan_photo_records(root.path(), &AtomicBool::new(false), &|_| {})
                .unwrap()
                .len(),
            1
        );
        let before = fs::read(path(root.path(), &audio, false).unwrap()).unwrap();
        repo.prepare_metadata_layout(root.path(), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(
            fs::read(path(root.path(), &audio, false).unwrap()).unwrap(),
            before
        );
    }
    #[cfg(unix)]
    #[test]
    fn already_flat_read_only_metadata_remains_readable_without_a_layout_marker() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let file = LibraryFile::new("2026/audio/diary/aud_20261114_120000.wav".into()).unwrap();
        let mut record = Record::new(&file);
        record.tags = Some(["diary".into()].into());
        write(root.path(), &file, &record, None).unwrap();
        let yaml = root.path().join("yaml");
        let old = fs::metadata(&yaml).unwrap().permissions();
        fs::set_permissions(&yaml, fs::Permissions::from_mode(0o555)).unwrap();
        let repo = crate::tag_store::YamlTags::default();
        let result =
            photosuite_application::organization::index::load_with_backfill(&repo, root.path());
        fs::set_permissions(&yaml, old).unwrap();
        let result = result.unwrap();
        assert!(result.catalog.contains("diary", file.as_str()));
        assert!(!root.path().join("yaml/layout.yaml").exists());
    }
    #[test]
    fn flattened_name_collision_preserves_both_records_and_never_marks_upgrade_complete() {
        let root = tempfile::tempdir().unwrap();
        let a = LibraryFile::new("2026/audio/diary/aud_20261114_120000.wav".into()).unwrap();
        let b = LibraryFile::new("2026/audio/comments/aud_20261114_120000.wav".into()).unwrap();
        let mut record = Record::new(&a);
        record.tags = Some(["diary".into()].into());
        write(root.path(), &a, &record, None).unwrap();
        let before = fs::read(path(root.path(), &a, false).unwrap()).unwrap();
        let old = old_record(root.path(), &b, &["comment"]);
        assert!(prepare_layout(root.path(), &AtomicBool::new(false), &Default::default()).is_err());
        assert_eq!(
            fs::read(path(root.path(), &a, false).unwrap()).unwrap(),
            before
        );
        assert_eq!(
            fs::read(root.path().join(format!("yaml/{}.yaml", b.as_str()))).unwrap(),
            old
        );
        assert!(!root.path().join("yaml/layout.yaml").exists());
    }
}
