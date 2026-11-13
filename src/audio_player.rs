//! Native playback bridge: application snapshots in, UI updates/clicks out.
use crate::domain::diary_timeline::DiaryTimeline;
use photosuite_application::diary::{AudioPlaybackState, AudioPlayer, VerificationProgress};
use std::path::Path;
#[cfg(target_os = "macos")]
#[path = "audio_player_projection.rs"]
mod projection;
pub struct NativeAudioPlayer {
    priority: std::sync::Arc<photosuite_application::work_priority::WorkPriority>,
    source: std::sync::Arc<dyn photosuite_application::photo_context::ContextPhotoSource>,
    context: std::sync::Mutex<Option<photosuite_application::photo_context::PhotoContext>>,
    session: std::sync::atomic::AtomicU64,
}
impl NativeAudioPlayer {
    pub fn new(
        source: std::sync::Arc<dyn photosuite_application::photo_context::ContextPhotoSource>,
        priority: std::sync::Arc<photosuite_application::work_priority::WorkPriority>,
    ) -> Self {
        Self {
            source,
            priority,
            context: Default::default(),
            session: std::sync::atomic::AtomicU64::new(0),
        }
    }
}
impl AudioPlayer for NativeAudioPlayer {
    fn open(
        &self,
        timeline: &DiaryTimeline,
        selected: &Path,
        photos: &[crate::domain::Media],
    ) -> Result<(), String> {
        self.open_mode(timeline, selected, photos, true)
    }
    fn open_embedded(
        &self,
        timeline: &DiaryTimeline,
        selected: &Path,
        photos: &[crate::domain::Media],
    ) -> Result<(), String> {
        self.open_mode(timeline, selected, photos, false)
    }
    fn transport(&self) -> photosuite_application::diary::AudioTransportSnapshot {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn photosuite_audio_transport_state(
                    session: u64,
                    fraction: *mut f64,
                    civil: *mut f64,
                ) -> i32;
            }
            let mut fraction = 0.;
            let mut civil = f64::NAN;
            let flags = unsafe {
                photosuite_audio_transport_state(
                    self.session.load(std::sync::atomic::Ordering::Relaxed),
                    &mut fraction,
                    &mut civil,
                )
            };
            photosuite_application::diary::AudioTransportSnapshot {
                active: flags & 1 != 0,
                playing: flags & 2 != 0,
                loading: flags & 4 != 0,
                failed: flags & 8 != 0,
                fraction: if fraction.is_finite() {
                    fraction.clamp(0., 1.)
                } else {
                    0.
                },
                current_date: if civil.is_finite() {
                    crate::domain::CaptureTime::from_civil_seconds(
                        civil.floor() as i64,
                        "Audiowiedergabe",
                    )
                    .ok()
                } else {
                    None
                },
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Default::default()
        }
    }
    fn control(&self, command: photosuite_application::diary::AudioControl) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            use photosuite_application::diary::AudioControl;
            unsafe extern "C" {
                fn photosuite_audio_control(session: u64, command: i32, fraction: f64) -> i32;
            }
            let (command, fraction) = match command {
                AudioControl::Toggle => (0, 0.),
                AudioControl::Seek(fraction) => {
                    if !fraction.is_finite() {
                        return Err("Ungültige Audioposition".into());
                    }
                    (1, fraction.clamp(0., 1.))
                }
                AudioControl::ShowWindow => (2, 0.),
                AudioControl::Stop => (3, 0.),
            };
            if unsafe {
                photosuite_audio_control(
                    self.session.load(std::sync::atomic::Ordering::Relaxed),
                    command,
                    fraction,
                )
            } == 0
            {
                Ok(())
            } else {
                Err("Audio-Sitzung nicht verfügbar".into())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = command;
            Err("Audiosteuerung nur unter macOS verfügbar".into())
        }
    }
    fn revise(
        &self,
        timeline: &DiaryTimeline,
        progress: &VerificationProgress,
    ) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let status = if progress.cancelled {
                "Prüfung abgebrochen · Grau: vorläufig · Blau: geprüft".into()
            } else if progress.checked == progress.total && progress.errors.is_empty() {
                format!(
                    "Alle {} Aufnahmen geprüft · Breiten nach Dauer",
                    progress.total
                )
            } else {
                format!(
                    "Geprüft {} / {} · {} Fehler · Grau: vorläufig · Blau: geprüft",
                    progress.checked,
                    progress.total,
                    progress.errors.len()
                )
            };
            projection::NativeLayout::new(timeline, progress)?.revise(
                self.session.load(std::sync::atomic::Ordering::Relaxed),
                &status,
            )
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (timeline, progress);
            Err("Audioplayer derzeit nur unter macOS verfügbar".into())
        }
    }
    fn state(&self) -> AudioPlaybackState {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn photosuite_audio_state(session: u64) -> i32;
            }
            let flags = unsafe {
                photosuite_audio_state(self.session.load(std::sync::atomic::Ordering::Relaxed))
            };
            AudioPlaybackState {
                is_open: flags & 1 != 0,
                has_started: flags & 2 != 0,
                failed: flags & 4 != 0,
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Default::default()
        }
    }
    fn poll(&self) -> photosuite_application::diary::AudioPlayerUpdate {
        #[cfg(target_os = "macos")]
        {
            use photosuite_application::photo_context::PhotoEvent;
            use std::ffi::CString;
            unsafe extern "C" {
                fn photosuite_audio_context_record(
                    session: *mut u64,
                    request: *mut u64,
                    slot: *mut u32,
                ) -> i32;
                fn photosuite_audio_context_click(
                    session: *mut u64,
                    request: *mut u64,
                    slot: *mut u32,
                ) -> i32;
                fn photosuite_audio_context_state(
                    session: *mut u64,
                    request: *mut u64,
                    civil: *mut f64,
                ) -> i32;
                fn photosuite_audio_context_slot(
                    session: u64,
                    request: u64,
                    slot: u32,
                    png: *const u8,
                    length: usize,
                    tooltip: *const std::ffi::c_char,
                    state: i32,
                );
                fn photosuite_audio_context_caption(
                    session: u64,
                    request: u64,
                    slot: u32,
                    date: *const std::ffi::c_char,
                    clock: *const std::ffi::c_char,
                );
                fn photosuite_audio_context_status(
                    session: u64,
                    request: u64,
                    status: *const std::ffi::c_char,
                );
            }
            let (mut session, mut request, mut civil) = (0, 0, 0.);
            let flags =
                unsafe { photosuite_audio_context_state(&mut session, &mut request, &mut civil) };
            let mut guard = self.context.lock().unwrap();
            if flags & 1 == 0 {
                *guard = None;
                return Default::default();
            }
            let Some(context) = guard.as_mut() else {
                return photosuite_application::diary::AudioPlayerUpdate {
                    is_open: true,
                    photo_to_reveal: None,
                    photo_to_record: None,
                };
            };
            if flags & 2 != 0 {
                context.request(request, civil);
            }
            for event in context.poll() {
                match event {
                    PhotoEvent::Failed { request, message } => {
                        if let Ok(text) = CString::new(message) {
                            unsafe {
                                photosuite_audio_context_status(session, request, text.as_ptr());
                                for slot in 0..5 {
                                    photosuite_audio_context_slot(
                                        session,
                                        request,
                                        slot,
                                        std::ptr::null(),
                                        0,
                                        text.as_ptr(),
                                        3,
                                    );
                                    photosuite_audio_context_caption(
                                        session,
                                        request,
                                        slot,
                                        std::ptr::null(),
                                        std::ptr::null(),
                                    );
                                }
                            }
                        }
                    }
                    PhotoEvent::Selection {
                        request,
                        photos,
                        unreadable,
                    } => {
                        let count = photos.iter().flatten().count();
                        let status = if count == 0 {
                            format!("Keine Fotos mit auswertbarem Datum ({unreadable} ohne Datum)")
                        } else if unreadable > 0 {
                            format!("{unreadable} Dateien ohne auswertbares Datum")
                        } else {
                            String::new()
                        };
                        if let Ok(text) = CString::new(status) {
                            unsafe {
                                photosuite_audio_context_status(session, request, text.as_ptr());
                            }
                        }
                        for (slot, photo) in photos.iter().enumerate() {
                            let tooltip = photo
                                .as_ref()
                                .map(|p| {
                                    format!(
                                        "{} {}\n{}\n{}",
                                        p.captured().date_label(),
                                        p.captured().time_label(),
                                        p.captured().origin(),
                                        p.path().display()
                                    )
                                })
                                .unwrap_or_else(|| "Kein zeitlicher Nachbar vorhanden".into());
                            if let Ok(text) = CString::new(tooltip) {
                                unsafe {
                                    photosuite_audio_context_slot(
                                        session,
                                        request,
                                        slot as u32,
                                        std::ptr::null(),
                                        0,
                                        text.as_ptr(),
                                        i32::from(photo.is_some()),
                                    );
                                    let date = CString::new(
                                        photo
                                            .as_ref()
                                            .map(|p| p.captured().date_label())
                                            .unwrap_or_default(),
                                    )
                                    .unwrap();
                                    let clock = CString::new(
                                        photo
                                            .as_ref()
                                            .map(|p| p.captured().time_label())
                                            .unwrap_or_default(),
                                    )
                                    .unwrap();
                                    photosuite_audio_context_caption(
                                        session,
                                        request,
                                        slot as u32,
                                        date.as_ptr(),
                                        clock.as_ptr(),
                                    );
                                }
                            }
                        }
                    }
                    PhotoEvent::Image {
                        request,
                        slot,
                        result,
                    } => match result {
                        Ok(png) => unsafe {
                            photosuite_audio_context_slot(
                                session,
                                request,
                                slot as u32,
                                png.as_ptr(),
                                png.len(),
                                std::ptr::null(),
                                2,
                            );
                        },
                        Err(error) => {
                            if let Ok(text) = CString::new(error) {
                                unsafe {
                                    photosuite_audio_context_slot(
                                        session,
                                        request,
                                        slot as u32,
                                        std::ptr::null(),
                                        0,
                                        text.as_ptr(),
                                        3,
                                    );
                                }
                            }
                        }
                    },
                }
            }
            let (mut clicked_session, mut clicked_request, mut slot) = (0, 0, 0);
            let clicked = unsafe {
                photosuite_audio_context_click(
                    &mut clicked_session,
                    &mut clicked_request,
                    &mut slot,
                )
            };
            let photo_to_reveal = if clicked != 0 && clicked_session == session {
                context.photo_at(clicked_request, slot as usize)
            } else {
                None
            };
            let (mut record_session, mut record_request, mut record_slot) = (0, 0, 0);
            let record = unsafe {
                photosuite_audio_context_record(
                    &mut record_session,
                    &mut record_request,
                    &mut record_slot,
                )
            };
            let photo_to_record = if record != 0 && record_session == session {
                context.photo_at(record_request, record_slot as usize)
            } else {
                None
            };
            photosuite_application::diary::AudioPlayerUpdate {
                is_open: true,
                photo_to_reveal,
                photo_to_record,
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Default::default()
        }
    }
}

impl NativeAudioPlayer {
    fn open_mode(
        &self,
        timeline: &DiaryTimeline,
        selected: &Path,
        photos: &[crate::domain::Media],
        show_window: bool,
    ) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let session =
                projection::NativeLayout::new(timeline, &VerificationProgress::default())?.open(
                    &format!(
                        "{} · {}{}",
                        crate::audio_labels::kind_label(timeline.kind()),
                        timeline.year(),
                        timeline
                            .month()
                            .map(|month| format!("/{month}"))
                            .unwrap_or_default()
                    ),
                    selected,
                    show_window,
                )?;
            self.session
                .store(session, std::sync::atomic::Ordering::Relaxed);
            let candidates = photos
                .iter()
                .filter(|p| p.is_photo())
                .map(|p| p.path().to_owned())
                .collect();
            *self.context.lock().unwrap() = Some(
                photosuite_application::photo_context::PhotoContext::with_priority(
                    self.source.clone(),
                    candidates,
                    self.priority.clone(),
                ),
            );
            Ok(())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (timeline, selected, photos);
            Err("Audioplayer derzeit nur unter macOS verfügbar".into())
        }
    }
}
