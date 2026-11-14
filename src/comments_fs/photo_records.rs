//! Authoritative photo records; every update is guarded by the current file revision.
use super::*;
use crate::media_records_fs::{Record as MediaRecord, parse};
use std::{
    io::Write,
    sync::atomic::{AtomicUsize, Ordering},
};
fn checked_bytes(path: &Path) -> Result<Vec<u8>, String> {
    crate::media_records_fs::read(path)?
        .ok_or_else(|| format!("Foto-YAML fehlt: {}", path.display()))
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
            MediaRecord::new(comment.photo())
        };
        if entry.file != comment.photo().as_str() {
            return Err("Foto-YAML gehört zu einem anderen Bild".into());
        }
        let mut catalog = CommentCatalog::default();
        add_photo(&mut catalog, &entry.file, entry.comments.clone())?;
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
        crate::media_records_fs::write(root, comment.photo(), &entry, previous.as_deref())?;
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
        crate::media_records_fs::visit(root, cancel, &self.priority, |photo, record, _| {
            if cancel.load(Ordering::Relaxed) || !self.priority.wait_background(cancel) {
                return Err("Neuaufbau abgebrochen; Gesamtdatei unverändert".into());
            }
            if !Media::from_path(photo.as_str().into(), String::new()).is_some_and(|m| m.is_photo())
            {
                return Ok(());
            }
            if record.comments.is_empty() {
                return Ok(());
            }
            let path = root.join(photo.metadata_path()?);

            if root.join(Comment::metadata_path(&photo)?.as_str()) != path {
                return Err(format!(
                    "Foto-YAML liegt am falschen Ort: {}",
                    path.display()
                ));
            }
            safe_reference(root, &photo)?;
            let mut parsed = CommentCatalog::default();
            add_photo(&mut parsed, &record.file, record.comments)?;
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
            Ok(())
        })?;
        Ok(catalog.entries().values().cloned().collect())
    }
}
