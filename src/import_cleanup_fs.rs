//! Read-only comparison plus a separately injected, reversible Trash operation.
use photosuite_application::import_cleanup::{CleanupPort, FileVersion, Proof, Trash};
use std::{
    fs::{self, File, Metadata},
    io::Read,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub struct NativeTrash;
impl Trash for NativeTrash {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            use std::{ffi::CString, os::unix::ffi::OsStrExt};
            unsafe extern "C" {
                fn photosuite_trash_file(path: *const std::ffi::c_char) -> i32;
            }
            let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
            if unsafe { photosuite_trash_file(path.as_ptr()) } == 0 {
                Ok(())
            } else {
                Err("macOS-Papierkorb nicht verfügbar; keine endgültige Löschung ausgeführt".into())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Err("Papierkorb derzeit nur auf macOS verfügbar".into())
        }
    }
}
pub struct SafeImportCleanup {
    pub trash: Arc<dyn Trash>,
}
fn version(meta: &Metadata) -> Result<FileVersion, String> {
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("Keine reguläre Datei; Quelle bleibt erhalten".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileVersion {
            device: meta.dev(),
            inode: meta.ino(),
            size: meta.len(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            changed: (meta.ctime(), meta.ctime_nsec()),
        })
    }
    #[cfg(not(unix))]
    {
        Err("Sichere Dateiversionsprüfung auf dieser Plattform nicht verfügbar".into())
    }
}
fn directory_id(path: &Path) -> Result<(u64, u64), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("Bibliotheks-/Quellordner nicht regulär".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok((meta.dev(), meta.ino()))
    }
    #[cfg(not(unix))]
    {
        Err("Ordneridentität auf dieser Plattform nicht verfügbar".into())
    }
}
fn inside(root: &Path, original_root: &Path, file: &Path) -> Result<PathBuf, String> {
    let relative = file
        .strip_prefix(original_root)
        .or_else(|_| file.strip_prefix(root))
        .map_err(|_| "Datei liegt außerhalb des aufgezeichneten Importordners")?;
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Ungültiger relativer Dateipfad".into());
    }
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err("Symbolische Dateipfade werden nicht freigegeben".into());
        }
    }
    Ok(path)
}
fn compare(
    source: &Path,
    target: &Path,
    cancel: &AtomicBool,
) -> Result<(FileVersion, FileVersion), String> {
    let before_source =
        version(&fs::symlink_metadata(source).map_err(|e| format!("Quelle fehlt: {e}"))?)?;
    let before_target =
        version(&fs::symlink_metadata(target).map_err(|e| format!("Ziel fehlt: {e}"))?)?;
    if (before_source.device, before_source.inode) == (before_target.device, before_target.inode) {
        return Err("Quelle und Ziel sind dieselbe Datei oder Hardlinks".into());
    }
    if before_source.size == 0 || before_source.size != before_target.size {
        return Err("Leere Datei oder unterschiedliche Dateigrößen".into());
    }
    let mut a = File::open(source).map_err(|e| e.to_string())?;
    let mut b = File::open(target).map_err(|e| e.to_string())?;
    if version(&a.metadata().map_err(|e| e.to_string())?)? != before_source
        || version(&b.metadata().map_err(|e| e.to_string())?)? != before_target
    {
        return Err("Datei während des Öffnens verändert".into());
    }
    let (mut x, mut y) = ([0u8; 65536], [0u8; 65536]);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Abgebrochen".into());
        }
        let n = a.read(&mut x).map_err(|e| e.to_string())?;
        if n == 0 {
            if b.read(&mut y).map_err(|e| e.to_string())? != 0 {
                return Err("Zielinhalt abweichend".into());
            }
            break;
        }
        b.read_exact(&mut y[..n]).map_err(|e| e.to_string())?;
        if x[..n] != y[..n] {
            return Err("Quelle und Ziel sind nicht byteidentisch".into());
        }
    }
    if version(&fs::symlink_metadata(source).map_err(|e| e.to_string())?)? != before_source
        || version(&fs::symlink_metadata(target).map_err(|e| e.to_string())?)? != before_target
    {
        return Err("Datei während der Prüfung verändert".into());
    }
    Ok((before_source, before_target))
}
impl CleanupPort for SafeImportCleanup {
    fn check(
        &self,
        source_root: &Path,
        target_root: &Path,
        source: &Path,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<Proof, String> {
        let a = fs::canonicalize(source_root).map_err(|e| e.to_string())?;
        let b = fs::canonicalize(target_root).map_err(|e| e.to_string())?;
        if a.starts_with(&b) || b.starts_with(&a) {
            return Err("Quelle und Bibliothek überschneiden sich".into());
        }
        let source_root_id = directory_id(&a)?;
        let target_root_id = directory_id(&b)?;
        let source = inside(&a, source_root, source)?;
        let target = inside(&b, target_root, target)?;
        let (source_version, target_version) = compare(&source, &target, cancel)?;
        Ok(Proof {
            source_root: a,
            target_root: b,
            source,
            target,
            source_version,
            target_version,
            source_root_id,
            target_root_id,
        })
    }
    fn trash_checked(&self, proof: &Proof, cancel: &AtomicBool) -> Result<(), String> {
        let current = self.check(
            &proof.source_root,
            &proof.target_root,
            &proof.source,
            &proof.target,
            cancel,
        )?;
        if current.source_root_id != proof.source_root_id
            || current.target_root_id != proof.target_root_id
            || current.source_version != proof.source_version
            || current.target_version != proof.target_version
        {
            return Err(
                "Quelle oder Ziel seit der Freigabe verändert; Quelle bleibt erhalten".into(),
            );
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Abgebrochen; Quelle bleibt erhalten".into());
        }
        self.trash.move_to_trash(&proof.source)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct TestTrash {
        directory: PathBuf,
        calls: Mutex<Vec<PathBuf>>,
    }
    impl Trash for TestTrash {
        fn move_to_trash(&self, path: &Path) -> Result<(), String> {
            fs::rename(path, self.directory.join(path.file_name().unwrap()))
                .map_err(|e| e.to_string())?;
            self.calls.lock().unwrap().push(path.into());
            Ok(())
        }
    }
    fn setup() -> (
        tempfile::TempDir,
        SafeImportCleanup,
        Arc<TestTrash>,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let source_root = dir.path().join("source");
        let target_root = dir.path().join("library");
        let trash_dir = dir.path().join("simulated-trash");
        for path in [&source_root, &target_root, &trash_dir] {
            fs::create_dir(path).unwrap();
        }
        let source = source_root.join("recording.wav");
        let target = target_root.join("aud_20231227_225132.wav");
        fs::write(&source, b"original bytes").unwrap();
        fs::write(&target, b"original bytes").unwrap();
        let trash = Arc::new(TestTrash {
            directory: trash_dir,
            calls: Mutex::new(Vec::new()),
        });
        (
            dir,
            SafeImportCleanup {
                trash: trash.clone(),
            },
            trash,
            source_root,
            target_root,
            source,
            target,
        )
    }
    #[test]
    fn check_is_readonly_and_approved_move_preserves_the_verified_target() {
        let (_dir, adapter, trash, a, b, source, target) = setup();
        let cancel = AtomicBool::new(false);
        let proof = adapter.check(&a, &b, &source, &target, &cancel).unwrap();
        assert!(source.exists());
        assert!(trash.calls.lock().unwrap().is_empty());
        adapter.trash_checked(&proof, &cancel).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(&target).unwrap(), b"original bytes");
        assert_eq!(
            fs::read(trash.directory.join("recording.wav")).unwrap(),
            b"original bytes"
        );
        assert_eq!(*trash.calls.lock().unwrap(), vec![proof.source.clone()]);
        assert!(adapter.trash_checked(&proof, &cancel).is_err());
    }
    #[test]
    fn changes_missing_target_and_same_content_replacement_invalidate_approval() {
        for scenario in 0..6 {
            let (_dir, adapter, trash, a, b, source, target) = setup();
            let cancel = AtomicBool::new(false);
            let proof = adapter.check(&a, &b, &source, &target, &cancel).unwrap();
            match scenario {
                0 => fs::write(&source, b"modified bytes").unwrap(),
                1 => fs::write(&target, b"modified bytes").unwrap(),
                2 => fs::remove_file(&target).unwrap(),
                3 => {
                    fs::rename(&target, b.join("replaced.wav")).unwrap();
                    fs::write(&target, b"original bytes").unwrap();
                }
                4 => {
                    fs::rename(&source, a.join("replaced.wav")).unwrap();
                    fs::write(&source, b"original bytes").unwrap();
                }
                _ => File::options()
                    .write(true)
                    .open(&target)
                    .unwrap()
                    .set_modified(
                        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2000),
                    )
                    .unwrap(),
            }
            assert!(adapter.trash_checked(&proof, &cancel).is_err());
            assert!(source.exists());
            assert!(trash.calls.lock().unwrap().is_empty());
        }
    }
    #[test]
    fn unsafe_boundaries_links_hardlinks_empty_files_and_cancel_are_rejected() {
        let (_dir, adapter, trash, a, b, source, target) = setup();
        let cancel = AtomicBool::new(false);
        assert!(adapter.check(&a, &a, &source, &target, &cancel).is_err());
        assert!(
            adapter
                .check(&a, &b, &source, &a.join("not-in-library.wav"), &cancel)
                .is_err()
        );
        let link = a.join("link.wav");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(adapter.check(&a, &b, &link, &target, &cancel).is_err());
        let linked = b.join("hardlink.wav");
        fs::hard_link(&source, &linked).unwrap();
        assert!(adapter.check(&a, &b, &source, &linked, &cancel).is_err());
        let proof = adapter.check(&a, &b, &source, &target, &cancel).unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(adapter.trash_checked(&proof, &cancel).is_err());
        cancel.store(false, Ordering::Relaxed);
        fs::write(&source, b"").unwrap();
        fs::write(&target, b"").unwrap();
        assert!(adapter.check(&a, &b, &source, &target, &cancel).is_err());
        assert!(trash.calls.lock().unwrap().is_empty());
    }
}
