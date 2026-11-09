//! Raster/video preview decoding, sizing and EXIF orientation.
use std::path::Path;
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
    pub priority: std::sync::Arc<photosuite_application::work_priority::WorkPriority>,
    pub videos: std::sync::Arc<dyn crate::ports::VideoPreviewSource>,
}
impl crate::ports::ImageDecoder for RasterDecoder {
    fn decode_preview(
        &self,
        path: &Path,
        turns: u8,
        max_edge: u32,
    ) -> Result<crate::ports::DecodedImage, String> {
        self.priority
            .wait_background(&std::sync::atomic::AtomicBool::new(false));
        if !(1..=4096).contains(&max_edge) {
            return Err("Ungültige Vorschauauflösung".into());
        }
        let pixels = if crate::domain::is_video(path) {
            let frame = self.videos.frame_at(path, 0.)?;
            image::DynamicImage::ImageRgba8(
                image::RgbaImage::from_raw(
                    frame.image.width(),
                    frame.image.height(),
                    frame.image.rgba().to_vec(),
                )
                .ok_or("Ungültiges Videobild")?,
            )
        } else {
            decode_oriented(path, turns).map_err(|e| e.to_string())?
        };
        let pixels = if pixels.width() > max_edge || pixels.height() > max_edge {
            pixels.thumbnail(max_edge, max_edge)
        } else {
            pixels
        };
        let rgba = pixels.to_rgba8();
        crate::ports::DecodedImage::new(rgba.width(), rgba.height(), rgba.into_raw())
    }

    fn decode(&self, path: &Path, turns: u8) -> Result<crate::ports::DecodedImage, String> {
        if crate::domain::is_video(path) {
            return self.videos.frame_at(path, 0.).map(|frame| frame.image);
        }
        let rgba = decode_oriented(path, turns)
            .map_err(|e| e.to_string())?
            .to_rgba8();
        crate::ports::DecodedImage::new(rgba.width(), rgba.height(), rgba.into_raw())
    }
}
#[cfg(test)]
mod preview_tests {
    use super::*;
    struct NoVideo;
    impl crate::ports::VideoPreviewSource for NoVideo {
        fn duration(&self, _: &Path) -> Result<f64, String> {
            unreachable!()
        }
        fn frame_at(&self, _: &Path, _: f64) -> Result<crate::ports::VideoFrame, String> {
            unreachable!()
        }
    }
    #[test]
    fn preview_sizes_preserve_orientation_and_source_bytes() {
        use crate::ports::ImageDecoder;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("photo.png");
        image::RgbImage::new(800, 400).save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let decoder = RasterDecoder {
            priority: Default::default(),
            videos: std::sync::Arc::new(NoVideo),
        };
        let small = decoder.decode_preview(&path, 1, 256).unwrap();
        assert_eq!((small.width(), small.height()), (128, 256));
        let large = decoder.decode_preview(&path, 0, 4096).unwrap();
        assert_eq!((large.width(), large.height()), (800, 400));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
