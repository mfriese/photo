//! Main-thread projection of validated application/domain values into copied C arrays.
use crate::domain::diary_timeline::DiaryTimeline;
use photosuite_application::diary::VerificationProgress;
use std::{
    ffi::{CString, c_char},
    os::unix::ffi::OsStrExt,
    path::Path,
};
unsafe extern "C" {
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
            layout.weights.push(clip.weight());
            layout.dates.push(
                clip.captured()
                    .map(|d| d.civil_seconds() as f64)
                    .unwrap_or(f64::NAN),
            );
            layout.durations.push(clip.duration().unwrap_or(0.));
            layout.states.push(state);
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
            Ok(())
        } else {
            Err(format!(
                "Audioplayer nicht mehr verfügbar oder Zeitleiste ungültig ({code})"
            ))
        }
    }
}
