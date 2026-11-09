//! Checked cleanup of app-owned staging files. No sweeping of pre-existing files.
use std::{
    fs::{self, File},
    os::unix::fs::MetadataExt,
    path::Path,
};
pub(crate) fn cleanup(
    staged: tempfile::NamedTempFile,
    result: Result<(), String>,
    kind: &str,
) -> Result<(), String> {
    let (file, mut temporary_path) = staged.into_parts();
    let path = temporary_path.to_path_buf();
    temporary_path.disable_cleanup(true);
    cleanup_alias(file, &path, result, kind)
}
/// Also handles a leftover hardlink alias from persist_noclobber's fallback.
pub(crate) fn cleanup_alias(
    file: File,
    path: &Path,
    result: Result<(), String>,
    kind: &str,
) -> Result<(), String> {
    let identity = file.metadata();
    drop(file);
    let cleanup = (|| {
        let current = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        let identity = identity.map_err(|e| e.to_string())?;
        if !current.is_file()
            || current.file_type().is_symlink()
            || current.dev() != identity.dev()
            || current.ino() != identity.ino()
        {
            return Err(
                "Pfad wurde ersetzt oder kann nicht sicher zugeordnet werden; bleibt erhalten"
                    .into(),
            );
        }
        fs::remove_file(path).map_err(|e| e.to_string())
    })();
    match cleanup {
        Ok(()) => result,
        Err(error) => {
            let context = match result {
                Ok(()) => "Zieldatei wurde vollständig geprüft. ".to_owned(),
                Err(message) => format!("{message}. "),
            };
            Err(format!(
                "{context}{kind} konnte nicht entfernt werden: {}: {error}",
                path.display()
            ))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn a_leftover_owned_hardlink_is_removed_without_removing_the_published_file() {
        let directory = tempfile::tempdir().unwrap();
        let mut stage = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        stage.write_all(b"verified").unwrap();
        let alias = stage.path().to_owned();
        let target = directory.path().join("published.jpg");
        fs::hard_link(&alias, &target).unwrap();
        let (file, mut path) = stage.into_parts();
        path.disable_cleanup(true);
        cleanup_alias(file, &alias, Ok(()), "Temporäre Importdatei").unwrap();
        assert!(!alias.exists());
        assert_eq!(fs::read(target).unwrap(), b"verified");
    }
    #[test]
    fn foreign_replacement_is_retained_when_cleaning_a_persisted_alias() {
        let directory = tempfile::tempdir().unwrap();
        let stage = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        let path = stage.path().to_owned();
        let (file, mut temporary) = stage.into_parts();
        temporary.disable_cleanup(true);
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"foreign").unwrap();
        assert!(cleanup_alias(file, &path, Ok(()), "Tempdatei").is_err());
        assert_eq!(fs::read(path).unwrap(), b"foreign");
    }
}
