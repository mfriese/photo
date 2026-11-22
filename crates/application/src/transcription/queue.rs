//! Durable library-wide transcription scheduling. UI state never owns inference/persistence rules.
use crate::{
    domain::{
        tags::LibraryFile,
        transcript::{Segment, SourceVersion, Transcript, TranscriptStatus},
    },
    transcription::Settings,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub source: LibraryFile,
    pub record: LibraryFile,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobState {
    Pending,
    Complete,
    Failed(String),
}
#[derive(Clone, Debug)]
pub struct Job {
    pub candidate: Candidate,
    pub version: SourceVersion,
    pub state: JobState,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Running,
    Paused,
    Stopped,
}
impl Mode {
    fn byte(self) -> u8 {
        match self {
            Self::Running => 0,
            Self::Paused => 1,
            Self::Stopped => 2,
        }
    }
    fn from_byte(v: u8) -> Self {
        match v {
            1 => Self::Paused,
            2 => Self::Stopped,
            _ => Self::Running,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Journal {
    pub jobs: BTreeMap<LibraryFile, Job>,
    pub mode: Mode,
    pub revision: Option<Vec<u8>>,
}
pub trait QueueStore: Send + Sync + 'static {
    fn load(&self, root: &Path) -> Result<Journal, String>;
    fn load_cancellable(&self, root: &Path, cancel: &AtomicBool) -> Result<Journal, String> {
        check(cancel)?;
        self.load(root)
    }
    fn save_mode(&self, root: &Path, mode: Mode) -> Result<(), String> {
        let mut journal = self.load(root)?;
        journal.mode = mode;
        self.save(root, &journal).map(|_| ())
    }
    fn load_mode(&self, root: &Path) -> Result<Mode, String> {
        self.load(root).map(|j| j.mode)
    }
    fn save(&self, root: &Path, journal: &Journal) -> Result<Vec<u8>, String>;
    fn version(&self, root: &Path, source: &LibraryFile) -> Result<SourceVersion, String>;
    fn existing(&self, root: &Path, candidate: &Candidate) -> Result<Option<Transcript>, String>;
    fn write_transcript(
        &self,
        root: &Path,
        candidate: &Candidate,
        transcript: &Transcript,
    ) -> Result<(), String>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Planning,
    LoadingModel,
    Transcribing,
    Saving,
}
#[derive(Clone, Debug)]
pub struct WorkProgress {
    pub phase: Phase,
    pub fraction: f32,
    /// A provisional recognized segment for the bounded live log, not persistence.
    pub finding: Option<String>,
}
#[derive(Clone, Debug)]
pub struct EngineOutput {
    pub language: String,
    pub segments: Vec<Segment>,
    pub no_audio: bool,
}
pub trait Engine: Send + Sync + 'static {
    fn release_model(&self) {}
    fn transcribe(
        &self,
        model: &ModelChoice,
        path: &Path,
        cancel: &AtomicBool,
        progress: &(dyn Fn(WorkProgress) + Sync),
    ) -> Result<EngineOutput, String>;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelChoice {
    pub path: PathBuf,
    pub settings: Settings,
}
pub trait AwakeLease: Send + Sync + 'static {}
pub trait PowerManager: Send + Sync + 'static {
    fn acquire(&self) -> Result<Arc<dyn AwakeLease>, String>;
}
#[derive(Clone, Debug)]
pub struct Receipt {
    pub source: LibraryFile,
    pub record: LibraryFile,
    pub excerpt: String,
}
/// Latest 16 recognized segments plus bounded file-start headings. Not persisted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    pub source: LibraryFile,
    pub text: Option<String>,
}
const LOG_LIMIT: usize = 16;
fn push_bounded<T>(queue: &mut VecDeque<T>, value: T) {
    if queue.len() == LOG_LIMIT {
        queue.pop_front();
    }
    queue.push_back(value);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Indicator {
    Idle,
    Busy,
    Paused,
}
#[derive(Clone, PartialEq, Eq)]
struct Catalog {
    root: PathBuf,
    sources: Vec<Candidate>,
    complete: bool,
}
fn push_log(log: &mut VecDeque<LogEntry>, value: LogEntry) {
    log.push_back(value);
    while log.iter().filter(|entry| entry.text.is_some()).count() > LOG_LIMIT {
        while let Some(entry) = log.pop_front() {
            if entry.text.is_some() {
                break;
            }
        }
    }
    while log.len() > LOG_LIMIT * 2 + 1 {
        // Empty/silent files must not evict one of the latest sixteen findings.
        if let Some(index) = log.iter().position(|entry| entry.text.is_none()) {
            log.remove(index);
        }
    }
}
enum Event {
    Mode(Result<Mode, String>),
    ModeSaved(Result<(), String>),
    Progress(WorkProgress),
    Done(Result<(Journal, Option<Receipt>), String>),
}
struct Active {
    worker: Option<std::thread::JoinHandle<()>>,
    receiver: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
    generation: u64,
    planning: bool,
    findings: Arc<Mutex<VecDeque<String>>>,
}
pub struct QueueSession {
    store: Arc<dyn QueueStore>,
    engine: Arc<dyn Engine>,
    power: Arc<dyn PowerManager>,
    awake: Option<Arc<dyn AwakeLease>>,
    catalog: Option<Catalog>,
    reconcile: bool,
    generation: u64,
    mode: Arc<AtomicU8>,
    mode_loaded: bool,
    automatic_start: bool,
    journal: Option<Journal>,
    active: Option<Active>,
    model: Option<ModelChoice>,
    hold: bool,
    planning_hold: bool,
    progress: WorkProgress,
    current: Option<LibraryFile>,
    last: Option<Receipt>,
    log: VecDeque<LogEntry>,
    error: String,
    controls_dirty: bool,
    retry_pending: bool,
    forced: BTreeSet<LibraryFile>,
    closed: bool,
}
fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Abgebrochen".into())
    } else {
        Ok(())
    }
}
fn persist(store: &dyn QueueStore, root: &Path, journal: &mut Journal) -> Result<(), String> {
    journal.revision = Some(store.save(root, journal)?);
    Ok(())
}
fn plan(
    store: &dyn QueueStore,
    catalog: &Catalog,
    cancel: &AtomicBool,
    forced: &BTreeSet<LibraryFile>,
) -> Result<Journal, String> {
    let mut journal = store.load_cancellable(&catalog.root, cancel)?;
    for candidate in &catalog.sources {
        check(cancel)?;
        let version = match store.version(&catalog.root, &candidate.source) {
            Ok(value) => value,
            Err(error) => {
                journal.jobs.insert(
                    candidate.source.clone(),
                    Job {
                        candidate: candidate.clone(),
                        version: SourceVersion::new(0, "0".into())?,
                        state: JobState::Failed(error),
                    },
                );
                continue;
            }
        };
        let cached = match store.existing(&catalog.root, candidate) {
            Ok(value) => value,
            Err(error) => {
                journal.jobs.insert(
                    candidate.source.clone(),
                    Job {
                        candidate: candidate.clone(),
                        version,
                        state: JobState::Failed(error),
                    },
                );
                continue;
            }
        };
        let state = if forced.contains(&candidate.source) {
            JobState::Pending
        } else if cached.as_ref().is_some_and(|t| t.source() == &version) {
            JobState::Complete
        } else if let Some(old) = journal
            .jobs
            .get(&candidate.source)
            .filter(|j| j.version == version && j.candidate == *candidate)
        {
            match &old.state {
                JobState::Failed(e) => JobState::Failed(e.clone()),
                _ => JobState::Pending,
            }
        } else {
            JobState::Pending
        };
        journal.jobs.insert(
            candidate.source.clone(),
            Job {
                candidate: candidate.clone(),
                version,
                state,
            },
        );
    }
    if catalog.complete {
        journal
            .jobs
            .retain(|source, _| catalog.sources.iter().any(|c| &c.source == source));
    }
    check(cancel)?;
    persist(store, &catalog.root, &mut journal)?;
    Ok(journal)
}
struct RunContext<'a> {
    force: bool,
    root: &'a Path,
    model: &'a ModelChoice,
    cancel: &'a AtomicBool,
    mode: &'a AtomicU8,
    progress: &'a (dyn Fn(WorkProgress) + Sync),
}
fn run(
    store: &dyn QueueStore,
    engine: &dyn Engine,
    mut journal: Journal,
    key: &LibraryFile,
    context: RunContext<'_>,
) -> Result<(Journal, Option<Receipt>), String> {
    let RunContext {
        force,
        root,
        model,
        cancel,
        mode,
        progress,
    } = context;
    let job = journal
        .jobs
        .get_mut(key)
        .ok_or("Transkriptionsauftrag fehlt")?;
    let result = (|| {
        check(cancel)?;
        let before = store.version(root, &job.candidate.source)?;
        job.version = before.clone();
        let transcript = if let Some(cached) = store
            .existing(root, &job.candidate)?
            .filter(|t| !force && t.source() == &before)
        {
            cached
        } else {
            let output = engine.transcribe(
                model,
                &root.join(job.candidate.source.as_str()),
                cancel,
                progress,
            )?;
            check(cancel)?;
            if store.version(root, &job.candidate.source)? != before {
                return Err("Datei wurde während der Transkription verändert; bisheriger Text bleibt erhalten".into());
            }
            super::quality::validate(&output.segments)?;
            let text = output
                .segments
                .iter()
                .map(|s| s.text().trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let status = if output.no_audio {
                TranscriptStatus::NoAudio
            } else if text.is_empty() {
                TranscriptStatus::NoSpeech
            } else {
                TranscriptStatus::Complete
            };
            let result = Transcript::new(
                before,
                status,
                model.settings.model().id.into(),
                output.language,
                text,
                output.segments,
            )?;
            progress(WorkProgress {
                phase: Phase::Saving,
                fraction: 1.,
                finding: None,
            });
            check(cancel)?;
            store.write_transcript(root, &job.candidate, &result)?;
            result
        };
        Ok(Receipt {
            source: job.candidate.source.clone(),
            record: job.candidate.record.clone(),
            excerpt: transcript.text().chars().take(1600).collect(),
        })
    })();
    let receipt = match result {
        Ok(receipt) => {
            job.state = JobState::Complete;
            Some(receipt)
        }
        Err(_) if cancel.load(Ordering::Relaxed) => {
            job.state = JobState::Pending;
            None
        }
        Err(error) => {
            job.state = JobState::Failed(error);
            None
        }
    };
    journal.mode = Mode::from_byte(mode.load(Ordering::Relaxed));
    persist(store, root, &mut journal)?;
    Ok((journal, receipt))
}
impl QueueSession {
    pub fn new(
        store: Arc<dyn QueueStore>,
        engine: Arc<dyn Engine>,
        power: Arc<dyn PowerManager>,
    ) -> Self {
        Self {
            store,
            engine,
            power,
            awake: None,
            catalog: None,
            reconcile: false,
            generation: 0,
            mode: Arc::new(AtomicU8::new(0)),
            mode_loaded: false,
            automatic_start: false,
            journal: None,
            active: None,
            model: None,
            hold: false,
            planning_hold: false,
            progress: WorkProgress {
                phase: Phase::Planning,
                fraction: 0.,
                finding: None,
            },
            current: None,
            last: None,
            log: VecDeque::new(),
            error: String::new(),
            controls_dirty: false,
            retry_pending: false,
            forced: BTreeSet::new(),
            closed: false,
        }
    }
    /// Desktop policy: a new program/library session always starts enabled.
    /// Subsequent pause/resume within that session remains authoritative.
    pub fn with_automatic_start(mut self) -> Self {
        self.automatic_start = true;
        self.mode_loaded = true;
        self.controls_dirty = true;
        self
    }
    pub fn planning(&self) -> bool {
        self.active.as_ref().is_some_and(|active| active.planning)
    }
    pub fn planned(&self) -> bool {
        self.journal.is_some() && !self.reconcile && !self.planning()
    }
    /// Cancel, drain bounded channels, join, then release native/GPU resources before process exit.
    /// Idempotent; a closed session cannot schedule more work.
    pub fn shutdown(&mut self) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.hold = true;
        self.planning_hold = true;
        self.reconcile = false;
        self.retry_pending = false;
        let mut failure = None;
        if let Some(mut active) = self.active.take() {
            active.cancel.store(true, Ordering::Relaxed);
            // Done uses a bounded sender; keep draining while joining to avoid a full-channel deadlock.
            while active
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished())
            {
                if let Ok(event) = active
                    .receiver
                    .recv_timeout(std::time::Duration::from_millis(10))
                {
                    match event {
                        Event::ModeSaved(Err(error)) => failure = Some(error),
                        Event::Done(Err(error)) if !active.planning => failure = Some(error),
                        _ => {}
                    }
                }
            }
            for event in active.receiver.try_iter() {
                match event {
                    Event::ModeSaved(Err(error)) => failure = Some(error),
                    Event::Done(Err(error)) if !active.planning => failure = Some(error),
                    _ => {}
                }
            }
            if let Some(worker) = active.worker.take()
                && worker.join().is_err()
            {
                failure = Some("Transkriptionsworker beim Beenden unerwartet beendet".to_owned());
            }
        }
        self.current = None;
        self.awake = None;
        self.engine.release_model();
        failure.map_or(Ok(()), Err)
    }
    /// A committed source-path change invalidates old revisions/results, while preserving user pause.
    pub fn media_paths_changed(&mut self) {
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Relaxed);
        }
        self.generation = self.generation.wrapping_add(1);
        self.journal = None;
        self.reconcile = true;
        self.current = None;
        self.last = None;
        self.error.clear();
        self.retry_pending = false;
    }
    pub fn reset(&mut self) {
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        self.catalog = None;
        self.forced.clear();
        self.journal = None;
        self.reconcile = false;
        self.current = None;
        self.last = None;
        self.log.clear();
        self.error.clear();
        self.awake = None;
        self.mode = Arc::new(AtomicU8::new(0));
        self.mode_loaded = false;
        self.controls_dirty = self.automatic_start;
        self.mode_loaded = self.automatic_start;
        self.retry_pending = false;
    }
    pub fn sync(&mut self, root: &Path, sources: Vec<Candidate>, complete: bool) {
        let catalog = Catalog {
            root: root.into(),
            sources,
            complete,
        };
        if self.catalog.as_ref() == Some(&catalog) {
            return;
        }
        if self.catalog.as_ref().is_none_or(|c| c.root != root) {
            if let Some(active) = &self.active {
                active.cancel.store(true, Ordering::Relaxed);
            }
            self.generation += 1;
            self.forced.clear();
            self.journal = None;
            self.retry_pending = false;
            if self.catalog.is_some() || !self.mode_loaded {
                self.mode = Arc::new(AtomicU8::new(0));
                self.mode_loaded = self.automatic_start;
                self.controls_dirty = self.automatic_start;
            }
            self.last = None;
            self.log.clear();
        }
        self.catalog = Some(catalog);
        self.reconcile = true;
        self.error.clear();
    }
    pub fn set_model(&mut self, model: Option<ModelChoice>) {
        self.model = model;
    }
    pub fn set_hold(&mut self, hold: bool) {
        let changed = hold && !self.hold;
        self.hold = hold;
        if changed
            && self.current.is_some()
            && let Some(active) = &self.active
        {
            active.cancel.store(true, Ordering::Relaxed);
        }
    }
    /// Gate expensive reconciliation as well as inference. Mode loading is lightweight.
    pub fn hold_planning(&mut self, hold: bool) {
        self.planning_hold = hold;
        if hold && let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn mode_loaded(&self) -> bool {
        self.mode_loaded
    }
    pub fn mode(&self) -> Mode {
        Mode::from_byte(self.mode.load(Ordering::Relaxed))
    }
    pub fn set_mode(&mut self, mode: Mode) {
        self.controls_dirty = true;
        self.mode_loaded = true;
        self.mode.store(mode.byte(), Ordering::Relaxed);
        if mode != Mode::Running
            && let Some(active) = &self.active
        {
            active.cancel.store(true, Ordering::Relaxed);
        }
    }
    /// Explicit session-scoped reprocessing. Existing texts remain until replacement succeeds.
    pub fn can_restart_all(&self) -> bool {
        !self.closed
            && self.active.is_none()
            && self
                .catalog
                .as_ref()
                .is_some_and(|catalog| catalog.complete && !catalog.sources.is_empty())
    }
    pub fn restart_all(&mut self) -> Result<usize, String> {
        if !self.can_restart_all() {
            return Err(
                "Bitte vollständigen Bestand abwarten und laufende Verarbeitung zuerst pausieren."
                    .into(),
            );
        }
        self.forced = self
            .catalog
            .as_ref()
            .unwrap()
            .sources
            .iter()
            .map(|candidate| candidate.source.clone())
            .collect();
        self.reconcile = true;
        self.error.clear();
        self.retry_pending = false;
        self.set_mode(Mode::Running);
        Ok(self.forced.len())
    }
    pub fn rescan(&mut self) {
        self.error.clear();
        self.reconcile = true;
    }
    /// Explicit recovery also re-plans a queue whose initial load/publication failed.
    pub fn resume(&mut self) {
        self.error.clear();
        if self.journal.is_none() {
            self.reconcile = true;
        }
        self.set_mode(Mode::Running);
    }
    pub fn retry_failed(&mut self) {
        self.retry_pending = true;
        self.resume();
        self.apply_retry();
    }
    fn apply_retry(&mut self) {
        if !self.retry_pending || self.active.is_some() {
            return;
        }
        if let Some(journal) = &mut self.journal {
            for job in journal.jobs.values_mut() {
                if matches!(job.state, JobState::Failed(_)) {
                    job.state = JobState::Pending;
                }
            }
            self.retry_pending = false;
            self.controls_dirty = true;
        }
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn progress(&self) -> &WorkProgress {
        &self.progress
    }
    pub fn current(&self) -> Option<&LibraryFile> {
        self.current.as_ref()
    }
    pub fn last(&self) -> Option<&Receipt> {
        self.last.as_ref()
    }
    pub fn failures(&self) -> Vec<(&LibraryFile, &str)> {
        self.journal
            .as_ref()
            .map(|j| {
                j.jobs
                    .iter()
                    .filter_map(|(key, job)| {
                        if let JobState::Failed(e) = &job.state {
                            Some((key, e.as_str()))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn log(&self) -> &VecDeque<LogEntry> {
        &self.log
    }
    pub fn counts(&self) -> (usize, usize, usize) {
        self.journal
            .as_ref()
            .map(|j| {
                let done = j
                    .jobs
                    .values()
                    .filter(|v| v.state == JobState::Complete)
                    .count();
                let failed = j
                    .jobs
                    .values()
                    .filter(|v| matches!(v.state, JobState::Failed(_)))
                    .count();
                (done, j.jobs.len(), failed)
            })
            .unwrap_or((0, self.catalog.as_ref().map_or(0, |c| c.sources.len()), 0))
    }
    pub fn indicator(&self) -> Indicator {
        if self.active.is_some() {
            Indicator::Busy
        } else {
            let (done, total, failed) = self.counts();
            if done < total || failed > 0 || !self.error.is_empty() {
                Indicator::Paused
            } else {
                Indicator::Idle
            }
        }
    }
    pub fn busy(&self) -> bool {
        self.active.is_some()
    }
    pub fn pending(&self) -> usize {
        let (done, total, failed) = self.counts();
        total.saturating_sub(done + failed)
    }
    pub fn waiting_for_model(&self) -> bool {
        self.model.is_none() && self.pending() > 0
    }
    pub fn held(&self) -> bool {
        self.hold
    }
    pub fn poll(&mut self) {
        let mut result = None;
        let mut loaded_mode = None;
        let mut saved_mode = None;
        if let Some(active) = &self.active {
            if active.generation == self.generation
                && let Some(source) = &self.current
            {
                let mut findings = active.findings.lock().unwrap_or_else(|e| e.into_inner());
                for text in findings.drain(..) {
                    push_log(
                        &mut self.log,
                        LogEntry {
                            source: source.clone(),
                            text: Some(text),
                        },
                    );
                }
            }
            for _ in 0..16 {
                match active.receiver.try_recv() {
                    Ok(Event::ModeSaved(value)) => {
                        saved_mode = Some(value);
                        break;
                    }
                    Ok(Event::Mode(value)) => {
                        loaded_mode = Some(value);
                        break;
                    }
                    Ok(Event::Progress(progress)) => {
                        if active.generation == self.generation {
                            self.progress = progress;
                        }
                    }
                    Ok(Event::Done(value)) => {
                        result = Some(value);
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        result = Some(Err("Transkriptionsworker unerwartet beendet".into()));
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if let Some(value) = saved_mode {
            let mut active = self.active.take().unwrap();
            if let Some(worker) = active.worker.take() {
                let _ = worker.join();
            }
            if active.generation == self.generation {
                match value {
                    Ok(()) => self.reconcile = true,
                    Err(error) => self.error = error,
                }
            }
        }
        if let Some(value) = loaded_mode {
            let mut active = self.active.take().unwrap();
            if let Some(worker) = active.worker.take() {
                let _ = worker.join();
            }
            if active.generation == self.generation {
                match value {
                    Ok(mode) => {
                        if !self.mode_loaded {
                            self.mode.store(mode.byte(), Ordering::Relaxed);
                            self.mode_loaded = true;
                        }
                    }
                    Err(error) => self.error = error,
                }
            }
        }
        if let Some(result) = result {
            let mut active = self.active.take().unwrap();
            if let Some(worker) = active.worker.take() {
                let _ = worker.join();
            }
            if active.generation == self.generation {
                if let Some(source) = &self.current {
                    let mut findings = active.findings.lock().unwrap_or_else(|e| e.into_inner());
                    for text in findings.drain(..) {
                        push_log(
                            &mut self.log,
                            LogEntry {
                                source: source.clone(),
                                text: Some(text),
                            },
                        );
                    }
                }
                match result {
                    Ok((journal, receipt)) => {
                        if !active.planning
                            && let Some(source) = &self.current
                            && journal
                                .jobs
                                .get(source)
                                .is_some_and(|job| matches!(job.state, JobState::Complete))
                        {
                            self.forced.remove(source);
                        }
                        if active.planning && !self.mode_loaded {
                            self.mode.store(journal.mode.byte(), Ordering::Relaxed);
                            self.mode_loaded = true;
                        }
                        self.journal = Some(journal);
                        if receipt.is_some() {
                            self.last = receipt;
                        }
                        self.error.clear();
                    }
                    Err(_) if active.cancel.load(Ordering::Relaxed) && active.planning => {
                        self.reconcile = true
                    }
                    Err(error) => self.error = error,
                }
            }
            self.current = None;
        }
        self.apply_retry();
        self.schedule();
        if self.active.is_none() {
            self.awake = None;
        }
    }
    fn schedule(&mut self) {
        if self.closed || self.active.is_some() || !self.error.is_empty() {
            return;
        }
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        let (tx, receiver) = mpsc::sync_channel(8);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let store = self.store.clone();
        let generation = self.generation;
        if self.controls_dirty && (self.planning_hold || self.journal.is_none()) {
            self.controls_dirty = false;
            let mode = self.mode();
            self.active = Some(Active {
                worker: None,
                receiver,
                cancel,
                generation,
                planning: false,
                findings: Default::default(),
            });
            let worker = std::thread::spawn(move || {
                let _ = tx.send(Event::ModeSaved(store.save_mode(&catalog.root, mode)));
            });
            self.active.as_mut().unwrap().worker = Some(worker);
            return;
        }
        if self.planning_hold && !self.mode_loaded {
            self.active = Some(Active {
                worker: None,
                receiver,
                cancel,
                generation,
                planning: true,
                findings: Default::default(),
            });
            let worker = std::thread::spawn(move || {
                let _ = tx.send(Event::Mode(store.load_mode(&catalog.root)));
            });
            self.active.as_mut().unwrap().worker = Some(worker);
            return;
        }
        if self.planning_hold && self.reconcile {
            return;
        }
        if self.reconcile {
            if self.mode() != Mode::Running && self.journal.is_none() {
                return;
            }
            self.reconcile = false;
            if self.mode() != Mode::Running || self.hold {
                self.awake = None;
            }
            self.progress = WorkProgress {
                phase: Phase::Planning,
                fraction: 0.,
                finding: None,
            };
            self.active = Some(Active {
                worker: None,
                receiver,
                cancel,
                generation,
                planning: true,
                findings: Default::default(),
            });
            let forced = self.forced.clone();
            let worker = std::thread::spawn(move || {
                let result = plan(store.as_ref(), &catalog, &stop, &forced).map(|j| (j, None));
                let _ = tx.send(Event::Done(result));
            });
            self.active.as_mut().unwrap().worker = Some(worker);
            return;
        }
        let Some(mut journal) = self.journal.clone() else {
            return;
        };
        if journal.mode != self.mode() || self.controls_dirty {
            self.controls_dirty = false;
            journal.mode = self.mode();
            self.active = Some(Active {
                worker: None,
                receiver,
                cancel,
                generation,
                planning: false,
                findings: Default::default(),
            });
            let worker = std::thread::spawn(move || {
                let result =
                    persist(store.as_ref(), &catalog.root, &mut journal).map(|()| (journal, None));
                let _ = tx.send(Event::Done(result));
            });
            self.active.as_mut().unwrap().worker = Some(worker);
            return;
        }
        if self.mode() != Mode::Running || self.hold {
            return;
        }
        let Some(model) = self.model.clone() else {
            return;
        };
        let Some(key) = journal.jobs.iter().find_map(|(key, j)| {
            if j.state == JobState::Pending {
                Some(key.clone())
            } else {
                None
            }
        }) else {
            return;
        };
        if self.awake.is_none() {
            match self.power.acquire() {
                Ok(lease) => self.awake = Some(lease),
                Err(e) => {
                    self.error = format!("Ruhezustand konnte nicht gesperrt werden: {e}");
                    return;
                }
            }
        }
        let lease = self.awake.as_ref().unwrap().clone();
        let engine = self.engine.clone();
        let mode = self.mode.clone();
        self.current = Some(key.clone());
        self.progress = WorkProgress {
            phase: Phase::LoadingModel,
            fraction: 0.,
            finding: None,
        };
        push_log(
            &mut self.log,
            LogEntry {
                source: key.clone(),
                text: None,
            },
        );
        let findings = Arc::new(Mutex::new(VecDeque::new()));
        self.active = Some(Active {
            worker: None,
            receiver,
            cancel,
            generation,
            planning: false,
            findings: findings.clone(),
        });
        let force = self.forced.contains(&key);
        let worker = std::thread::spawn(move || {
            let _lease = lease;
            let notify = |mut p: WorkProgress| {
                if let Some(text) = p.finding.take()
                    && !text.trim().is_empty()
                {
                    let text = text.chars().take(2000).collect();
                    let mut pending = findings.lock().unwrap_or_else(|e| e.into_inner());
                    push_bounded(&mut pending, text);
                }
                let _ = tx.try_send(Event::Progress(p));
            };
            let result = run(
                store.as_ref(),
                engine.as_ref(),
                journal,
                &key,
                RunContext {
                    force,
                    root: &catalog.root,
                    model: &model,
                    cancel: &stop,
                    mode: &mode,
                    progress: &notify,
                },
            );
            if result.is_err()
                || stop.load(Ordering::Relaxed)
                || result
                    .as_ref()
                    .is_ok_and(|(j, _)| !j.jobs.values().any(|v| v.state == JobState::Pending))
            {
                engine.release_model();
            }
            let _ = tx.send(Event::Done(result));
        });
        self.active.as_mut().unwrap().worker = Some(worker);
    }
}
impl Drop for QueueSession {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, atomic::AtomicUsize};
    #[derive(Default)]
    struct Store {
        journals: Mutex<BTreeMap<PathBuf, Journal>>,
        texts: Mutex<BTreeMap<String, Transcript>>,
        writes: AtomicUsize,
        changed: AtomicBool,
        fail_journal: AtomicBool,
    }
    fn storage_key(root: &Path, c: &Candidate) -> String {
        format!(
            "{}:{}:{}",
            root.display(),
            c.record.as_str(),
            c.source.as_str()
        )
    }
    impl QueueStore for Store {
        fn load(&self, root: &Path) -> Result<Journal, String> {
            Ok(self
                .journals
                .lock()
                .unwrap()
                .get(root)
                .cloned()
                .unwrap_or_default())
        }
        fn save(&self, root: &Path, j: &Journal) -> Result<Vec<u8>, String> {
            if self.fail_journal.load(Ordering::SeqCst) {
                return Err("disk full".into());
            }
            let mut j = j.clone();
            let revision = vec![self.writes.fetch_add(1, Ordering::SeqCst) as u8];
            j.revision = Some(revision.clone());
            self.journals.lock().unwrap().insert(root.into(), j);
            Ok(revision)
        }
        fn version(&self, _: &Path, _: &LibraryFile) -> Result<SourceVersion, String> {
            SourceVersion::new(
                if self.changed.load(Ordering::SeqCst) {
                    2
                } else {
                    1
                },
                "0".into(),
            )
        }
        fn existing(&self, root: &Path, c: &Candidate) -> Result<Option<Transcript>, String> {
            Ok(self
                .texts
                .lock()
                .unwrap()
                .get(&storage_key(root, c))
                .cloned())
        }
        fn write_transcript(
            &self,
            root: &Path,
            c: &Candidate,
            t: &Transcript,
        ) -> Result<(), String> {
            self.texts
                .lock()
                .unwrap()
                .insert(storage_key(root, c), t.clone());
            Ok(())
        }
    }
    #[derive(Default)]
    struct PowerState {
        held: AtomicUsize,
        acquired: AtomicUsize,
        fail: AtomicBool,
    }
    struct Power(Arc<PowerState>);
    struct Lease(Arc<PowerState>);
    impl AwakeLease for Lease {}
    impl Drop for Lease {
        fn drop(&mut self) {
            self.0.held.fetch_sub(1, Ordering::SeqCst);
        }
    }
    impl PowerManager for Power {
        fn acquire(&self) -> Result<Arc<dyn AwakeLease>, String> {
            if self.0.fail.load(Ordering::SeqCst) {
                return Err("power denied".into());
            }
            self.0.held.fetch_add(1, Ordering::SeqCst);
            self.0.acquired.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(Lease(self.0.clone())))
        }
    }
    struct TestEngine {
        calls: AtomicUsize,
        released: AtomicUsize,
        block: AtomicBool,
        panic: AtomicBool,
        power: Arc<PowerState>,
        fail_save: Option<Arc<Store>>,
    }
    impl Engine for TestEngine {
        fn transcribe(
            &self,
            _: &ModelChoice,
            path: &Path,
            stop: &AtomicBool,
            progress: &(dyn Fn(WorkProgress) + Sync),
        ) -> Result<EngineOutput, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(
                self.power.held.load(Ordering::SeqCst) > 0,
                "sleep assertion must cover inference"
            );
            assert!(!self.panic.load(Ordering::SeqCst), "simulated panic");
            while self.block.load(Ordering::SeqCst) && !stop.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            check(stop)?;
            if path.to_string_lossy().contains("bad") {
                return Err("codec error".into());
            }
            progress(WorkProgress {
                phase: Phase::Transcribing,
                fraction: 0.5,
                finding: Some("Erkannter Text".into()),
            });
            if path.to_string_lossy().contains("burst") {
                for i in 0..100 {
                    progress(WorkProgress {
                        phase: Phase::Transcribing,
                        fraction: i as f32 / 100.,
                        finding: Some(format!("Segment {i}")),
                    });
                }
            }
            if let Some(store) = &self.fail_save {
                store.fail_journal.store(true, Ordering::SeqCst);
            }
            Ok(EngineOutput {
                language: "en".into(),
                no_audio: false,
                segments: vec![Segment::new(
                    0,
                    1000,
                    "Grüße: # Beispiel\nZweite Zeile".into(),
                )?],
            })
        }
        fn release_model(&self) {
            self.released.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn repetition_failure_never_replaces_a_previously_confirmed_transcript() {
        struct LoopEngine;
        impl Engine for LoopEngine {
            fn transcribe(
                &self,
                _: &ModelChoice,
                _: &Path,
                _: &AtomicBool,
                _: &(dyn Fn(WorkProgress) + Sync),
            ) -> Result<EngineOutput, String> {
                Ok(EngineOutput {
                    language: "de".into(),
                    no_audio: false,
                    segments: vec![Segment::new(0, 1000, "Danke ".repeat(20)).unwrap()],
                })
            }
        }
        let (_, store, _, power) = setup();
        let root = Path::new("/fixture");
        let candidate = candidate("a.wav");
        let old = Transcript::new(
            SourceVersion::new(99, "0".into()).unwrap(),
            TranscriptStatus::Complete,
            "tiny".into(),
            "de".into(),
            "Bestätigter Text".into(),
            vec![Segment::new(0, 1000, "Bestätigter Text".into()).unwrap()],
        )
        .unwrap();
        store
            .texts
            .lock()
            .unwrap()
            .insert(storage_key(root, &candidate), old.clone());
        let mut queue =
            QueueSession::new(store.clone(), Arc::new(LoopEngine), Arc::new(Power(power)));
        queue.sync(root, vec![candidate.clone()], true);
        queue.set_model(Some(choice()));
        until(&mut queue, |queue| queue.counts().2 == 1);
        assert_eq!(
            store.texts.lock().unwrap()[&storage_key(root, &candidate)],
            old
        );
        assert!(
            matches!(&store.journals.lock().unwrap()[root].jobs[&candidate.source].state, JobState::Failed(error) if error.contains("Wiederholungsschleife"))
        );
    }
    #[test]
    fn explicit_restart_reprocesses_cached_files_once_and_keeps_old_text_until_success() {
        let (mut queue, store, engine, _) = setup();
        let root = Path::new("/fixture");
        let file = candidate("a.wav");
        queue.sync(root, vec![file.clone()], true);
        queue.set_model(Some(choice()));
        until(&mut queue, |q| q.counts() == (1, 1, 0) && !q.busy());
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        let old = store.texts.lock().unwrap()[&storage_key(root, &file)].clone();
        assert_eq!(queue.restart_all().unwrap(), 1);
        assert_eq!(store.texts.lock().unwrap()[&storage_key(root, &file)], old);
        queue.set_mode(Mode::Paused);
        until(&mut queue, |q| q.planned() && !q.busy());
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        queue.resume();
        until(&mut queue, |q| {
            engine.calls.load(Ordering::SeqCst) == 2 && q.counts() == (1, 1, 0) && !q.busy()
        });
        assert!(queue.forced.is_empty());
        queue.rescan();
        until(&mut queue, |q| q.planned() && !q.busy());
        assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
        queue.forced.insert(file.source);
        queue.sync(Path::new("/other"), Vec::new(), true);
        assert!(queue.forced.is_empty());
    }
    fn candidate(name: &str) -> Candidate {
        let source = LibraryFile::new(format!("2026/11/{name}")).unwrap();
        Candidate {
            record: source.clone(),
            source,
        }
    }
    fn choice() -> ModelChoice {
        ModelChoice {
            path: "/global/model.bin".into(),
            settings: Settings::new("tiny", "en").unwrap(),
        }
    }
    fn setup() -> (QueueSession, Arc<Store>, Arc<TestEngine>, Arc<PowerState>) {
        let store = Arc::new(Store::default());
        let power = Arc::new(PowerState::default());
        let engine = Arc::new(TestEngine {
            calls: AtomicUsize::new(0),
            released: AtomicUsize::new(0),
            block: AtomicBool::new(false),
            panic: AtomicBool::new(false),
            power: power.clone(),
            fail_save: None,
        });
        (
            QueueSession::new(
                store.clone(),
                engine.clone(),
                Arc::new(Power(power.clone())),
            ),
            store,
            engine,
            power,
        )
    }
    struct GpsSource {
        blocked: AtomicBool,
        calls: AtomicUsize,
    }
    impl crate::media_geo::Repository for GpsSource {
        fn inspect(
            &self,
            _: &Path,
            _: &LibraryFile,
            stop: &AtomicBool,
        ) -> Result<crate::media_geo::Snapshot, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            while self.blocked.load(Ordering::SeqCst) && !stop.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            Ok(crate::media_geo::Snapshot {
                fingerprint: crate::media_geo::Fingerprint {
                    bytes: 1,
                    modified: "1".into(),
                },
                position: crate::media_geo::Position::Missing,
                resolved: None,
            })
        }
        fn save_place(
            &self,
            _: &Path,
            _: &LibraryFile,
            _: &crate::media_geo::Fingerprint,
            _: &crate::media_geo::ResolvedPlace,
            _: &AtomicBool,
        ) -> Result<(), String> {
            unreachable!()
        }
    }
    fn ready() -> crate::processing::Readiness {
        crate::processing::Readiness {
            library_open: true,
            inventory_complete: true,
            comments_ready: true,
            ..Default::default()
        }
    }
    fn pipeline_until(
        gps: &mut crate::geo_sweep::Session,
        queue: &mut QueueSession,
        condition: impl Fn(&crate::geo_sweep::Session, &QueueSession) -> bool,
    ) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        while !condition(gps, queue) {
            crate::processing::advance(gps, queue, ready());
            assert!(
                std::time::Instant::now() < deadline,
                "pipeline stalled: {} {:?}, gps {}",
                queue.error(),
                queue.counts(),
                gps.complete()
            );
            std::thread::yield_now();
        }
    }
    #[test]
    fn shutdown_and_drop_wait_for_active_inference_release_resources_and_do_not_restart() {
        struct Slow {
            entered: AtomicBool,
            finished: AtomicBool,
            releases: AtomicUsize,
        }
        impl Engine for Slow {
            fn transcribe(
                &self,
                _: &ModelChoice,
                _: &Path,
                stop: &AtomicBool,
                _: &(dyn Fn(WorkProgress) + Sync),
            ) -> Result<EngineOutput, String> {
                self.entered.store(true, Ordering::SeqCst);
                while !stop.load(Ordering::SeqCst) {
                    std::thread::yield_now();
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
                self.finished.store(true, Ordering::SeqCst);
                Err("cancelled".into())
            }
            fn release_model(&self) {
                if self.entered.load(Ordering::SeqCst) {
                    assert!(
                        self.finished.load(Ordering::SeqCst),
                        "native resources released before worker finished"
                    );
                }
                self.releases.fetch_add(1, Ordering::SeqCst);
            }
        }
        for explicit in [true, false] {
            let (_unused, store, _engine, power) = setup();
            let engine = Arc::new(Slow {
                entered: AtomicBool::new(false),
                finished: AtomicBool::new(false),
                releases: AtomicUsize::new(0),
            });
            let mut queue = QueueSession::new(
                store.clone(),
                engine.clone(),
                Arc::new(Power(power.clone())),
            );
            let root = Path::new("/shutdown");
            queue.sync(root, vec![candidate("a.wav")], true);
            queue.set_model(Some(choice()));
            until(&mut queue, |_| engine.entered.load(Ordering::SeqCst));
            if explicit {
                queue.shutdown().unwrap();
                let released = engine.releases.load(Ordering::SeqCst);
                queue.shutdown().unwrap();
                queue.poll();
                assert!(!queue.busy());
                assert_eq!(engine.releases.load(Ordering::SeqCst), released);
            } else {
                drop(queue);
            }
            assert!(engine.finished.load(Ordering::SeqCst));
            assert!(engine.releases.load(Ordering::SeqCst) > 0);
            assert_eq!(power.held.load(Ordering::SeqCst), 0);
            assert!(store.texts.lock().unwrap().is_empty());
            assert!(matches!(
                store.journals.lock().unwrap()[root].jobs[&candidate("a.wav").source].state,
                JobState::Pending
            ));
        }
    }
    #[test]
    fn shutdown_drains_a_full_completion_channel_before_joining() {
        let (mut queue, _, _, _) = setup();
        let (tx, receiver) = mpsc::sync_channel(1);
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        tx.send(Event::Progress(WorkProgress {
            phase: Phase::Planning,
            fraction: 0.,
            finding: None,
        }))
        .unwrap();
        let worker = std::thread::spawn(move || {
            tx.send(Event::Done(Ok((Journal::default(), None))))
                .unwrap();
            done.store(true, Ordering::SeqCst);
        });
        queue.active = Some(Active {
            worker: Some(worker),
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
            generation: 0,
            planning: true,
            findings: Default::default(),
        });
        queue.shutdown().unwrap();
        assert!(finished.load(Ordering::SeqCst));
        assert!(!queue.busy());
    }
    #[test]
    fn retry_requested_during_inference_is_preserved_when_the_active_job_returns() {
        let (mut queue, store, engine, _power) = setup();
        let root = Path::new("/library");
        let second = candidate("b.wav");
        let mut journal = Journal::default();
        journal.jobs.insert(
            second.source.clone(),
            Job {
                candidate: second.clone(),
                version: SourceVersion::new(1, "0".into()).unwrap(),
                state: JobState::Failed("previous failure".into()),
            },
        );
        store.journals.lock().unwrap().insert(root.into(), journal);
        engine.block.store(true, Ordering::SeqCst);
        queue.sync(root, vec![candidate("a.wav"), second], true);
        queue.set_model(Some(choice()));
        until(&mut queue, |q| {
            q.current().is_some() && engine.calls.load(Ordering::SeqCst) == 1
        });
        queue.retry_failed();
        engine.block.store(false, Ordering::SeqCst);
        until(&mut queue, |q| q.counts() == (2, 2, 0));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn automatic_start_ignores_a_previous_pause_but_respects_new_user_pause_and_restart() {
        let (queue, store, engine, _) = setup();
        let root = Path::new("/library");
        store.journals.lock().unwrap().insert(
            root.into(),
            Journal {
                mode: Mode::Paused,
                ..Default::default()
            },
        );
        let mut queue = queue.with_automatic_start();
        queue.hold_planning(true);
        queue.sync(root, vec![candidate("a.wav")], false);
        until(&mut queue, |q| q.mode_loaded() && !q.busy());
        assert_eq!(queue.mode(), Mode::Running);
        assert_eq!(store.journals.lock().unwrap()[root].mode, Mode::Running);
        queue.set_mode(Mode::Paused);
        until(&mut queue, |q| !q.busy());
        queue.sync(root, vec![candidate("a.wav")], true);
        assert_eq!(queue.mode(), Mode::Paused);
        queue.reset();
        queue.sync(root, vec![candidate("a.wav")], false);
        until(&mut queue, |q| !q.busy());
        assert_eq!(queue.mode(), Mode::Running);
        queue.reset();
        queue.set_mode(Mode::Paused);
        queue.sync(root, vec![candidate("a.wav")], true);
        assert_eq!(
            queue.mode(),
            Mode::Paused,
            "an immediate user pause must survive first inventory synchronization"
        );
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn paused_mode_is_saved_before_any_expensive_planning_and_survives_a_fresh_session() {
        let (mut queue, store, engine, power) = setup();
        let root = Path::new("/library");
        queue.sync(root, vec![candidate("a.wav")], false);
        queue.hold_planning(true);
        queue.set_mode(Mode::Paused);
        until(&mut queue, |q| q.mode_loaded() && !q.busy());
        assert_eq!(store.journals.lock().unwrap()[root].mode, Mode::Paused);
        assert!(store.journals.lock().unwrap()[root].jobs.is_empty());
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        let mut restored = QueueSession::new(store, engine, Arc::new(Power(power)));
        restored.sync(root, vec![candidate("a.wav")], false);
        restored.hold_planning(true);
        until(&mut restored, |q| q.mode_loaded() && !q.busy());
        assert_eq!(restored.mode(), Mode::Paused);
    }
    #[test]
    fn per_step_repeat_waits_for_cancelled_inference_and_preserves_earlier_audio_results() {
        use crate::{
            audio_library::{Progress, Repository, Session, Snapshot, Stage},
            transcription::{Locations, ModelSession, ModelStore, Prepared, Settings},
        };
        struct Audio {
            steps: Mutex<Vec<Stage>>,
            power: Arc<PowerState>,
        }
        impl Repository for Audio {
            fn read(&self, _: &Path) -> Result<Snapshot, String> {
                Ok(Snapshot::default())
            }
            fn run(
                &self,
                _: &Path,
                stage: Stage,
                _: &AtomicBool,
                _: &dyn Fn(Progress),
            ) -> Result<(), String> {
                assert_eq!(
                    self.power.held.load(Ordering::SeqCst),
                    0,
                    "repeat overlapped inference"
                );
                self.steps.lock().unwrap().push(stage);
                Ok(())
            }
        }
        struct Models;
        impl ModelStore for Models {
            fn locations(&self) -> Locations {
                Locations {
                    models: "models".into(),
                    preferences: "prefs".into(),
                }
            }
            fn inspect(
                &self,
                _: &AtomicBool,
                _: &(dyn Fn(crate::transcription::Progress) + Sync),
            ) -> Result<Prepared, String> {
                Ok(Default::default())
            }
            fn prepare(
                &self,
                _: &Settings,
                _: &AtomicBool,
                _: &(dyn Fn(crate::transcription::Progress) + Sync),
            ) -> Result<Prepared, String> {
                unreachable!()
            }
        }
        let (mut queue, _, engine, power) = setup();
        let root = Path::new("/library");
        let repo = Arc::new(Audio {
            steps: Mutex::new(Vec::new()),
            power,
        });
        let mut audio = Session::new(Some(repo.clone()));
        audio.open(root);
        let limit = std::time::Instant::now() + std::time::Duration::from_secs(4);
        while !audio.complete() {
            audio.poll(true);
            assert!(std::time::Instant::now() < limit);
            std::thread::yield_now();
        }
        repo.steps.lock().unwrap().clear();
        let source = Arc::new(GpsSource {
            blocked: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        });
        let mut gps = crate::geo_sweep::Session::new(source, Arc::default());
        gps.sync(root, vec![]);
        while !gps.complete() {
            gps.poll(true);
            assert!(std::time::Instant::now() < limit);
            std::thread::yield_now();
        }
        engine.block.store(true, Ordering::SeqCst);
        queue.sync(root, vec![candidate("a.wav")], true);
        queue.set_model(Some(choice()));
        until(&mut queue, |q| {
            q.current().is_some() && engine.calls.load(Ordering::SeqCst) == 1
        });
        let mut repeats = crate::processing::Repeats::default();
        repeats.request(crate::processing::Step::Years);
        repeats.request(crate::processing::Step::Years);
        let mut models = ModelSession::new(Arc::new(Models));
        while repeats.pending() || !audio.complete() {
            repeats.advance(&mut audio, &mut gps, &mut queue, &mut models, ready());
            assert!(
                std::time::Instant::now() < limit,
                "repeat stalled {}",
                queue.error()
            );
            std::thread::yield_now();
        }
        assert_eq!(*repo.steps.lock().unwrap(), vec![Stage::Years]);
        assert!(audio.stage_progress(Stage::Durations).0);
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        queue.set_mode(Mode::Paused);
        repeats.request(crate::processing::Step::Transcription);
        while repeats.pending() {
            repeats.advance(&mut audio, &mut gps, &mut queue, &mut models, ready());
            assert!(std::time::Instant::now() < limit);
        }
        assert_eq!(
            queue.mode(),
            Mode::Paused,
            "per-step repeat must not implicitly resume a paused queue"
        );
    }
    #[test]
    fn serial_coordinator_finishes_all_audio_stages_before_gps_and_delayed_inference() {
        use crate::audio_library::{Progress, Repository, Session, Snapshot, Stage};
        struct Audio {
            stages: Mutex<Vec<Stage>>,
            gps: Arc<GpsSource>,
            engine: Arc<TestEngine>,
        }
        impl Repository for Audio {
            fn read(&self, _: &Path) -> Result<Snapshot, String> {
                Ok(Snapshot::default())
            }
            fn run(
                &self,
                _: &Path,
                stage: Stage,
                _: &AtomicBool,
                _: &dyn Fn(Progress),
            ) -> Result<(), String> {
                assert_eq!(
                    self.gps.calls.load(Ordering::SeqCst),
                    0,
                    "GPS started before audio maintenance completed"
                );
                assert_eq!(
                    self.engine.calls.load(Ordering::SeqCst),
                    0,
                    "inference overlapped audio"
                );
                self.stages.lock().unwrap().push(stage);
                Ok(())
            }
        }
        let (mut queue, _, engine, _) = setup();
        let source = Arc::new(GpsSource {
            blocked: AtomicBool::new(true),
            calls: AtomicUsize::new(0),
        });
        let repository = Arc::new(Audio {
            stages: Mutex::new(Vec::new()),
            gps: source.clone(),
            engine: engine.clone(),
        });
        let root = Path::new("/library");
        let mut audio = Session::new(Some(repository.clone()));
        audio.open(root);
        let mut gps = crate::geo_sweep::Session::new(source.clone(), Arc::default());
        gps.sync(root, vec![LibraryFile::new("photo.jpg".into()).unwrap()]);
        queue.sync(root, vec![candidate("a.wav")], true);
        queue.set_model(Some(choice()));
        let limit = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while source.calls.load(Ordering::SeqCst) == 0 {
            crate::processing::advance_with_audio(&mut audio, &mut gps, &mut queue, ready(), false);
            assert!(
                std::time::Instant::now() < limit,
                "preflight stalled {} {}",
                audio.error(),
                queue.error()
            );
            std::thread::yield_now();
        }
        assert_eq!(
            *repository.stages.lock().unwrap(),
            [
                Stage::Inventory,
                Stage::Durations,
                Stage::Metadata,
                Stage::Years
            ]
        );
        assert!(!queue.busy());
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        source.blocked.store(false, Ordering::SeqCst);
        let released = std::time::Instant::now();
        while queue.counts() != (1, 1, 0) {
            crate::processing::advance_with_audio(&mut audio, &mut gps, &mut queue, ready(), false);
            assert!(
                std::time::Instant::now() < limit,
                "inference stalled {}",
                queue.error()
            );
            std::thread::yield_now();
        }
        assert!(released.elapsed() >= std::time::Duration::from_secs(2));
        assert!(gps.complete());
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn gps_completion_automatically_starts_transcription_without_another_ui_action() {
        let (mut queue, _store, engine, _power) = setup();
        let source = Arc::new(GpsSource {
            blocked: AtomicBool::new(true),
            calls: AtomicUsize::new(0),
        });
        let mut gps = crate::geo_sweep::Session::new(source.clone(), Arc::default());
        let root = Path::new("/library");
        gps.sync(root, vec![LibraryFile::new("photo.jpg".into()).unwrap()]);
        queue.sync(root, vec![candidate("a.wav")], true);
        queue.set_model(Some(choice()));
        pipeline_until(&mut gps, &mut queue, |_, _| {
            source.calls.load(Ordering::SeqCst) > 0
        });
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        assert!(queue.held());
        source.blocked.store(false, Ordering::SeqCst);
        pipeline_until(&mut gps, &mut queue, |_, q| q.counts() == (1, 1, 0));
        assert!(gps.complete());
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        assert_eq!(queue.pending(), 0);
    }
    #[test]
    fn retry_after_failed_initial_planning_rebuilds_the_queue_and_advances_after_gps() {
        let (mut queue, store, engine, _power) = setup();
        let source = Arc::new(GpsSource {
            blocked: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        });
        let mut gps = crate::geo_sweep::Session::new(source.clone(), Arc::default());
        let root = Path::new("/library");
        gps.sync(root, vec![LibraryFile::new("photo.jpg".into()).unwrap()]);
        store.fail_journal.store(true, Ordering::SeqCst);
        queue.sync(root, vec![candidate("a.wav")], true);
        queue.set_model(Some(choice()));
        pipeline_until(&mut gps, &mut queue, |_, q| !q.error().is_empty());
        assert!(queue.journal.is_none());
        queue.set_mode(Mode::Running);
        pipeline_until(&mut gps, &mut queue, |g, _| g.complete());
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        store.fail_journal.store(false, Ordering::SeqCst);
        queue.retry_failed();
        pipeline_until(&mut gps, &mut queue, |_, q| q.counts() == (1, 1, 0));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            source.calls.load(Ordering::SeqCst),
            1,
            "transcription recovery must not re-read all photos"
        );
    }
    #[test]
    fn resume_recovers_a_global_start_error_after_successful_gps() {
        let (mut queue, _store, engine, power) = setup();
        let source = Arc::new(GpsSource {
            blocked: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        });
        let mut gps = crate::geo_sweep::Session::new(source.clone(), Arc::default());
        let root = Path::new("/library");
        gps.sync(root, vec![LibraryFile::new("photo.jpg".into()).unwrap()]);
        queue.sync(root, vec![candidate("a.wav")], true);
        queue.set_model(Some(choice()));
        power.fail.store(true, Ordering::SeqCst);
        pipeline_until(&mut gps, &mut queue, |_, q| !q.error().is_empty());
        assert!(gps.complete());
        power.fail.store(false, Ordering::SeqCst);
        queue.resume();
        pipeline_until(&mut gps, &mut queue, |_, q| q.counts() == (1, 1, 0));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    }
    fn until(queue: &mut QueueSession, condition: impl Fn(&QueueSession) -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        loop {
            queue.poll();
            if condition(queue) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "queue timeout: {} {:?}",
                queue.error(),
                queue.counts()
            );
            std::thread::yield_now();
        }
    }
    fn released(power: &PowerState) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while power.held.load(Ordering::SeqCst) > 0 {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn live_findings_are_fifo_bounded_and_library_switch_discards_history() {
        let (mut queue, _, _, _) = setup();
        queue.set_model(Some(choice()));
        let sources = (0..24).map(|i| candidate(&format!("{i:02}.wav"))).collect();
        queue.sync(Path::new("/library"), sources, true);
        until(&mut queue, |q| !q.busy() && q.counts() == (24, 24, 0));
        let findings = queue
            .log()
            .iter()
            .filter(|e| e.text.is_some())
            .collect::<Vec<_>>();
        assert_eq!(findings.len(), 16);
        assert!(findings[0].source.as_str().ends_with("08.wav"));
        assert!(findings[15].source.as_str().ends_with("23.wav"));
        assert!(queue.log().len() <= 33);
        assert!(
            queue
                .log()
                .iter()
                .any(|e| e.text.is_none() && e.source.as_str().ends_with("23.wav"))
        );
        for i in 0..100 {
            push_log(
                &mut queue.log,
                LogEntry {
                    source: candidate(&format!("silent-{i}.wav")).source,
                    text: None,
                },
            );
        }
        assert_eq!(queue.log().iter().filter(|e| e.text.is_some()).count(), 16);
        assert!(queue.log().len() <= 33);
        queue.sync(Path::new("/other"), vec![], true);
        assert!(queue.log().is_empty());
        queue.reset();
        assert!(queue.log().is_empty());
    }

    #[test]
    fn fast_segment_bursts_retain_last_sixteen_without_a_ui_poll() {
        let (mut queue, store, _, _) = setup();
        queue.set_model(Some(choice()));
        queue.sync(Path::new("/library"), vec![candidate("burst.wav")], true);
        until(&mut queue, |q| q.current().is_some());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        while store.texts.lock().unwrap().is_empty() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        until(&mut queue, |q| !q.busy() && q.counts() == (1, 1, 0));
        let findings = queue
            .log()
            .iter()
            .filter_map(|e| e.text.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(findings.len(), 16);
        assert_eq!(findings[0], "Segment 84");
        assert_eq!(findings[15], "Segment 99");
    }

    #[test]
    fn whole_batch_holds_one_sleep_assertion_and_completed_sources_are_not_transcribed_twice() {
        let (mut queue, store, engine, power) = setup();
        queue.set_model(Some(choice()));
        let sources = vec![candidate("a.wav"), candidate("b.mp4")];
        queue.sync(Path::new("/library"), sources.clone(), true);
        until(&mut queue, |q| !q.busy() && q.counts() == (2, 2, 0));
        released(&power);
        assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
        assert_eq!(power.acquired.load(Ordering::SeqCst), 1);
        assert_eq!(engine.released.load(Ordering::SeqCst), 1);
        assert_eq!(queue.indicator(), Indicator::Idle);
        queue.rescan();
        until(&mut queue, |q| !q.busy() && q.counts() == (2, 2, 0));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
        let mut restarted =
            QueueSession::new(store, engine.clone(), Arc::new(Power(power.clone())));
        restarted.set_model(Some(choice()));
        restarted.sync(Path::new("/library"), sources, true);
        until(&mut restarted, |q| !q.busy() && q.counts() == (2, 2, 0));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn pause_releases_sleep_assertion_persists_and_resume_finishes_unfinished_file() {
        let (mut queue, store, engine, power) = setup();
        engine.block.store(true, Ordering::SeqCst);
        queue.set_model(Some(choice()));
        queue.sync(Path::new("/library"), vec![candidate("a.wav")], true);
        until(&mut queue, |_| engine.calls.load(Ordering::SeqCst) > 0);
        assert_eq!(queue.indicator(), Indicator::Busy);
        queue.set_mode(Mode::Paused);
        until(&mut queue, |q| !q.busy() && q.mode() == Mode::Paused);
        released(&power);
        assert_eq!(queue.indicator(), Indicator::Paused);
        assert!(store.texts.lock().unwrap().is_empty());
        assert_eq!(
            store.load(Path::new("/library")).unwrap().mode,
            Mode::Paused
        );
        engine.block.store(false, Ordering::SeqCst);
        queue.set_mode(Mode::Running);
        until(&mut queue, |q| !q.busy() && q.counts() == (1, 1, 0));
        released(&power);
        assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn missing_model_and_capture_hold_show_pause_without_acquiring_sleep_assertion() {
        let (mut queue, _, engine, power) = setup();
        queue.sync(Path::new("/library"), vec![candidate("a.mp4")], true);
        until(&mut queue, |q| !q.busy() && q.counts().1 == 1);
        assert!(queue.waiting_for_model());
        assert_eq!(queue.indicator(), Indicator::Paused);
        queue.set_hold(true);
        queue.set_model(Some(choice()));
        for _ in 0..3 {
            queue.poll();
        }
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        assert_eq!(power.acquired.load(Ordering::SeqCst), 0);
        queue.set_hold(false);
        until(&mut queue, |q| !q.busy() && q.counts() == (1, 1, 0));
        released(&power);
    }
    #[test]
    fn changed_sources_fail_safely_and_retry_is_explicit() {
        let (mut queue, store, engine, power) = setup();
        engine.block.store(true, Ordering::SeqCst);
        queue.set_model(Some(choice()));
        queue.sync(Path::new("/library"), vec![candidate("a.wav")], true);
        until(&mut queue, |_| engine.calls.load(Ordering::SeqCst) > 0);
        store.changed.store(true, Ordering::SeqCst);
        engine.block.store(false, Ordering::SeqCst);
        until(&mut queue, |q| !q.busy() && q.counts().2 == 1);
        released(&power);
        assert!(store.texts.lock().unwrap().is_empty());
        queue.retry_failed();
        until(&mut queue, |q| !q.busy() && q.counts() == (1, 1, 0));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn one_bad_file_does_not_block_others_and_dropping_or_switching_context_cancels_old_work() {
        let (mut queue, store, engine, power) = setup();
        queue.set_model(Some(choice()));
        queue.sync(
            Path::new("/library"),
            vec![candidate("bad.wav"), candidate("good.mp4")],
            true,
        );
        until(&mut queue, |q| !q.busy() && q.counts() == (1, 2, 1));
        released(&power);
        assert_eq!(store.texts.lock().unwrap().len(), 1);
        engine.block.store(true, Ordering::SeqCst);
        queue.sync(Path::new("/old"), vec![candidate("old.wav")], true);
        until(&mut queue, |_| engine.calls.load(Ordering::SeqCst) > 2);
        queue.sync(Path::new("/new"), vec![candidate("new.wav")], true);
        engine.block.store(false, Ordering::SeqCst);
        until(&mut queue, |q| !q.busy() && q.counts() == (1, 1, 0));
        released(&power);
        assert_eq!(queue.last().unwrap().source, candidate("new.wav").source);
        assert!(
            !store
                .texts
                .lock()
                .unwrap()
                .keys()
                .any(|key| key.contains("/old"))
        );
        engine.block.store(true, Ordering::SeqCst);
        queue.sync(Path::new("/drop"), vec![candidate("drop.wav")], true);
        until(&mut queue, |q| q.current().is_some());
        drop(queue);
        released(&power);
    }
    #[test]
    fn power_or_worker_failure_never_leaves_a_busy_spinner_or_sleep_lock() {
        let (mut queue, _, engine, power) = setup();
        power.fail.store(true, Ordering::SeqCst);
        queue.set_model(Some(choice()));
        queue.sync(Path::new("/library"), vec![candidate("a.wav")], true);
        until(&mut queue, |q| !q.error().is_empty());
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        assert_eq!(queue.indicator(), Indicator::Paused);
        power.fail.store(false, Ordering::SeqCst);
        engine.panic.store(true, Ordering::SeqCst);
        queue.rescan();
        until(&mut queue, |q| q.error().contains("unerwartet"));
        assert!(!queue.busy());
        released(&power);
    }
    #[test]
    fn transcript_commit_survives_failed_journal_and_is_recovered_without_inference() {
        let store = Arc::new(Store::default());
        let power = Arc::new(PowerState::default());
        let engine = Arc::new(TestEngine {
            calls: AtomicUsize::new(0),
            released: AtomicUsize::new(0),
            block: AtomicBool::new(false),
            panic: AtomicBool::new(false),
            power: power.clone(),
            fail_save: Some(store.clone()),
        });
        let mut queue = QueueSession::new(
            store.clone(),
            engine.clone(),
            Arc::new(Power(power.clone())),
        );
        queue.set_model(Some(choice()));
        queue.sync(Path::new("/library"), vec![candidate("a.wav")], true);
        until(&mut queue, |q| !q.error().is_empty());
        released(&power);
        assert_eq!(store.texts.lock().unwrap().len(), 1);
        store.fail_journal.store(false, Ordering::SeqCst);
        queue.rescan();
        until(&mut queue, |q| !q.busy() && q.counts() == (1, 1, 0));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    }
}
