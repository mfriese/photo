//! Gallery Trash adapter; only regular photo/video files inside the selected library.
use crate::domain::{Media, tags::LibraryFile};
use photosuite_application::{media_trash::MediaTrash, ports::Trash};
use std::{fs, path::Path, sync::Arc};
pub struct LibraryTrash {
    pub trash: Arc<dyn Trash>,
}
impl MediaTrash for LibraryTrash {
    fn trash(&self, root: &Path, file: &LibraryFile) -> Result<(), String> {
        if !Media::from_path(file.as_str().into(), String::new())
            .is_some_and(|m| m.is_photo() || m.is_video())
        {
            return Err("Nur Fotos und Videos können hier entfernt werden".into());
        }
        if Path::new(file.as_str())
            .components()
            .next()
            .is_some_and(|c| c.as_os_str() == "yaml")
        {
            return Err("Verwaltungsdaten bleiben erhalten".into());
        }
        let target = root.join(file.as_str());
        let mut current = target.as_path();
        loop {
            let metadata =
                fs::symlink_metadata(current).map_err(|e| format!("{}: {e}", current.display()))?;
            if metadata.file_type().is_symlink()
                || (if current == target {
                    !metadata.is_file()
                } else {
                    !metadata.is_dir()
                })
            {
                return Err(format!("Kein regulärer Medienpfad: {}", current.display()));
            }
            if current == root {
                break;
            }
            current = current
                .parent()
                .ok_or("Medienpfad außerhalb der Bibliothek")?;
        }
        self.trash.move_to_trash(&target)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(std::sync::Mutex<Vec<std::path::PathBuf>>);
    impl Trash for Fake {
        fn move_to_trash(&self, path: &Path) -> Result<(), String> {
            self.0.lock().unwrap().push(path.into());
            Ok(())
        }
    }
    #[test]
    fn only_regular_media_are_delegated_and_metadata_is_never_removed() {
        let root = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake(Default::default()));
        let adapter = LibraryTrash {
            trash: fake.clone(),
        };
        fs::create_dir_all(root.path().join("2026/11")).unwrap();
        let file = LibraryFile::new("2026/11/a.jpg".into()).unwrap();
        let path = root.path().join(file.as_str());
        fs::write(&path, b"original").unwrap();
        adapter.trash(root.path(), &file).unwrap();
        assert_eq!(*fake.0.lock().unwrap(), vec![path.clone()]);
        assert_eq!(fs::read(path).unwrap(), b"original");
        for name in ["2026/11/missing.jpg", "2026/11", "yaml/photo.jpg"] {
            assert!(
                adapter
                    .trash(root.path(), &LibraryFile::new(name.into()).unwrap())
                    .is_err()
            );
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                root.path().join("2026/11/a.jpg"),
                root.path().join("2026/11/link.jpg"),
            )
            .unwrap();
            std::os::unix::fs::symlink(root.path().join("2026/11"), root.path().join("linked"))
                .unwrap();
            for name in ["2026/11/link.jpg", "linked/a.jpg"] {
                assert!(
                    adapter
                        .trash(root.path(), &LibraryFile::new(name.into()).unwrap())
                        .is_err()
                );
            }
        }
        assert_eq!(fake.0.lock().unwrap().len(), 1);
    }
}
