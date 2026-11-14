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
    reference_locked: bool,
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
            reference_locked: false,
        }
    }
    pub fn from_media(media: &crate::Media) -> Self {
        let mut clip = Self::from_filename(media.path().into());
        if media.has_recording_time() {
            clip.captured = media.reference_time().cloned();
            clip.reference_locked = true;
        }
        clip
    }
    pub fn with_reference_time(mut self, time: Option<CaptureTime>) -> Self {
        self.captured = time;
        self.reference_locked = true;
        self
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
    month: Option<String>,
    photo: Option<String>,
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
            month: None,
            photo: None,
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
    pub fn with_month(mut self, month: Option<String>) -> Result<Self, String> {
        if let Some(month) = &month
            && (month.len() != 2 || !month.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)))
        {
            return Err("Ungültiger Audiomonat".into());
        }
        self.month = month;
        Ok(self)
    }
    pub fn with_photo(mut self, photo: Option<String>) -> Result<Self, String> {
        if let Some(photo) = &photo {
            crate::comments::Comment::metadata_path(&crate::tags::LibraryFile::new(
                photo.clone(),
            )?)?;
        }
        self.photo = photo;
        Ok(self)
    }
    pub fn photo_reference(&self) -> Option<&str> {
        self.photo.as_deref()
    }
    pub fn month(&self) -> Option<&str> {
        self.month.as_deref()
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
    /// Apply measured durations without changing dates or ordering; validate the batch atomically.
    pub fn refine_durations(&mut self, updates: &[(PathBuf, f64)]) -> Result<(), String> {
        if updates
            .iter()
            .any(|(_, d)| !d.is_finite() || *d <= 0. || !(*d * WEIGHT_PER_SECOND).is_finite())
        {
            return Err("Ungültige Audiodauer".into());
        }
        let updates: std::collections::HashMap<_, _> = updates
            .iter()
            .map(|(path, duration)| (path.as_path(), *duration))
            .collect();
        let known: HashSet<_> = self.clips.iter().map(|c| c.path()).collect();
        if updates.keys().any(|path| !known.contains(path)) {
            return Err("Aufnahme gehört nicht zum Verlauf".into());
        }
        let durations: Vec<_> = self
            .clips
            .iter()
            .map(|c| updates.get(c.path()).copied().or(c.duration()))
            .collect();
        let weights: Vec<_> = durations
            .iter()
            .flatten()
            .map(|d| d * WEIGHT_PER_SECOND)
            .collect();
        if weights.is_empty() {
            return Ok(());
        }
        let provisional = weights.iter().sum::<f64>() / weights.len() as f64;
        let total: f64 = self
            .clips
            .iter()
            .zip(&durations)
            .map(|(clip, d)| {
                d.map(|d| d * WEIGHT_PER_SECOND).unwrap_or_else(|| {
                    if clip.provisional_weight.is_some() {
                        provisional
                    } else {
                        clip.weight()
                    }
                })
            })
            .sum();
        if !total.is_finite() {
            return Err("Gesamtgewicht der Audioleiste zu groß".into());
        }
        for (clip, duration) in self.clips.iter_mut().zip(durations) {
            clip.duration = duration;
            if duration.is_none() && clip.provisional_weight.is_some() {
                clip.provisional_weight = Some(provisional);
            }
        }
        Ok(())
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
        if !clip.reference_locked {
            clip.captured = Some(
                clip.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(crate::audio_date::comment_time)
                    .unwrap_or_else(|| recording.captured().clone()),
            );
        }
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
    fn duration_batch_is_atomic_and_does_not_invent_or_change_dates() {
        let a = PathBuf::from("aud_20261113_120000.wav");
        let b = PathBuf::from("unknown.wav");
        let mut timeline = DiaryTimeline::new(
            "2026".into(),
            vec![
                DiaryClip::from_filename(a.clone()),
                DiaryClip::from_filename(b.clone()),
            ],
        )
        .unwrap();
        timeline
            .refine_durations(&[(a.clone(), 1.), (b.clone(), 3.)])
            .unwrap();
        assert_eq!(
            timeline.clips()[1].weight() / timeline.clips()[0].weight(),
            3.
        );
        assert!(timeline.clips()[1].captured().is_none());
        let stamp = timeline.clips()[0].captured().unwrap().stamp().to_owned();
        for invalid in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(
                timeline
                    .refine_durations(&[(a.clone(), 5.), (b.clone(), invalid)])
                    .is_err()
            );
            assert_eq!(timeline.clips()[0].duration(), Some(1.));
        }
        assert_eq!(timeline.clips()[0].captured().unwrap().stamp(), stamp);
    }
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
    fn comment_keeps_photo_reference_time_while_duration_is_refined() {
        let comment = crate::comments::Comment::new(
            "12345678-1234-1234-1234-123456789abc".into(),
            crate::tags::LibraryFile::new("2026/audio/c/com_20261113_120000.m4a".into()).unwrap(),
            crate::tags::LibraryFile::new("2008/09/img_20080916_035811.jpg".into()).unwrap(),
        )
        .unwrap();
        let media =
            crate::Media::from_audio_asset(comment.audio().as_str().into(), "2026/audio/c".into())
                .unwrap()
                .with_comment_recording(Some(&comment));
        let mut timeline =
            DiaryTimeline::new("2026".into(), vec![DiaryClip::from_media(&media)]).unwrap();
        timeline
            .refine(
                Recording::new(
                    media.path().into(),
                    CaptureTime::parse("2027:01:01 00:00:00", "metadata").unwrap(),
                    15.,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            timeline.clips()[0].captured().unwrap().stamp(),
            "20080916_035811"
        );
        assert_eq!(timeline.clips()[0].duration(), Some(15.));
        assert_eq!(media.group(), "2026/audio/c");
    }
}
