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
        let kind = if folder.eq_ignore_ascii_case("diary") {
            AudioKind::Diary
        } else if folder.eq_ignore_ascii_case("audio") {
            match parts.next() {
                None => AudioKind::All,
                Some(name) if name.eq_ignore_ascii_case("diary") => AudioKind::Diary,
                Some(name) if name.eq_ignore_ascii_case("comments") => AudioKind::Comments,
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
            && audio_year(media.group()) == Some(self.year)
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
pub fn audio_year(group: &str) -> Option<&str> {
    AudioScope::parse(group).map(AudioScope::year)
}
/// Explicit prefixes take precedence; old category directories remain readable.
/// Unknown filenames in the flat audio directory belong only to the combined view.
pub fn audio_kind(media: &crate::Media) -> Option<AudioKind> {
    if !media.is_audio() {
        return None;
    }
    let scope = AudioScope::parse(media.group())?;
    let name = media.path().file_stem()?.to_str()?;
    if name
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("comment_"))
    {
        Some(AudioKind::Comments)
    } else if name
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("aud_"))
    {
        Some(AudioKind::Diary)
    } else {
        match scope.kind {
            AudioKind::All => None,
            kind => Some(kind),
        }
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
    fn flat_prefixes_separate_categories_and_combined_includes_unknown_names() {
        let all = AudioScope::parse("2023/audio").unwrap();
        let diary = AudioScope::parse("2023/audio/diary").unwrap();
        let comments = AudioScope::parse("2023/audio/comments").unwrap();
        for (name, expected) in [
            ("aud_20231227_225132.wav", Some(AudioKind::Diary)),
            ("COMMENT_20231227_225132_002.mp4", Some(AudioKind::Comments)),
            ("recorder.wav", None),
        ] {
            let item = media(name, "2023/audio");
            assert_eq!(audio_kind(&item), expected);
            assert!(all.includes(&item));
            assert_eq!(diary.includes(&item), expected == Some(AudioKind::Diary));
            assert_eq!(
                comments.includes(&item),
                expected == Some(AudioKind::Comments)
            );
        }
        assert!(!all.includes(&media("aud_20231227_225132.wav", "2024/audio")));
        assert!(!all.includes(&media("aud_20231227_225132.wav", "2023/01")));
    }
    #[test]
    fn legacy_directories_are_readable_and_explicit_prefix_wins() {
        let diary = AudioScope::parse("2023/audio/diary").unwrap();
        for group in ["2023/diary", "2023/audio/diary"] {
            assert!(diary.includes(&media("old.wav", group)));
        }
        let comment = media("comment_20230101_120000.wav", "2023/diary");
        assert!(!diary.includes(&comment));
        assert_eq!(audio_kind(&comment), Some(AudioKind::Comments));
        for invalid in [
            "0000/audio",
            "20230/audio",
            "2023/audio/other",
            "2023/diary/other",
            "2023/audio/comments/other",
        ] {
            assert!(AudioScope::parse(invalid).is_none());
        }
    }
}
