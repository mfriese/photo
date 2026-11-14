//! Preferences boundary and asynchronous persistence; independent of UI/file format.
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: String,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub command: bool,
    pub mac_cmd: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub preview_size: usize,
    pub square_previews: bool,
    pub show_library: bool,
    pub videos_only: bool,
    pub write_exif: bool,
    pub reopen_library: bool,
    pub last_library: Option<PathBuf>,
    /// Runtime routing only; never serialized inside a library settings file.
    pub library_scope: Option<PathBuf>,
    pub shortcuts: BTreeMap<String, Option<Shortcut>>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            preview_size: 2,
            square_previews: true,
            show_library: true,
            videos_only: false,
            write_exif: false,
            reopen_library: true,
            last_library: None,
            library_scope: None,
            shortcuts: BTreeMap::new(),
        }
    }
}
impl Preferences {
    /// Explicit library paths take priority; Finder's legacy launch token is not a path.
    pub fn startup_library(
        &self,
        arguments: impl IntoIterator<Item = std::ffi::OsString>,
    ) -> Option<PathBuf> {
        let mut arguments = arguments.into_iter().peekable();
        while let Some(argument) = arguments.next() {
            let text = argument.to_string_lossy();
            if text == "--" {
                return arguments.next().map(PathBuf::from);
            }
            if text.starts_with("-psn_") {
                continue;
            }
            if text.starts_with('-') {
                if !text.contains('=')
                    && arguments
                        .peek()
                        .is_some_and(|value| !value.to_string_lossy().starts_with('-'))
                {
                    arguments.next();
                }
                continue;
            }
            return Some(PathBuf::from(argument));
        }
        self.reopen_library
            .then(|| self.last_library.clone())
            .flatten()
    }
}
pub trait PreferencesStore: Send + Sync + 'static {
    fn load(&self) -> Result<Preferences, String>;
    fn save(&self, preferences: &Preferences) -> Result<(), String>;
    fn load_with_status(&self) -> Result<(Preferences, Option<String>), String> {
        self.load().map(|p| (p, None))
    }
    fn load_for_startup(
        &self,
        _explicit: Option<&std::path::Path>,
    ) -> Result<(Preferences, Option<String>), String> {
        self.load_with_status()
    }
    fn load_library(&self, _root: &std::path::Path) -> Result<Option<Preferences>, String> {
        Ok(None)
    }
}
#[derive(Default)]
struct Mailbox {
    // Latest snapshot per scope: switching libraries must not discard the previous save.
    pending: BTreeMap<Option<PathBuf>, Preferences>,
    stopping: bool,
}
pub struct Persistence {
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
    errors: mpsc::Receiver<String>,
    worker: Option<thread::JoinHandle<()>>,
    last: Preferences,
    worker_error: Option<String>,
}
impl Persistence {
    pub fn new(store: Arc<dyn PreferencesStore>, initial: Preferences) -> Self {
        let mailbox = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let inbox = mailbox.clone();
        let (errors_tx, errors) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            loop {
                let preferences = {
                    let (lock, wake) = &*inbox;
                    let mut state = lock.lock().unwrap();
                    while state.pending.is_empty() && !state.stopping {
                        state = wake.wait(state).unwrap();
                    }
                    match state.pending.pop_first() {
                        Some((_, value)) => value,
                        None => break,
                    }
                };
                // Never hold the mailbox lock while calling a slow/failing store.
                if let Err(error) = store.save(&preferences) {
                    let _ = errors_tx.try_send(error);
                }
            }
        });
        Self {
            mailbox,
            errors,
            worker: Some(worker),
            last: initial,
            worker_error: None,
        }
    }
    pub fn update(&mut self, preferences: Preferences) {
        if preferences != self.last {
            self.last = preferences;
            self.flush_current();
        }
    }
    /// One replaceable pending snapshot per scope; unchanged startup snapshots are still persisted.
    pub fn flush_current(&mut self) {
        if self
            .worker
            .as_ref()
            .is_some_and(thread::JoinHandle::is_finished)
        {
            self.worker_error = Some("Preferences-Dienst wurde unerwartet beendet".into());
            return;
        }
        let (lock, wake) = &*self.mailbox;
        lock.lock()
            .unwrap()
            .pending
            .insert(self.last.library_scope.clone(), self.last.clone());
        wake.notify_one();
    }
    pub fn poll_error(&mut self) -> Option<String> {
        self.worker_error
            .take()
            .or_else(|| match self.errors.try_recv() {
                Ok(error) => Some(error),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some("Preferences-Dienst wurde unerwartet beendet".into())
                }
            })
    }
}
impl Drop for Persistence {
    fn drop(&mut self) {
        let (lock, wake) = &*self.mailbox;
        lock.lock().unwrap().stopping = true;
        wake.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Single-flight, latest-request-wins library settings reads. No UI/file-format knowledge.
type LibraryResult = Result<Option<Preferences>, String>;
struct LibraryRead {
    root: PathBuf,
    receiver: mpsc::Receiver<LibraryResult>,
}
#[derive(Default)]
pub struct LibrarySettings {
    active: Option<LibraryRead>,
    queued: Option<(Arc<dyn PreferencesStore>, PathBuf)>,
}
impl LibrarySettings {
    pub fn request(&mut self, store: Arc<dyn PreferencesStore>, root: PathBuf) {
        self.queued = Some((store, root));
        self.launch();
    }
    fn launch(&mut self) {
        if self.active.is_some() {
            return;
        }
        let Some((store, root)) = self.queued.take() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.active = Some(LibraryRead {
            root: root.clone(),
            receiver,
        });
        thread::spawn(move || {
            let _ = sender.send(store.load_library(&root));
        });
    }
    pub fn is_loading(&self) -> bool {
        self.active.is_some()
    }
    pub fn poll(&mut self) -> Option<(PathBuf, Result<Option<Preferences>, String>)> {
        let active = self.active.as_ref()?;
        let result = match active.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Bibliotheks-Einstellungen: Hintergrundauftrag unerwartet beendet".into())
            }
        };
        let root = self.active.take().unwrap().root;
        if self.queued.is_some() {
            self.launch();
            return None;
        }
        Some((root, result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    #[derive(Default)]
    struct MemoryStore(Mutex<Vec<Preferences>>);
    impl PreferencesStore for MemoryStore {
        fn load(&self) -> Result<Preferences, String> {
            Ok(Default::default())
        }
        fn save(&self, p: &Preferences) -> Result<(), String> {
            self.0.lock().unwrap().push(p.clone());
            Ok(())
        }
    }
    #[test]
    fn shutdown_flushes_latest_settings_and_library() {
        let store = Arc::new(MemoryStore::default());
        let initial = Preferences::default();
        let mut persistence = Persistence::new(store.clone(), initial.clone());
        persistence.update(initial.clone());
        let changed = Preferences {
            preview_size: 4,
            last_library: Some("/Fotos/Familie".into()),
            ..initial
        };
        persistence.update(changed.clone());
        drop(persistence);
        let saved = store.0.lock().unwrap();
        assert_eq!(saved.last(), Some(&changed));
        assert_eq!(saved.len(), 1);
    }
    #[test]
    fn macos_launch_options_never_replace_the_saved_library() {
        let prefs = Preferences {
            last_library: Some("/Fotos".into()),
            ..Default::default()
        };
        for arguments in [
            vec!["-NSDocumentRevisionsDebugMode", "YES"],
            vec!["-ApplePersistenceIgnoreState", "NO"],
            vec!["-psn_0_123", "-NSFoo", "YES"],
        ] {
            assert_eq!(
                prefs.startup_library(arguments.into_iter().map(Into::into)),
                prefs.last_library
            );
        }
        assert_eq!(
            prefs.startup_library(["-NSFoo".into(), "YES".into(), "/Andere Fotos".into()]),
            Some("/Andere Fotos".into())
        );
    }
    #[test]
    fn startup_restores_saved_library_and_respects_explicit_path_and_opt_out() {
        let mut preferences = Preferences {
            last_library: Some("/Fotos/Familie".into()),
            ..Default::default()
        };
        assert_eq!(preferences.startup_library([]), preferences.last_library);
        assert_eq!(
            preferences.startup_library(["-psn_0_1234".into()]),
            preferences.last_library
        );
        assert_eq!(
            preferences.startup_library(["/Andere Fotos".into()]),
            Some("/Andere Fotos".into())
        );
        preferences.reopen_library = false;
        assert_eq!(preferences.startup_library([]), None);
        assert_eq!(
            preferences.startup_library(["/Andere Fotos".into()]),
            Some("/Andere Fotos".into())
        );
        assert_eq!(Preferences::default().startup_library([]), None);
    }
    struct BrokenStore;
    impl PreferencesStore for BrokenStore {
        fn load(&self) -> Result<Preferences, String> {
            Ok(Default::default())
        }
        fn save(&self, _: &Preferences) -> Result<(), String> {
            panic!("test worker failure")
        }
    }
    #[test]
    fn unexpected_worker_exit_is_reported() {
        let mut persistence = Persistence::new(Arc::new(BrokenStore), Preferences::default());
        persistence.update(Preferences {
            preview_size: 4,
            ..Default::default()
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(error) = persistence.poll_error() {
                assert!(error.contains("unerwartet"));
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn slow_store_keeps_only_latest_pending_snapshot_and_flushes_it_on_shutdown() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct SlowStore {
            entered: AtomicBool,
            release: AtomicBool,
            saved: Mutex<Vec<Preferences>>,
        }
        impl PreferencesStore for SlowStore {
            fn load(&self) -> Result<Preferences, String> {
                Ok(Default::default())
            }
            fn save(&self, value: &Preferences) -> Result<(), String> {
                self.entered.store(true, Ordering::SeqCst);
                while !self.release.load(Ordering::SeqCst) {
                    std::thread::yield_now();
                }
                self.saved.lock().unwrap().push(value.clone());
                Ok(())
            }
        }
        let store = Arc::new(SlowStore {
            entered: AtomicBool::new(false),
            release: AtomicBool::new(false),
            saved: Mutex::new(Vec::new()),
        });
        let mut persistence = Persistence::new(store.clone(), Preferences::default());
        persistence.flush_current();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !store.entered.load(Ordering::SeqCst) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        for i in 0..1000 {
            persistence.update(Preferences {
                last_library: Some(format!("library/{i}").into()),
                ..Default::default()
            });
        }
        store.release.store(true, Ordering::SeqCst);
        drop(persistence);
        let saved = store.saved.lock().unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(
            saved.last().unwrap().last_library,
            Some("library/999".into())
        );
    }
}

#[cfg(test)]
mod scoped_tests {
    use super::*;
    use std::{
        path::Path,
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };
    #[test]
    fn pending_saves_coalesce_per_library_without_losing_previous_scope_or_locator() {
        struct Slow {
            calls: Mutex<Vec<Preferences>>,
            release: AtomicBool,
        }
        impl PreferencesStore for Slow {
            fn load(&self) -> Result<Preferences, String> {
                Ok(Default::default())
            }
            fn save(&self, p: &Preferences) -> Result<(), String> {
                self.calls.lock().unwrap().push(p.clone());
                while !self.release.load(Ordering::SeqCst) {
                    std::thread::yield_now();
                }
                Ok(())
            }
        }
        let store = Arc::new(Slow {
            calls: Mutex::new(Vec::new()),
            release: AtomicBool::new(false),
        });
        let mut persistence = Persistence::new(store.clone(), Preferences::default());
        let mut a = Preferences {
            library_scope: Some("a".into()),
            preview_size: 0,
            ..Default::default()
        };
        persistence.update(a.clone());
        let deadline = Instant::now() + Duration::from_secs(3);
        while store.calls.lock().unwrap().is_empty() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        a.preview_size = 1;
        persistence.update(a.clone());
        let b = Preferences {
            library_scope: Some("b".into()),
            preview_size: 3,
            ..Default::default()
        };
        persistence.update(b.clone());
        let location = Preferences {
            last_library: Some("b".into()),
            ..Default::default()
        };
        persistence.update(location.clone());
        a.preview_size = 4;
        persistence.update(a.clone());
        store.release.store(true, Ordering::SeqCst);
        drop(persistence);
        let calls = store.calls.lock().unwrap();
        assert_eq!(calls.len(), 4);
        assert_eq!(
            calls
                .iter()
                .rfind(|p| p.library_scope.as_deref() == Some(Path::new("a"))),
            Some(&a)
        );
        assert!(calls.contains(&b));
        assert!(calls.contains(&location));
    }
    #[test]
    fn library_reads_keep_only_latest_request_and_discard_old_results() {
        struct Slow {
            calls: Mutex<Vec<PathBuf>>,
            release: AtomicBool,
        }
        impl PreferencesStore for Slow {
            fn load(&self) -> Result<Preferences, String> {
                Ok(Default::default())
            }
            fn save(&self, _: &Preferences) -> Result<(), String> {
                Ok(())
            }
            fn load_library(&self, root: &Path) -> Result<Option<Preferences>, String> {
                self.calls.lock().unwrap().push(root.into());
                if root == Path::new("a") {
                    while !self.release.load(Ordering::SeqCst) {
                        std::thread::yield_now();
                    }
                }
                Ok(Some(Preferences {
                    preview_size: 4,
                    ..Default::default()
                }))
            }
        }
        let store = Arc::new(Slow {
            calls: Mutex::new(Vec::new()),
            release: AtomicBool::new(false),
        });
        let mut session = LibrarySettings::default();
        session.request(store.clone(), "a".into());
        let deadline = Instant::now() + Duration::from_secs(3);
        while store.calls.lock().unwrap().is_empty() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        session.request(store.clone(), "b".into());
        session.request(store.clone(), "c".into());
        assert!(session.poll().is_none());
        store.release.store(true, Ordering::SeqCst);
        let (root, result) = loop {
            if let Some(result) = session.poll() {
                break result;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(root, PathBuf::from("c"));
        assert_eq!(result.unwrap().unwrap().preview_size, 4);
        assert!(!session.is_loading());
        assert_eq!(
            *store.calls.lock().unwrap(),
            vec![PathBuf::from("a"), PathBuf::from("c")]
        );
    }
}

mod startup;
pub use startup::{StartupOutcome, StartupPreferences};
