//! Independent, exclusively published metadata records and read-only reconstruction scanning.
use super::*;
use std::{
    io::Write,
    sync::atomic::{AtomicUsize, Ordering},
};
const SIDECAR_MAX: u64 = 64 * 1024;
#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Sidecar {
    version: u32,
    id: String,
    audio: String,
    photo: String,
    recorded_at: String,
}
impl Sidecar {
    fn from_comment(c: &Comment) -> Self {
        Self {
            version: 1,
            id: c.id().into(),
            audio: c.audio().as_str().into(),
            photo: c.photo().as_str().into(),
            recorded_at: c.recorded_at().iso().into(),
        }
    }
}
fn checked_bytes(path: &Path) -> Result<Vec<u8>, String> {
    if !crate::metadata_fs::check_file(path)? {
        return Err(format!("Kommentar-YAML fehlt: {}", path.display()));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(SIDECAR_MAX + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > SIDECAR_MAX {
        return Err(format!("Kommentar-YAML über 64 KiB: {}", path.display()));
    }
    Ok(bytes)
}
fn parse(bytes: &[u8]) -> Result<Sidecar, String> {
    let s: Sidecar = serde_saphyr::from_str(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if s.version != 1 {
        return Err("Unbekannte Kommentar-YAML-Version".into());
    }
    Ok(s)
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
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => present = false,
            Err(e) => return Err(e.to_string()),
        }
        if current == root {
            break;
        }
        current = current.parent().ok_or("Verweis außerhalb der Bibliothek")?;
    }
    Ok(present)
}
impl YamlComments {
    pub(super) fn publish_sidecar(&self, root: &Path, comment: &Comment) -> Result<(), String> {
        directory(root)?;
        let path = root.join(comment.sidecar().as_str());
        let entry = Sidecar::from_comment(comment);
        safe_reference(root, &comment.sidecar())?;
        if path.try_exists().map_err(|e| e.to_string())? {
            let current = parse(&checked_bytes(&path)?)?;
            if current == entry {
                return Ok(());
            }
            return Err(format!(
                "Kommentar-YAML bereits anders belegt: {}",
                path.display()
            ));
        }
        // Only our temporary serialization is cleaned up; the audio is never removed.
        let text = serde_saphyr::to_string(&entry).map_err(|e| e.to_string())?;
        let mut temp = tempfile::Builder::new()
            .prefix("photosuite-comment-")
            .suffix(".yaml")
            .tempfile()
            .map_err(|e| e.to_string())?;
        temp.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        let result =
            self.copier
                .copy_verified(temp.path(), &path, &AtomicBool::new(false), &|_| {});
        crate::staging_fs::cleanup(temp, result, "Temporäre Kommentar-YAML")
    }
    pub(super) fn read_recovery_base(&self, root: &Path) -> Result<RecoveryBase, String> {
        let revision = read(root)?;
        let mut warning = String::new();
        let catalog = match revision.as_deref() {
            None => CommentCatalog::default(),
            Some(bytes) => match decode(bytes) {
                Ok(c) => {
                    let mut missing = CommentCatalog::default();
                    for comment in c.entries().values() {
                        if !safe_reference(root, &comment.sidecar())? {
                            missing.insert(comment.clone())?;
                        }
                    }
                    missing
                }
                Err(error) => {
                    // Preserve an unreadable old index before replacing the derived cache.
                    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
                    let path = loop {
                        let path = root.join(format!(
                            "photosuite-comments-recovery-{}-{}.yaml",
                            std::process::id(),
                            SEQUENCE.fetch_add(1, Ordering::Relaxed)
                        ));
                        match fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)
                        {
                            Ok(mut file) => {
                                file.write_all(bytes).map_err(|e| e.to_string())?;
                                file.sync_all().map_err(|e| e.to_string())?;
                                break path;
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                            Err(e) => return Err(e.to_string()),
                        }
                    };
                    warning = format!("Defekter Index erhalten unter {}: {error}", path.display());
                    CommentCatalog::default()
                }
            },
        };
        Ok(RecoveryBase {
            stored: StoredComments { catalog, revision },
            warning,
        })
    }
    pub(super) fn scan_comment_sidecars(
        &self,
        root: &Path,
        cancel: &AtomicBool,
        progress: &dyn Fn(RebuildProgress),
    ) -> Result<Vec<Comment>, String> {
        directory(root)?;
        let mut comments = Vec::new();
        let mut checked = 0;
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            if cancel.load(Ordering::Relaxed) {
                return Err("Neuaufbau abgebrochen; Gesamtdatei unverändert".into());
            }
            if !self.priority.wait_background(cancel) {
                return Err("Neuaufbau abgebrochen; Gesamtdatei unverändert".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("._"))
            {
                continue;
            }
            if entry.file_type().is_symlink()
                && path
                    .file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("y"))
                && path.parent().is_some_and(|p| {
                    p.file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("audio"))
                })
            {
                return Err(format!(
                    "Symbolischer Kommentar-YAML-Ordner: {}; Gesamtdatei bleibt unverändert",
                    path.display()
                ));
            }
            let metadata_folder = path.parent().is_some_and(|p| {
                p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("y"))
                    && p.parent().is_some_and(|a| {
                        a.file_name()
                            .is_some_and(|n| n.eq_ignore_ascii_case("audio"))
                    })
            });
            if !metadata_folder
                || !path.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml")
                })
            {
                continue;
            }
            let sidecar = parse(&checked_bytes(path)?)
                .map_err(|e| format!("{}: {e}; Index bleibt unverändert", path.display()))?;
            let audio = LibraryFile::new(sidecar.audio)?;
            let photo = LibraryFile::new(sidecar.photo)?;
            safe_reference(root, &audio)?;
            let photo_exists = safe_reference(root, &photo)?;
            let media = Media::from_path(root.join(photo.as_str()), String::new());
            let date = media
                .as_ref()
                .and_then(crate::domain::capture_from_filename)
                .or_else(|| {
                    if photo_exists {
                        media.as_ref().and_then(|m| self.dates.captured_at(m).ok())
                    } else {
                        None
                    }
                });
            let comment = Comment::new(
                sidecar.id,
                audio,
                photo,
                RecordedAt::parse(&sidecar.recorded_at)?,
                date,
            )?;
            if root
                .join(comment.sidecar().as_str())
                .with_extension(path.extension().unwrap_or_default())
                != path
            {
                return Err(format!(
                    "Kommentar-YAML liegt nicht am erwarteten Ort: {}",
                    path.display()
                ));
            }
            comments.push(comment);
            checked += 1;
            progress(RebuildProgress {
                checked,
                phase: format!("{checked} Kommentar-YAMLs geprüft"),
            });
        }
        Ok(comments)
    }
}
