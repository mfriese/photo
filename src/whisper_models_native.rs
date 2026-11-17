//! macOS Foundation HTTPS streaming and CommonCrypto SHA-256. No external executable.
#[cfg(target_os = "macos")]
use photosuite_application::transcription::Stage;
use photosuite_application::transcription::{ModelTransport, Progress};
use std::{path::Path, sync::atomic::AtomicBool};
pub struct NativeModelTransport;
#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::{
        ffi::{CStr, CString, c_void},
        fs::{File, OpenOptions},
        os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
        sync::atomic::Ordering,
    };
    struct Context<'a> {
        cancel: &'a AtomicBool,
        progress: &'a (dyn Fn(Progress) + Sync),
        stage: Stage,
        total: u64,
        panicked: AtomicBool,
    }
    unsafe extern "C" fn progress(raw: *mut c_void, bytes: u64) {
        let state = unsafe { &*raw.cast::<Context<'_>>() };
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (state.progress)(Progress {
                stage: state.stage,
                bytes,
                total: state.total,
            })
        }))
        .is_err()
        {
            state.panicked.store(true, Ordering::Relaxed);
        }
    }
    unsafe extern "C" fn cancel(raw: *mut c_void) -> i32 {
        let state = unsafe { &*raw.cast::<Context<'_>>() };
        i32::from(state.panicked.load(Ordering::Relaxed) || state.cancel.load(Ordering::Relaxed))
    }
    unsafe extern "C" {
        fn photosuite_asset_download(
            url: *const std::ffi::c_char,
            fd: i32,
            expected: u64,
            limit: u64,
            context: *mut c_void,
            progress: unsafe extern "C" fn(*mut c_void, u64),
            cancel: unsafe extern "C" fn(*mut c_void) -> i32,
            error: *mut std::ffi::c_char,
            capacity: usize,
        ) -> i32;
        fn photosuite_model_sha256(
            fd: i32,
            context: *mut c_void,
            progress: unsafe extern "C" fn(*mut c_void, u64),
            cancel: unsafe extern "C" fn(*mut c_void) -> i32,
            digest: *mut std::ffi::c_char,
            error: *mut std::ffi::c_char,
            capacity: usize,
        ) -> i32;
    }
    fn checked(path: &Path, write: bool) -> Result<File, String> {
        let file = OpenOptions::new()
            .read(!write)
            .write(write)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Modell benötigt eine reguläre Datei".into());
        }
        if write {
            file.set_len(0).map_err(|e| e.to_string())?;
        }
        Ok(file)
    }
    fn download(
        url: &str,
        path: &Path,
        expected: u64,
        limit: u64,
        stop: &AtomicBool,
        notify: &(dyn Fn(Progress) + Sync),
    ) -> Result<(), String> {
        if stop.load(Ordering::Relaxed) {
            return Err("Abgebrochen".into());
        }
        if !url.starts_with("https://") {
            return Err("Nur HTTPS-Modellquellen sind erlaubt".into());
        }
        let file = checked(path, true)?;
        let url = CString::new(url).map_err(|e| e.to_string())?;
        let state = Context {
            cancel: stop,
            progress: notify,
            stage: Stage::Downloading,
            total: expected,
            panicked: AtomicBool::new(false),
        };
        let mut error = [0i8; 1024];
        // Native call completes all delegate writes before returning; file/context remain borrowed.
        let status = unsafe {
            photosuite_asset_download(
                url.as_ptr(),
                file.as_raw_fd(),
                expected,
                limit,
                (&state as *const Context<'_>).cast_mut().cast(),
                progress,
                cancel,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if status != 0 {
            return Err(unsafe { CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned());
        }
        file.sync_all().map_err(|e| e.to_string())
    }
    impl ModelTransport for NativeModelTransport {
        fn download(
            &self,
            url: &str,
            path: &Path,
            expected: u64,
            stop: &AtomicBool,
            notify: &(dyn Fn(Progress) + Sync),
        ) -> Result<(), String> {
            download(url, path, expected, expected, stop, notify)
        }
        fn download_bounded(
            &self,
            url: &str,
            path: &Path,
            limit: u64,
            stop: &AtomicBool,
            notify: &(dyn Fn(Progress) + Sync),
        ) -> Result<(), String> {
            download(url, path, 0, limit, stop, notify)
        }
        fn sha256(
            &self,
            path: &Path,
            stop: &AtomicBool,
            notify: &(dyn Fn(Progress) + Sync),
        ) -> Result<String, String> {
            let file = checked(path, false)?;
            let total = file.metadata().map_err(|e| e.to_string())?.len();
            let state = Context {
                cancel: stop,
                progress: notify,
                stage: Stage::Checking,
                total,
                panicked: AtomicBool::new(false),
            };
            let mut error = [0i8; 1024];
            let mut digest = [0i8; 65];
            let status = unsafe {
                photosuite_model_sha256(
                    file.as_raw_fd(),
                    (&state as *const Context<'_>).cast_mut().cast(),
                    progress,
                    cancel,
                    digest.as_mut_ptr(),
                    error.as_mut_ptr(),
                    error.len(),
                )
            };
            if status != 0 {
                return Err(unsafe { CStr::from_ptr(error.as_ptr()) }
                    .to_string_lossy()
                    .into_owned());
            }
            Ok(unsafe { CStr::from_ptr(digest.as_ptr()) }
                .to_string_lossy()
                .into_owned())
        }
    }
}
#[cfg(not(target_os = "macos"))]
impl ModelTransport for NativeModelTransport {
    fn download(
        &self,
        _: &str,
        _: &Path,
        _: u64,
        _: &AtomicBool,
        _: &(dyn Fn(Progress) + Sync),
    ) -> Result<(), String> {
        Err("Modelldownload derzeit nur auf macOS unterstützt".into())
    }
    fn download_bounded(
        &self,
        _: &str,
        _: &Path,
        _: u64,
        _: &AtomicBool,
        _: &(dyn Fn(Progress) + Sync),
    ) -> Result<(), String> {
        Err("Downloads derzeit nur auf macOS unterstützt".into())
    }
    fn sha256(
        &self,
        _: &Path,
        _: &AtomicBool,
        _: &(dyn Fn(Progress) + Sync),
    ) -> Result<String, String> {
        Err("Modellprüfung derzeit nur auf macOS unterstützt".into())
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    #[test]
    fn native_sha256_matches_known_vector_and_cancellation_stops_before_network() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("vector.bin");
        std::fs::write(&file, b"abc").unwrap();
        let transport = NativeModelTransport;
        let cancelled = AtomicBool::new(false);
        assert_eq!(
            transport.sha256(&file, &cancelled, &|_| {}).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        cancelled.store(true, Ordering::Relaxed);
        assert!(transport.sha256(&file, &cancelled, &|_| {}).is_err());
        assert!(
            transport
                .download(
                    "https://invalid.example/model",
                    &file,
                    10,
                    &cancelled,
                    &|_| {}
                )
                .is_err()
        );
        assert!(
            transport
                .download(
                    "http://invalid.example/model",
                    &file,
                    10,
                    &AtomicBool::new(false),
                    &|_| {}
                )
                .is_err()
        );
        assert_eq!(
            std::fs::read(file).unwrap(),
            b"abc",
            "cancelled/invalid requests must not even truncate a destination"
        );
    }
}
