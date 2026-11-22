//! Explicit, cancellable rebuild of disposable library indexes.
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
pub trait Rebuilder: Send + Sync + 'static {
    fn rebuild(
        &self,
        root: &Path,
        cancel: &AtomicBool,
        progress: &(dyn Fn(usize) + Sync),
    ) -> Result<(), String>;
}
enum Event {
    Progress(usize),
    Done(Result<(), String>),
}
struct Job {
    root: PathBuf,
    cancel: Arc<AtomicBool>,
    events: mpsc::Receiver<Event>,
}
/// Presentation-independent follow-up after rebuilding the active library.
#[derive(Debug, PartialEq, Eq)]
pub enum AfterRebuild {
    ReloadLibrary { root: PathBuf },
}
pub struct Session {
    store: Arc<dyn Rebuilder>,
    job: Option<Job>,
    checked: usize,
    error: String,
    completed: Option<PathBuf>,
}
impl Session {
    pub fn new(store: Arc<dyn Rebuilder>) -> Self {
        Self {
            store,
            job: None,
            checked: 0,
            error: String::new(),
            completed: None,
        }
    }
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
    pub fn checked(&self) -> usize {
        self.checked
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn start(&mut self, root: PathBuf) {
        if self.busy() {
            return;
        }
        self.checked = 0;
        self.error.clear();
        self.completed = None;
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let store = self.store.clone();
        let (tx, events) = mpsc::sync_channel(16);
        let source = root.clone();
        self.job = Some(Job {
            root,
            cancel,
            events,
        });
        std::thread::spawn(move || {
            let notify = |n| {
                let _ = tx.try_send(Event::Progress(n));
            };
            let result = store.rebuild(&source, &stop, &notify);
            let _ = tx.send(Event::Done(result));
        });
    }
    pub fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn poll(&mut self) {
        let mut done = false;
        if let Some(job) = &self.job {
            for _ in 0..32 {
                match job.events.try_recv() {
                    Ok(Event::Progress(n)) => self.checked = n,
                    Ok(Event::Done(result)) => {
                        match result {
                            Ok(()) if !job.cancel.load(Ordering::Relaxed) => {
                                self.completed = Some(job.root.clone())
                            }
                            Ok(()) => {}
                            Err(e) => self.error = e,
                        }
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.error = "Datenbank-Neuaufbau unerwartet beendet".into();
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if done {
            self.job = None;
        }
    }
    /// Keep a successful refresh pending while recording data prevents reloading.
    /// A completion for a library that is no longer active must never navigate back.
    pub fn after_rebuild(
        &mut self,
        active_root: Option<&Path>,
        recording_pending: bool,
    ) -> Option<AfterRebuild> {
        let root = self.completed.as_deref()?;
        if active_root != Some(root) {
            self.completed = None;
            return None;
        }
        if recording_pending {
            return None;
        }
        self.completed
            .take()
            .map(|root| AfterRebuild::ReloadLibrary { root })
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Blocked {
        calls: AtomicUsize,
        entered: AtomicBool,
    }
    impl Rebuilder for Blocked {
        fn rebuild(
            &self,
            _: &Path,
            stop: &AtomicBool,
            notify: &(dyn Fn(usize) + Sync),
        ) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.entered.store(true, Ordering::SeqCst);
            notify(7);
            while !stop.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            Err("Abgebrochen".into())
        }
    }
    #[test]
    fn completed_rebuild_waits_for_recording_and_is_consumed_once() {
        let mut session = Session::new(Arc::new(Blocked {
            calls: AtomicUsize::new(0),
            entered: AtomicBool::new(false),
        }));
        session.completed = Some("library".into());
        assert!(
            session
                .after_rebuild(Some(Path::new("library")), true)
                .is_none()
        );
        assert_eq!(
            session.after_rebuild(Some(Path::new("library")), false),
            Some(AfterRebuild::ReloadLibrary {
                root: "library".into()
            })
        );
        assert!(
            session
                .after_rebuild(Some(Path::new("library")), false)
                .is_none()
        );
    }
    #[test]
    fn completed_rebuild_never_reopens_an_inactive_library() {
        let mut session = Session::new(Arc::new(Blocked {
            calls: AtomicUsize::new(0),
            entered: AtomicBool::new(false),
        }));
        session.completed = Some("old".into());
        assert!(
            session
                .after_rebuild(Some(Path::new("new")), false)
                .is_none()
        );
        assert!(
            session
                .after_rebuild(Some(Path::new("old")), false)
                .is_none()
        );
    }
    #[test]
    fn rebuild_is_nonblocking_serial_cancellable_and_does_not_publish_a_cancelled_result() {
        let store = Arc::new(Blocked {
            calls: AtomicUsize::new(0),
            entered: AtomicBool::new(false),
        });
        let mut session = Session::new(store.clone());
        session.start("library".into());
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !store.entered.load(Ordering::SeqCst) {
            assert!(std::time::Instant::now() < until);
            std::thread::yield_now();
        }
        session.start("another".into());
        assert_eq!(store.calls.load(Ordering::SeqCst), 1);
        session.cancel();
        while session.busy() {
            session.poll();
            assert!(std::time::Instant::now() < until);
            std::thread::yield_now();
        }
        assert!(
            session
                .after_rebuild(Some(Path::new("library")), false)
                .is_none()
        );
        assert!(session.error().contains("Abgebrochen"));
    }
}
