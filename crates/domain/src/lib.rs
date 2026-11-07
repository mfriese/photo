#![forbid(unsafe_code)]
mod capture_time;
pub use capture_time::CaptureTime;
pub mod commands;
use std::path::PathBuf;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Photo,
    Video,
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
        let kind = if is_video(&path) {
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
