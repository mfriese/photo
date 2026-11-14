//! A continuous year recording: durations define segment widths, dates define order.
use crate::CaptureTime;
use std::path::{Path, PathBuf};
#[derive(Clone, Debug)]
pub struct Recording {
    path: PathBuf,
    captured: CaptureTime,
    duration: f64,
}
impl Recording {
    pub fn new(path: PathBuf, captured: CaptureTime, duration: f64) -> Result<Self, String> {
        if !duration.is_finite() || duration <= 0. {
            return Err("Ungültige Audiodauer".into());
        }
        Ok(Self {
            path,
            captured,
            duration,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn captured(&self) -> &CaptureTime {
        &self.captured
    }
    pub fn duration(&self) -> f64 {
        self.duration
    }
}
/// Audio categories share one year, but can be selected independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioKind {
    All,
    Diary,
    Comments,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioScope<'a> {
    year: &'a str,
    kind: AudioKind,
}
impl<'a> AudioScope<'a> {
    pub fn for_year(year: &'a str, kind: AudioKind) -> Option<Self> {
        (year.len() == 4
            && year.bytes().all(|b| b.is_ascii_digit())
            && year.parse::<u16>().is_ok_and(|y| y > 0))
        .then_some(Self { year, kind })
    }
    pub fn parse(group: &'a str) -> Option<Self> {
        let mut parts = group.split(std::path::MAIN_SEPARATOR);
        let year = parts.next()?;
        let folder = parts.next()?;
        let kind = if folder == "audio" {
            match parts.next() {
                None => AudioKind::All,
                Some("diary") => AudioKind::Diary,
                Some("comments") => AudioKind::Comments,
                _ => return None,
            }
        } else {
            return None;
        };
        if parts.next().is_some() {
            return None;
        }
        Self::for_year(year, kind)
    }
    pub fn year(self) -> &'a str {
        self.year
    }
    pub fn kind(self) -> AudioKind {
        self.kind
    }
    pub fn includes(self, media: &crate::Media) -> bool {
        media.is_audio()
            && media
                .reference_year()
                .or_else(|| media.reference_time().map(|d| d.year()))
                .or_else(|| audio_year(media.group()))
                == Some(self.year)
            && (self.kind == AudioKind::All || audio_kind(media) == Some(self.kind))
    }
    /// Canonical view key. Diary/comments are virtual filters, not new import directories.
    pub fn group(self) -> String {
        let root = std::path::Path::new(self.year).join("audio");
        match self.kind {
            AudioKind::All => root,
            AudioKind::Diary => root.join("diary"),
            AudioKind::Comments => root.join("comments"),
        }
        .to_string_lossy()
        .into_owned()
    }
}
/// The selected tree view, distinct from a physical audio folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioViewScope<'a> {
    audio: Option<AudioScope<'a>>,
    month: Option<&'a str>,
    photo: Option<&'a str>,
}
impl<'a> AudioViewScope<'a> {
    pub fn parse(group: &'a str) -> Option<Self> {
        if group == "all" {
            return Some(Self {
                audio: None,
                month: None,
                photo: None,
            });
        }
        if let Some(photo) = group.strip_prefix("photo/") {
            let file = crate::tags::LibraryFile::new(photo.into()).ok()?;
            crate::comments::Comment::metadata_path(&file).ok()?;
            let year = photo.split(std::path::MAIN_SEPARATOR).next()?;
            return Some(Self {
                audio: Some(AudioScope::for_year(year, AudioKind::Comments)?),
                month: None,
                photo: Some(photo),
            });
        }
        if let Some(audio) = AudioScope::parse(group) {
            return Some(Self {
                audio: Some(audio),
                month: None,
                photo: None,
            });
        }
        let mut parts = group.split(std::path::MAIN_SEPARATOR);
        let year = parts.next()?;
        let month = parts.next();
        if parts.next().is_some() {
            return None;
        }
        if month
            .is_some_and(|m| m.len() != 2 || !m.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)))
        {
            return None;
        }
        Some(Self {
            audio: Some(AudioScope::for_year(year, AudioKind::All)?),
            month,
            photo: None,
        })
    }
    pub fn year(self) -> &'a str {
        self.audio.map(AudioScope::year).unwrap_or("")
    }
    pub fn kind(self) -> AudioKind {
        self.audio.map(AudioScope::kind).unwrap_or(AudioKind::All)
    }
    pub fn is_library(self) -> bool {
        self.audio.is_none()
    }
    pub fn month(self) -> Option<&'a str> {
        self.month
    }
    pub fn photo_reference(self) -> Option<&'a str> {
        self.photo
    }
    pub fn group(self) -> String {
        if self.is_library() {
            return "all".into();
        }
        if let Some(photo) = self.photo {
            return format!("photo/{photo}");
        }
        self.month
            .map(|month| {
                Path::new(self.year())
                    .join(month)
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_else(|| self.audio.expect("year scope").group())
    }
    pub fn includes(self, media: &crate::Media) -> bool {
        if self.photo.is_some() {
            return media.is_audio() && audio_kind(media) == Some(AudioKind::Comments);
        }
        self.audio.is_none_or(|audio| audio.includes(media))
            && media.is_audio()
            && self.month.is_none_or(|month| {
                crate::capture_from_filename(media).is_some_and(|date| date.month() == month)
            })
    }
}
pub fn audio_year(group: &str) -> Option<&str> {
    AudioScope::parse(group).map(AudioScope::year)
}
/// Classify a recording by its canonical audio/diary or audio/comments folder.
pub fn recording_kind(path: &Path) -> Option<AudioKind> {
    let parent = path.parent()?;
    let parts: Vec<_> = parent.components().collect();
    for start in (0..parts.len()).rev() {
        let group: std::path::PathBuf = parts[start..].iter().collect();
        let group = group.to_str()?;
        if AudioScope::parse(group).is_some() {
            return crate::Media::from_audio_asset(path.into(), group.into())
                .as_ref()
                .and_then(audio_kind);
        }
    }
    None
}
pub fn audio_kind(media: &crate::Media) -> Option<AudioKind> {
    if !media.is_audio() {
        return None;
    }
    let scope = AudioScope::parse(media.group())?;
    match scope.kind {
        AudioKind::All => None,
        kind => Some(kind),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recordings_validate_duration_and_clock_crosses_midnight() {
        let date = CaptureTime::parse("2023:12:27 23:59:59", "test").unwrap();
        assert!(Recording::new("a.wav".into(), date.clone(), f64::INFINITY).is_err());
        assert!(Recording::new("a.wav".into(), date.clone(), 0.).is_err());
        let recording = Recording::new("a.wav".into(), date, 30.).unwrap();
        let now = recording.captured().shifted(2.25).unwrap();
        assert_eq!(now.date_label(), "28.12.2023");
        assert_eq!(now.time_label(), "00:00:01");
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    fn media(name: &str, group: &str) -> crate::Media {
        crate::Media::from_library_path(Path::new(group).join(name), group.into()).unwrap()
    }
    #[test]
    fn only_canonical_folders_define_audio_categories() {
        let diary = AudioScope::parse("2023/audio/diary").unwrap();
        assert!(diary.includes(&media("arbitrary.wav", "2023/audio/diary")));
        assert_eq!(
            audio_kind(&media("aud_20230101_120000.wav", "2023/audio/comments")),
            Some(AudioKind::Comments)
        );
        assert_eq!(
            audio_kind(&media("comment_20230101_120000.wav", "2023/audio")),
            None
        );
        for invalid in [
            "2023/diary",
            "2023/audio/c",
            "2023/audio/d",
            "2023/Audio/diary",
            "0000/audio",
            "20230/audio",
            "2023/audio/other",
            "2023/audio/comments/other",
        ] {
            assert!(AudioScope::parse(invalid).is_none(), "{invalid}");
        }
    }
}

#[cfg(test)]
mod dedicated_audio_tests {
    use super::*;
    #[test]
    fn descriptive_folders_define_categories_and_combined_playback() {
        for (folder, name, kind) in [
            (
                "2026/audio/diary",
                "aud_20261113_120000.mp4",
                AudioKind::Diary,
            ),
            (
                "2026/audio/comments",
                "com_20261113_130000.m4a",
                AudioKind::Comments,
            ),
            ("2026/audio/diary", "old.wav", AudioKind::Diary),
        ] {
            let m = crate::Media::from_library_path(Path::new(folder).join(name), folder.into())
                .unwrap();
            assert!(m.is_audio());
            assert_eq!(audio_kind(&m), Some(kind));
            assert!(AudioScope::parse("2026/audio").unwrap().includes(&m));
            assert_eq!(AudioScope::parse(folder).unwrap().kind(), kind);
        }
        for obsolete in ["2026/audio/d", "2026/audio/c", "2026/audio/y"] {
            assert!(AudioScope::parse(obsolete).is_none());
        }
        assert_eq!(
            crate::audio_date::date_from_filename("com_20261113_130000_002")
                .unwrap()
                .stamp(),
            "20261113_130000"
        );
    }
}

#[cfg(test)]
mod month_scope_tests {
    use super::*;
    #[test]
    fn view_months_are_validated_and_do_not_change_physical_audio_detection() {
        assert!(AudioScope::parse("2008/09").is_none());
        assert!(audio_year("2008/09").is_none());
        let scope = AudioViewScope::parse("2008/09").unwrap();
        assert_eq!(scope.month(), Some("09"));
        assert_eq!(scope.kind(), AudioKind::All);
        assert_eq!(scope.group(), "2008/09");
        for invalid in [
            "2008/00",
            "2008/13",
            "2008/9",
            "2008/audio/unknown",
            "2008/09/extra",
        ] {
            assert!(AudioViewScope::parse(invalid).is_none());
        }
    }
}

#[cfg(test)]
mod projected_kind_tests {
    use super::*;
    #[test]
    fn projected_paths_use_domain_category_rules_without_io() {
        assert_eq!(
            recording_kind(Path::new("/Fotos/2026/audio/diary/aud_20261113_120000.wav")),
            Some(AudioKind::Diary)
        );
        assert_eq!(
            recording_kind(Path::new("2026/audio/comments/com_20261113_120000.m4a")),
            Some(AudioKind::Comments)
        );
        assert_eq!(recording_kind(Path::new("2026/audio/unknown.wav")), None);
    }
}
