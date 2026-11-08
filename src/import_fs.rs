//! Filesystem and metadata adapters for the import service.
use crate::{
    domain::{CaptureTime, Media},
    import::{CopyProgress, ImportAdapter},
    ports::MediaRepository,
};
use std::{
    fs::{self, File},
    io::{BufReader, Read, Seek, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
pub struct FileImportAdapter {
    pub audio: std::sync::Arc<dyn photosuite_application::diary::AudioSource>,
    pub tool: crate::exiftool::ExifTool,
    pub repository: std::sync::Arc<dyn MediaRepository>,
}
fn regular(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(format!("Keine reguläre Datei: {}", path.display()));
    }
    Ok(())
}
fn compare(a: &Path, b: &Path) -> Result<bool, String> {
    regular(a)?;
    regular(b)?;
    if fs::metadata(a).map_err(|e| e.to_string())?.len()
        != fs::metadata(b).map_err(|e| e.to_string())?.len()
    {
        return Ok(false);
    }
    let mut a = BufReader::new(File::open(a).map_err(|e| e.to_string())?);
    let mut b = BufReader::new(File::open(b).map_err(|e| e.to_string())?);
    let (mut x, mut y) = ([0u8; 65536], [0u8; 65536]);
    loop {
        let count = a.read(&mut x).map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(b.read(&mut y).map_err(|e| e.to_string())? == 0);
        }
        b.read_exact(&mut y[..count]).map_err(|e| e.to_string())?;
        if x[..count] != y[..count] {
            return Ok(false);
        }
    }
}
/// Automatic tempfile drop is best-effort only. Always report a failed cleanup explicitly.
fn cleanup_staging(
    staged: tempfile::NamedTempFile,
    result: Result<(), String>,
) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let (file, mut temporary_path) = staged.into_parts();
    let path = temporary_path.to_path_buf();
    // Do not let best-effort Drop retry removal of an unverified/replaced path.
    temporary_path.disable_cleanup(true);
    let identity = file.metadata();
    drop(file); // Close before unlinking, including on removable filesystems.
    let cleanup = (|| {
        let current = match fs::symlink_metadata(&path) {
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
        fs::remove_file(&path).map_err(|e| e.to_string())
    })();
    match cleanup {
        Ok(()) => result,
        Err(error) => {
            let context = match result {
                Ok(()) => "Zieldatei wurde vollständig geprüft. ".to_owned(),
                Err(message) => format!("{message}. "),
            };
            Err(format!(
                "{context}Temporäre Importdatei konnte nicht entfernt werden: {}: {error}",
                path.display()
            ))
        }
    }
}
/// Preserve no-overwrite semantics even when rename-no-replace and hardlinks are unavailable.
/// The portable path is deliberately not an ordinary rename: that could replace an existing file.
fn finish_publication(
    result: Result<File, tempfile::PersistError>,
    source: &Path,
    target: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    match result {
        Ok(_) => Ok(()),
        Err(error) if publication_unsupported(&error.error) => {
            publish_portable(error.file, source, target, cancel)
        }
        Err(error) => {
            let message = format!(
                "Zieldatei konnte nicht veröffentlicht werden (vorhandene Ziele werden nicht überschrieben): {}",
                error.error
            );
            cleanup_staging(error.file, Err(message))
        }
    }
}
fn publication_unsupported(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::Unsupported || matches!(error.raw_os_error(), Some(45 | 95))
}
fn publish_portable(
    staged: tempfile::NamedTempFile,
    source: &Path,
    target: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let mut staged = staged;
    let result = write_portable(&mut staged, source, target, cancel);
    cleanup_staging(staged, result)
}
fn write_portable(
    staged: &mut tempfile::NamedTempFile,
    source: &Path,
    target: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    if cancel.load(Ordering::Relaxed) {
        return Err("Abgebrochen".into());
    }
    staged.as_file_mut().rewind().map_err(|e| e.to_string())?;
    // O_EXCL also rejects existing files, directories and symlinks without touching them.
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| {
            format!("Zieldatei konnte nicht neu angelegt werden (kein Überschreiben): {e}")
        })?;
    let identity = output.metadata().map_err(|e| format!("Import abgebrochen; neu angelegte Zieldatei konnte nicht sicher identifiziert werden und bleibt erhalten: {}: {e}", target.display()))?;
    let result = (|| {
        let mut buffer = vec![0; 1024 * 1024];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("Abgebrochen".into());
            }
            let count = staged.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            output
                .write_all(&buffer[..count])
                .map_err(|e| e.to_string())?;
        }
        output.sync_all().map_err(|e| e.to_string())?;
        if !compare(staged.path(), target)? || !compare(source, target)? {
            return Err("Veröffentlichte Kopie stimmt nicht mit Quelle überein".into());
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Abgebrochen".into());
        }
        Ok(())
    })();
    if result.is_err() {
        // Remove only the destination created by this attempt, never a replaced foreign file.
        if fs::symlink_metadata(target).is_ok_and(|m| {
            !m.file_type().is_symlink() && m.dev() == identity.dev() && m.ino() == identity.ino()
        }) {
            fs::remove_file(target).map_err(|e| format!("Import fehlgeschlagen; eigene unvollständige Zieldatei konnte nicht entfernt werden: {}: {e}", target.display()))?;
        }
    }
    result
}

impl ImportAdapter for FileImportAdapter {
    fn validate(&self, source: &Path, target: &Path) -> Result<(), String> {
        let source = fs::canonicalize(source).map_err(|e| e.to_string())?;
        let target = fs::canonicalize(target).map_err(|e| e.to_string())?;
        if !source.is_dir() || !target.is_dir() {
            return Err("Quelle und Bibliothek müssen Verzeichnisse sein".into());
        }
        if source.starts_with(&target) || target.starts_with(&source) {
            return Err("Quelle und Bibliothek dürfen sich nicht überschneiden".into());
        }
        Ok(())
    }
    fn discover(&self, source: &Path) -> Result<Vec<Media>, String> {
        let (items, errors) = self.repository.scan(source);
        if errors > 0 {
            return Err(format!(
                "Scan unvollständig: {errors} nicht lesbare Einträge"
            ));
        }
        Ok(items)
    }
    fn capture_time(&self, media: &Media) -> Result<CaptureTime, String> {
        regular(media.path())?;
        if media.is_audio() {
            return self.audio.capture_time(media.path());
        }
        if media.is_video() {
            let output = self
                .tool
                .command()
                .args(["-s3", "-d", "%Y:%m:%d %H:%M:%S", "-QuickTime:CreateDate"])
                .arg(fs::canonicalize(media.path()).map_err(|e| e.to_string())?)
                .output()
                .map_err(|e| format!("Video-Metadaten benötigen ExifTool: {e}"))?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).into_owned());
            }
            return CaptureTime::parse(
                String::from_utf8_lossy(&output.stdout).trim(),
                "QuickTime CreateDate (ohne Zeitzonenänderung)",
            );
        }
        let file = File::open(media.path()).map_err(|e| e.to_string())?;
        let exif = exif::Reader::new()
            .read_from_container(&mut BufReader::new(file))
            .map_err(|e| format!("EXIF nicht lesbar: {e}"))?;
        let field = exif
            .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
            .ok_or("EXIF DateTimeOriginal fehlt")?;
        let exif::Value::Ascii(values) = &field.value else {
            return Err("DateTimeOriginal ist kein Text".into());
        };
        let value = values.first().ok_or("DateTimeOriginal leer")?;
        CaptureTime::parse(
            std::str::from_utf8(value)
                .map_err(|e| e.to_string())?
                .trim_end_matches('\0'),
            "EXIF DateTimeOriginal",
        )
    }
    fn exists(&self, path: &Path) -> Result<bool, String> {
        for dir in [path.parent(), path.parent().and_then(Path::parent)]
            .into_iter()
            .flatten()
        {
            match fs::symlink_metadata(dir) {
                Ok(m) if m.file_type().is_symlink() || !m.is_dir() => {
                    return Err("Zielpfad enthält einen Link oder ungültigen Ordner".into());
                }
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
                _ => {}
            }
        }

        match fs::symlink_metadata(path) {
            Ok(_) => {
                regular(path)?;
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
    fn identical(&self, source: &Path, target: &Path) -> Result<bool, String> {
        compare(source, target)
    }
    fn copy_verified(
        &self,
        source: &Path,
        target: &Path,
        cancel: &AtomicBool,
        progress: &dyn Fn(CopyProgress),
    ) -> Result<(), String> {
        regular(source)?;
        let parent = target.parent().ok_or("Ziel ohne Verzeichnis")?;
        // Only two generated components (year/month) are created; reject symlink escapes.
        let year = parent.parent().ok_or("Jahr fehlt")?;
        for dir in [year, parent] {
            match fs::create_dir(dir) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
            let m = fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
            if m.file_type().is_symlink() || !m.is_dir() {
                return Err("Zielordner ist kein reguläres Verzeichnis".into());
            }
        }
        let mut input = File::open(source).map_err(|e| e.to_string())?;
        let length = input.metadata().map_err(|e| e.to_string())?.len();
        let mut temp = tempfile::Builder::new()
            .prefix(".photosuite-import-")
            .suffix(".tmp")
            .tempfile_in(parent)
            .map_err(|e| e.to_string())?;
        let prepared = (|| {
            let mut buffer = vec![0; 1024 * 1024];
            let mut copied = 0;
            let mut reported = 0;
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Abgebrochen".into());
                }
                let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                temp.write_all(&buffer[..count])
                    .map_err(|e| e.to_string())?;
                copied += count as u64;
                if copied - reported >= 8 * 1024 * 1024 {
                    progress(CopyProgress::Bytes(copied, length));
                    reported = copied;
                }
            }
            temp.as_file().sync_all().map_err(|e| e.to_string())?;
            progress(CopyProgress::Bytes(copied, length));
            progress(CopyProgress::Verify);
            if copied != length || !compare(source, temp.path())? {
                return Err("Kopie stimmt nicht mit Quelle überein".into());
            }
            if cancel.load(Ordering::Relaxed) {
                return Err("Abgebrochen".into());
            }
            progress(CopyProgress::Publish);
            Ok(())
        })();
        if let Err(error) = prepared {
            return cleanup_staging(temp, Err(error));
        }
        finish_publication(temp.persist_noclobber(target), source, target, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import;
    use std::sync::Arc;
    fn adapter() -> FileImportAdapter {
        let audio = Arc::new(crate::audio::NativeAudioSource {
            tool: crate::exiftool::ExifTool::development(),
        });
        FileImportAdapter {
            audio: audio.clone(),
            tool: crate::exiftool::ExifTool::development(),
            repository: Arc::new(crate::infrastructure::FileSystemRepository {
                audio,
                priority: Default::default(),
            }),
        }
    }
    fn photo(path: &Path, color: u8) {
        image::RgbImage::from_pixel(8, 4, image::Rgb([color, 30, 90]))
            .save(path)
            .unwrap();
        assert!(
            crate::exiftool::ExifTool::development()
                .command()
                .args([
                    "-overwrite_original",
                    "-DateTimeOriginal=2008:09:16 03:58:11"
                ])
                .arg(path)
                .status()
                .unwrap()
                .success()
        );
    }
    fn run(source: &Path, target: &Path, preview: bool) -> Vec<import::Event> {
        let job = import::start(Arc::new(adapter()), source.into(), target.into(), preview);
        let mut events = Vec::new();
        loop {
            let event = job
                .events
                .recv_timeout(std::time::Duration::from_secs(20))
                .unwrap();
            let done = event.is_terminal();
            events.push(event);
            if done {
                break;
            }
        }
        events
    }
    #[test]
    fn import_exif_paths_duplicates_collision_and_repeat() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        photo(&src.path().join("a.jpg"), 10);
        fs::copy(src.path().join("a.jpg"), src.path().join("b.jpg")).unwrap();
        photo(&src.path().join("c.jpg"), 220);
        let original = fs::read(src.path().join("a.jpg")).unwrap();
        let plan = run(src.path(), dst.path(), true);
        assert!(
            plan.last()
                .unwrap()
                .message
                .contains("2 geplant, 1 übersprungen")
        );
        assert_eq!(fs::read_dir(dst.path()).unwrap().count(), 0);
        let events = run(src.path(), dst.path(), false);
        assert!(
            events
                .last()
                .unwrap()
                .message
                .contains("2 kopiert, 1 übersprungen, 0 Fehler")
        );
        let base = dst.path().join("2008/09/img_20080916_035811.jpg");
        assert_eq!(fs::read(&base).unwrap(), original);
        assert!(
            dst.path()
                .join("2008/09/img_20080916_035811_001.jpg")
                .exists()
        );
        assert_eq!(fs::read(src.path().join("a.jpg")).unwrap(), original);
        assert!(
            run(src.path(), dst.path(), false)
                .last()
                .unwrap()
                .message
                .contains("0 kopiert, 3 übersprungen")
        );
        assert_eq!(fs::read_dir(base.parent().unwrap()).unwrap().count(), 2);
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn audio_import_plans_year_diary_compares_collisions_and_preserves_source() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        crate::audio::test_wave(&source.path().join("a.wav"), 1, true);
        fs::copy(source.path().join("a.wav"), source.path().join("b.wav")).unwrap();
        crate::audio::test_wave(&source.path().join("c.wav"), 2, true);
        let originals: Vec<_> = ["a.wav", "b.wav", "c.wav"]
            .into_iter()
            .map(|name| (name, fs::read(source.path().join(name)).unwrap()))
            .collect();
        let before = fs::read(source.path().join("a.wav")).unwrap();
        let plan = run(source.path(), target.path(), true);
        assert!(
            plan.last()
                .unwrap()
                .message
                .contains("2 geplant, 1 übersprungen"),
            "{:?}",
            plan.last()
        );
        assert!(
            plan.iter()
                .any(|e| e.message.contains("RIFF:DateTimeOriginal"))
        );
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
        let events = run(source.path(), target.path(), false);
        assert!(
            events
                .last()
                .unwrap()
                .message
                .contains("2 kopiert, 1 übersprungen, 0 Fehler")
        );
        let first = target.path().join("2008/diary/aud_20080916_035811.wav");
        assert_eq!(fs::read(first).unwrap(), before);
        assert!(
            target
                .path()
                .join("2008/diary/aud_20080916_035811_001.wav")
                .exists()
        );
        assert_eq!(fs::read(source.path().join("a.wav")).unwrap(), before);
        for (name, original) in originals {
            assert_eq!(
                fs::read(source.path().join(name)).unwrap(),
                original,
                "Quelle darf niemals gelöscht oder verändert werden"
            );
        }
        assert_eq!(fs::read_dir(source.path()).unwrap().count(), 3);
        assert!(
            run(source.path(), target.path(), false)
                .last()
                .unwrap()
                .message
                .contains("0 kopiert, 3 übersprungen")
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn undated_audio_import_name_preserves_selected_date_after_copy() {
        use photosuite_application::diary::AudioSource;
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let path = source.path().join("aud_20080916_035811.wav");
        crate::audio::test_wave(&path, 1, false);
        let events = run(source.path(), target.path(), false);
        assert!(
            events
                .last()
                .unwrap()
                .message
                .contains("1 kopiert, 0 übersprungen, 0 Fehler")
        );
        let imported = target.path().join("2008/diary/aud_20080916_035811.wav");
        let date = crate::audio::NativeAudioSource {
            tool: crate::exiftool::ExifTool::development(),
        }
        .capture_time(&imported)
        .unwrap();
        assert_eq!(date.stamp(), "20080916_035811");
        assert!(date.origin().contains("Importdateiname"));
        assert!(path.exists());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn recorder_filename_is_used_by_preview_and_import_without_changing_source() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let path = source.path().join("2023-12-27_22-51-32.wav");
        crate::audio::test_wave(&path, 1, false);
        let original = fs::read(&path).unwrap();
        let preview = run(source.path(), target.path(), true);
        assert!(
            preview
                .iter()
                .any(|event| event.message.contains("FALLBACK Audio-Dateiname"))
        );
        assert!(
            preview
                .iter()
                .any(|event| event.message.contains("2023/diary/aud_20231227_225132.wav"))
        );
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
        let result = run(source.path(), target.path(), false);
        assert!(
            result
                .last()
                .unwrap()
                .message
                .contains("1 kopiert, 0 übersprungen, 0 Fehler")
        );
        assert_eq!(
            fs::read(target.path().join("2023/diary/aud_20231227_225132.wav")).unwrap(),
            original
        );
        assert_eq!(fs::read(path).unwrap(), original);
    }
    #[test]
    fn missing_exif_does_not_use_filesystem_date() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        image::RgbImage::new(2, 2)
            .save(src.path().join("no-date.jpg"))
            .unwrap();
        let events = run(src.path(), dst.path(), false);
        assert!(
            events
                .last()
                .unwrap()
                .message
                .contains("0 kopiert, 0 übersprungen, 1 Fehler")
        );
        assert_eq!(fs::read_dir(dst.path()).unwrap().count(), 0);
    }
    #[test]
    fn video_name_collision_skips_even_different_contents() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/preview.mp4"),
            src.path().join("a.mp4"),
        )
        .unwrap();
        let folder = dst.path().join("2008/09");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("img_20080916_035811.mp4"),
            b"different video bytes",
        )
        .unwrap();
        let events = run(src.path(), dst.path(), false);
        assert!(
            events
                .last()
                .unwrap()
                .message
                .contains("0 kopiert, 1 übersprungen, 0 Fehler"),
            "{:?}",
            events.last()
        );
    }
    #[test]
    fn overlap_cancel_and_no_clobber() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        assert!(adapter().validate(src.path(), src.path()).is_err());
        fs::write(src.path().join("a.jpg"), b"source").unwrap();
        let target = dst.path().join("2024/01/a.jpg");
        let cancel = AtomicBool::new(true);
        assert!(
            adapter()
                .copy_verified(&src.path().join("a.jpg"), &target, &cancel, &|_| {})
                .is_err()
        );
        assert!(!target.exists());
        fs::write(&target, b"existing").unwrap();
        assert!(
            adapter()
                .copy_verified(
                    &src.path().join("a.jpg"),
                    &target,
                    &AtomicBool::new(false),
                    &|_| {}
                )
                .is_err()
        );
        assert_eq!(fs::read(target).unwrap(), b"existing");
    }
    #[cfg(unix)]
    #[test]
    fn destination_symlinks_are_rejected() {
        let dst = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dst.path().join("2008")).unwrap();
        assert!(adapter().exists(&dst.path().join("2008/09/a.jpg")).is_err());
    }
}

#[cfg(test)]
mod portable_publication_tests {
    use super::*;
    fn staged(parent: &Path, bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new()
            .prefix(".photosuite-import-")
            .suffix(".tmp")
            .tempfile_in(parent)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.as_file().sync_all().unwrap();
        file
    }
    fn unsupported(file: tempfile::NamedTempFile) -> Result<File, tempfile::PersistError> {
        Err(tempfile::PersistError {
            file,
            error: std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "simulated exFAT unsupported publication",
            ),
        })
    }
    #[test]
    fn cleanup_failure_is_visible_and_replaced_files_are_never_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let temp = staged(dir.path(), b"temporary");
        let path = temp.path().to_owned();
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"foreign replacement").unwrap();
        let error = cleanup_staging(temp, Ok(())).unwrap_err();
        assert!(error.contains("Zieldatei wurde vollständig geprüft"));
        assert!(error.contains("Temporäre Importdatei konnte nicht entfernt werden"));
        assert!(error.contains(path.to_str().unwrap()));
        assert_eq!(fs::read(&path).unwrap(), b"foreign replacement");
    }
    #[test]
    fn cleanup_retains_primary_error_and_already_absent_staging_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let temp = staged(dir.path(), b"temporary");
        let path = temp.path().to_owned();
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        let error = cleanup_staging(temp, Err("Kopie fehlgeschlagen".into())).unwrap_err();
        assert!(error.contains("Kopie fehlgeschlagen"));
        assert!(error.contains("nicht entfernt"));
        assert!(path.is_dir());
        let temp = staged(dir.path(), b"temporary");
        fs::remove_file(temp.path()).unwrap();
        cleanup_staging(temp, Ok(())).unwrap();
    }
    #[test]
    fn unsupported_publication_copies_and_verifies_without_changing_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.wav");
        let target = dir.path().join("target.wav");
        let bytes = vec![37; 2 * 1024 * 1024 + 17];
        fs::write(&source, &bytes).unwrap();
        let temp = staged(dir.path(), &bytes);
        let temp_path = temp.path().to_owned();
        finish_publication(unsupported(temp), &source, &target, &AtomicBool::new(false)).unwrap();
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(fs::read(&target).unwrap(), bytes);
        assert!(!temp_path.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        #[cfg(target_os = "macos")]
        assert!(publication_unsupported(&std::io::Error::from_raw_os_error(
            45
        )));
    }
    #[test]
    fn portable_publication_never_overwrites_a_colliding_file_or_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.mp3");
        let target = dir.path().join("target.mp3");
        fs::write(&source, b"source").unwrap();
        fs::write(&target, b"existing").unwrap();
        assert!(
            finish_publication(
                unsupported(staged(dir.path(), b"source")),
                &source,
                &target,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert_eq!(fs::read(&target).unwrap(), b"existing");
        let link = dir.path().join("link.mp3");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(
            finish_publication(
                unsupported(staged(dir.path(), b"source")),
                &source,
                &link,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&target).unwrap(), b"existing");
        assert_eq!(fs::read(source).unwrap(), b"source");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
    }
    #[test]
    fn changed_source_invalidates_portable_copy_and_removes_only_own_destination() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.wav");
        let target = dir.path().join("target.wav");
        fs::write(&source, b"changed-source").unwrap();
        assert!(
            finish_publication(
                unsupported(staged(dir.path(), b"old-source")),
                &source,
                &target,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(!target.exists());
        assert_eq!(fs::read(source).unwrap(), b"changed-source");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn cancelled_portable_publication_leaves_source_and_no_destination_or_temp() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.m4a");
        let target = dir.path().join("target.m4a");
        fs::write(&source, b"source").unwrap();
        assert!(
            finish_publication(
                unsupported(staged(dir.path(), b"source")),
                &source,
                &target,
                &AtomicBool::new(true)
            )
            .is_err()
        );
        assert!(!target.exists());
        assert_eq!(fs::read(source).unwrap(), b"source");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn other_publication_errors_are_not_masked_by_a_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.wav");
        let target = dir.path().join("target.wav");
        fs::write(&source, b"source").unwrap();
        let error = tempfile::PersistError {
            file: staged(dir.path(), b"source"),
            error: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        };
        let result = finish_publication(Err(error), &source, &target, &AtomicBool::new(false));
        assert!(result.unwrap_err().contains("denied"));
        assert!(!target.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
