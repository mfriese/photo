//! Main-thread projection of validated application/domain values into copied C arrays.
use crate::domain::diary_timeline::DiaryTimeline;
use photosuite_application::diary::VerificationProgress;
use std::{
    ffi::{CString, c_char},
    os::unix::ffi::OsStrExt,
    path::Path,
};
unsafe extern "C" {
    fn photosuite_audio_recorded_dates(session: u64, dates: *const f64, count: usize) -> i32;
    fn photosuite_audio_visibility(session: u64, masks: *const u32, count: usize) -> i32;
    fn photosuite_audio_control(session: u64, command: i32, fraction: f64) -> i32;
    fn photosuite_audio_open_mode(
        paths: *const *const c_char,
        labels: *const *const c_char,
        weights: *const f64,
        dates: *const f64,
        durations: *const f64,
        states: *const u32,
        count: usize,
        year: *const c_char,
        selected: *const c_char,
        session: *mut u64,
        show_window: i32,
    ) -> i32;
    fn photosuite_audio_revise(
        session: u64,
        paths: *const *const c_char,
        labels: *const *const c_char,
        weights: *const f64,
        dates: *const f64,
        durations: *const f64,
        states: *const u32,
        count: usize,
        status: *const c_char,
    ) -> i32;
}
pub(super) struct NativeLayout {
    paths: Vec<CString>,
    labels: Vec<CString>,
    weights: Vec<f64>,
    dates: Vec<f64>,
    durations: Vec<f64>,
    states: Vec<u32>,
    visibility: Vec<u32>,
    recorded_dates: Vec<f64>,
}
impl NativeLayout {
    pub(super) fn new(
        timeline: &DiaryTimeline,
        progress: &VerificationProgress,
    ) -> Result<Self, String> {
        let mut layout = Self {
            paths: Vec::new(),
            labels: Vec::new(),
            weights: Vec::new(),
            dates: Vec::new(),
            durations: Vec::new(),
            states: Vec::new(),
            visibility: Vec::new(),
            recorded_dates: Vec::new(),
        };
        for clip in timeline.clips() {
            layout
                .paths
                .push(CString::new(clip.path().as_os_str().as_bytes()).map_err(|e| e.to_string())?);
            let state = if clip.is_verified() {
                1
            } else if progress.errors.contains_key(clip.path()) {
                2
            } else {
                0
            };
            let date = clip
                .captured()
                .map(|d| format!("{} {} · {}", d.date_label(), d.time_label(), d.origin()))
                .unwrap_or_else(|| "Datum wird geprüft".into());
            let detail = progress
                .errors
                .get(clip.path())
                .map(|e| format!("Prüfung fehlgeschlagen: {e}"))
                .unwrap_or_else(|| {
                    if state == 0 {
                        "Vorläufig · Breite nach Dateigröße".into()
                    } else {
                        "Geprüft · Breite nach Dauer".into()
                    }
                });
            layout.labels.push(
                CString::new(format!("{}\n{date}\n{detail}", clip.path().display()))
                    .map_err(|e| e.to_string())?,
            );
            let actual = if crate::domain::diary::recording_kind(clip.path())
                == Some(crate::domain::diary::AudioKind::Comments)
            {
                clip.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(crate::domain::audio_date::date_from_filename)
            } else {
                clip.captured().cloned()
            };
            layout
                .recorded_dates
                .push(actual.map(|d| d.civil_seconds() as f64).unwrap_or(f64::NAN));
            layout.weights.push(clip.weight());
            layout.dates.push(
                clip.captured()
                    .map(|d| d.civil_seconds() as f64)
                    .unwrap_or(f64::NAN),
            );
            layout.durations.push(clip.duration().unwrap_or(0.));
            layout.states.push(state);
            layout
                .visibility
                .push(match crate::domain::diary::recording_kind(clip.path()) {
                    Some(crate::domain::diary::AudioKind::Diary) => 3,
                    Some(crate::domain::diary::AudioKind::Comments) => 5,
                    _ => 1,
                });
        }
        Ok(layout)
    }
    pub(super) fn open(
        &self,
        year: &str,
        selected: &Path,
        show_window: bool,
    ) -> Result<u64, String> {
        let year = CString::new(year).map_err(|e| e.to_string())?;
        let selected = CString::new(selected.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let paths: Vec<_> = self.paths.iter().map(|s| s.as_ptr()).collect();
        let labels: Vec<_> = self.labels.iter().map(|s| s.as_ptr()).collect();
        let mut session = 0;
        let status = unsafe {
            photosuite_audio_open_mode(
                paths.as_ptr(),
                labels.as_ptr(),
                self.weights.as_ptr(),
                self.dates.as_ptr(),
                self.durations.as_ptr(),
                self.states.as_ptr(),
                paths.len(),
                year.as_ptr(),
                selected.as_ptr(),
                &mut session,
                i32::from(show_window),
            )
        };
        if status == 0 {
            if unsafe {
                photosuite_audio_visibility(
                    session,
                    self.visibility.as_ptr(),
                    self.visibility.len(),
                )
            } != 0
            {
                unsafe {
                    photosuite_audio_control(session, 3, 0.);
                };
                return Err("Player-Kategorien konnten nicht gesetzt werden".into());
            }
            if unsafe {
                photosuite_audio_recorded_dates(
                    session,
                    self.recorded_dates.as_ptr(),
                    self.recorded_dates.len(),
                )
            } != 0
            {
                unsafe {
                    photosuite_audio_control(session, 3, 0.);
                }
                return Err("Aufnahmedaten konnten nicht angezeigt werden".into());
            }
            Ok(session)
        } else {
            Err(format!(
                "Audioplayer konnte nicht geöffnet werden ({status})"
            ))
        }
    }
    pub(super) fn revise(&self, session: u64, status: &str) -> Result<(), String> {
        let status = CString::new(status).map_err(|e| e.to_string())?;
        let paths: Vec<_> = self.paths.iter().map(|s| s.as_ptr()).collect();
        let labels: Vec<_> = self.labels.iter().map(|s| s.as_ptr()).collect();
        let code = unsafe {
            photosuite_audio_revise(
                session,
                paths.as_ptr(),
                labels.as_ptr(),
                self.weights.as_ptr(),
                self.dates.as_ptr(),
                self.durations.as_ptr(),
                self.states.as_ptr(),
                paths.len(),
                status.as_ptr(),
            )
        };
        if code == 0 {
            if unsafe {
                photosuite_audio_visibility(
                    session,
                    self.visibility.as_ptr(),
                    self.visibility.len(),
                )
            } != 0
            {
                return Err("Player-Kategorien konnten nicht aktualisiert werden".into());
            }
            if unsafe {
                photosuite_audio_recorded_dates(
                    session,
                    self.recorded_dates.as_ptr(),
                    self.recorded_dates.len(),
                )
            } != 0
            {
                return Err("Aufnahmedaten konnten nicht aktualisiert werden".into());
            }
            Ok(())
        } else {
            Err(format!(
                "Audioplayer nicht mehr verfügbar oder Zeitleiste ungültig ({code})"
            ))
        }
    }
}
