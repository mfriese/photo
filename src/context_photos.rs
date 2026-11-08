//! Read-only photo dates and ImageIO thumbnails; no collection cache files.
use crate::domain::CaptureTime;
use photosuite_application::photo_context::ContextPhotoSource;
use std::{io::BufReader, path::Path};
pub struct PhotoContextAdapter;
impl ContextPhotoSource for PhotoContextAdapter {
    fn captured_at(&self, path: &Path) -> Result<CaptureTime, String> {
        let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("Kein reguläres Foto".into());
        }
        let original = (|| {
            let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            let exif = exif::Reader::new()
                .read_from_container(&mut BufReader::new(file))
                .map_err(|e| e.to_string())?;
            let field = exif
                .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
                .ok_or("DateTimeOriginal fehlt")?;
            let exif::Value::Ascii(values) = &field.value else {
                return Err("Aufnahmezeit ist kein Text".into());
            };
            let text = std::str::from_utf8(values.first().ok_or("Aufnahmezeit fehlt")?)
                .map_err(|e| e.to_string())?
                .trim_end_matches('\0');
            CaptureTime::parse(text, "EXIF DateTimeOriginal")
        })();
        original.or_else(|error| {
            CaptureTime::from_photo_filename(
                path.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
            )
            .ok_or(error)
        })
    }
    fn thumbnail(&self, path: &Path, edge: u32) -> Result<Vec<u8>, String> {
        let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("Kein reguläres Foto".into());
        }
        #[cfg(target_os = "macos")]
        {
            use std::{ffi::CString, os::unix::ffi::OsStrExt};
            unsafe extern "C" {
                fn photosuite_context_thumbnail(
                    path: *const std::ffi::c_char,
                    edge: u32,
                    bytes: *mut *mut u8,
                    length: *mut usize,
                ) -> i32;
                fn photosuite_free(bytes: *mut std::ffi::c_void);
            }
            let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
            let mut bytes = std::ptr::null_mut();
            let mut length = 0;
            let status = unsafe {
                photosuite_context_thumbnail(path.as_ptr(), edge, &mut bytes, &mut length)
            };
            if status != 0 {
                return Err(format!("Keine Fotovorschau ({status})"));
            }
            let png = unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec();
            unsafe {
                photosuite_free(bytes.cast());
            }
            Ok(png)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = edge;
            Err("Kontextfotos derzeit nur auf macOS verfügbar".into())
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn photo_dates_prefer_exif_and_oriented_thumbnails_preserve_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("img_20080916_035811.jpg");
        image::RgbImage::new(80, 40).save(&path).unwrap();
        assert_eq!(
            PhotoContextAdapter.captured_at(&path).unwrap().stamp(),
            "20080916_035811"
        );
        let status = crate::exiftool::ExifTool::development()
            .command()
            .args([
                "-overwrite_original",
                "-DateTimeOriginal=2023:12:27 22:51:32",
                "-Orientation#=6",
            ])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            PhotoContextAdapter.captured_at(&path).unwrap().stamp(),
            "20231227_225132"
        );
        let png = PhotoContextAdapter.thumbnail(&path, 32).unwrap();
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!((image.width(), image.height()), (16, 32));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let undated = dir.path().join("undated.jpg");
        image::RgbImage::new(20, 20).save(&undated).unwrap();
        assert!(PhotoContextAdapter.captured_at(&undated).is_err());
        let link = dir.path().join("link.jpg");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(PhotoContextAdapter.captured_at(&link).is_err());
        assert!(PhotoContextAdapter.thumbnail(&link, 32).is_err());
    }
}
