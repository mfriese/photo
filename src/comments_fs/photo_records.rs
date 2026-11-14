//! Authoritative photo records; every update is guarded by the current file revision.
use super::*;
use std::{
    io::Write,
    sync::atomic::{AtomicUsize, Ordering},
};
const SIDECAR_MAX: u64 = 1024 * 1024;
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PhotoSidecar {
    version: u32,
    photo: String,
    comments: std::collections::BTreeMap<String, String>,
}
fn checked_bytes(path: &Path) -> Result<Vec<u8>, String> {
    if !crate::metadata_fs::check_file(path)? {
        return Err(format!("Foto-YAML fehlt: {}", path.display()));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(SIDECAR_MAX + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > SIDECAR_MAX {
        return Err("Foto-Kommentar-YAML überschreitet 1 MiB".into());
    }
    Ok(bytes)
}
fn parse(bytes: &[u8]) -> Result<PhotoSidecar, String> {
    let entry: PhotoSidecar =
        serde_saphyr::from_str(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if entry.version != 2 {
        return Err("Foto-Kommentar-YAML benötigt Version 2; keine Altformat-Unterstützung".into());
    }
    Ok(entry)
}
fn safe_reference(root: &Path, relative: &LibraryFile) -> Result<bool, String> {
    let path = root.join(relative.as_str());
    let mut current = path.as_path();
    let mut present = true;
    loop {
        match fs::symlink_metadata(current) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!(
                    "Symbolischer Kommentarverweis: {}",
                    current.display()
                ));
            }
            Ok(m) if current == path && !m.is_file() => {
                return Err(format!(
                    "Kommentarverweis ist keine reguläre Datei: {}",
                    current.display()
                ));
            }
            Ok(m) if current != path && !m.is_dir() => {
                return Err(format!(
                    "Kommentar-Vorfahre ist kein Verzeichnis: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => present = false,
            Err(e) => return Err(e.to_string()),
        };
        if current == root {
            break;
        }
        current = current.parent().ok_or("Verweis außerhalb der Bibliothek")?;
    }
    Ok(present)
}
// Do not traverse photo/video/diary trees to rebuild a metadata-only index.
fn source_branch(source: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(source) else {
        return false;
    };
    let mut parts = relative.components();
    parts.next().is_none_or(|part| {
        part.as_os_str().to_str().is_some_and(|year| {
            crate::domain::diary::AudioScope::for_year(year, crate::domain::diary::AudioKind::All)
                .is_some()
        })
    })
}
impl YamlComments {
    pub(super) fn update_photo_record(
        &self,
        root: &Path,
        comment: &Comment,
    ) -> Result<CommentCatalog, String> {
        directory(root)?;
        let relative = comment.sidecar();
        safe_reference(root, &relative)?;
        safe_reference(root, comment.photo())?;
        safe_reference(root, comment.audio())?;
        let path = root.join(relative.as_str());
        let previous = if crate::metadata_fs::check_file(&path)? {
            Some(checked_bytes(&path)?)
        } else {
            None
        };
        let mut entry = if let Some(bytes) = &previous {
            parse(bytes)?
        } else {
            PhotoSidecar {
                version: 2,
                photo: comment.photo().as_str().into(),
                comments: Default::default(),
            }
        };
        if entry.photo != comment.photo().as_str() {
            return Err("Foto-YAML gehört zu einem anderen Bild".into());
        }
        let mut catalog = CommentCatalog::default();
        add_photo(&mut catalog, &entry.photo, entry.comments.clone())?;
        catalog.insert(comment.clone())?;
        let normalized = catalog
            .entries()
            .values()
            .map(|c| (c.id().to_owned(), c.audio().as_str().to_owned()))
            .collect();
        if entry.comments == normalized {
            return Ok(catalog);
        }
        entry.comments = normalized;
        let bytes = serde_saphyr::to_string(&entry)
            .map_err(|e| e.to_string())?
            .into_bytes();
        if bytes.len() as u64 > SIDECAR_MAX {
            return Err("Foto-Kommentar-YAML überschreitet 1 MiB".into());
        }
        crate::metadata_fs::library_path(root, relative.as_str(), true)?;
        safe_reference(root, &relative)?;
        crate::metadata_fs::atomic_write_if_current(&path, &bytes, previous.as_deref())?;
        Ok(catalog)
    }
    pub(super) fn read_rebuild_base(&self, root: &Path) -> Result<RebuildBase, String> {
        let revision = read(root)?;
        let mut warning = String::new();
        if let Some(bytes) = revision.as_deref()
            && let Err(error) = decode(bytes)
        {
            static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
            loop {
                let relative = format!(
                    "yaml/comments-index-recovery-{}-{}.yaml",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                );
                let path = crate::metadata_fs::library_path(root, &relative, true)?;
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                {
                    Ok(mut file) => {
                        file.write_all(bytes).map_err(|e| e.to_string())?;
                        file.sync_all().map_err(|e| e.to_string())?;
                        warning = format!(
                            "Unlesbarer Index erhalten unter {}: {error}",
                            path.display()
                        );
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        Ok(RebuildBase { revision, warning })
    }
    pub(super) fn read_photo_records(
        &self,
        root: &Path,
        cancel: &AtomicBool,
        progress: &dyn Fn(RebuildProgress),
    ) -> Result<Vec<Comment>, String> {
        directory(root)?;
        let mut catalog = CommentCatalog::default();
        let mut checked = 0;
        let marker = crate::metadata_fs::library_path(root, "yaml/.scan", false)?;
        let source = marker.parent().ok_or("Metadatenverzeichnis fehlt")?;
        if !source.try_exists().map_err(|e| e.to_string())? {
            return Ok(Vec::new());
        }
        for entry in walkdir::WalkDir::new(source)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| source_branch(source, entry.path()))
        {
            if cancel.load(Ordering::Relaxed) || !self.priority.wait_background(cancel) {
                return Err("Neuaufbau abgebrochen; Gesamtdatei unverändert".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("._"))
            {
                continue;
            }
            let relative = path.strip_prefix(source).map_err(|e| e.to_string())?;
            if relative.components().count() <= 1 && !entry.file_type().is_dir() {
                return Err(format!(
                    "Metadaten-Vorfahre ist kein reguläres Verzeichnis: {}",
                    path.display()
                ));
            }
            if entry.file_type().is_symlink() {
                return Err(format!(
                    "Symbolischer Foto-Metadatenpfad: {}",
                    path.display()
                ));
            }
            if !entry.file_type().is_file() || path.extension().is_none_or(|e| e != "yaml") {
                continue;
            }
            // Only image-named records are part of this schema; audio-named records are not read.
            let Some(name) = path.file_stem().and_then(|n| n.to_str()) else {
                continue;
            };
            if !Media::from_path(name.into(), String::new()).is_some_and(|m| m.is_photo()) {
                continue;
            }
            let record =
                parse(&checked_bytes(path)?).map_err(|e| format!("{}: {e}", path.display()))?;
            let photo = LibraryFile::new(record.photo.clone())?;
            if root.join(Comment::metadata_path(&photo)?.as_str()) != path {
                return Err(format!(
                    "Foto-YAML liegt am falschen Ort: {}",
                    path.display()
                ));
            }
            safe_reference(root, &photo)?;
            let mut parsed = CommentCatalog::default();
            add_photo(&mut parsed, &record.photo, record.comments)?;
            // Empty image records are valid and carry no comment mapping.
            for comment in parsed.entries().values() {
                if root.join(comment.sidecar().as_str()) != path {
                    return Err(format!(
                        "Foto-YAML liegt am falschen Ort: {}",
                        path.display()
                    ));
                }
                safe_reference(root, comment.audio())?;
                catalog.insert(comment.clone())?;
            }
            checked += 1;
            progress(RebuildProgress {
                checked,
                phase: format!("{checked} Foto-Kommentar-YAMLs geprüft"),
            });
        }
        Ok(catalog.entries().values().cloned().collect())
    }
}
