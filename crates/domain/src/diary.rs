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
