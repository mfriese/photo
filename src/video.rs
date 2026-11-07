//! Native platform adapter. No external video executable is launched.
use crate::ports::DecodedImage;
use std::path::Path;
#[cfg(target_os = "macos")]
fn frame_at(path: &Path, seconds: f64) -> Result<crate::ports::VideoFrame, String> {
    use std::{
        ffi::{CString, c_char, c_int, c_void},
        os::unix::ffi::OsStrExt,
    };
    unsafe extern "C" {
        fn photosuite_video_frame(
            path: *const c_char,
            seconds: f64,
            actual_seconds: *mut f64,
            bytes: *mut *mut u8,
            length: *mut usize,
        ) -> c_int;
        fn photosuite_free(bytes: *mut c_void);
    }
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    let mut bytes = std::ptr::null_mut();
    let mut length = 0;
    // Native adapter returns a malloc allocation valid until explicitly freed.
    let mut actual = seconds;
    let status = unsafe {
        photosuite_video_frame(path.as_ptr(), seconds, &mut actual, &mut bytes, &mut length)
    };
    if status != 0 {
        return Err(format!(
            "Keine Videovorschau: Codec oder Datei nicht unterstützt ({status})"
        ));
    }
    let decoded = image::load_from_memory(unsafe { std::slice::from_raw_parts(bytes, length) });
    unsafe {
        photosuite_free(bytes.cast());
    }
    let rgba = decoded.map_err(|e| e.to_string())?.to_rgba8();
    Ok(crate::ports::VideoFrame {
        seconds: actual,
        image: DecodedImage {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        },
    })
}
pub struct NativeVideoSource;
impl crate::ports::VideoPreviewSource for NativeVideoSource {
    fn duration(&self, path: &Path) -> Result<f64, String> {
        #[cfg(target_os = "macos")]
        {
            use std::{ffi::CString, os::unix::ffi::OsStrExt};
            unsafe extern "C" {
                fn photosuite_video_duration(path: *const std::ffi::c_char) -> f64;
            }
            let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
            let duration = unsafe { photosuite_video_duration(path.as_ptr()) };
            if duration.is_finite() && duration > 0. {
                Ok(duration)
            } else {
                Err("Videodauer nicht lesbar".into())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Err("Videovorschau derzeit nur auf macOS verfügbar".into())
        }
    }
    fn frame_at(&self, path: &Path, seconds: f64) -> Result<crate::ports::VideoFrame, String> {
        #[cfg(target_os = "macos")]
        {
            frame_at(path, seconds)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (path, seconds);
            Err("Videovorschau derzeit nur auf macOS verfügbar".into())
        }
    }
}
#[cfg(test)]
fn preview(path: &Path) -> Result<DecodedImage, String> {
    use crate::ports::VideoPreviewSource;
    NativeVideoSource.frame_at(path, 0.).map(|f| f.image)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn native_video_preview_decodes_real_mp4_without_ffmpeg() {
        let frame = preview(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/preview.mp4"
        )))
        .unwrap();
        assert_eq!((frame.width, frame.height), (96, 64));
        assert_eq!(frame.rgba.len(), 96 * 64 * 4);
        assert!(frame.rgba[2] > 200);
    }
    #[test]
    fn invalid_video_returns_error() {
        assert!(preview(Path::new("/nonexistent/photosuite-test.mp4")).is_err());
    }
}

#[cfg(all(test, target_os = "macos"))]
mod storyboard_tests {
    use super::*;
    use photosuite_application::storyboard::{self, Event};
    use std::sync::Arc;
    #[test]
    fn native_storyboard_decodes_six_different_frames_with_timestamps() {
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/storyboard.mp4"
        ));
        let rx = storyboard::load(Arc::new(NativeVideoSource), path.into());
        let mut frames = Vec::new();
        let mut duration = 0.;
        loop {
            match rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap() {
                Event::Started(d) => duration = d,
                Event::Frame(index, result) => {
                    assert_eq!(index, frames.len());
                    frames.push(result.unwrap());
                }
                Event::Failed(e) => panic!("{e}"),
                Event::Done => break,
            }
        }
        assert_eq!(frames.len(), 6);
        assert!((duration - 6.).abs() < 0.01);
        for pair in frames.windows(2) {
            assert!(pair[0].seconds < pair[1].seconds);
            assert_ne!(pair[0].image.rgba, pair[1].image.rgba);
        }
        for frame in frames {
            assert!((0.0..duration).contains(&frame.seconds));
            assert_eq!((frame.image.width, frame.image.height), (96, 64));
        }
    }
}

pub struct NativeVideoPlayer;
impl crate::ports::VideoPlayer for NativeVideoPlayer {
    fn play(&self, path: &Path) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            use std::{ffi::CString, os::unix::ffi::OsStrExt};
            unsafe extern "C" {
                fn photosuite_video_play(path: *const std::ffi::c_char) -> i32;
            }
            let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
            // Called on eframe's main UI thread. Native loading is asynchronous.
            match unsafe { photosuite_video_play(path.as_ptr()) } {
                0 => Ok(()),
                1 => Err("Videoplayer muss auf dem UI-Thread gestartet werden".into()),
                _ => Err("Videodatei kann nicht geöffnet werden".into()),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Err("Integrierte Wiedergabe derzeit nur auf macOS verfügbar".into())
        }
    }
}
