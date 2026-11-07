//! Preferences boundary and asynchronous persistence; independent of UI/file format.
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, mpsc},
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
        arguments
            .into_iter()
            .find(|argument| !argument.to_string_lossy().starts_with("-psn_"))
            .map(PathBuf::from)
            .or_else(|| {
                self.reopen_library
                    .then(|| self.last_library.clone())
                    .flatten()
            })
    }
}
pub trait PreferencesStore: Send + Sync + 'static {
    fn load(&self) -> Result<Preferences, String>;
    fn save(&self, preferences: &Preferences) -> Result<(), String>;
}
pub struct Persistence {
    sender: Option<mpsc::Sender<Preferences>>,
    errors: mpsc::Receiver<String>,
    worker: Option<thread::JoinHandle<()>>,
    last: Preferences,
    worker_error: Option<String>,
}
impl Persistence {
    pub fn new(store: Arc<dyn PreferencesStore>, initial: Preferences) -> Self {
        let (tx, rx) = mpsc::channel::<Preferences>();
        let (errors_tx, errors) = mpsc::channel();
        let worker = thread::spawn(move || {
            while let Ok(mut preferences) = rx.recv() {
                while let Ok(newer) = rx.try_recv() {
                    preferences = newer;
                }
                if let Err(e) = store.save(&preferences) {
                    let _ = errors_tx.send(e);
                }
            }
        });
        Self {
            sender: Some(tx),
            errors,
            worker: Some(worker),
            last: initial,
            worker_error: None,
        }
    }
    pub fn update(&mut self, preferences: Preferences) {
        if preferences != self.last {
            self.last = preferences.clone();
            if self.sender.as_ref().unwrap().send(preferences).is_err() {
                self.worker_error = Some("Preferences-Dienst wurde unerwartet beendet".into());
            }
        }
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
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
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
}
