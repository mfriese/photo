//! Application boundary contracts. No GUI or concrete adapter types.
use crate::domain::Media;
/// Read-only repository port owned by the application core.
pub trait MediaRepository: Send + Sync + 'static {
    /// Shallow read-only directory index. None preserves compatibility with flat repositories.
    fn partitions(&self, _root: &std::path::Path) -> Result<Option<Vec<String>>, String> {
        Ok(None)
    }
    /// Read one indexed directory, or only root files for the empty partition.
    fn scan_partition(
        &self,
        root: &std::path::Path,
        _partition: &str,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> (Vec<Media>, usize) {
        self.scan_cancellable(root, cancel)
    }
    fn scan(&self, root: &std::path::Path) -> (Vec<Media>, usize);
    /// Override for cooperative cancellation. The session still bounds non-cooperative adapters.
    fn scan_cancellable(
        &self,
        root: &std::path::Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> (Vec<Media>, usize) {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return (Vec::new(), 0);
        }
        self.scan(root)
    }
}

/// Framework-independent pixels at the image adapter boundary.
pub struct DecodedImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
impl DecodedImage {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, String> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || expected != Some(rgba.len()) {
            return Err("Ungültige RGBA-Abmessungen oder Pixelzahl".into());
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}
pub trait ImageDecoder: Send + Sync + 'static {
    fn decode(&self, path: &std::path::Path, turns: u8) -> Result<DecodedImage, String>;
    /// Preview request; production adapters must bound both dimensions by max_edge.
    fn decode_preview(
        &self,
        path: &std::path::Path,
        turns: u8,
        max_edge: u32,
    ) -> Result<DecodedImage, String> {
        let image = self.decode(path, turns)?;
        if max_edge == 0 || image.width() > max_edge || image.height() > max_edge {
            return Err("Bildadapter kann die angeforderte Vorschauauflösung nicht liefern".into());
        }
        Ok(image)
    }
}
pub trait MediaOpener: Send + Sync + 'static {
    fn open(&self, path: &std::path::Path) -> Result<(), String>;
}

pub trait OrientationWriter: Send + Sync + 'static {
    fn write_rotation(&self, path: &std::path::Path, turns: u8) -> Result<(), String>;
}

pub struct VideoFrame {
    pub image: DecodedImage,
    pub seconds: f64,
}
pub trait VideoPreviewSource: Send + Sync + 'static {
    fn duration(&self, path: &std::path::Path) -> Result<f64, String>;
    fn frame_at(&self, path: &std::path::Path, seconds: f64) -> Result<VideoFrame, String>;
}

/// Native playback UI is invoked on the desktop UI thread; loading is asynchronous.
pub trait VideoPlayer: Send + Sync + 'static {
    fn play(&self, path: &std::path::Path) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoded_pixels_cannot_cross_the_port_with_inconsistent_dimensions() {
        for (w, h, data) in [
            (0, 1, vec![]),
            (1, 0, vec![]),
            (2, 2, vec![0; 15]),
            (u32::MAX, u32::MAX, vec![]),
        ] {
            assert!(DecodedImage::new(w, h, data).is_err());
        }
        let image = DecodedImage::new(1, 1, vec![1, 2, 3, 255]).unwrap();
        assert_eq!((image.width(), image.height()), (1, 1));
        assert_eq!(image.rgba(), &[1, 2, 3, 255]);
    }
}
