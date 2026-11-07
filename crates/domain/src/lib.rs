#![forbid(unsafe_code)]
mod capture_time;
pub use capture_time::CaptureTime;
pub mod commands;
pub mod diary;
use std::path::PathBuf;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Photo,
    Video,
    Audio,
}
#[derive(Clone, Debug)]
pub struct Media {
    path: PathBuf,
    group: String,
    kind: MediaKind,
}
impl Media {
    pub fn from_path(path: PathBuf, group: String) -> Option<Self> {
        let ext = path.extension()?.to_string_lossy().to_lowercase();
        let kind = if is_audio_extension(&path) {
            MediaKind::Audio
        } else if is_video(&path) {
            MediaKind::Video
        } else if matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "tif" | "tiff" | "gif" | "bmp"
        ) {
            MediaKind::Photo
        } else {
            return None;
        };
        Some(Self { path, group, kind })
    }
    /// The adapter has confirmed an audio-only asset, including MP4/MOV containers.
    pub fn from_audio_asset(path: PathBuf, group: String) -> Option<Self> {
        if !is_audio_extension(&path) && !is_video(&path) {
            return None;
        }
        Some(Self {
            path,
            group,
            kind: MediaKind::Audio,
        })
    }
    pub fn is_audio(&self) -> bool {
        self.kind == MediaKind::Audio
    }
    pub fn is_photo(&self) -> bool {
        self.kind == MediaKind::Photo
    }
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn group(&self) -> &str {
        &self.group
    }
    pub fn is_video(&self) -> bool {
        self.kind == MediaKind::Video
    }
}

pub fn matches_group(group: &str, selected: Option<&str>) -> bool {
    selected.is_none_or(|selected| std::path::Path::new(group).starts_with(selected))
}

pub fn is_video(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
            .as_str(),
        "mp4" | "mov" | "m4v" | "mkv" | "avi" | "webm"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn media_kind_cannot_disagree_with_supported_extension() {
        assert!(Media::from_path("a.JPG".into(), String::new()).is_some_and(|m| !m.is_video()));
        assert!(Media::from_path("a.MOV".into(), String::new()).is_some_and(|m| m.is_video()));
        assert!(Media::from_path("a.txt".into(), String::new()).is_none());
    }
}
pub mod duplicates;

pub fn is_audio_extension(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "wav"
            | "wave"
            | "mp3"
            | "m4a"
            | "m4b"
            | "aac"
            | "aif"
            | "aiff"
            | "aifc"
            | "caf"
            | "flac"
            | "ac3"
            | "eac3"
            | "amr"
            | "au"
            | "snd"
            | "mp2"
    )
}
pub mod audio_date;
