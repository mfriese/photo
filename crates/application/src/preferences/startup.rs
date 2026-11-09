//! Initial configuration is read off the UI thread, including removable-library YAML.
use super::{Preferences, PreferencesStore};
use std::{
    ffi::OsString,
    sync::{Arc, mpsc},
};
type InitialResult = Result<(Preferences, Option<String>), String>;
pub struct StartupOutcome {
    pub result: InitialResult,
    pub arguments: Vec<OsString>,
}
pub struct StartupPreferences {
    receiver: Option<mpsc::Receiver<InitialResult>>,
    arguments: Vec<OsString>,
}
impl StartupPreferences {
    pub fn new(store: Arc<dyn PreferencesStore>, arguments: Vec<OsString>) -> Self {
        let explicit = Preferences::default().startup_library(arguments.clone());
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(store.load_for_startup(explicit.as_deref()));
        });
        Self {
            receiver: Some(receiver),
            arguments,
        }
    }
    pub fn poll(&mut self) -> Option<StartupOutcome> {
        let receiver = self.receiver.as_ref()?;
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Start-Einstellungen: Hintergrundauftrag unerwartet beendet".into())
            }
        };
        self.receiver = None;
        Some(StartupOutcome {
            result,
            arguments: std::mem::take(&mut self.arguments),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::Path,
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };
    struct Slow {
        entered: AtomicBool,
        release: AtomicBool,
        explicit: Mutex<Option<std::path::PathBuf>>,
    }
    impl PreferencesStore for Slow {
        fn load(&self) -> Result<Preferences, String> {
            panic!("explicit startup boundary must be used")
        }
        fn save(&self, _: &Preferences) -> Result<(), String> {
            Ok(())
        }
        fn load_for_startup(&self, explicit: Option<&Path>) -> InitialResult {
            *self.explicit.lock().unwrap() = explicit.map(Into::into);
            self.entered.store(true, Ordering::SeqCst);
            while !self.release.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            Ok((
                Preferences {
                    preview_size: 4,
                    ..Default::default()
                },
                None,
            ))
        }
    }
    #[test]
    fn blocked_startup_is_nonblocking_and_explicit_library_is_supplied() {
        let store = Arc::new(Slow {
            entered: AtomicBool::new(false),
            release: AtomicBool::new(false),
            explicit: Mutex::new(None),
        });
        let args = vec![OsString::from("/Chosen")];
        let mut startup = StartupPreferences::new(store.clone(), args.clone());
        let deadline = Instant::now() + Duration::from_secs(3);
        while !store.entered.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(startup.poll().is_none());
        assert_eq!(
            store.explicit.lock().unwrap().as_deref(),
            Some(Path::new("/Chosen"))
        );
        store.release.store(true, Ordering::SeqCst);
        let outcome = loop {
            if let Some(outcome) = startup.poll() {
                break outcome;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(outcome.arguments, args);
        assert_eq!(outcome.result.unwrap().0.preview_size, 4);
        assert!(startup.poll().is_none());
    }
}
