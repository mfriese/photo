//! Application boundary contracts. No GUI or concrete adapter types.
use crate::domain::Media;
/// Read-only repository port owned by the application core.
pub trait MediaRepository: Send + Sync + 'static {
    fn scan(&self, root: &std::path::Path) -> (Vec<Media>, usize);
}

/// Framework-independent pixels at the image adapter boundary.
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub trait ImageDecoder: Send + Sync + 'static {
    fn decode(&self, path: &std::path::Path, turns: u8) -> Result<DecodedImage, String>;
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
