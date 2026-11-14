//! One YAML catalog per library. No media access, EXIF reads or filename-only guessing.
use crate::domain::tags::TagCatalog;
use photosuite_application::organization::{CatalogRepository, StoredCatalog};
use std::{collections::BTreeMap, fs, io::Read, path::Path};
const FILE: &str = "yaml/tags.yaml";
const MAX: u64 = 32 * 1024 * 1024;
#[derive(Default)]
pub struct YamlTags {
    pub priority: std::sync::Arc<photosuite_application::work_priority::WorkPriority>,
}
fn read(root: &Path) -> Result<Option<Vec<u8>>, String> {
    read_named(root, FILE)
}
fn read_named(root: &Path, name: &str) -> Result<Option<Vec<u8>>, String> {
    let metadata = fs::symlink_metadata(root).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Tag-Bibliothek ist kein reguläres Verzeichnis".into());
    }
    let path = crate::metadata_fs::library_path(root, name, false)?;
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
        return Err("Tag-Datei überschreitet 32 MiB; bleibt unverändert".into());
    }
    Ok(Some(bytes))
}
fn decode(bytes: &[u8]) -> Result<TagCatalog, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_saphyr::from_str(text).map_err(|e| e.to_string())?;
    let object = value
        .as_object()
        .ok_or("Tag-Datei muss eine YAML-Key-Value-Liste sein")?;
    for key in object.keys() {
        if !["version", "tags", "albums"].contains(&key.as_str()) {
            return Err(format!(
                "Unbekannter Tag-Eintrag {key}; Datei bleibt unverändert"
            ));
        }
    }
    if object.get("version").is_some_and(|v| v.as_u64() != Some(1)) {
        return Err("Unbekannte Tag-Version; Datei bleibt unverändert".into());
    }
    let tags: BTreeMap<String, Vec<String>> = object
        .get("tags")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let albums: Vec<String> = object
        .get("albums")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    TagCatalog::from_entries(tags, albums)
}
fn encode(catalog: &TagCatalog) -> Result<Vec<u8>, String> {
    let tags: BTreeMap<_, Vec<_>> = catalog
        .tags()
        .iter()
        .map(|(tag, files)| (tag, files.iter().map(|f| f.as_str()).collect()))
        .collect();
    let value = serde_json::json!({"version":1,"tags":tags,"albums":catalog.albums()});
    let text = serde_saphyr::to_string(&value).map_err(|e| e.to_string())?;
    let bytes=format!("# PhotoSuite – Tags und virtuelle Alben\n# Dateipfade sind relativ zu dieser Bibliothek. Originale werden nicht verändert.\n{text}").into_bytes();
    if bytes.len() as u64 > MAX {
        return Err("Tag-Daten überschreiten 32 MiB".into());
    }
    Ok(bytes)
}
impl CatalogRepository for YamlTags {
    fn prepare_metadata_layout(
        &self,
        root: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), String> {
        crate::media_records_fs::prepare_layout(root, cancel, &self.priority)
    }
    fn supports_file_records(&self) -> bool {
        true
    }
    fn read_file_tags(
        &self,
        root: &Path,
        file: &crate::domain::tags::LibraryFile,
    ) -> Result<photosuite_application::organization::FileTags, String> {
        let path = crate::media_records_fs::path(root, file, false)?;
        let revision = crate::media_records_fs::read(&path)?;
        let tags = if let Some(bytes) = &revision {
            let record = crate::media_records_fs::parse(bytes)?;
            if record.photo != file.as_str() {
                return Err("Medien-YAML gehört zu einer anderen Datei".into());
            }
            record.tags
        } else {
            None
        };
        Ok(photosuite_application::organization::FileTags {
            file: file.clone(),
            tags,
            revision,
        })
    }
    fn write_file_tags(
        &self,
        root: &Path,
        file: &crate::domain::tags::LibraryFile,
        tags: &std::collections::BTreeSet<String>,
        expected: Option<&[u8]>,
    ) -> Result<(), String> {
        let path = crate::media_records_fs::path(root, file, false)?;
        let current = crate::media_records_fs::read(&path)?;
        if current.as_deref() != expected {
            return Err("Medien-YAML wurde gleichzeitig verändert; erneut versuchen".into());
        }
        let mut record = if let Some(bytes) = &current {
            crate::media_records_fs::parse(bytes)?
        } else {
            crate::media_records_fs::Record::new(file)
        };
        record.tags = Some(tags.clone());
        crate::media_records_fs::write(root, file, &record, expected)
    }
    fn scan_file_tags(
        &self,
        root: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<photosuite_application::organization::FileTags>, String> {
        let mut records = Vec::new();
        crate::media_records_fs::visit(root, cancel, &self.priority, |file, record, _| {
            records.push(photosuite_application::organization::FileTags {
                file,
                tags: record.tags,
                revision: None,
            });
            Ok(())
        })?;
        Ok(records)
    }
    fn load_definitions(&self, root: &Path) -> Result<StoredCatalog, String> {
        let revision = read_named(root, "yaml/tag-definitions.yaml")?;
        let catalog = revision
            .as_deref()
            .map(decode)
            .transpose()?
            .unwrap_or_default();
        if !catalog.files().is_empty() {
            return Err("Tagdefinitionen dürfen keine Dateizuordnungen enthalten".into());
        }
        Ok(StoredCatalog { catalog, revision })
    }
    fn save_definitions(
        &self,
        root: &Path,
        catalog: &TagCatalog,
        expected: Option<&[u8]>,
    ) -> Result<(), String> {
        let path = crate::metadata_fs::library_path(root, "yaml/tag-definitions.yaml", true)?;
        crate::metadata_fs::atomic_write_if_current(
            &path,
            &encode(&catalog.definitions())?,
            expected,
        )
    }
    fn index_revision(&self, root: &Path) -> Result<Option<Vec<u8>>, String> {
        read(root)
    }
    fn replace_index(
        &self,
        root: &Path,
        catalog: &TagCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        let current = read(root)?;
        if current.as_deref() != expected {
            return Err("Tagindex wurde extern verändert".into());
        }
        if let Some(bytes) = &current
            && decode(bytes).is_err()
        {
            use std::io::Write;
            static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            loop {
                let relative = format!(
                    "yaml/tags-recovery-{}-{}.yaml",
                    std::process::id(),
                    SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                );
                let path = crate::metadata_fs::library_path(root, &relative, true)?;
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                {
                    Ok(mut f) => {
                        f.write_all(bytes).map_err(|e| e.to_string())?;
                        f.sync_all().map_err(|e| e.to_string())?;
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        let bytes = encode(catalog)?;
        let path = crate::metadata_fs::library_path(root, FILE, true)?;
        crate::metadata_fs::atomic_write_if_current(&path, &bytes, expected)?;
        Ok(bytes)
    }
    fn load(&self, root: &Path) -> Result<StoredCatalog, String> {
        let revision = read(root)?;
        let catalog = revision
            .as_deref()
            .map(decode)
            .transpose()?
            .unwrap_or_default();
        Ok(StoredCatalog { catalog, revision })
    }
    fn save(
        &self,
        root: &Path,
        catalog: &TagCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        let current = read(root)?;
        if current.as_deref() != expected {
            return Err("yaml/tags.yaml wurde außerhalb dieser Sitzung verändert; kein Überschreiben. Änderungen verwerfen und Datei neu laden.".into());
        }
        if let Some(bytes) = current.as_deref() {
            decode(bytes)?;
        }
        let bytes = encode(catalog)?;
        let path = crate::metadata_fs::library_path(root, FILE, true)?;
        crate::metadata_fs::atomic_write_if_current(&path, &bytes, expected)
            .map_err(|e| format!("{}: {e}", root.join(FILE).display()))?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tags::LibraryFile;
    #[test]
    fn yaml_roundtrip_keeps_relative_membership_albums_and_originals() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("2023/12")).unwrap();
        fs::write(dir.path().join("2023/12/photo.jpg"), b"original").unwrap();
        let mut loaded = YamlTags::default().load(dir.path()).unwrap();
        assert!(loaded.revision.is_none());
        loaded.catalog.create("Sommer: Urlaub #1").unwrap();
        loaded
            .catalog
            .assign(
                "Sommer: Urlaub #1",
                LibraryFile::new("2023/12/photo.jpg".into()).unwrap(),
                true,
            )
            .unwrap();
        loaded.catalog.set_album("Sommer: Urlaub #1", true).unwrap();
        let revision = YamlTags::default()
            .save(dir.path(), &loaded.catalog, None)
            .unwrap();
        let restored = YamlTags::default().load(dir.path()).unwrap();
        assert_eq!(restored.catalog, loaded.catalog);
        assert_eq!(restored.revision.as_deref(), Some(revision.as_slice()));
        let text = String::from_utf8(revision).unwrap();
        assert!(text.contains("tags:"));
        assert!(text.contains("albums:"));
        assert!(text.contains("sommer: urlaub #1"));
        assert!(!text.contains("Sommer: Urlaub #1"));
        assert!(text.contains("2023/12/photo.jpg"));
        assert!(!text.contains(dir.path().to_str().unwrap()));
        assert_eq!(
            fs::read(dir.path().join("2023/12/photo.jpg")).unwrap(),
            b"original"
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    #[test]
    fn malformed_unknown_conflicting_and_external_updates_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("yaml")).unwrap();
        for text in [
            "tags: [",
            "version: 2\ntags: {}\n",
            "tags: {a: [../outside.jpg]}\nalbums: [a]\n",
            "tags: {}\nalbums: [missing]\n",
            "tags: {}\ncustom: true\n",
        ] {
            fs::write(dir.path().join(FILE), text).unwrap();
            assert!(YamlTags::default().load(dir.path()).is_err());
            assert!(
                YamlTags::default()
                    .save(dir.path(), &TagCatalog::default(), None)
                    .is_err()
            );
            assert_eq!(fs::read_to_string(dir.path().join(FILE)).unwrap(), text);
        }
        fs::write(
            dir.path().join(FILE),
            "tags: {a: [2023/a.jpg]}\nalbums: [a]\n",
        )
        .unwrap();
        let loaded = YamlTags::default().load(dir.path()).unwrap();
        let external = "tags: {b: []}\nalbums: [b]\n";
        fs::write(dir.path().join(FILE), external).unwrap();
        assert!(
            YamlTags::default()
                .save(dir.path(), &loaded.catalog, loaded.revision.as_deref())
                .is_err()
        );
        assert_eq!(fs::read_to_string(dir.path().join(FILE)).unwrap(), external);
    }
    #[test]
    fn symlinks_and_read_only_libraries_do_not_modify_other_files() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("yaml")).unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        fs::write(outside.path(), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join(FILE)).unwrap();
        assert!(YamlTags::default().load(dir.path()).is_err());
        assert!(
            YamlTags::default()
                .save(dir.path(), &TagCatalog::default(), None)
                .is_err()
        );
        assert_eq!(fs::read(outside.path()).unwrap(), b"outside");
        fs::remove_file(dir.path().join(FILE)).unwrap();
        let old = fs::metadata(dir.path().join("yaml")).unwrap().permissions();
        fs::set_permissions(dir.path().join("yaml"), fs::Permissions::from_mode(0o555)).unwrap();
        let result = YamlTags::default().save(dir.path(), &TagCatalog::default(), None);
        fs::set_permissions(dir.path().join("yaml"), old).unwrap();
        assert!(result.is_err());
        assert!(!dir.path().join(FILE).exists());
    }
}

#[cfg(test)]
mod file_record_tests {
    use super::*;
    use crate::domain::{comments::Comment, tags::LibraryFile};
    use photosuite_application::{comments::CommentRepository, organization::index};
    fn file() -> LibraryFile {
        LibraryFile::new("1980/09/img_19800916_035811.jpg".into()).unwrap()
    }
    fn comment(n: u8) -> Comment {
        Comment::new(
            format!("12345678-1234-1234-1234-123456789ab{n}"),
            LibraryFile::new(format!("2026/audio/comments/com_20261114_12000{n}.m4a")).unwrap(),
            file(),
        )
        .unwrap()
    }
    #[test]
    fn backfill_and_both_writers_preserve_tags_comments_and_rebuild_definitions() {
        let root = tempfile::tempdir().unwrap();
        let tags = YamlTags::default();
        let comments = crate::comments_fs::YamlComments {
            priority: Default::default(),
        };
        let mut catalog = TagCatalog::default();
        catalog.create("SCAN").unwrap();
        catalog.assign("scan", file(), true).unwrap();
        catalog.set_album("scan", true).unwrap();
        catalog.create("empty").unwrap();
        tags.save(root.path(), &catalog, None).unwrap();
        comments
            .write_photo_record(root.path(), &comment(1))
            .unwrap();
        let stored = index::load_with_backfill(&tags, root.path()).unwrap();
        assert_eq!(stored.catalog, catalog);
        let path = crate::media_records_fs::path(root.path(), &file(), false).unwrap();
        let record = crate::media_records_fs::parse(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(record.version, 3);
        assert!(record.tags.unwrap().contains("scan"));
        assert_eq!(record.comments.len(), 1);
        let old = tags.read_file_tags(root.path(), &file()).unwrap();
        comments
            .write_photo_record(root.path(), &comment(2))
            .unwrap();
        assert!(
            tags.write_file_tags(
                root.path(),
                &file(),
                &["dia".into()].into(),
                old.revision.as_deref()
            )
            .is_err()
        );
        assert_eq!(
            tags.read_file_tags(root.path(), &file())
                .unwrap()
                .tags
                .unwrap(),
            ["scan".into()].into()
        );
        fs::remove_file(root.path().join(FILE)).unwrap();
        let rebuilt = index::rebuild(&tags, root.path()).unwrap();
        assert_eq!(rebuilt.catalog, catalog);
        let c = comments
            .scan_photo_records(
                root.path(),
                &std::sync::atomic::AtomicBool::new(false),
                &|_| {},
            )
            .unwrap();
        assert_eq!(c.len(), 2);
        let mut cleared = catalog.clone();
        cleared.assign("scan", file(), false).unwrap();
        index::save(
            &tags,
            root.path(),
            &catalog,
            &cleared,
            rebuilt.revision.as_deref(),
            false,
        )
        .unwrap();
        let rebuilt = index::rebuild(&tags, root.path()).unwrap();
        assert!(!rebuilt.catalog.contains("scan", file().as_str()));
        assert!(rebuilt.catalog.albums().contains("scan"));
        assert_eq!(
            crate::media_records_fs::parse(&fs::read(path).unwrap())
                .unwrap()
                .comments
                .len(),
            2
        );
    }
    #[test]
    fn corrupt_sources_block_rebuild_and_corrupt_cache_is_preserved_for_recovery() {
        let root = tempfile::tempdir().unwrap();
        let repo = YamlTags::default();
        let mut batch = index::ImportTags::new(
            std::sync::Arc::new(YamlTags::default()),
            root.path().into(),
            &["scan".into()],
        )
        .unwrap();
        batch.assign(&root.path().join(file().as_str())).unwrap();
        batch.finish().unwrap();
        fs::write(root.path().join(FILE), b"bad: [").unwrap();
        let rebuilt = index::rebuild(&repo, root.path()).unwrap();
        assert!(rebuilt.catalog.contains("scan", file().as_str()));
        assert!(fs::read_dir(root.path().join("yaml")).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("tags-recovery-")
        }));
        let before = fs::read(root.path().join(FILE)).unwrap();
        fs::write(root.path().join(file().metadata_path().unwrap()), b"bad: [").unwrap();
        assert!(index::rebuild(&repo, root.path()).is_err());
        assert_eq!(fs::read(root.path().join(FILE)).unwrap(), before);
    }
    #[test]
    fn import_cache_conflict_preserves_completed_single_file_sources() {
        let root = tempfile::tempdir().unwrap();
        let repo = std::sync::Arc::new(YamlTags::default());
        let mut batch = index::ImportTags::new(
            repo.clone(),
            root.path().into(),
            &["SCAN".into(), "dia".into()],
        )
        .unwrap();
        batch.assign(&root.path().join(file().as_str())).unwrap();
        repo.save(root.path(), &TagCatalog::default(), None)
            .unwrap();
        let before = fs::read(root.path().join(FILE)).unwrap();
        assert!(batch.finish().is_err());
        assert_eq!(fs::read(root.path().join(FILE)).unwrap(), before);
        let restored = index::rebuild(repo.as_ref(), root.path()).unwrap();
        assert!(restored.catalog.contains("scan", file().as_str()));
        assert!(restored.catalog.contains("dia", file().as_str()));
    }
}
