//! Database-first audio projection and sequential maintenance. No filesystem/GUI dependencies.
use crate::domain::Media;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Inventory,
    Durations,
    Metadata,
    Years,
}
const STAGES: [Stage; 4] = [
    Stage::Inventory,
    Stage::Durations,
    Stage::Metadata,
    Stage::Years,
];
impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Inventory => "Audiobestand",
            Self::Durations => "Audiodauern",
            Self::Metadata => "Medienzuordnungen",
            Self::Years => "Audio-Jahresübersicht",
        }
    }
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub media: Vec<Media>,
    pub durations: Vec<(PathBuf, f64)>,
    pub years: Vec<(String, f64)>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Default, Debug)]
pub struct Progress {
    pub checked: usize,
    pub total: usize,
    pub current: Option<PathBuf>,
}
pub trait Repository: Send + Sync + 'static {
    /// Read only SQLite; do not stat/open audio files or enumerate media directories.
    fn read(&self, root: &Path) -> Result<Snapshot, String>;
    fn run(
        &self,
        root: &Path,
        stage: Stage,
        stop: &AtomicBool,
        progress: &dyn Fn(Progress),
    ) -> Result<(), String>;
}
impl Snapshot {
    /// Playback keeps available photo context, while audio membership comes solely from SQLite.
    pub fn with_photos(&self, inventory: &[Media]) -> Vec<Media> {
        self.media
            .iter()
            .cloned()
            .chain(inventory.iter().filter(|m| m.is_photo()).cloned())
            .collect()
    }
}
/// Logical comment placement is authoritative for year totals, not physical recording folders.
pub fn year_totals(snapshot: &Snapshot) -> Vec<(String, f64)> {
    let durations = snapshot
        .durations
        .iter()
        .map(|(p, s)| (p.as_path(), *s))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut years = std::collections::BTreeMap::<String, f64>::new();
    for media in &snapshot.media {
        if let Some(time) = crate::domain::capture_from_filename(media)
            && let Some(seconds) = durations.get(media.path())
        {
            *years.entry(time.year().into()).or_default() += seconds;
        }
    }
    years.into_iter().collect()
}
struct Worker {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
    rx: mpsc::Receiver<Result<Snapshot, String>>,
    progress: mpsc::Receiver<Progress>,
    generation: u64,
    stage: Option<Stage>,
}
pub struct Session {
    repository: Option<Arc<dyn Repository>>,
    root: Option<PathBuf>,
    generation: u64,
    worker: Option<Worker>,
    snapshot: Snapshot,
    loaded: bool,
    next: usize,
    revision: u64,
    progress: Progress,
    error: String,
    admitted_at: Option<Instant>,
    comments: Option<
        Vec<(
            crate::domain::tags::LibraryFile,
            crate::domain::tags::LibraryFile,
        )>,
    >,
}
impl Session {
    pub fn new(repository: Option<Arc<dyn Repository>>) -> Self {
        let loaded = repository.is_none();
        Self {
            repository,
            root: None,
            generation: 0,
            worker: None,
            snapshot: Snapshot::default(),
            loaded,
            next: 0,
            revision: 0,
            progress: Progress::default(),
            error: String::new(),
            admitted_at: None,
            comments: None,
        }
    }
    pub fn open(&mut self, root: &Path) {
        if let Some(w) = &self.worker {
            w.stop.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        self.root = Some(root.into());
        self.comments = None;
        self.snapshot = Snapshot::default();
        self.loaded = self.repository.is_none();
        self.next = 0;
        self.revision += 1;
        self.error.clear();
        self.admitted_at = None;
    }
    /// Only authoritative changes invalidate maintenance, never transient empty loading catalogs.
    pub fn observe_comments(&mut self, catalog: &crate::domain::comments::CommentCatalog) {
        let mut links = catalog
            .entries()
            .values()
            .map(|c| (c.audio().clone(), c.photo().clone()))
            .collect::<Vec<_>>();
        links.sort();
        if self.comments.as_ref().is_some_and(|old| old != &links) {
            self.retry();
        }
        self.comments = Some(links);
    }
    pub fn retry(&mut self) {
        if let Some(w) = &self.worker {
            w.stop.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        self.next = 0;
        self.error.clear();
        self.admitted_at = None;
    }
    pub fn poll(&mut self, allowed: bool) {
        if let Some(w) = &self.worker {
            if !allowed && w.stage.is_some() {
                w.stop.store(true, Ordering::Relaxed);
            }
            for progress in w.progress.try_iter() {
                if w.generation == self.generation {
                    self.progress = progress;
                }
            }
            if !w.handle.is_finished() {
                return;
            }
            let w = self.worker.take().unwrap();
            let joined = w.handle.join().is_ok();
            if w.generation == self.generation && !w.stop.load(Ordering::Relaxed) {
                let result = if joined {
                    w.rx.try_recv()
                        .unwrap_or_else(|_| Err("Audioverarbeitung ohne Ergebnis beendet".into()))
                } else {
                    Err("Audioverarbeitung unerwartet beendet".into())
                };
                match result {
                    Ok(snapshot) => {
                        self.snapshot = snapshot;
                        self.loaded = true;
                        self.revision += 1;
                        if w.stage.is_some() {
                            self.next += 1;
                        }
                    }
                    Err(e) => self.error = e,
                }
            }
        }
        if self.root.is_none() || self.repository.is_none() || !self.error.is_empty() {
            return;
        }
        let stage = if !self.loaded {
            None
        } else if allowed {
            STAGES.get(self.next).copied()
        } else {
            return;
        };
        if self.loaded && stage.is_none() {
            return;
        }
        let root = self.root.clone().unwrap();
        let repo = self.repository.clone().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = stop.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let (ptx, progress) = mpsc::sync_channel(1);
        self.progress = Progress::default();
        let handle = thread::spawn(move || {
            let result = (|| {
                if let Some(stage) = stage {
                    repo.run(&root, stage, &cancel, &|p| {
                        let _ = ptx.try_send(p);
                    })?;
                }
                if cancel.load(Ordering::Relaxed) {
                    return Err("Abgebrochen".into());
                }
                repo.read(&root)
            })();
            let _ = tx.send(result);
        });
        self.worker = Some(Worker {
            stop,
            handle,
            rx,
            progress,
            generation: self.generation,
            stage,
        });
    }
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }
    pub fn complete(&self) -> bool {
        self.repository.is_none()
            || (self.loaded && self.next == STAGES.len() && self.error.is_empty())
    }
    pub fn loaded(&self) -> bool {
        self.loaded
    }
    pub fn stage(&self) -> Option<Stage> {
        STAGES.get(self.next).copied()
    }
    pub fn progress(&self) -> &Progress {
        &self.progress
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn inference_ready(&mut self, ready: bool) -> bool {
        if !ready {
            self.admitted_at = None;
            return false;
        }
        self.admitted_at.get_or_insert_with(Instant::now).elapsed() >= Duration::from_secs(2)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(w) = &self.worker {
            w.stop.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, atomic::AtomicUsize};
    #[derive(Default)]
    struct Store {
        reads: AtomicUsize,
        steps: Mutex<Vec<(PathBuf, Stage)>>,
        block: AtomicBool,
        entered: AtomicBool,
        active: AtomicUsize,
    }
    impl Repository for Store {
        fn read(&self, _: &Path) -> Result<Snapshot, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(Snapshot::default())
        }
        fn run(
            &self,
            root: &Path,
            stage: Stage,
            stop: &AtomicBool,
            p: &dyn Fn(Progress),
        ) -> Result<(), String> {
            assert_eq!(
                self.active.fetch_add(1, Ordering::SeqCst),
                0,
                "maintenance overlapped"
            );
            self.entered.store(true, Ordering::SeqCst);
            while self.block.load(Ordering::SeqCst) && !stop.load(Ordering::SeqCst) {
                thread::yield_now();
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            if stop.load(Ordering::SeqCst) {
                return Err("cancelled".into());
            }
            self.steps.lock().unwrap().push((root.into(), stage));
            p(Progress {
                checked: 1,
                total: 1,
                current: None,
            });
            Ok(())
        }
    }
    fn until(session: &mut Session, allowed: bool, test: impl Fn(&Session) -> bool) {
        let limit = Instant::now() + Duration::from_secs(3);
        while !test(session) {
            session.poll(allowed);
            assert!(
                Instant::now() < limit,
                "audio workflow stalled: {}",
                session.error()
            );
            thread::yield_now();
        }
    }
    #[test]
    fn startup_reads_only_projection_even_when_paused_then_runs_each_step_once_in_order() {
        let store = Arc::new(Store::default());
        let mut session = Session::new(Some(store.clone()));
        session.open(Path::new("/library"));
        until(&mut session, false, |s| s.loaded() && !s.busy());
        assert_eq!(store.reads.load(Ordering::SeqCst), 1);
        assert!(store.steps.lock().unwrap().is_empty());
        until(&mut session, true, Session::complete);
        assert_eq!(
            *store.steps.lock().unwrap(),
            [
                Stage::Inventory,
                Stage::Durations,
                Stage::Metadata,
                Stage::Years
            ]
            .map(|s| (PathBuf::from("/library"), s))
        );
        let reads = store.reads.load(Ordering::SeqCst);
        for _ in 0..20 {
            session.poll(true);
        }
        assert_eq!(store.reads.load(Ordering::SeqCst), reads);
    }
    #[test]
    fn switching_library_waits_for_cancelled_worker_and_never_publishes_old_results() {
        let store = Arc::new(Store::default());
        store.block.store(true, Ordering::SeqCst);
        let mut session = Session::new(Some(store.clone()));
        session.open(Path::new("/old"));
        until(&mut session, true, |_| store.entered.load(Ordering::SeqCst));
        session.open(Path::new("/new"));
        store.block.store(false, Ordering::SeqCst);
        until(&mut session, true, Session::complete);
        assert!(
            store
                .steps
                .lock()
                .unwrap()
                .iter()
                .all(|(root, _)| root == Path::new("/new"))
        );
        assert_eq!(store.active.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn pause_cancels_current_step_and_resume_retries_without_skipping_it() {
        let store = Arc::new(Store::default());
        store.block.store(true, Ordering::SeqCst);
        let mut session = Session::new(Some(store.clone()));
        session.open(Path::new("/library"));
        until(&mut session, true, |_| store.entered.load(Ordering::SeqCst));
        until(&mut session, false, |s| !s.busy());
        assert_eq!(session.stage(), Some(Stage::Inventory));
        store.block.store(false, Ordering::SeqCst);
        until(&mut session, true, Session::complete);
        assert_eq!(store.steps.lock().unwrap().len(), 4);
    }
    #[test]
    fn inference_waits_for_stable_preflight_and_resets_delay_when_readiness_is_lost() {
        let mut session = Session::new(None);
        assert!(!session.inference_ready(true));
        session.admitted_at = Some(Instant::now() - Duration::from_secs(3));
        assert!(session.inference_ready(true));
        assert!(!session.inference_ready(false));
        assert!(!session.inference_ready(true));
    }
}
