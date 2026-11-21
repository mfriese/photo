//! Single-flight startup readiness with bounded retries, deadline and stale-result protection.
use super::{Preferences, PreferencesStore};
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
type InitialResult = Result<(Preferences, Option<String>), String>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupState {
    Accessing,
    RetryWaiting,
    Ready,
    Unavailable,
    Cancelled,
}
#[derive(Clone)]
pub struct StartupLocation {
    pub preferences: Preferences,
    pub library: Option<PathBuf>,
}
pub struct StartupRead {
    pub preferences: Preferences,
    pub library: Option<PathBuf>,
    pub ready: bool,
    pub warning: Option<String>,
}
pub struct StartupOutcome {
    pub result: InitialResult,
    pub arguments: Vec<OsString>,
    pub library: Option<PathBuf>,
    pub state: StartupState,
}
enum Event {
    Located(u64, StartupLocation),
    Finished(u64, Result<StartupRead, String>),
}
struct Request {
    generation: u64,
    explicit: Option<PathBuf>,
}
pub struct StartupPreferences {
    receiver: mpsc::Receiver<Event>,
    sender: Option<mpsc::SyncSender<Request>>,
    arguments: Vec<OsString>,
    explicit: Option<PathBuf>,
    generation: u64,
    state: StartupState,
    in_flight: bool,
    started: Instant,
    attempt_started: Instant,
    next_retry: Option<Instant>,
    attempts: usize,
    location: Option<StartupLocation>,
    target: Option<PathBuf>,
    error: String,
    terminal: Option<StartupOutcome>,
    token: Arc<std::sync::atomic::AtomicU64>,
}
impl StartupPreferences {
    pub const LIMIT: Duration = Duration::from_secs(60);
    pub const RETRY: Duration = Duration::from_secs(5);
    pub fn new(store: Arc<dyn PreferencesStore>, arguments: Vec<OsString>) -> Self {
        Self::at(store, arguments, Instant::now())
    }
    fn at(store: Arc<dyn PreferencesStore>, arguments: Vec<OsString>, now: Instant) -> Self {
        let explicit = Preferences::default().startup_library(arguments.clone());
        let (send, receiver) = mpsc::sync_channel(16);
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let token = Arc::new(std::sync::atomic::AtomicU64::new(1));
        let worker_token = token.clone();
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                if worker_token.load(std::sync::atomic::Ordering::Relaxed) != request.generation {
                    if send
                        .send(Event::Finished(
                            request.generation,
                            Err("Startauftrag verworfen".into()),
                        ))
                        .is_err()
                    {
                        break;
                    }
                    continue;
                }
                let progress = |location| {
                    let _ = send.send(Event::Located(request.generation, location));
                };
                let result = store.startup_attempt(request.explicit.as_deref(), &progress);
                if send
                    .send(Event::Finished(request.generation, result))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut session = Self {
            receiver,
            sender: Some(sender),
            arguments,
            explicit: explicit.clone(),
            generation: 1,
            state: StartupState::Accessing,
            in_flight: false,
            started: now,
            attempt_started: now,
            next_retry: Some(now),
            attempts: 0,
            location: None,
            target: explicit,
            error: String::new(),
            terminal: None,
            token,
        };
        session.submit(now);
        session
    }
    pub fn waiting(&self) -> bool {
        matches!(
            self.state,
            StartupState::Accessing | StartupState::RetryWaiting
        )
    }
    pub fn busy(&self) -> bool {
        self.in_flight
    }
    pub fn state(&self) -> StartupState {
        self.state
    }
    pub fn target(&self) -> Option<&std::path::Path> {
        self.target.as_deref()
    }
    pub fn attempts(&self) -> usize {
        self.attempts
    }
    pub fn elapsed(&self) -> Duration {
        Instant::now()
            .saturating_duration_since(self.started)
            .min(Self::LIMIT)
    }
    pub fn retry_in(&self) -> Duration {
        self.next_retry
            .map(|time| time.saturating_duration_since(Instant::now()))
            .unwrap_or_default()
    }
    pub fn slow_access(&self) -> bool {
        self.in_flight
            && Instant::now().saturating_duration_since(self.attempt_started) >= Self::RETRY
    }
    /// Local locator becomes available before potentially slow library I/O completes.
    pub fn location_preferences(&self) -> Option<&Preferences> {
        self.location.as_ref().map(|location| &location.preferences)
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    fn submit(&mut self, now: Instant) {
        if self.in_flight || !self.waiting() {
            return;
        }
        let Some(sender) = &self.sender else {
            return;
        };
        match sender.try_send(Request {
            generation: self.generation,
            explicit: self.explicit.clone(),
        }) {
            Ok(()) => {
                self.in_flight = true;
                self.attempts += 1;
                self.attempt_started = now;
                self.next_retry = None;
                self.state = StartupState::Accessing;
            }
            Err(_) => self.finish(StartupState::Unavailable),
        }
    }
    fn finish(&mut self, state: StartupState) {
        self.state = state;
        self.generation += 1;
        self.token
            .store(self.generation, std::sync::atomic::Ordering::Relaxed);
        self.next_retry = None;
        self.terminal = Some(StartupOutcome {
            result: Ok((
                self.location
                    .as_ref()
                    .map(|location| location.preferences.clone())
                    .unwrap_or_default(),
                None,
            )),
            arguments: vec![],
            library: None,
            state,
        });
    }
    pub fn cancel(&mut self) {
        if self.waiting() {
            self.finish(StartupState::Cancelled);
        }
    }
    /// Manual navigation owns its new state and must not receive the old startup completion.
    pub fn cancel_for_navigation(&mut self) {
        self.cancel();
        self.terminal = None;
    }
    pub fn retry(&mut self) {
        self.retry_at(Instant::now());
    }
    fn retry_at(&mut self, now: Instant) {
        self.generation += 1;
        self.token
            .store(self.generation, std::sync::atomic::Ordering::Relaxed);
        self.started = now;
        self.attempt_started = now;
        self.attempts = 0;
        self.error.clear();
        self.terminal = None;
        self.explicit = self.target.clone().or_else(|| self.explicit.clone());
        self.state = StartupState::Accessing;
        self.next_retry = Some(now);
        self.submit(now);
    }
    pub fn poll(&mut self) -> Option<StartupOutcome> {
        self.poll_at(Instant::now())
    }
    fn poll_at(&mut self, now: Instant) -> Option<StartupOutcome> {
        // Enforce the deadline before accepting results that arrived after the UI stopped waiting.
        if self.waiting() && now.saturating_duration_since(self.started) >= Self::LIMIT {
            self.finish(StartupState::Unavailable);
        }
        for _ in 0..16 {
            match self.receiver.try_recv() {
                Ok(Event::Located(generation, location)) => {
                    if generation == self.generation && self.waiting() {
                        self.target = location.library.clone();
                        self.location = Some(location);
                    }
                }
                Ok(Event::Finished(generation, result)) => {
                    self.in_flight = false;
                    if generation != self.generation || !self.waiting() {
                        continue;
                    }
                    match result {
                        Ok(read) if !read.ready => {
                            self.error = read
                                .warning
                                .unwrap_or_else(|| "Bibliothek aktuell nicht erreichbar.".into());
                            self.target = read.library.clone();
                            self.location = Some(StartupLocation {
                                preferences: read.preferences,
                                library: read.library,
                            });
                            self.state = StartupState::RetryWaiting;
                            self.next_retry = Some(now + Self::RETRY);
                        }
                        Ok(read) => {
                            self.state = StartupState::Ready;
                            self.terminal = Some(StartupOutcome {
                                result: Ok((read.preferences, read.warning)),
                                arguments: std::mem::take(&mut self.arguments),
                                library: read.library,
                                state: StartupState::Ready,
                            });
                        }
                        Err(error) => {
                            self.state = StartupState::Ready;
                            self.terminal = Some(StartupOutcome {
                                result: Err(error),
                                arguments: std::mem::take(&mut self.arguments),
                                library: None,
                                state: StartupState::Ready,
                            });
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.in_flight = false;
                    if self.waiting() {
                        self.error = "Startdienst wurde unerwartet beendet".into();
                        self.finish(StartupState::Unavailable);
                    }
                    break;
                }
            }
        }
        if self.waiting() && self.next_retry.is_some_and(|time| now >= time) {
            self.submit(now);
        }
        self.terminal.take()
    }
}

impl Drop for StartupPreferences {
    fn drop(&mut self) {
        self.token
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.sender.take();
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

#[cfg(test)]
mod readiness_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct Store {
        calls: AtomicUsize,
        block: AtomicBool,
        available: AtomicBool,
    }
    impl PreferencesStore for Store {
        fn load(&self) -> Result<Preferences, String> {
            unreachable!()
        }
        fn save(&self, _: &Preferences) -> Result<(), String> {
            panic!("startup readiness must not write")
        }
        fn startup_attempt(
            &self,
            _: Option<&std::path::Path>,
            progress: &dyn Fn(StartupLocation),
        ) -> Result<StartupRead, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let preferences = Preferences {
                last_library: Some("/External/Photos".into()),
                ..Default::default()
            };
            progress(StartupLocation {
                preferences: preferences.clone(),
                library: preferences.last_library.clone(),
            });
            while self.block.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            Ok(StartupRead {
                library: preferences.last_library.clone(),
                preferences,
                ready: self.available.load(Ordering::SeqCst),
                warning: Some("Gerät antwortet noch nicht".into()),
            })
        }
    }
    fn wait(
        session: &mut StartupPreferences,
        now: Instant,
        condition: impl Fn(&StartupPreferences) -> bool,
    ) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !condition(session) {
            assert!(session.poll_at(now).is_none());
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn failures_retry_only_after_five_seconds_and_stop_after_sixty_without_losing_locator() {
        let store = Arc::new(Store {
            calls: Default::default(),
            block: AtomicBool::new(false),
            available: AtomicBool::new(false),
        });
        let now = Instant::now();
        let mut session = StartupPreferences::at(store.clone(), vec![], now);
        wait(&mut session, now, |s| {
            s.state() == StartupState::RetryWaiting
        });
        assert_eq!(session.attempts(), 1);
        assert!(session.poll_at(now + Duration::from_secs(4)).is_none());
        assert_eq!(store.calls.load(Ordering::SeqCst), 1);
        assert!(session.poll_at(now + Duration::from_secs(5)).is_none());
        wait(&mut session, now + Duration::from_secs(5), |s| {
            s.state() == StartupState::RetryWaiting
        });
        assert_eq!(session.attempts(), 2);
        assert_eq!(store.calls.load(Ordering::SeqCst), 2);
        let outcome = session.poll_at(now + Duration::from_secs(60)).unwrap();
        assert_eq!(outcome.state, StartupState::Unavailable);
        assert!(outcome.library.is_none());
        assert_eq!(
            outcome.result.unwrap().0.last_library,
            Some("/External/Photos".into())
        );
        assert!(session.poll_at(now + Duration::from_secs(65)).is_none());
        assert_eq!(store.calls.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn blocked_access_never_overlaps_and_late_success_after_timeout_is_discarded() {
        let store = Arc::new(Store {
            calls: Default::default(),
            block: AtomicBool::new(true),
            available: AtomicBool::new(true),
        });
        let now = Instant::now();
        let mut session = StartupPreferences::at(store.clone(), vec![], now);
        wait(&mut session, now, |s| s.target().is_some());
        for seconds in [5, 10, 55] {
            assert!(
                session
                    .poll_at(now + Duration::from_secs(seconds))
                    .is_none()
            );
        }
        assert_eq!(store.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            session
                .poll_at(now + Duration::from_secs(60))
                .unwrap()
                .state,
            StartupState::Unavailable
        );
        store.block.store(false, Ordering::SeqCst);
        wait(&mut session, now + Duration::from_secs(61), |s| !s.busy());
        assert_eq!(session.state(), StartupState::Unavailable);
    }
    #[test]
    fn cancellation_is_immediate_and_retry_waits_for_the_old_access_before_starting_another() {
        let store = Arc::new(Store {
            calls: Default::default(),
            block: AtomicBool::new(true),
            available: AtomicBool::new(true),
        });
        let now = Instant::now();
        let mut session = StartupPreferences::at(store.clone(), vec![], now);
        wait(&mut session, now, |s| s.target().is_some());
        session.cancel();
        assert_eq!(session.poll_at(now).unwrap().state, StartupState::Cancelled);
        assert!(!session.waiting());
        session.retry_at(now + Duration::from_secs(1));
        assert_eq!(session.attempts(), 0);
        assert_eq!(store.calls.load(Ordering::SeqCst), 1);
        store.block.store(false, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(3);
        let outcome = loop {
            if let Some(outcome) = session.poll_at(now + Duration::from_secs(2)) {
                break outcome;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(outcome.state, StartupState::Ready);
        assert_eq!(outcome.library, Some("/External/Photos".into()));
        assert_eq!(store.calls.load(Ordering::SeqCst), 2);
    }
}
