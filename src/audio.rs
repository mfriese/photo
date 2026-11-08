//! macOS audio probing, embedded metadata and native year-player adapters.
use crate::domain::{CaptureTime, diary::YearTimeline};
use photosuite_application::diary::{AudioPlayer, AudioProbe, AudioSource};
use std::path::Path;
pub struct NativeAudioSource {
    pub tool: crate::exiftool::ExifTool,
}
impl AudioSource for NativeAudioSource {
    fn probe(&self, path: &Path) -> Result<AudioProbe, String> {
        #[cfg(target_os = "macos")]
        {
            use std::{
                ffi::{CStr, CString},
                os::unix::ffi::OsStrExt,
            };
            unsafe extern "C" {
                fn photosuite_audio_probe(
                    path: *const std::ffi::c_char,
                    duration: *mut f64,
                    audio_only: *mut i32,
                    created: *mut std::ffi::c_char,
                    capacity: usize,
                ) -> i32;
            }
            let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
            let mut duration = 0.;
            let mut only = 0;
            let mut created = [0i8; 128];
            let status = unsafe {
                photosuite_audio_probe(
                    path.as_ptr(),
                    &mut duration,
                    &mut only,
                    created.as_mut_ptr(),
                    created.len(),
                )
            };
            if status != 0 {
                return Err(format!("Audio nicht von macOS abspielbar (Code {status})"));
            }
            let date = unsafe { CStr::from_ptr(created.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            Ok(AudioProbe {
                duration,
                audio_only: only != 0,
                file_created: (!date.is_empty()).then_some(date),
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Err("Diary-Audio derzeit nur unter macOS verfügbar".into())
        }
    }
    fn capture_time(&self, path: &Path) -> Result<CaptureTime, String> {
        let probe = self.probe(path)?;
        if !probe.audio_only {
            return Err("Datei enthält Video oder keine Audiospur".into());
        }
        let output = self
            .tool
            .command()
            .args([
                "-j",
                "-G1",
                "-s",
                "-RIFF:DateTimeOriginal",
                "-RIFF:DateCreated",
                "-RIFF:TimeCreated",
                "-ID3:RecordingTime",
                "-XMP-exif:DateTimeOriginal",
                "-QuickTime:CreationDate",
                "-Keys:CreationDate",
                "-QuickTime:CreateDate",
                "-XMP-xmp:CreateDate",
            ])
            .arg(std::fs::canonicalize(path).map_err(|e| e.to_string())?)
            .output()
            .map_err(|e| format!("Audio-Metadaten benötigen ExifTool: {e}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into());
        }
        let values: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
        let object = values
            .get(0)
            .and_then(|v| v.as_object())
            .ok_or("Audio-Metadatenantwort ungültig")?;
        let tags = object
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
            .collect();
        crate::domain::audio_date::select_audio_date(
            &tags,
            path.file_stem().and_then(|s| s.to_str()),
            probe.file_created.as_deref(),
        )
    }
}
pub struct NativeAudioPlayer;
impl AudioPlayer for NativeAudioPlayer {
    fn open(&self, timeline: &YearTimeline, start: f64) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            use std::{ffi::CString, os::unix::ffi::OsStrExt};
            unsafe extern "C" {
                fn photosuite_audio_open(
                    paths: *const *const std::ffi::c_char,
                    labels: *const *const std::ffi::c_char,
                    durations: *const f64,
                    recorded_at: *const f64,
                    count: usize,
                    year: *const std::ffi::c_char,
                    start: f64,
                ) -> i32;
            }
            let paths: Vec<_> = timeline
                .recordings()
                .iter()
                .map(|r| CString::new(r.path().as_os_str().as_bytes()).map_err(|e| e.to_string()))
                .collect::<Result<_, _>>()?;
            let labels: Vec<_> = timeline
                .recordings()
                .iter()
                .map(|r| {
                    CString::new(format!(
                        "{} · {}",
                        r.captured().stamp(),
                        r.captured().origin()
                    ))
                    .map_err(|e| e.to_string())
                })
                .collect::<Result<_, _>>()?;
            let path_ptrs: Vec<_> = paths.iter().map(|s| s.as_ptr()).collect();
            let label_ptrs: Vec<_> = labels.iter().map(|s| s.as_ptr()).collect();
            let durations: Vec<_> = timeline.recordings().iter().map(|r| r.duration()).collect();
            let recorded_at: Vec<_> = timeline
                .recordings()
                .iter()
                .map(|r| r.captured().civil_seconds() as f64)
                .collect();
            let year = CString::new(timeline.year()).map_err(|e| e.to_string())?;
            let status = unsafe {
                photosuite_audio_open(
                    path_ptrs.as_ptr(),
                    label_ptrs.as_ptr(),
                    durations.as_ptr(),
                    recorded_at.as_ptr(),
                    paths.len(),
                    year.as_ptr(),
                    start,
                )
            };
            if status == 0 {
                Ok(())
            } else {
                Err(format!(
                    "Audioplayer konnte nicht geöffnet werden ({status})"
                ))
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (timeline, start);
            Err("Audioplayer derzeit nur unter macOS verfügbar".into())
        }
    }
}

#[cfg(test)]
pub(crate) fn test_wave(path: &Path, seconds: u32, bext: bool) {
    let samples = 8000 * seconds;
    let mut contents = Vec::new();
    contents.extend_from_slice(b"WAVEfmt ");
    contents.extend_from_slice(&16u32.to_le_bytes());
    contents.extend_from_slice(&1u16.to_le_bytes());
    contents.extend_from_slice(&1u16.to_le_bytes());
    contents.extend_from_slice(&8000u32.to_le_bytes());
    contents.extend_from_slice(&16000u32.to_le_bytes());
    contents.extend_from_slice(&2u16.to_le_bytes());
    contents.extend_from_slice(&16u16.to_le_bytes());
    if bext {
        let mut data = vec![0u8; 602];
        data[320..330].copy_from_slice(b"2008-09-16");
        data[330..338].copy_from_slice(b"03:58:11");
        contents.extend_from_slice(b"bext");
        contents.extend_from_slice(&602u32.to_le_bytes());
        contents.extend_from_slice(&data);
    }
    contents.extend_from_slice(b"data");
    contents.extend_from_slice(&(samples * 2).to_le_bytes());
    contents.resize(contents.len() + samples as usize * 2, 0);
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(contents.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&contents);
    std::fs::write(path, bytes).unwrap();
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn native_wave_duration_and_real_broadcast_date_are_read_without_modification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recording.wav");
        test_wave(&path, 2, true);
        let before = std::fs::read(&path).unwrap();
        let source = NativeAudioSource {
            tool: crate::exiftool::ExifTool::development(),
        };
        let info = source.probe(&path).unwrap();
        assert!(info.audio_only);
        assert!((info.duration - 2.).abs() < 0.01);
        let date = source.capture_time(&path).unwrap();
        assert_eq!(date.stamp(), "20080916_035811");
        assert!(date.origin().contains("RIFF:DateTimeOriginal"));
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
    #[test]
    fn undated_audio_uses_explicitly_labelled_birth_date_and_audio_mp4_is_recognized() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recording.wav");
        test_wave(&path, 1, false);
        let source = NativeAudioSource {
            tool: crate::exiftool::ExifTool::development(),
        };
        assert!(
            source
                .capture_time(&path)
                .unwrap()
                .origin()
                .contains("FALLBACK")
        );
        let m4a = dir.path().join("recording.m4a");
        assert!(
            std::process::Command::new("/usr/bin/afconvert")
                .args(["-f", "m4af", "-d", "aac"])
                .arg(&path)
                .arg(&m4a)
                .status()
                .unwrap()
                .success()
        );
        let mp4 = dir.path().join("recording.mp4");
        std::fs::rename(m4a, &mp4).unwrap();
        assert!(source.probe(&mp4).unwrap().audio_only);
        let repository = crate::infrastructure::FileSystemRepository {
            audio: std::sync::Arc::new(source),
        };
        let (items, errors) = crate::ports::MediaRepository::scan(&repository, dir.path());
        assert_eq!(errors, 0);
        assert!(items.iter().find(|m| m.path() == mp4).unwrap().is_audio());
    }
}
