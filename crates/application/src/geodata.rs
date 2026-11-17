//! Preparation of shared offline data only; no geocoding and no media access.
use crate::downloads::Progress;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
pub const SOURCE: &str = "https://download.geonames.org/export/dump/cities500.zip";
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct Locations {
    pub archive: PathBuf,
    pub preferences: PathBuf,
}
#[derive(Clone, Debug, Default)]
pub struct Prepared {
    pub enabled: bool,
    pub ready: bool,
    pub bytes: u64,
    pub warning: String,
}
pub trait Store: Send + Sync + 'static {
    /// Pure path calculation only; no I/O.
    fn locations(&self) -> Locations;
    fn inspect(
        &self,
        cancel: &AtomicBool,
        notify: &(dyn Fn(Progress) + Sync),
    ) -> Result<Prepared, String>;
    /// Explicit user confirmation; false keeps previously downloaded data.
    fn prepare(
        &self,
        enabled: bool,
        cancel: &AtomicBool,
        notify: &(dyn Fn(Progress) + Sync),
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
pub struct Session {
    store: Arc<dyn Store>,
    job: Option<Job>,
    prepared: Option<Prepared>,
    progress: Option<Progress>,
    error: String,
    requested: bool,
    cancelling: bool,
}
impl Session {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            store,
            job: None,
            prepared: None,
            progress: None,
            error: String::new(),
            requested: false,
            cancelling: false,
        }
    }
    pub fn locations(&self) -> Locations {
        self.store.locations()
    }
    pub fn prepared(&self) -> Option<&Prepared> {
        self.prepared.as_ref()
    }
    pub fn progress(&self) -> Option<Progress> {
        self.progress
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
    pub fn cancelling(&self) -> bool {
        self.cancelling
    }
    pub fn ensure_loaded(&mut self) {
        if !self.requested {
            self.reload();
        }
    }
    pub fn reload(&mut self) {
        if !self.busy() {
            self.launch(None);
        }
    }
    pub fn apply(&mut self, enabled: bool) {
        if !self.busy() {
            self.launch(Some(enabled));
        }
    }
    pub fn cancel(&mut self) {
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
            self.cancelling = true;
        }
    }
    fn launch(&mut self, enabled: Option<bool>) {
        self.requested = true;
        self.error.clear();
        self.progress = None;
        self.cancelling = false;
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let store = self.store.clone();
        let (tx, receiver) = mpsc::sync_channel(16);
        self.job = Some(Job { cancel, receiver });
        std::thread::spawn(move || {
            let notify = |p| {
                let _ = tx.try_send(Event::Progress(p));
            };
            let result = match enabled {
                Some(enabled) => store.prepare(enabled, &stop, &notify),
                None => store.inspect(&stop, &notify),
            };
            let _ = tx.send(Event::Done(result));
        });
    }
    pub fn poll(&mut self) {
        let mut done = false;
        if let Some(job) = &self.job {
            for _ in 0..32 {
                match job.receiver.try_recv() {
                    Ok(Event::Progress(p)) => self.progress = Some(p),
                    Ok(Event::Done(result)) => {
                        match result {
                            Ok(p) => self.prepared = Some(p),
                            Err(e) => self.error = e,
                        }
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.error =
                            "Geodatenvorbereitung unerwartet beendet; erneut versuchen".into();
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
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    #[derive(Default)]
    struct Fake {
        reads: AtomicUsize,
        writes: AtomicUsize,
        block: AtomicBool,
        entered: AtomicBool,
    }
    impl Store for Fake {
        fn locations(&self) -> Locations {
            Locations {
                archive: "/Global/cities500.zip".into(),
                preferences: "/Global/geodata.plist".into(),
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
            enabled: bool,
            stop: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<Prepared, String> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            self.entered.store(true, Ordering::SeqCst);
            while self.block.load(Ordering::SeqCst) {
                if stop.load(Ordering::Relaxed) {
                    return Err("Abgebrochen".into());
                }
                std::thread::yield_now();
            }
            Ok(Prepared {
                enabled,
                ready: enabled,
                ..Default::default()
            })
        }
    }
    fn drain(s: &mut Session) {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while s.busy() {
            s.poll();
            assert!(std::time::Instant::now() < until);
            std::thread::yield_now();
        }
    }
    #[test]
    fn preparation_is_lazy_serial_nonblocking_and_cancel_preserves_previous_state() {
        let store = Arc::new(Fake::default());
        let mut s = Session::new(store.clone());
        assert_eq!(store.reads.load(Ordering::SeqCst), 0);
        s.ensure_loaded();
        drain(&mut s);
        s.ensure_loaded();
        assert_eq!(store.reads.load(Ordering::SeqCst), 1);
        store.block.store(true, Ordering::SeqCst);
        s.apply(true);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !store.entered.load(Ordering::SeqCst) {
            assert!(std::time::Instant::now() < until);
            std::thread::yield_now();
        }
        s.apply(false);
        assert_eq!(store.writes.load(Ordering::SeqCst), 1);
        s.cancel();
        drain(&mut s);
        assert!(!s.prepared().unwrap().enabled);
        assert!(s.error().contains("Abgebrochen"));
        store.block.store(false, Ordering::SeqCst);
        s.apply(true);
        drain(&mut s);
        assert!(s.prepared().unwrap().ready);
        assert!(s.error().is_empty());
    }
    #[test]
    fn disconnected_worker_finishes_with_error_instead_of_staying_busy() {
        struct Crash;
        impl Store for Crash {
            fn locations(&self) -> Locations {
                unreachable!()
            }
            fn inspect(
                &self,
                _: &AtomicBool,
                _: &(dyn Fn(Progress) + Sync),
            ) -> Result<Prepared, String> {
                panic!("test worker failure")
            }
            fn prepare(
                &self,
                _: bool,
                _: &AtomicBool,
                _: &(dyn Fn(Progress) + Sync),
            ) -> Result<Prepared, String> {
                unreachable!()
            }
        }
        let mut s = Session::new(Arc::new(Crash));
        s.ensure_loaded();
        drain(&mut s);
        assert!(!s.error().is_empty());
    }
}
