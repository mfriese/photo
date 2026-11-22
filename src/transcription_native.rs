//! whisper-rs adapter with bounded AVFoundation PCM windows and RAII power assertions.
use photosuite_application::transcription::{
    ModelTransport,
    queue::{AwakeLease, Engine, EngineOutput, ModelChoice, Phase, PowerManager, WorkProgress},
};
use std::{
    path::Path,
    sync::{Arc, Mutex, atomic::AtomicBool},
};
pub struct NativeEngine {
    transport: Arc<dyn ModelTransport>,
    cache: Mutex<Option<ModelHandle>>,
}
impl NativeEngine {
    pub fn new(transport: Arc<dyn ModelTransport>) -> Self {
        Self {
            transport,
            cache: Mutex::new(None),
        }
    }
}
pub struct NativePower;
#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use crate::domain::transcript::Segment;
    use std::{
        ffi::{CStr, CString, c_char, c_void},
        os::unix::ffi::OsStrExt,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::SystemTime,
    };
    const WINDOW: usize = 16000 * 60 * 5;
    pub(super) struct ModelHandle {
        context: whisper_rs::WhisperContext,
        path: PathBuf,
        bytes: u64,
        modified: Option<SystemTime>,
    }
    unsafe extern "C" {
        fn photosuite_transcription_audio(
            path: *const c_char,
            samples_context: *mut c_void,
            samples: unsafe extern "C" fn(*mut c_void, *const f32, usize) -> i32,
            control_context: *mut c_void,
            cancel: unsafe extern "C" fn(*mut c_void) -> i32,
            duration: *mut f64,
            error: *mut c_char,
            capacity: usize,
        ) -> i32;
        fn photosuite_transcription_awake() -> u32;
        fn photosuite_transcription_allow_sleep(id: u32);
    }
    struct Lease(u32);
    impl AwakeLease for Lease {}
    impl Drop for Lease {
        fn drop(&mut self) {
            unsafe { photosuite_transcription_allow_sleep(self.0) }
        }
    }
    impl PowerManager for NativePower {
        fn acquire(&self) -> Result<Arc<dyn AwakeLease>, String> {
            let id = unsafe { photosuite_transcription_awake() };
            if id == 0 {
                Err("macOS hat die Ruhezustandssperre abgelehnt".into())
            } else {
                Ok(Arc::new(Lease(id)))
            }
        }
    }
    struct Bridge<'a> {
        cancel: &'a AtomicBool,
        notify: &'a (dyn Fn(WorkProgress) + Sync),
        failed: AtomicBool,
        total_ms: AtomicU64,
        start_ms: AtomicU64,
        chunk_ms: AtomicU64,
        segments: Mutex<Vec<Segment>>,
    }
    unsafe extern "C" fn cancelled(raw: *mut c_void) -> i32 {
        let b = unsafe { &*raw.cast::<Bridge<'_>>() };
        i32::from(b.cancel.load(Ordering::Relaxed) || b.failed.load(Ordering::Relaxed))
    }
    unsafe extern "C" fn progress(raw: *mut c_void, percent: i32) {
        let b = unsafe { &*raw.cast::<Bridge<'_>>() };
        let fraction = (b.start_ms.load(Ordering::Relaxed) as f64
            + b.chunk_ms.load(Ordering::Relaxed) as f64 * percent.clamp(0, 100) as f64 / 100.)
            / b.total_ms.load(Ordering::Relaxed).max(1) as f64;
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (b.notify)(WorkProgress {
                phase: Phase::Transcribing,
                fraction: fraction.clamp(0., 1.) as f32,
                finding: None,
            })
        }))
        .is_err()
        {
            b.failed.store(true, Ordering::Relaxed);
        }
    }
    // The callbacks borrow Bridge only during synchronous WhisperState::full.
    // Shared state is atomic/locked; no callback mutates the whisper context.
    unsafe extern "C" fn whisper_cancelled(raw: *mut c_void) -> bool {
        unsafe { cancelled(raw) != 0 }
    }
    unsafe extern "C" fn whisper_progress(
        _: *mut whisper_rs::WhisperSysContext,
        _: *mut whisper_rs::WhisperSysState,
        percent: i32,
        raw: *mut c_void,
    ) {
        unsafe { progress(raw, percent) }
    }
    unsafe extern "C" fn whisper_segments(
        _: *mut whisper_rs::WhisperSysContext,
        state: *mut whisper_rs::WhisperSysState,
        count: i32,
        raw: *mut c_void,
    ) {
        let bridge = unsafe { &*raw.cast::<Bridge<'_>>() };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Called synchronously by whisper after finalized segments; read only.
            let total =
                unsafe { whisper_rs::whisper_rs_sys::whisper_full_n_segments_from_state(state) };
            for index in (total - count.max(0)).max(0)..total {
                if bridge.cancel.load(Ordering::Relaxed) {
                    break;
                }
                let text = unsafe {
                    whisper_rs::whisper_rs_sys::whisper_full_get_segment_text_from_state(
                        state, index,
                    )
                };
                if text.is_null() {
                    continue;
                }
                let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                (bridge.notify)(WorkProgress {
                    phase: Phase::Transcribing,
                    fraction: ((bridge.start_ms.load(Ordering::Relaxed)
                        + (unsafe {
                            whisper_rs::whisper_rs_sys::whisper_full_get_segment_t1_from_state(
                                state, index,
                            )
                        }
                        .max(0) as u64)
                            .saturating_mul(10)
                            .min(bridge.chunk_ms.load(Ordering::Relaxed)))
                        as f64
                        / bridge.total_ms.load(Ordering::Relaxed).max(1) as f64)
                        .clamp(0., 1.) as f32,
                    finding: Some(text.chars().take(2000).collect()),
                });
            }
        }));
        if result.is_err() {
            bridge.failed.store(true, Ordering::Relaxed);
        }
    }
    struct Decode<'a> {
        engine: &'a NativeEngine,
        cache: &'a mut Option<ModelHandle>,
        choice: &'a ModelChoice,
        bridge: &'a Bridge<'a>,
        buffer: Vec<f32>,
        processed: u64,
        duration: *const f64,
        language: String,
        error: Option<String>,
        repetitions: photosuite_application::transcription::quality::RepetitionGuard,
    }
    impl Decode<'_> {
        fn ensure_model(&mut self) -> Result<(), String> {
            let meta = std::fs::symlink_metadata(&self.choice.path).map_err(|e| e.to_string())?;
            if !meta.is_file()
                || meta.file_type().is_symlink()
                || meta.len() != self.choice.settings.model().bytes
            {
                return Err(
                    "Whisper-Modell fehlt oder wurde verändert; in Settings erneut vorbereiten"
                        .into(),
                );
            }
            if self.cache.as_ref().is_some_and(|m| {
                m.path == self.choice.path
                    && m.bytes == meta.len()
                    && m.modified == meta.modified().ok()
            }) {
                return Ok(());
            }
            *self.cache = None;
            (self.bridge.notify)(WorkProgress {
                phase: Phase::LoadingModel,
                fraction: 0.,
                finding: None,
            });
            let digest =
                self.engine
                    .transport
                    .sha256(&self.choice.path, self.bridge.cancel, &|_| {})?;
            if digest != self.choice.settings.model().sha256 {
                return Err("Whisper-Modell-Prüfsumme stimmt nicht".into());
            }
            if self.bridge.cancel.load(Ordering::Relaxed) {
                return Err("Abgebrochen".into());
            }
            let mut params = whisper_rs::WhisperContextParameters::default();
            params.use_gpu(true);
            let context = whisper_rs::WhisperContext::new_with_params(&self.choice.path, params)
                .or_else(|_| {
                    let mut params = whisper_rs::WhisperContextParameters::default();
                    params.use_gpu(false);
                    whisper_rs::WhisperContext::new_with_params(&self.choice.path, params)
                })
                .map_err(|e| format!("Whisper-Modell konnte nicht geladen werden: {e}"))?;
            *self.cache = Some(ModelHandle {
                context,
                path: self.choice.path.clone(),
                bytes: meta.len(),
                modified: meta.modified().ok(),
            });
            Ok(())
        }
        fn flush(&mut self) -> Result<(), String> {
            if self.buffer.is_empty() {
                return Ok(());
            }
            if self.buffer.iter().all(|v| v.abs() <= 0.000001) {
                self.processed += self.buffer.len() as u64;
                self.buffer.clear();
                return Ok(());
            }
            self.ensure_model()?;
            if self.bridge.cancel.load(Ordering::Relaxed) {
                return Err("Abgebrochen".into());
            }
            let duration = unsafe { *self.duration };
            let total_ms = if duration.is_finite() && duration > 0. {
                (duration * 1000.) as u64
            } else {
                (self.processed + self.buffer.len() as u64) / 16
            };
            self.bridge.total_ms.store(total_ms, Ordering::Relaxed);
            self.bridge
                .start_ms
                .store(self.processed / 16, Ordering::Relaxed);
            self.bridge
                .chunk_ms
                .store(self.buffer.len() as u64 / 16, Ordering::Relaxed);
            let mut params =
                whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
            let threads = std::thread::available_parallelism().map_or(1, |n| n.get() / 2);
            params.set_n_threads(threads.clamp(1, 4) as i32);
            params.set_language(Some(self.choice.settings.language()));
            params.set_no_context(true);
            // Decode each chunk independently. Feeding unverified output back as
            // a prompt can propagate hallucinations into subsequent chunks.
            // Keep decoding conservative instead of retrying at higher temperatures.
            params.set_temperature(0.0);
            params.set_temperature_inc(0.0);
            params.set_suppress_blank(true);
            // Pin the library's established thresholds; do not tighten them
            // without speech samples, which could otherwise hide quiet speech.
            params.set_no_speech_thold(0.6);
            params.set_logprob_thold(-1.0);
            params.set_entropy_thold(2.4);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_print_special(false);
            let raw = (self.bridge as *const Bridge<'_>).cast_mut().cast();
            // SAFETY: full is synchronous, params are not cloned or retained. Bridge
            // outlives state/full, accepts concurrent callbacks and contains panics.
            // The raw API avoids owned 'static closures for these borrowed callbacks.
            unsafe {
                params.set_abort_callback(Some(whisper_cancelled));
                params.set_abort_callback_user_data(raw);
                params.set_progress_callback(Some(whisper_progress));
                params.set_progress_callback_user_data(raw);
                params.set_new_segment_callback(Some(whisper_segments));
                params.set_new_segment_callback_user_data(raw);
            }
            let mut state = self
                .cache
                .as_ref()
                .unwrap()
                .context
                .create_state()
                .map_err(|e| format!("Whisper-Zustand konnte nicht erstellt werden: {e}"))?;
            let result = state.full(params, &self.buffer);
            if self.bridge.cancel.load(Ordering::Relaxed)
                || self.bridge.failed.load(Ordering::Relaxed)
            {
                return Err("Abgebrochen oder Fortschrittsmeldung fehlgeschlagen".into());
            }
            result.map_err(|e| format!("Whisper-Transkription fehlgeschlagen: {e}"))?;
            if self.language.is_empty() {
                self.language = whisper_rs::get_lang_str(state.full_lang_id_from_state())
                    .unwrap_or(self.choice.settings.language())
                    .to_owned();
            }
            let mut values = self
                .bridge
                .segments
                .lock()
                .map_err(|_| "Transkriptabschnitte nicht verfügbar")?;
            let mut text_bytes = values.iter().map(|s| s.text().len()).sum::<usize>();
            let offset = self.bridge.start_ms.load(Ordering::Relaxed);
            let duration = self.bridge.chunk_ms.load(Ordering::Relaxed);
            for segment in state.as_iter() {
                let text = segment
                    .to_str_lossy()
                    .map_err(|e| e.to_string())?
                    .trim()
                    .to_owned();
                if text.is_empty() {
                    continue;
                }
                text_bytes += text.len();
                if values.len() >= 50000 || text_bytes > 2 * 1024 * 1024 {
                    return Err("Transkript ist zu umfangreich".into());
                }
                let start = (segment.start_timestamp().max(0) as u64)
                    .saturating_mul(10)
                    .min(duration);
                let end = (segment.end_timestamp().max(0) as u64)
                    .saturating_mul(10)
                    .min(duration)
                    .max(start);
                self.repetitions.push_text(&text)?;
                values.push(Segment::new(offset + start, offset + end, text)?);
            }
            drop(values);
            self.processed += self.buffer.len() as u64;
            self.buffer.clear();
            Ok(())
        }
    }
    unsafe extern "C" fn samples(raw: *mut c_void, data: *const f32, count: usize) -> i32 {
        let state = unsafe { &mut *raw.cast::<Decode<'_>>() };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if count > 16 * 1024 * 1024 {
                return Err("Audioblock überschreitet das Limit".to_owned());
            }
            let mut values = unsafe { std::slice::from_raw_parts(data, count) };
            while !values.is_empty() {
                let take = (WINDOW - state.buffer.len()).min(values.len());
                state.buffer.extend(
                    values[..take]
                        .iter()
                        .map(|v| if v.is_finite() { v.clamp(-1., 1.) } else { 0. }),
                );
                values = &values[take..];
                if state.buffer.len() == WINDOW {
                    state.flush()?;
                }
                if state.bridge.cancel.load(Ordering::Relaxed) {
                    return Err("Abgebrochen".into());
                }
            }
            Ok::<(), String>(())
        }));
        match result {
            Ok(Ok(())) => 0,
            Ok(Err(error)) => {
                state.error = Some(error);
                1
            }
            Err(_) => {
                state.error = Some("Audioverarbeitung unerwartet beendet".into());
                1
            }
        }
    }
    impl Engine for NativeEngine {
        fn transcribe(
            &self,
            choice: &ModelChoice,
            path: &Path,
            cancel: &AtomicBool,
            notify: &(dyn Fn(WorkProgress) + Sync),
        ) -> Result<EngineOutput, String> {
            let bridge = Bridge {
                cancel,
                notify,
                failed: AtomicBool::new(false),
                total_ms: AtomicU64::new(0),
                start_ms: AtomicU64::new(0),
                chunk_ms: AtomicU64::new(0),
                segments: Mutex::new(Vec::new()),
            };
            let mut cache = self
                .cache
                .lock()
                .map_err(|_| "Whisper-Engine nicht verfügbar")?;
            let duration = std::cell::UnsafeCell::new(0.);
            let mut error = [0i8; 2048];
            let mut decode = Decode {
                engine: self,
                cache: &mut cache,
                choice,
                bridge: &bridge,
                buffer: Vec::with_capacity(WINDOW),
                processed: 0,
                duration: duration.get(),
                language: String::new(),
                error: None,
                repetitions: Default::default(),
            };
            let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
            let status = unsafe {
                photosuite_transcription_audio(
                    path.as_ptr(),
                    (&mut decode as *mut Decode<'_>).cast(),
                    samples,
                    (&bridge as *const Bridge<'_>).cast_mut().cast(),
                    cancelled,
                    duration.get(),
                    error.as_mut_ptr(),
                    error.len(),
                )
            };
            if let Some(error) = decode.error.take() {
                return Err(error);
            }
            if cancel.load(Ordering::Relaxed) {
                return Err("Abgebrochen".into());
            }
            if status == 1 {
                return Ok(EngineOutput {
                    language: choice.settings.language().into(),
                    segments: Vec::new(),
                    no_audio: true,
                });
            }
            if status != 0 {
                return Err(unsafe { CStr::from_ptr(error.as_ptr()) }
                    .to_string_lossy()
                    .into_owned());
            }
            decode.flush()?;
            let language = if decode.language.is_empty() {
                choice.settings.language().into()
            } else {
                std::mem::take(&mut decode.language)
            };
            drop(decode);
            let mut segments = bridge
                .segments
                .into_inner()
                .map_err(|_| "Transkript nicht verfügbar")?;
            segments.sort_by_key(|s| s.start_ms());
            Ok(EngineOutput {
                language,
                segments,
                no_audio: false,
            })
        }
        fn release_model(&self) {
            // A Rust callback panic must not leave Metal allocations alive until static teardown.
            self.cache
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
        }
    }
}
#[cfg(target_os = "macos")]
use platform::ModelHandle;
#[cfg(not(target_os = "macos"))]
struct ModelHandle;
#[cfg(not(target_os = "macos"))]
impl Engine for NativeEngine {
    fn transcribe(
        &self,
        _: &ModelChoice,
        _: &Path,
        _: &AtomicBool,
        _: &(dyn Fn(WorkProgress) + Sync),
    ) -> Result<EngineOutput, String> {
        Err("Transkription derzeit nur auf macOS unterstützt".into())
    }
}
#[cfg(not(target_os = "macos"))]
impl PowerManager for NativePower {
    fn acquire(&self) -> Result<Arc<dyn AwakeLease>, String> {
        Err("Ruhezustandssperre derzeit nur auf macOS unterstützt".into())
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires WHISPER_TEST_MODEL; runs native inference in isolated subprocesses"]
    fn native_queue_shutdown_exits_cleanly_with_active_and_cached_resources() {
        use std::{
            io::{Read, Seek, SeekFrom},
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        for mode in ["active", "cached"] {
            let mut log = tempfile::tempfile().unwrap();
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "transcription_native::tests::native_shutdown_child",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("PHOTOSUITE_SHUTDOWN_CHILD", mode)
                .env_remove("GGML_BACKTRACE_LLDB")
                .stdout(Stdio::from(log.try_clone().unwrap()))
                .stderr(Stdio::from(log.try_clone().unwrap()))
                .spawn()
                .unwrap();
            let limit = Instant::now() + Duration::from_secs(180);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if Instant::now() >= limit {
                    child.kill().unwrap();
                    let _ = child.wait();
                    panic!("native shutdown subprocess timed out: {mode}");
                }
                std::thread::sleep(Duration::from_millis(25));
            };
            log.seek(SeekFrom::Start(0)).unwrap();
            let mut output = String::new();
            log.take(2 * 1024 * 1024)
                .read_to_string(&mut output)
                .unwrap();
            assert!(
                status.success(),
                "native process exit failed ({mode}): {status}\n{output}"
            );
            assert!(
                output.contains("SHUTDOWN_OK"),
                "child did not complete shutdown: {output}"
            );
            assert!(
                output.contains("Metal"),
                "native shutdown test did not exercise Metal: {output}"
            );
            println!("Whisper subprocess {mode}: clean Metal process exit");
        }
    }
    #[test]
    #[ignore = "Subprocess helper; never downloads models"]
    fn native_shutdown_child() {
        use crate::domain::tags::LibraryFile;
        use photosuite_application::transcription::{
            Settings,
            queue::{Candidate, Mode, QueueSession},
        };
        use std::time::{Duration, Instant};
        let Ok(mode) = std::env::var("PHOTOSUITE_SHUTDOWN_CHILD") else {
            return;
        };
        let model = std::env::var_os("WHISPER_TEST_MODEL").expect("WHISPER_TEST_MODEL");
        let id = std::env::var("WHISPER_TEST_MODEL_ID").unwrap_or("tiny-q5_1".into());
        let choice = ModelChoice {
            path: model.into(),
            settings: Settings::new(&id, "en").unwrap(),
        };
        let engine = Arc::new(NativeEngine::new(Arc::new(
            crate::whisper_models_native::NativeModelTransport,
        )));
        let folder = tempfile::tempdir().unwrap();
        let source = LibraryFile::new("2026/audio/diary/aud_20261121_120000.wav".into()).unwrap();
        let path = folder.path().join(source.as_str());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jfk.wav"),
            &path,
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut queue = QueueSession::new(
            Arc::new(crate::transcription_queue_fs::SqliteTranscriptionQueue::default()),
            engine.clone(),
            Arc::new(NativePower),
        );
        if mode == "active" {
            queue.sync(
                folder.path(),
                vec![Candidate {
                    source: source.clone(),
                    record: source,
                }],
                true,
            );
            queue.set_model(Some(choice));
            let limit = Instant::now() + Duration::from_secs(150);
            loop {
                queue.poll();
                assert!(queue.error().is_empty(), "{}", queue.error());
                if queue.progress().phase == Phase::Transcribing && queue.progress().fraction > 0. {
                    break;
                }
                assert!(Instant::now() < limit, "inference did not start");
                std::thread::sleep(Duration::from_millis(5));
            }
            queue.shutdown().unwrap();
            assert!(!queue.busy());
        } else {
            let output = engine
                .transcribe(&choice, &path, &AtomicBool::new(false), &|_| {})
                .unwrap();
            assert!(!output.no_audio);
            assert!(engine.cache.lock().unwrap().is_some());
            queue.shutdown().unwrap();
        }
        assert!(engine.cache.lock().unwrap().is_none());
        queue.set_mode(Mode::Running);
        queue.poll();
        assert!(!queue.busy());
        assert_eq!(std::fs::read(path).unwrap(), before);
        println!("SHUTDOWN_OK {mode}");
    }
    #[test]
    #[ignore = "Diagnostic integration: requires WHISPER_TEST_MODEL and WHISPER_TEST_MEDIA; uses only temporary output"]
    fn supplied_media_transcript_survives_yaml_round_trip() {
        use crate::domain::{
            tags::LibraryFile,
            transcript::{Transcript, TranscriptStatus},
        };
        use photosuite_application::transcription::queue::{Candidate, QueueStore};
        let original = std::path::PathBuf::from(
            std::env::var_os("WHISPER_TEST_MEDIA").expect("WHISPER_TEST_MEDIA"),
        );
        let before = std::fs::read(&original).unwrap();
        let folder = tempfile::tempdir().unwrap();
        let source = LibraryFile::new("2026/11/img_20261115_120000.mp4".into()).unwrap();
        let path = folder.path().join(source.as_str());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &before).unwrap();
        let model = std::env::var("WHISPER_TEST_MODEL_ID").unwrap_or("large-v3-turbo".into());
        let choice = ModelChoice {
            path: std::env::var_os("WHISPER_TEST_MODEL")
                .expect("WHISPER_TEST_MODEL")
                .into(),
            settings: photosuite_application::transcription::Settings::new(&model, "de").unwrap(),
        };
        let engine =
            NativeEngine::new(Arc::new(crate::whisper_models_native::NativeModelTransport));
        let output = engine
            .transcribe(&choice, &path, &AtomicBool::new(false), &|_| {})
            .expect("native inference");
        let text = output
            .segments
            .iter()
            .map(|s| s.text())
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!(
            "Inference completed: {} segments, {} text bytes",
            output.segments.len(),
            text.len()
        );
        let store = crate::transcription_queue_fs::SqliteTranscriptionQueue::default();
        let value = Transcript::new(
            store.version(folder.path(), &source).unwrap(),
            if output.no_audio {
                TranscriptStatus::NoAudio
            } else if text.is_empty() {
                TranscriptStatus::NoSpeech
            } else {
                TranscriptStatus::Complete
            },
            model,
            output.language,
            text,
            output.segments,
        )
        .unwrap();
        let candidate = Candidate {
            source: source.clone(),
            record: source,
        };
        store
            .write_transcript(folder.path(), &candidate, &value)
            .expect("publish transcript YAML");
        assert_eq!(
            store.existing(folder.path(), &candidate).unwrap().as_ref(),
            Some(&value)
        );
        assert_eq!(std::fs::read(&original).unwrap(), before);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        engine.release_model();
    }

    #[test]
    #[ignore = "Requires WHISPER_TEST_MODEL pointing to verified tiny-q5_1 weights; never downloads automatically"]
    fn native_whisper_transcribes_real_speech_and_mpeg4_audio_without_modifying_sources() {
        let model = std::path::PathBuf::from(
            std::env::var_os("WHISPER_TEST_MODEL").expect("WHISPER_TEST_MODEL"),
        );
        let audio =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jfk.wav");
        let before = std::fs::read(&audio).unwrap();
        let engine =
            NativeEngine::new(Arc::new(crate::whisper_models_native::NativeModelTransport));
        let choice = ModelChoice {
            path: model,
            settings: photosuite_application::transcription::Settings::new("tiny-q5_1", "en")
                .unwrap(),
        };
        let stop = AtomicBool::new(false);
        let progress_seen = AtomicBool::new(false);
        let finding_seen = AtomicBool::new(false);
        let result = engine
            .transcribe(&choice, &audio, &stop, &|p| {
                if p.finding.as_ref().is_some_and(|text| !text.is_empty()) {
                    finding_seen.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                if p.phase == Phase::Transcribing {
                    progress_seen.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            })
            .unwrap();
        assert!(progress_seen.load(std::sync::atomic::Ordering::Relaxed));
        assert!(finding_seen.load(std::sync::atomic::Ordering::Relaxed));
        let text = result
            .segments
            .iter()
            .map(|s| s.text())
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        assert!(text.contains("country"), "unexpected speech: {text}");
        assert!(!result.no_audio);
        assert_eq!(std::fs::read(&audio).unwrap(), before);
        let folder = tempfile::tempdir().unwrap();
        let m4a = folder.path().join("speech.m4a");
        assert!(
            std::process::Command::new("/usr/bin/afconvert")
                .args(["-f", "m4af", "-d", "aac"])
                .arg(&audio)
                .arg(&m4a)
                .status()
                .unwrap()
                .success()
        );
        let mp4 = folder.path().join("speech.mp4");
        std::fs::rename(m4a, &mp4).unwrap();
        let result = engine.transcribe(&choice, &mp4, &stop, &|_| {}).unwrap();
        assert!(
            result
                .segments
                .iter()
                .any(|s| s.text().to_lowercase().contains("country"))
        );
        // Abort through the actual whisper-rs callback, then reuse the context.
        let cancelled = engine.transcribe(&choice, &audio, &stop, &|p| {
            if p.phase == Phase::Transcribing {
                stop.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
        assert!(cancelled.is_err());
        assert!(stop.swap(false, std::sync::atomic::Ordering::Relaxed));
        let automatic = ModelChoice {
            path: choice.path.clone(),
            settings: photosuite_application::transcription::Settings::new("tiny-q5_1", "auto")
                .unwrap(),
        };
        let result = engine
            .transcribe(&automatic, &audio, &stop, &|_| {})
            .unwrap();
        assert_eq!(result.language, "en");
        assert!(
            result
                .segments
                .iter()
                .any(|s| s.text().to_lowercase().contains("country"))
        );
        assert_eq!(std::fs::read(&audio).unwrap(), before);
        engine.release_model();
        assert!(engine.cache.lock().unwrap().is_none());
    }
}

#[cfg(all(test, target_os = "macos"))]
mod media_tests {
    use super::*;
    #[test]
    fn silent_audio_and_video_without_audio_are_valid_empty_results_without_loading_a_model() {
        let engine =
            NativeEngine::new(Arc::new(crate::whisper_models_native::NativeModelTransport));
        let model = ModelChoice {
            path: "/nonexistent/no-model-needed-for-empty-media.bin".into(),
            settings: photosuite_application::transcription::Settings::new("tiny", "en").unwrap(),
        };
        let directory = tempfile::tempdir().unwrap();
        let wave = directory.path().join("silence.wav");
        crate::audio::test_wave(&wave, 1, false);
        let audio = engine
            .transcribe(&model, &wave, &AtomicBool::new(false), &|_| {})
            .unwrap();
        assert!(!audio.no_audio);
        assert!(audio.segments.is_empty());
        let video =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/preview.mp4");
        let output = engine
            .transcribe(&model, &video, &AtomicBool::new(false), &|_| {})
            .unwrap();
        assert!(output.no_audio);
        assert!(output.segments.is_empty());
        assert!(engine.cache.lock().unwrap().is_none());
    }
}
