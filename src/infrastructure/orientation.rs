//! Explicit JPEG EXIF orientation writing; no automatic bulk changes.
use std::path::Path;
/// Metadata-only JPEG writes: no pixel recompression, ExifTool updates the original without a retained backup.
pub struct ExifOrientationWriter {
    pub tool: crate::exiftool::ExifTool,
}
impl crate::ports::OrientationWriter for ExifOrientationWriter {
    fn write_rotation(&self, path: &Path, turns: u8) -> Result<(), String> {
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        if !matches!(ext.as_str(), "jpg" | "jpeg") {
            return Err("EXIF-Schreiben wird derzeit nur für JPEG unterstützt.".into());
        }
        if std::fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Symbolische Links werden nicht verändert.".into());
        }
        let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        let output = self
            .tool
            .command()
            .args(["-s3", "-n", "-IFD0:Orientation"])
            .arg(&path)
            .output()
            .map_err(|e| format!("ExifTool benötigt: {e}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let original: u8 = if text.trim().is_empty() {
            1
        } else {
            text.trim()
                .parse()
                .map_err(|_| "Ungültige EXIF-Orientierung")?
        };
        if !(1..=8).contains(&original) {
            return Err("Ungültige EXIF-Orientierung".into());
        }
        let mut orientation = original;
        for _ in 0..turns % 4 {
            orientation = [0, 6, 7, 8, 5, 2, 3, 4, 1][orientation as usize];
        }
        if orientation == original {
            return Ok(());
        }
        let output = self
            .tool
            .command()
            .args(["-P", "-n", "-overwrite_original_in_place"])
            .arg(format!("-IFD0:Orientation={orientation}"))
            .arg(&path)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod write_tests {
    use super::super::images::decode_oriented;
    use super::*;
    use crate::ports::OrientationWriter;
    fn writer() -> ExifOrientationWriter {
        ExifOrientationWriter {
            tool: crate::exiftool::ExifTool::development(),
        }
    }
    #[test]
    fn metadata_rotation_keeps_pixels_without_backup() {
        // Explicit integration test; missing tool is a failure, not a silent skip.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jpg");
        image::RgbImage::from_pixel(8, 4, image::Rgb([20, 100, 200]))
            .save(&path)
            .unwrap();

        let pixels = image::open(&path).unwrap();
        writer().write_rotation(&path, 1).unwrap();
        assert_eq!(image::open(&path).unwrap(), pixels);
        assert!(!dir.path().join("test.jpg_original").exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(
            (
                decode_oriented(&path, 0).unwrap().width(),
                decode_oriented(&path, 0).unwrap().height()
            ),
            (4, 8)
        );
        writer().write_rotation(&path, 3).unwrap();
        assert_eq!(decode_oriented(&path, 0).unwrap(), pixels);
    }
    #[test]
    fn unsupported_format_is_never_modified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.png");
        std::fs::write(&path, b"unchanged").unwrap();
        assert!(writer().write_rotation(&path, 1).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"unchanged");
    }
}
