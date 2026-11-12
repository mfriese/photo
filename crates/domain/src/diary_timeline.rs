//! Progressive diary layout: filename draft first, measured duration after verification.
use crate::{CaptureTime, audio_date::date_from_filename, diary::Recording};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
// A common scale keeps byte estimates and measured clips comparable during refinement.
// It is deliberately not an estimate of any individual file's bitrate.
const WEIGHT_PER_SECOND: f64 = 16_000.;
#[derive(Clone, Debug)]
pub struct DiaryClip {
    path: PathBuf,
    bytes: u64,
    provisional_weight: Option<f64>,
    captured: Option<CaptureTime>,
    duration: Option<f64>,
}
impl DiaryClip {
    /// Equal provisional slots until actual durations are available; no I/O required.
    pub fn from_filename(path: PathBuf) -> Self {
        let mut clip = Self::from_file(path, 1);
        clip.provisional_weight = Some(WEIGHT_PER_SECOND);
        clip
    }
    pub fn from_file(path: PathBuf, bytes: u64) -> Self {
        let captured = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(date_from_filename);
        Self {
            path,
            bytes,
            provisional_weight: None,
            captured,
            duration: None,
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn captured(&self) -> Option<&CaptureTime> {
        self.captured.as_ref()
    }
    pub fn duration(&self) -> Option<f64> {
        self.duration
    }
    pub fn is_verified(&self) -> bool {
        self.duration.is_some()
    }
    pub fn weight(&self) -> f64 {
        self.duration
            .map(|d| d * WEIGHT_PER_SECOND)
            .or(self.provisional_weight)
            .unwrap_or(self.bytes.max(1) as f64)
    }
}
#[derive(Clone, Debug)]
pub struct DiaryTimeline {
    year: String,
    kind: crate::diary::AudioKind,
    clips: Vec<DiaryClip>,
}
impl DiaryTimeline {
    pub fn new(year: String, clips: Vec<DiaryClip>) -> Result<Self, String> {
        if year.len() != 4
            || year.parse::<u16>().ok().filter(|v| *v > 0).is_none()
            || clips.is_empty()
        {
            return Err("Diary benötigt ein gültiges Jahr und mindestens eine Aufnahme".into());
        }
        let mut paths = HashSet::new();
        if clips.iter().any(|c| !paths.insert(c.path.clone())) {
            return Err("Doppelte Aufnahme im Jahresverlauf".into());
        }
        let mut timeline = Self {
            year,
            clips,
            kind: crate::diary::AudioKind::All,
        };
        timeline.sort();
        Ok(timeline)
    }
    fn sort(&mut self) {
        self.clips.sort_by(|a, b| {
            match (&a.captured, &b.captured) {
                (Some(a), Some(b)) => a.stamp().cmp(b.stamp()),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then(a.path.cmp(&b.path))
        });
    }
    pub fn with_kind(mut self, kind: crate::diary::AudioKind) -> Self {
        self.kind = kind;
        self
    }
    pub fn kind(&self) -> crate::diary::AudioKind {
        self.kind
    }
    pub fn year(&self) -> &str {
        &self.year
    }
    pub fn clips(&self) -> &[DiaryClip] {
        &self.clips
    }
    pub fn refine(&mut self, recording: Recording) -> Result<(), String> {
        if !(recording.duration() * WEIGHT_PER_SECOND).is_finite() {
            return Err("Audiodauer überschreitet den Darstellungsbereich".into());
        }
        let weight = recording.duration() * WEIGHT_PER_SECOND;
        let (sum, count) = self
            .clips
            .iter()
            .filter(|c| c.is_verified() && c.path != recording.path())
            .fold((weight, 1usize), |(sum, count), clip| {
                (sum + clip.weight(), count + 1)
            });
        let provisional = sum / count as f64;
        let total: f64 = self
            .clips
            .iter()
            .map(|clip| {
                if clip.path == recording.path() {
                    weight
                } else if !clip.is_verified() && clip.provisional_weight.is_some() {
                    provisional
                } else {
                    clip.weight()
                }
            })
            .sum();
        if !total.is_finite() {
            return Err("Gesamtgewicht der Jahresleiste zu groß".into());
        }
        let clip = self
            .clips
            .iter_mut()
            .find(|c| c.path == recording.path())
            .ok_or("Aufnahme gehört nicht zum Jahresverlauf")?;
        clip.captured = Some(
            clip.path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(crate::audio_date::comment_time)
                .unwrap_or_else(|| recording.captured().clone()),
        );
        clip.duration = Some(recording.duration());
        for clip in &mut self.clips {
            if !clip.is_verified() && clip.provisional_weight.is_some() {
                clip.provisional_weight = Some(provisional);
            }
        }
        self.sort();
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn name_only_draft_stays_balanced_during_refinement_then_has_exact_duration_ratios() {
        let a = PathBuf::from("2023-12-27_22-51-32.wav");
        let b = PathBuf::from("2023-12-28_22-51-32.wav");
        let mut timeline = DiaryTimeline::new(
            "2023".into(),
            vec![
                DiaryClip::from_filename(a.clone()),
                DiaryClip::from_filename(b.clone()),
            ],
        )
        .unwrap();
        assert_eq!(timeline.clips()[0].weight(), timeline.clips()[1].weight());
        let date = CaptureTime::parse("2023:12:27 22:51:32", "metadata").unwrap();
        timeline
            .refine(Recording::new(a, date.clone(), 10.).unwrap())
            .unwrap();
        assert_eq!(timeline.clips()[0].weight(), timeline.clips()[1].weight());
        assert!(!timeline.clips()[1].is_verified());
        timeline
            .refine(Recording::new(b, date, 30.).unwrap())
            .unwrap();
        assert_eq!(
            timeline.clips()[1].weight() / timeline.clips()[0].weight(),
            3.
        );
    }
    #[test]
    fn filename_order_byte_ratios_and_refinement_keep_identity() {
        let a = PathBuf::from("2023-12-27_22-51-32.wav");
        let b = PathBuf::from("2023-12-28_22-51-32.mp3");
        let mut timeline = DiaryTimeline::new(
            "2023".into(),
            vec![
                DiaryClip::from_file(b.clone(), 300),
                DiaryClip::from_file(a.clone(), 100),
            ],
        )
        .unwrap();
        assert_eq!(timeline.clips()[0].path(), a);
        assert_eq!(
            timeline.clips()[1].weight() / timeline.clips()[0].weight(),
            3.
        );
        assert!(!timeline.clips()[0].is_verified());
        let corrected = CaptureTime::parse("2023:12:26 00:00:00", "metadata").unwrap();
        timeline
            .refine(Recording::new(b.clone(), corrected, 10.).unwrap())
            .unwrap();
        assert_eq!(timeline.clips()[0].path(), b);
        assert!(timeline.clips()[0].is_verified());
        assert_eq!(
            timeline.clips()[0].captured().unwrap().stamp(),
            "20231226_000000"
        );
        timeline
            .refine(
                Recording::new(
                    a,
                    CaptureTime::parse("2023:12:27 00:00:00", "test").unwrap(),
                    30.,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            timeline.clips()[1].weight() / timeline.clips()[0].weight(),
            3.
        );
        assert!(
            timeline
                .refine(
                    Recording::new(
                        "outside.wav".into(),
                        CaptureTime::parse("2023:12:27 00:00:00", "test").unwrap(),
                        1.
                    )
                    .unwrap()
                )
                .is_err()
        );
    }
    #[test]
    fn unknown_dates_zero_files_and_duplicate_paths_are_explicit() {
        let clip = DiaryClip::from_file("unknown.wav".into(), 0);
        assert!(clip.captured().is_none());
        assert!(clip.weight() > 0.);
        assert!(DiaryTimeline::new("2023".into(), vec![clip.clone(), clip]).is_err());
    }
    #[test]
    fn overflowing_revision_is_rejected_without_mutating_the_clip() {
        let mut timeline = DiaryTimeline::new(
            "2023".into(),
            vec![
                DiaryClip::from_file("a.wav".into(), 1),
                DiaryClip::from_file("b.wav".into(), 1),
            ],
        )
        .unwrap();
        let date = CaptureTime::parse("2023:12:27 00:00:00", "test").unwrap();
        timeline
            .refine(Recording::new("a.wav".into(), date.clone(), f64::MAX / 20000.).unwrap())
            .unwrap();
        assert!(
            timeline
                .refine(Recording::new("b.wav".into(), date, f64::MAX / 20000.).unwrap())
                .is_err()
        );
        assert!(
            timeline
                .clips()
                .iter()
                .find(|c| c.path() == Path::new("b.wav"))
                .unwrap()
                .duration()
                .is_none()
        );
    }
}

#[cfg(test)]
mod comment_tests {
    use super::*;
    #[test]
    fn comment_keeps_photo_time_when_duration_and_recording_metadata_arrive() {
        let comment = PathBuf::from("2023/audio/comment_20230101_120000.wav");
        let diary = PathBuf::from("2023/audio/aud_20230102_120000.wav");
        let mut timeline = DiaryTimeline::new(
            "2023".into(),
            vec![
                DiaryClip::from_filename(diary),
                DiaryClip::from_filename(comment.clone()),
            ],
        )
        .unwrap();
        let recorded = CaptureTime::parse("2026:11:10 18:00:00", "actual recording").unwrap();
        timeline
            .refine(Recording::new(comment.clone(), recorded, 15.).unwrap())
            .unwrap();
        assert_eq!(timeline.clips()[0].path(), comment);
        assert_eq!(
            timeline.clips()[0].captured().unwrap().stamp(),
            "20230101_120000"
        );
        assert_eq!(timeline.clips()[0].duration(), Some(15.));
    }
}
