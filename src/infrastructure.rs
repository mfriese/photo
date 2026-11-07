use crate::domain::Media;
use crate::ports::MediaRepository;
use std::path::Path;
pub struct FileSystemRepository;
impl MediaRepository for FileSystemRepository {
    fn scan(&self, root: &Path) -> (Vec<Media>, usize) {
        let mut items = Vec::new();
        let mut errors = 0;
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let parent = path
                .parent()
                .unwrap_or(root)
                .strip_prefix(root)
                .unwrap_or(Path::new(""));
            let group = if parent.as_os_str().is_empty() {
                "Ohne Unterordner".into()
            } else {
                parent.to_string_lossy().into_owned()
            };
            if let Some(media) = Media::from_path(path.into(), group) {
                items.push(media);
            }
        }
        items.sort_by(|a, b| a.path().cmp(b.path()));
        (items, errors)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_filters_and_preserves_originals() {
        let dir = tempfile::tempdir().unwrap();
        let month = dir.path().join("2024/03");
        std::fs::create_dir_all(&month).unwrap();
        for name in ["a.JPG", "b.mov", "ignore.txt"] {
            std::fs::write(month.join(name), b"original").unwrap();
        }
        let (items, errors) = FileSystemRepository.scan(dir.path());
        assert_eq!(errors, 0);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].group(), "2024/03");
        assert!(items[1].is_video());
        assert_eq!(std::fs::read(month.join("a.JPG")).unwrap(), b"original");
    }
}

/// Decode pixels and apply EXIF orientation exactly once, then session rotation.
pub fn decode_oriented(path: &Path, turns: u8) -> image::ImageResult<image::DynamicImage> {
    use image::ImageDecoder;
    let mut decoder = image::ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut pixels = image::DynamicImage::from_decoder(decoder)?;
    pixels.apply_orientation(orientation);
    Ok(match turns % 4 {
        1 => pixels.rotate90(),
        2 => pixels.rotate180(),
        3 => pixels.rotate270(),
        _ => pixels,
    })
}

#[cfg(test)]
mod orientation_tests {
    use super::*;
    #[test]
    fn exif_rotation_and_manual_rotation_preserve_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("portrait.jpg");
        let pixels = image::RgbImage::from_pixel(6, 4, image::Rgb([255, 30, 0]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode_image(&pixels)
            .unwrap();
        // EXIF little-endian TIFF, orientation=6 (90 degrees clockwise).
        let exif: &[u8] =
            b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut fixture = jpeg[..2].to_vec();
        fixture.extend_from_slice(&[0xff, 0xe1]);
        fixture.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        fixture.extend_from_slice(exif);
        fixture.extend_from_slice(&jpeg[2..]);
        std::fs::write(&path, &fixture).unwrap();
        let oriented = decode_oriented(&path, 0).unwrap();
        assert_eq!((oriented.width(), oriented.height()), (4, 6));
        let manual = decode_oriented(&path, 1).unwrap();
        assert_eq!((manual.width(), manual.height()), (6, 4));
        assert_eq!(decode_oriented(&path, 4).unwrap(), oriented);
        assert_eq!(std::fs::read(path).unwrap(), fixture);
    }
}

pub struct RasterDecoder {
    pub videos: std::sync::Arc<dyn crate::ports::VideoPreviewSource>,
}
impl crate::ports::ImageDecoder for RasterDecoder {
    fn decode(&self, path: &Path, turns: u8) -> Result<crate::ports::DecodedImage, String> {
        if crate::domain::is_video(path) {
            return self.videos.frame_at(path, 0.).map(|frame| frame.image);
        }
        let rgba = decode_oriented(path, turns)
            .map_err(|e| e.to_string())?
            .to_rgba8();
        Ok(crate::ports::DecodedImage {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        })
    }
}
pub struct SystemMediaOpener;
impl crate::ports::MediaOpener for SystemMediaOpener {
    fn open(&self, path: &Path) -> Result<(), String> {
        open::that(path).map_err(|e| e.to_string())
    }
}

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
