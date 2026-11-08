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
#[derive(Clone, Debug)]
pub struct YearTimeline {
    year: String,
    recordings: Vec<Recording>,
    offsets: Vec<f64>,
    duration: f64,
}
impl YearTimeline {
    pub fn new(year: String, mut recordings: Vec<Recording>) -> Result<Self, String> {
        if year.len() != 4
            || year.parse::<u16>().ok().filter(|y| *y > 0).is_none()
            || recordings.is_empty()
        {
            return Err("Das Diary benötigt ein gültiges Jahr und mindestens eine Aufnahme".into());
        }
        recordings.sort_by(|a, b| {
            a.captured
                .stamp()
                .cmp(b.captured.stamp())
                .then(a.path.cmp(&b.path))
        });
        let mut duration = 0.;
        let mut offsets = Vec::new();
        for recording in &recordings {
            offsets.push(duration);
            duration += recording.duration;
            if !duration.is_finite() {
                return Err("Gesamtdauer zu groß".into());
            }
        }
        Ok(Self {
            year,
            recordings,
            offsets,
            duration,
        })
    }
    pub fn year(&self) -> &str {
        &self.year
    }
    pub fn recordings(&self) -> &[Recording] {
        &self.recordings
    }
    pub fn duration(&self) -> f64 {
        self.duration
    }
    pub fn offset(&self, index: usize) -> Option<f64> {
        self.offsets.get(index).copied()
    }
    pub fn recorded_time(&self, seconds: f64) -> Result<CaptureTime, String> {
        let (index, local) = self.locate(seconds)?;
        self.recordings[index].captured.shifted(local)
    }
    pub fn locate(&self, seconds: f64) -> Result<(usize, f64), String> {
        if !seconds.is_finite() {
            return Err("Ungültige Zeitposition".into());
        }
        let seconds = seconds.clamp(0., self.duration);
        let index = self
            .offsets
            .partition_point(|start| *start <= seconds)
            .saturating_sub(1);
        Ok((
            index,
            (seconds - self.offsets[index]).min(self.recordings[index].duration),
        ))
    }
}
pub fn diary_year(group: &str) -> Option<&str> {
    let mut components = group.split(std::path::MAIN_SEPARATOR);
    let year = components.next()?;
    (year.len() == 4
        && year.parse::<u16>().ok().is_some_and(|y| y > 0)
        && components.next()?.eq_ignore_ascii_case("diary"))
    .then_some(year)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_clock_uses_recorded_date_plus_local_position_across_midnight() {
        let recording = Recording::new(
            "a.wav".into(),
            CaptureTime::parse("2023:12:27 23:59:59", "test").unwrap(),
            30.,
        )
        .unwrap();
        let timeline = YearTimeline::new("2023".into(), vec![recording]).unwrap();
        let now = timeline.recorded_time(2.25).unwrap();
        assert_eq!(now.date_label(), "28.12.2023");
        assert_eq!(now.time_label(), "00:00:01");
        assert_eq!(timeline.recorded_time(0.).unwrap().time_label(), "23:59:59");
    }
    #[test]
    fn segments_follow_dates_and_have_proportional_durations_and_boundary_seeking() {
        let a = Recording::new(
            "a.wav".into(),
            CaptureTime::parse("2026:01:01 00:00:00", "test").unwrap(),
            10.,
        )
        .unwrap();
        let b = Recording::new(
            "b.wav".into(),
            CaptureTime::parse("2026:09:01 00:00:00", "test").unwrap(),
            30.,
        )
        .unwrap();
        let t = YearTimeline::new("2026".into(), vec![b, a]).unwrap();
        assert_eq!(t.duration(), 40.);
        assert_eq!(t.offset(1), Some(10.));
        assert_eq!(t.recordings()[0].duration() / t.duration(), 0.25);
        assert_eq!(t.locate(10.).unwrap(), (1, 0.));
        assert_eq!(t.locate(25.).unwrap(), (1, 15.));
        assert_eq!(t.locate(40.).unwrap(), (1, 30.));
        assert_eq!(t.locate(-1.).unwrap(), (0, 0.));
        assert!(t.locate(f64::NAN).is_err());
        assert!(
            Recording::new(
                "x".into(),
                t.recordings()[0].captured.clone(),
                f64::INFINITY
            )
            .is_err()
        );
    }
}
