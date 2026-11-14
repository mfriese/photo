//! One YAML catalog per library. No media access, EXIF reads or filename-only guessing.
use crate::domain::tags::TagCatalog;
use photosuite_application::organization::{CatalogRepository, StoredCatalog};
use std::{collections::BTreeMap, fs, io::Read, path::Path};
const FILE: &str = "yaml/tags.yaml";
const MAX: u64 = 32 * 1024 * 1024;
pub struct YamlTags;
fn read(root: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = fs::symlink_metadata(root).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Tag-Bibliothek ist kein reguläres Verzeichnis".into());
    }
    let path = crate::metadata_fs::library_path(root, FILE, false)?;
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
        let mut loaded = YamlTags.load(dir.path()).unwrap();
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
        let revision = YamlTags.save(dir.path(), &loaded.catalog, None).unwrap();
        let restored = YamlTags.load(dir.path()).unwrap();
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
            assert!(YamlTags.load(dir.path()).is_err());
            assert!(
                YamlTags
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
        let loaded = YamlTags.load(dir.path()).unwrap();
        let external = "tags: {b: []}\nalbums: [b]\n";
        fs::write(dir.path().join(FILE), external).unwrap();
        assert!(
            YamlTags
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
        assert!(YamlTags.load(dir.path()).is_err());
        assert!(
            YamlTags
                .save(dir.path(), &TagCatalog::default(), None)
                .is_err()
        );
        assert_eq!(fs::read(outside.path()).unwrap(), b"outside");
        fs::remove_file(dir.path().join(FILE)).unwrap();
        let old = fs::metadata(dir.path().join("yaml")).unwrap().permissions();
        fs::set_permissions(dir.path().join("yaml"), fs::Permissions::from_mode(0o555)).unwrap();
        let result = YamlTags.save(dir.path(), &TagCatalog::default(), None);
        fs::set_permissions(dir.path().join("yaml"), old).unwrap();
        assert!(result.is_err());
        assert!(!dir.path().join(FILE).exists());
    }
}
