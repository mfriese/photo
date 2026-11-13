//! Thread-safe native capture adapter; all native commands serialize internally.
use photosuite_application::comments::*;
#[cfg(target_os = "macos")]
pub struct NativeCommentRecorder {
    pointer: usize,
}
#[cfg(not(target_os = "macos"))]
pub struct NativeCommentRecorder;
impl NativeCommentRecorder {
    pub fn new() -> Self {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn photosuite_comment_recorder_create() -> *mut std::ffi::c_void;
            }
            Self {
                pointer: unsafe { photosuite_comment_recorder_create() } as usize,
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self
        }
    }
}
#[cfg(target_os = "macos")]
fn json(pointer: *mut std::ffi::c_char) -> Result<serde_json::Value, String> {
    unsafe extern "C" {
        fn photosuite_free(pointer: *mut std::ffi::c_void);
    }
    if pointer.is_null() {
        return Err("Aufnahmeantwort fehlt".into());
    }
    // SAFETY: native API returns malloc-owned NUL-terminated JSON, copied before freeing.
    let result = unsafe {
        serde_json::from_slice(std::ffi::CStr::from_ptr(pointer).to_bytes())
            .map_err(|e| e.to_string())
    };
    unsafe { photosuite_free(pointer.cast()) };
    result
}
impl CommentRecorder for NativeCommentRecorder {
    fn devices(&self) -> Result<Vec<InputDevice>, String> {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn photosuite_comment_devices() -> *mut std::ffi::c_char;
            }
            let value = json(unsafe { photosuite_comment_devices() })?;
            value
                .as_array()
                .ok_or("Ungültige Mikrofonliste".into())
                .map(|devices| {
                    devices
                        .iter()
                        .filter_map(|d| {
                            Some(InputDevice {
                                id: d["id"].as_str()?.into(),
                                name: d["name"].as_str()?.into(),
                                is_default: d["is_default"].as_bool().unwrap_or(false),
                            })
                        })
                        .collect()
                })
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("Audioaufnahme ist derzeit nur unter macOS verfügbar".into())
        }
    }
    fn start(&self, device: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn photosuite_comment_start(
                    pointer: *mut std::ffi::c_void,
                    device: *const std::ffi::c_char,
                ) -> i32;
            }
            let device = std::ffi::CString::new(device).map_err(|e| e.to_string())?;
            let status = unsafe { photosuite_comment_start(self.pointer as _, device.as_ptr()) };
            if status != 0 {
                return Err(
                    "Vorherige Aufnahme wird noch abgeschlossen. Bitte kurz warten.".into(),
                );
            }
            Ok(())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = device;
            Err("Audioaufnahme ist derzeit nur unter macOS verfügbar".into())
        }
    }
    fn command(&self, command: CaptureCommand) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn photosuite_comment_command(pointer: *mut std::ffi::c_void, command: i32);
            }
            if let CaptureCommand::Seek(seconds) = command {
                unsafe extern "C" {
                    fn photosuite_comment_seek(pointer: *mut std::ffi::c_void, seconds: f64);
                }
                unsafe { photosuite_comment_seek(self.pointer as _, seconds) };
                return Ok(());
            }
            let code = match command {
                CaptureCommand::Pause => 0,
                CaptureCommand::Resume => 1,
                CaptureCommand::Finish => 2,
                CaptureCommand::Preview => 3,
                CaptureCommand::Discard => 4,
                CaptureCommand::Seek(_) => unreachable!(),
            };
            unsafe { photosuite_comment_command(self.pointer as _, code) };
            Ok(())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = command;
            Ok(())
        }
    }
    fn snapshot(&self) -> CaptureSnapshot {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn photosuite_comment_snapshot(
                    pointer: *mut std::ffi::c_void,
                ) -> *mut std::ffi::c_char;
            }
            match json(unsafe { photosuite_comment_snapshot(self.pointer as _) }) {
                Ok(v) => {
                    let phase = match v["phase"].as_str() {
                        Some("preparing") => CapturePhase::Preparing,
                        Some("recording") => CapturePhase::Recording,
                        Some("paused") => CapturePhase::Paused,
                        Some("finishing") => CapturePhase::Finishing,
                        Some("ready") => CapturePhase::Ready,
                        Some("error") => CapturePhase::Error,
                        _ => CapturePhase::Idle,
                    };
                    let id = v["id"].as_str().unwrap_or("").to_owned();
                    let take = (phase == CapturePhase::Ready).then(|| CapturedTake {
                        id: id.clone(),
                        path: v["path"].as_str().unwrap_or("").into(),
                        recorded_at: v["recorded_at"].as_str().unwrap_or("").into(),
                    });
                    CaptureSnapshot {
                        phase,
                        id,
                        take,
                        seconds: v["seconds"]
                            .as_f64()
                            .filter(|n| n.is_finite() && *n >= 0.)
                            .unwrap_or(0.),
                        edit_failed: v["edit_failed"].as_bool().unwrap_or(false),
                        position: v["position"]
                            .as_f64()
                            .filter(|n| n.is_finite() && *n >= 0.)
                            .unwrap_or(0.),
                        wave_start: v["wave_start"]
                            .as_f64()
                            .filter(|n| n.is_finite() && *n >= 0.)
                            .unwrap_or(0.),
                        wave_span: v["wave_span"]
                            .as_f64()
                            .filter(|n| n.is_finite() && *n > 0.)
                            .unwrap_or(32.),
                        level: v["level"].as_f64().unwrap_or(0.).clamp(0., 1.) as f32,
                        peak: v["peak"].as_f64().unwrap_or(-160.) as f32,
                        wave: v["wave"]
                            .as_array()
                            .map(|values| {
                                values
                                    .iter()
                                    .take(CAPTURE_WAVE_SAMPLES)
                                    .map(|n| n.as_f64().unwrap_or(0.).clamp(0., 1.) as f32)
                                    .collect()
                            })
                            .unwrap_or_default(),
                        playing: v["playing"].as_bool().unwrap_or(false),
                        message: v["message"].as_str().unwrap_or("").into(),
                    }
                }
                Err(message) => CaptureSnapshot {
                    phase: CapturePhase::Error,
                    message,
                    ..Default::default()
                },
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            CaptureSnapshot::default()
        }
    }
}
impl Drop for NativeCommentRecorder {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn photosuite_comment_recorder_destroy(pointer: *mut std::ffi::c_void);
            }
            unsafe { photosuite_comment_recorder_destroy(self.pointer as _) }
        }
    }
}
