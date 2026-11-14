//! Shared regular-file checks and synced replacement of library metadata, never media.
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
/// Resolve a library metadata target without following symbolic directory ancestors.
/// Reads never create directories; only an explicit write may create missing parents.
pub(crate) fn library_path(
    root: &Path,
    relative: &str,
    create: bool,
) -> Result<std::path::PathBuf, String> {
    let relative = Path::new(relative);
    if relative
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || relative.as_os_str().is_empty()
    {
        return Err("Ungültiger Verwaltungspfad".into());
    }
    let mut current = root.to_path_buf();
    let mut directories = vec![current.clone()];
    for part in relative
        .parent()
        .ok_or("Verwaltungspfad ohne Elternverzeichnis")?
        .components()
    {
        current.push(part);
        directories.push(current.clone());
    }
    for (index, directory) in directories.iter().enumerate() {
        match fs::symlink_metadata(directory) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(format!(
                    "Verwaltungspfad ist kein reguläres Verzeichnis: {}",
                    directory.display()
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && index > 0 => {
                if create {
                    match fs::create_dir(directory) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(e) => return Err(e.to_string()),
                    }
                    let m = fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
                    if !m.is_dir() || m.file_type().is_symlink() {
                        return Err(format!(
                            "Unsicheres Verwaltungsverzeichnis: {}",
                            directory.display()
                        ));
                    }
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(root.join(relative))
}

pub(crate) fn check_file(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(format!(
            "Keine reguläre Verwaltungsdatei: {}",
            path.display()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    replace(path, bytes, || check_file(path).map(|_| ()))
}
pub(crate) fn atomic_write_if_current(
    path: &Path,
    bytes: &[u8],
    expected: Option<&[u8]>,
) -> Result<(), String> {
    replace(path, bytes, || verify_current(path, expected))
}
/// Validate the external revision after preparing/syncing the replacement, just before rename.
fn verify_current(path: &Path, expected: Option<&[u8]>) -> Result<(), String> {
    let present = check_file(path)?;
    match (present, expected) {
        (false, None) => Ok(()),
        (true, Some(expected)) => {
            let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
            if file.metadata().map_err(|e| e.to_string())?.len() != expected.len() as u64 {
                return Err(
                    "Verwaltungsdatei wurde außerhalb dieser Sitzung verändert; kein Überschreiben"
                        .into(),
                );
            }
            let mut buffer = [0u8; 65536];
            let mut offset = 0;
            loop {
                let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                if offset + count > expected.len()
                    || buffer[..count] != expected[offset..offset + count]
                {
                    return Err("Verwaltungsdatei wurde außerhalb dieser Sitzung verändert; kein Überschreiben".into());
                }
                offset += count;
            }
            if offset != expected.len() {
                return Err("Verwaltungsdatei wurde während der Prüfung verändert".into());
            }
            Ok(())
        }
        _ => Err("Verwaltungsdatei wurde neu angelegt oder entfernt; kein Überschreiben".into()),
    }
}
fn replace(
    path: &Path,
    bytes: &[u8],
    verify: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    check_file(path)?;
    let parent = path.parent().ok_or("Verwaltungspfad ohne Verzeichnis")?;
    let mut temp = tempfile::Builder::new()
        .prefix(".photosuite-settings-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|e| e.to_string())?;
    let prepared = (|| {
        temp.write_all(bytes).map_err(|e| e.to_string())?;
        temp.flush().map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        verify()
    })();
    if let Err(error) = prepared {
        return crate::staging_fs::cleanup(temp, Err(error), "Temporäre Verwaltungsdatei");
    }
    match temp.persist(path) {
        Ok(_) => Ok(()),
        Err(error) => crate::staging_fs::cleanup(
            error.file,
            Err(error.error.to_string()),
            "Temporäre Verwaltungsdatei",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_library_paths_are_created_only_for_writes_and_escape_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let relative = "yaml/1980/09/photo.jpg.yaml";
        let path = library_path(root.path(), relative, false).unwrap();
        assert!(!path.parent().unwrap().exists());
        assert!(library_path(root.path(), "../outside.yaml", true).is_err());
        assert!(library_path(root.path(), "/outside.yaml", true).is_err());
        assert_eq!(library_path(root.path(), relative, true).unwrap(), path);
        assert!(path.parent().unwrap().is_dir());
        fs::write(root.path().join("yaml/1980/blocked"), b"not a folder").unwrap();
        assert!(library_path(root.path(), "yaml/1980/blocked/photo.jpg.yaml", true).is_err());
    }
    #[test]
    fn concurrent_change_after_preparing_staging_is_detected_and_staging_is_removed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tags.yaml");
        fs::write(&path, b"original").unwrap();
        let result = replace(&path, b"candidate", || {
            fs::write(&path, b"external").unwrap();
            verify_current(&path, Some(b"original"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"external");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    #[test]
    fn revision_checks_missing_and_existing_files_before_commit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tags.yaml");
        atomic_write_if_current(&path, b"first", None).unwrap();
        assert!(atomic_write_if_current(&path, b"second", None).is_err());
        atomic_write_if_current(&path, b"second", Some(b"first")).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
