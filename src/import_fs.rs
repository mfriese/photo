//! Filesystem and metadata adapters for the import service.
use crate::{
    domain::{CaptureTime, Media},
    import::{CopyProgress, ImportAdapter},
    ports::MediaRepository,
};
use std::{
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
pub struct FileImportAdapter {
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
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let mut buffer = vec![0; 1024 * 1024];
        let mut copied = 0;
        let mut reported = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("Abgebrochen; temporäre Kopie entfernt".into());
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
            return Err("Abgebrochen; temporäre Kopie entfernt".into());
        }
        progress(CopyProgress::Publish);
        temp.persist_noclobber(target).map_err(|e| {
            format!("Zieldatei nicht veröffentlicht (womöglich neue Kollision): {e}")
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import;
    use std::sync::Arc;
    fn adapter() -> FileImportAdapter {
        FileImportAdapter {
            tool: crate::exiftool::ExifTool::development(),
            repository: Arc::new(crate::infrastructure::FileSystemRepository),
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
