//! Global transcription preparation only. No audio access or inference is started here.
pub mod catalog;
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    model: String,
    language: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "large-v3-turbo".into(),
            language: "de".into(),
        }
    }
}
impl Settings {
    pub fn new(model: &str, language: &str) -> Result<Self, String> {
        let spec = catalog::model(model).ok_or("Unbekanntes Whisper-Modell")?;
        if !catalog::LANGUAGES.iter().any(|(code, _)| *code == language) {
            return Err("Unbekannte Transkriptionssprache".into());
        }
        if !spec.supports_language(language) {
            return Err("Diese Sprache wird vom ausgewählten Modell nicht unterstützt".into());
        }
        Ok(Self {
            model: model.into(),
            language: language.into(),
        })
    }
    pub fn model(&self) -> &'static catalog::Model {
        catalog::model(&self.model).expect("validated model")
    }
    pub fn language(&self) -> &str {
        &self.language
    }
}
pub use crate::downloads::{AssetTransport as ModelTransport, Progress, Stage};
#[derive(Clone, Debug, Default)]
pub struct Prepared {
    pub settings: Settings,
    pub available: BTreeSet<String>,
    pub ready_path: Option<PathBuf>,
    pub warning: String,
}
#[derive(Clone, Debug)]
pub struct Locations {
    pub models: PathBuf,
    pub preferences: PathBuf,
}
pub trait ModelStore: Send + Sync + 'static {
    /// Pure paths only; must not access the filesystem on the UI thread.
    fn locations(&self) -> Locations;
    fn inspect(
        &self,
        cancel: &AtomicBool,
        progress: &(dyn Fn(Progress) + Sync),
    ) -> Result<Prepared, String>;
    fn prepare(
        &self,
        settings: &Settings,
        cancel: &AtomicBool,
        progress: &(dyn Fn(Progress) + Sync),
    ) -> Result<Prepared, String>;
}
enum Event {
    Progress(Progress),
    Done(Result<Prepared, String>),
}
struct Job {
    cancel: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Event>,
}
pub struct ModelSession {
    store: Arc<dyn ModelStore>,
    job: Option<Job>,
    prepared: Option<Prepared>,
    progress: Progress,
    error: String,
    cancelling: bool,
    requested: bool,
}
impl ModelSession {
    pub fn new(store: Arc<dyn ModelStore>) -> Self {
        Self {
            store,
            job: None,
            prepared: None,
            progress: Progress {
                stage: Stage::Loading,
                bytes: 0,
                total: 0,
            },
            error: String::new(),
            cancelling: false,
            requested: false,
        }
    }
    pub fn prepared(&self) -> Option<&Prepared> {
        self.prepared.as_ref()
    }
    pub fn progress(&self) -> Progress {
        self.progress
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn report_error(&mut self, error: String) {
        self.error = error;
    }
    pub fn cancelling(&self) -> bool {
        self.cancelling
    }
    pub fn ensure_loaded(&mut self) {
        if !self.requested {
            self.reload();
        }
    }
    pub fn locations(&self) -> Locations {
        self.store.locations()
    }
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
    /// Queue controls request a new check without starting concurrent model I/O.
    pub fn defer_reload(&mut self) {
        if !self.busy() {
            self.requested = false;
            self.prepared = None;
            self.error.clear();
        }
    }
    pub fn reload(&mut self) {
        if !self.busy() {
            self.launch(None);
        }
    }
    pub fn apply(&mut self, settings: Settings) -> Result<(), String> {
        if self.busy() {
            return Err("Modellvorbereitung läuft bereits".into());
        }
        self.launch(Some(settings));
        Ok(())
    }
    pub fn cancel(&mut self) {
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
            self.cancelling = true;
        }
    }
    fn launch(&mut self, settings: Option<Settings>) {
        self.requested = true;
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let store = self.store.clone();
        let (tx, receiver) = mpsc::sync_channel(16);
        self.job = Some(Job { cancel, receiver });
        self.error.clear();
        self.cancelling = false;
        self.progress = Progress {
            stage: Stage::Loading,
            bytes: 0,
            total: 0,
        };
        std::thread::spawn(move || {
            let progress = |p| {
                let _ = tx.try_send(Event::Progress(p));
            };
            let result = match settings {
                Some(settings) => store.prepare(&settings, &stop, &progress),
                None => store.inspect(&stop, &progress),
            };
            let _ = tx.send(Event::Done(result));
        });
    }
    pub fn poll(&mut self) {
        let mut done = false;
        if let Some(job) = &self.job {
            for _ in 0..32 {
                match job.receiver.try_recv() {
                    Ok(Event::Progress(p)) => self.progress = p,
                    Ok(Event::Done(result)) => {
                        match result {
                            Ok(value) => self.prepared = Some(value),
                            Err(error) => self.error = error,
                        }
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.error =
                            "Modellvorbereitung unerwartet beendet; erneut versuchen".into();
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if done {
            self.job = None;
            self.cancelling = false;
        }
    }
}
impl Drop for ModelSession {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    #[derive(Default)]
    struct Store {
        reads: AtomicUsize,
        prepares: AtomicUsize,
        block: AtomicBool,
        entered: AtomicBool,
    }
    impl ModelStore for Store {
        fn locations(&self) -> Locations {
            Locations {
                models: "/Global/models".into(),
                preferences: "/Global/settings.plist".into(),
            }
        }
        fn inspect(
            &self,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<Prepared, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(Default::default())
        }
        fn prepare(
            &self,
            settings: &Settings,
            cancel: &AtomicBool,
            notify: &(dyn Fn(Progress) + Sync),
        ) -> Result<Prepared, String> {
            self.prepares.fetch_add(1, Ordering::SeqCst);
            self.entered.store(true, Ordering::SeqCst);
            while self.block.load(Ordering::SeqCst) && !cancel.load(Ordering::Relaxed) {
                notify(Progress {
                    stage: Stage::Downloading,
                    bytes: 10,
                    total: 100,
                });
                std::thread::yield_now();
            }
            if cancel.load(Ordering::Relaxed) {
                return Err("Abgebrochen".into());
            }
            Ok(Prepared {
                settings: settings.clone(),
                ready_path: Some("/Global/models/model.bin".into()),
                ..Default::default()
            })
        }
    }
    fn drain(session: &mut ModelSession) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while session.busy() {
            session.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn catalog_and_language_rules_reject_invalid_or_english_only_mismatches() {
        assert_eq!(Settings::default().model().id, "large-v3-turbo");
        assert_eq!(Settings::default().language(), "de");
        assert!(Settings::new("../model", "de").is_err());
        assert!(Settings::new("tiny", "unknown").is_err());
        assert!(Settings::new("tiny.en", "de").is_err());
        assert!(Settings::new("tiny.en", "auto").is_err());
        assert!(Settings::new("tiny.en", "en").is_ok());
        assert!(Settings::new("tiny", "yue").is_err());
        assert!(Settings::new("large-v3-turbo", "yue").is_ok());
        assert!(Settings::new("tiny", "auto").is_ok());
        let mut ids = BTreeSet::new();
        for model in catalog::MODELS {
            assert!(ids.insert(model.id));
            assert_eq!(model.sha256.len(), 64);
            assert!(model.sha256.bytes().all(|c| c.is_ascii_hexdigit()));
            assert!(model.bytes > 0);
            assert!(model.url().contains(catalog::MODEL_REVISION));
        }
        assert!(catalog::LANGUAGES.iter().any(|(c, _)| *c == "de"));
    }
    #[test]
    fn global_preparation_is_lazy_nonblocking_serial_and_cancel_keeps_previous_choice() {
        let store = Arc::new(Store::default());
        let mut session = ModelSession::new(store.clone());
        assert!(!session.busy());
        assert_eq!(store.reads.load(Ordering::SeqCst), 0);
        session.ensure_loaded();
        drain(&mut session);
        session.ensure_loaded();
        assert_eq!(store.reads.load(Ordering::SeqCst), 1);
        let previous = session.prepared.as_ref().unwrap().settings.clone();
        store.block.store(true, Ordering::SeqCst);
        session.apply(Settings::new("tiny", "fr").unwrap()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !store.entered.load(Ordering::SeqCst) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(session.apply(Settings::default()).is_err());
        session.cancel();
        drain(&mut session);
        assert_eq!(session.prepared.as_ref().unwrap().settings, previous);
        assert!(session.error.contains("Abgebrochen"));
        store.block.store(false, Ordering::SeqCst);
        session.apply(Settings::new("tiny", "fr").unwrap()).unwrap();
        drain(&mut session);
        assert_eq!(session.prepared.as_ref().unwrap().settings.language(), "fr");
        assert_eq!(store.prepares.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn crashed_store_does_not_leave_permanent_busy_state() {
        struct Crash;
        impl ModelStore for Crash {
            fn locations(&self) -> Locations {
                Locations {
                    models: PathBuf::new(),
                    preferences: PathBuf::new(),
                }
            }
            fn inspect(
                &self,
                _: &AtomicBool,
                _: &(dyn Fn(Progress) + Sync),
            ) -> Result<Prepared, String> {
                panic!("simulated crash")
            }
            fn prepare(
                &self,
                _: &Settings,
                _: &AtomicBool,
                _: &(dyn Fn(Progress) + Sync),
            ) -> Result<Prepared, String> {
                unreachable!()
            }
        }
        let mut session = ModelSession::new(Arc::new(Crash));
        session.ensure_loaded();
        drain(&mut session);
        assert!(!session.error.is_empty());
    }
}

pub mod queue;
