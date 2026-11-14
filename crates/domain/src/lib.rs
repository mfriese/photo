#![forbid(unsafe_code)]
mod capture_time;
pub use capture_time::CaptureTime;
pub mod audio_edit;
pub mod commands;
pub mod comments;
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
    recorded_time: Option<CaptureTime>,
    reference_time: Option<CaptureTime>,
    reference_year: Option<String>,
}
impl Media {
    pub fn from_path(path: PathBuf, group: String) -> Option<Self> {
        // AppleDouble companion files are metadata, never collection/import media.
        if path.file_name()?.to_string_lossy().starts_with("._") {
            return None;
        }
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
        Some(Self {
            path,
            group,
            kind,
            recorded_time: None,
            reference_time: None,
            reference_year: None,
        })
    }
    /// Fast browsing classification: diary containers are provisional audio candidates.
    /// Playback/import must still validate their actual tracks through the audio port.
    pub fn from_library_path(path: PathBuf, group: String) -> Option<Self> {
        let in_diary = diary::audio_year(&group).is_some();
        let mut media = Self::from_path(path, group)?;
        if in_diary && media.is_video() {
            media.kind = MediaKind::Audio;
        }
        Some(media)
    }
    /// The adapter has confirmed an audio-only asset, including MP4/MOV containers.
    pub fn from_audio_asset(path: PathBuf, group: String) -> Option<Self> {
        if path.file_name()?.to_string_lossy().starts_with("._") {
            return None;
        }
        if !is_audio_extension(&path) && !is_video(&path) {
            return None;
        }
        Some(Self {
            path,
            group,
            kind: MediaKind::Audio,
            recorded_time: None,
            reference_time: None,
            reference_year: None,
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
    /// Explicit recording time never changes the file's physical tree location.
    pub fn with_comment_recording(mut self, comment: Option<&comments::Comment>) -> Self {
        self.recorded_time = if self.is_audio() {
            comment
                .filter(|c| self.path.ends_with(c.audio().as_str()))
                .map(|c| c.recorded_at().time().clone())
        } else {
            None
        };
        self.reference_year = comment
            .filter(|c| self.is_audio() && self.path.ends_with(c.audio().as_str()))
            .map(|c| c.photo_year().to_owned());
        self.reference_time = comment
            .filter(|c| self.is_audio() && self.path.ends_with(c.audio().as_str()))
            .and_then(comments::Comment::reference_time);
        self
    }
    /// Logical playback placement; physical path and actual recording time stay unchanged.
    pub fn reference_time(&self) -> Option<&CaptureTime> {
        if self.reference_year.is_some() {
            self.reference_time.as_ref()
        } else {
            self.recorded_time.as_ref()
        }
    }
    pub fn reference_year(&self) -> Option<&str> {
        self.reference_time
            .as_ref()
            .map(CaptureTime::year)
            .or(self.reference_year.as_deref())
    }
    pub fn has_recording_time(&self) -> bool {
        self.recorded_time.is_some()
    }
    pub fn recording_time(&self) -> Option<&CaptureTime> {
        self.recorded_time.as_ref()
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
    fn appledouble_companions_are_not_media_but_other_hidden_media_remain_supported() {
        for name in ["._aud_20231227_225132.wav", "._photo.jpg", "._video.mp4"] {
            assert!(Media::from_path(name.into(), "2023/audio/diary".into()).is_none());
            assert!(Media::from_library_path(name.into(), "2023/audio/diary".into()).is_none());
            assert!(Media::from_audio_asset(name.into(), "2023/audio/diary".into()).is_none());
        }
        assert!(Media::from_path(".recording.wav".into(), "2023/audio/diary".into()).is_some());
    }
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

pub mod photo_context;

pub mod diary_timeline;

pub mod tags;

/// Filename fast path for gallery captions. Never performs I/O or invents a date.
pub fn capture_from_filename(media: &Media) -> Option<CaptureTime> {
    if media.has_recording_time() {
        return media.reference_time().cloned();
    }
    let stem = media.path().file_stem()?.to_str()?;
    if media.is_audio() {
        return audio_date::date_from_filename(stem);
    }
    CaptureTime::from_photo_filename(stem).or_else(|| {
        audio_date::date_from_filename(stem).and_then(|date| {
            CaptureTime::from_civil_seconds(date.civil_seconds(), "Mediendateiname").ok()
        })
    })
}
