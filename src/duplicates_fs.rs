//! Conservative full-resolution pixel comparison; never decides from hashes.
use photosuite_application::duplicates::{Comparison, DuplicateImages};
use std::{fs, path::Path};
pub struct PixelDuplicates;
fn read_regular(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Nur reguläre Dateien ohne symbolische Links werden geprüft".into());
    }
    if metadata.len() > 256 * 1024 * 1024 {
        return Err("Datei überschreitet das Vergleichslimit von 256 MiB".into());
    }
    fs::read(path).map_err(|e| e.to_string())
}
struct Pixels {
    width: u32,
    height: u32,
    data: Vec<u16>,
    profile: Option<Vec<u8>>,
    color: String,
    png_color: Vec<u8>,
    exif_color: Option<u32>,
}
fn decode(bytes: &[u8]) -> Result<Pixels, String> {
    use image::ImageDecoder;
    let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
    if !matches!(format, image::ImageFormat::Jpeg | image::ImageFormat::Png) {
        return Err("Für dieses Format ist kein sicherer vollständiger Einzelbildvergleich freigegeben (nur JPEG/PNG). Datei bleibt erhalten.".into());
    }
    if format == image::ImageFormat::Jpeg && bytes.windows(4).any(|w| w == b"MPF\0") {
        return Err("JPEG mit Mehrbild-Metadaten: Datei bleibt erhalten".into());
    }
    let mut png_color = Vec::new();
    if format == image::ImageFormat::Png {
        let mut offset = 8usize;
        while offset + 12 <= bytes.len() {
            let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            let end = offset
                .checked_add(length)
                .and_then(|n| n.checked_add(12))
                .filter(|&n| n <= bytes.len())
                .ok_or("Ungültiger PNG-Chunk")?;
            let kind = &bytes[offset + 4..offset + 8];
            if kind == b"acTL" {
                return Err("Animiertes PNG: Mehrbilddatei bleibt erhalten".into());
            }
            if [b"gAMA", b"cHRM", b"sRGB", b"cICP"]
                .iter()
                .any(|k| kind == *k)
            {
                png_color.extend_from_slice(&bytes[offset + 4..end - 4]);
            }
            offset = end;
        }
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > 32_000_000 {
        return Err("Bild überschreitet das Vergleichslimit von 32 Megapixeln".into());
    }
    let color = format!("{:?}", decoder.original_color_type());
    let profile = decoder.icc_profile().map_err(|e| e.to_string())?;
    let metadata = decoder.exif_metadata().map_err(|e| e.to_string())?;
    let exif_color = if let Some(raw) = metadata {
        let exif = exif::Reader::new()
            .read_raw(raw)
            .map_err(|e| format!("Ungültige EXIF-Daten: {e}"))?;
        if let Some(orientation) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
            && !orientation
                .value
                .get_uint(0)
                .is_some_and(|v| (1..=8).contains(&v))
        {
            return Err("Ungültige EXIF-Orientierung: keine sichere Duplikatfreigabe".into());
        }
        exif.get_field(exif::Tag::ColorSpace, exif::In::PRIMARY)
            .and_then(|f| f.value.get_uint(0))
    } else {
        None
    };
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    let pixels = image.to_rgba16();
    Ok(Pixels {
        width: pixels.width(),
        height: pixels.height(),
        data: pixels.into_raw(),
        profile,
        color,
        png_color,
        exif_color,
    })
}
fn compare_bytes(a: &[u8], b: &[u8]) -> Result<Comparison, String> {
    let a = decode(a)?;
    let b = decode(b)?;
    if (a.width, a.height) != (b.width, b.height) {
        return Ok(Comparison::Different(format!(
            "Unterschiedliche Abmessungen: {}×{} / {}×{}",
            a.width, a.height, b.width, b.height
        )));
    }
    if a.profile != b.profile
        || a.color != b.color
        || a.png_color != b.png_color
        || a.exif_color != b.exif_color
    {
        return Ok(Comparison::Uncertain(
            "Farbprofil, Farbraum oder Farbtiefe weicht ab; keine Freigabe als exaktes Duplikat"
                .into(),
        ));
    }
    let changed = a
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.data.as_chunks::<4>().0.iter())
        .filter(|(x, y)| x != y)
        .count();
    if changed == 0 {
        Ok(Comparison::Identical)
    } else {
        Ok(Comparison::Different(format!(
            "{changed} von {} Pixeln unterscheiden sich",
            u64::from(a.width) * u64::from(a.height)
        )))
    }
}
impl DuplicateImages for PixelDuplicates {
    fn compare(&self, a: &Path, b: &Path) -> Result<Comparison, String> {
        compare_bytes(&read_regular(a)?, &read_regular(b)?)
    }
    fn trash_identical(
        &self,
        library: &Path,
        keeper: &Path,
        candidate: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), String> {
        let root = fs::canonicalize(library).map_err(|e| e.to_string())?;
        let keep = fs::canonicalize(keeper).map_err(|e| e.to_string())?;
        let target = fs::canonicalize(candidate).map_err(|e| e.to_string())?;
        if keep == target || !keep.starts_with(&root) || !target.starts_with(&root) {
            return Err(
                "Dateien müssen verschieden sein und innerhalb der gewählten Bibliothek liegen"
                    .into(),
            );
        }
        for file in [keeper, candidate] {
            let mut parent = file.parent();
            while let Some(p) = parent {
                if fs::symlink_metadata(p)
                    .map_err(|e| e.to_string())?
                    .file_type()
                    .is_symlink()
                {
                    return Err("Symbolische Zielpfade werden nicht verändert".into());
                }
                if p == library {
                    break;
                }
                parent = p.parent();
            }
        }
        let a = read_regular(keeper)?;
        let b = read_regular(candidate)?;
        if compare_bytes(&a, &b)? != Comparison::Identical {
            return Err("Erneute Pixelprüfung: Dateien sind nicht zweifelsfrei identisch".into());
        }
        if read_regular(keeper)? != a || read_regular(candidate)? != b {
            return Err("Datei wurde während der Prüfung geändert".into());
        }
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Abgebrochen; Datei bleibt erhalten".into());
        }
        trash(candidate)
    }
}
#[cfg(target_os = "macos")]
fn trash(path: &Path) -> Result<(), String> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    unsafe extern "C" {
        fn photosuite_trash_file(path: *const std::ffi::c_char) -> i32;
    }
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    if unsafe { photosuite_trash_file(path.as_ptr()) } == 0 {
        Ok(())
    } else {
        Err("Datei konnte nicht in den macOS-Papierkorb verschoben werden".into())
    }
}
#[cfg(not(target_os = "macos"))]
fn trash(_: &Path) -> Result<(), String> {
    Err("Papierkorb derzeit nur auf macOS verfügbar".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn exif(path: &Path, tag: &str) {
        assert!(
            std::process::Command::new("exiftool")
                .arg("-overwrite_original")
                .arg(tag)
                .arg(path)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    #[test]
    fn same_jpeg_pixels_despite_different_metadata() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a.jpg");
        let b = d.path().join("b.jpg");
        image::RgbImage::from_pixel(12, 8, image::Rgb([4, 140, 35]))
            .save(&a)
            .unwrap();
        exif(&a, "-ColorSpace#=1");
        fs::copy(&a, &b).unwrap();
        exif(&b, "-DateTimeOriginal=2008:09:16 03:58:11");
        assert_ne!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
        assert_eq!(
            PixelDuplicates.compare(&a, &b).unwrap(),
            Comparison::Identical
        );
    }
    #[test]
    fn exif_rotation_is_applied_to_full_pixels() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a.png");
        let b = d.path().join("b.png");
        let pixels = image::RgbImage::from_fn(3, 2, |x, y| {
            image::Rgb([(x * 60) as u8, (y * 70) as u8, 20])
        });
        pixels.save(&a).unwrap();
        image::imageops::rotate90(&pixels).save(&b).unwrap();
        exif(&b, "-Orientation#=8");
        assert_eq!(
            PixelDuplicates.compare(&a, &b).unwrap(),
            Comparison::Identical
        );
    }
    #[test]
    fn sixteen_bit_low_order_difference_is_not_lost() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a.png");
        let b = d.path().join("b.png");
        image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::from_pixel(
            2,
            2,
            image::Rgb([1, 300, 400]),
        )
        .save(&a)
        .unwrap();
        image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::from_pixel(
            2,
            2,
            image::Rgb([2, 300, 400]),
        )
        .save(&b)
        .unwrap();
        assert!(matches!(
            PixelDuplicates.compare(&a, &b).unwrap(),
            Comparison::Different(_)
        ));
        assert!(
            PixelDuplicates
                .trash_identical(d.path(), &a, &b, &std::sync::atomic::AtomicBool::new(false))
                .is_err()
        );
        assert!(a.exists() && b.exists());
    }
    #[test]
    fn damaged_or_multiframe_format_never_qualifies() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a.gif");
        image::RgbImage::new(2, 2).save(&a).unwrap();
        assert!(PixelDuplicates.compare(&a, &a).is_err());
        fs::write(&a, b"broken").unwrap();
        assert!(PixelDuplicates.compare(&a, &a).is_err());
    }
    #[test]
    fn different_png_gamma_is_uncertain() {
        fn gamma(bytes: &[u8], value: u32) -> Vec<u8> {
            let mut body = b"gAMA".to_vec();
            body.extend_from_slice(&value.to_be_bytes());
            let mut crc = 0xffffffffu32;
            for b in &body {
                crc ^= *b as u32;
                for _ in 0..8 {
                    crc = if crc & 1 != 0 {
                        (crc >> 1) ^ 0xedb88320
                    } else {
                        crc >> 1
                    };
                }
            }
            let mut result = bytes[..33].to_vec();
            result.extend_from_slice(&4u32.to_be_bytes());
            result.extend_from_slice(&body);
            result.extend_from_slice(&(!crc).to_be_bytes());
            result.extend_from_slice(&bytes[33..]);
            result
        }
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("a.png");
        image::RgbImage::new(2, 2).save(&path).unwrap();
        let bytes = fs::read(path).unwrap();
        assert!(matches!(
            compare_bytes(&gamma(&bytes, 45455), &gamma(&bytes, 100000)).unwrap(),
            Comparison::Uncertain(_)
        ));
    }
}
