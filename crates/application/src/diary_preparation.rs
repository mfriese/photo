//! Read-only duration preparation for the visible page; never starts the audio transport.
use crate::{
    diary::{AudioProbe, AudioSource, VerificationProgress},
    domain::{CaptureTime, diary_timeline::DiaryTimeline},
    work_priority::WorkPriority,
};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
const CACHE_LIMIT: usize = 8192;
#[derive(Default)]
struct Cache {
    generation: u64,
    revision: u64,
    values: HashMap<PathBuf, AudioProbe>,
    order: VecDeque<PathBuf>,
}
struct PreparedSource {
    inner: Arc<dyn AudioSource>,
    cache: Mutex<Cache>,
}
impl PreparedSource {
    fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.generation += 1;
        cache.revision += 1;
        cache.values.clear();
        cache.order.clear();
    }
}
impl AudioSource for PreparedSource {
    fn cached_probe(&self, path: &Path) -> Option<AudioProbe> {
        self.cache.lock().unwrap().values.get(path).cloned()
    }
    fn probe(&self, path: &Path) -> Result<AudioProbe, String> {
        let generation = {
            let cache = self.cache.lock().unwrap();
            if let Some(probe) = cache.values.get(path) {
                return Ok(probe.clone());
            }
            cache.generation
        };
        let probe = self.inner.probe(path)?;
        if !probe.audio_only || !probe.duration.is_finite() || probe.duration <= 0. {
            return Err("Keine gültige reine Audioaufnahme mit positiver Dauer".into());
        }
        let mut cache = self.cache.lock().unwrap();
        if generation == cache.generation {
            if !cache.values.contains_key(path) {
                if cache.values.len() >= CACHE_LIMIT
                    && let Some(old) = cache.order.pop_front()
                {
                    cache.values.remove(&old);
                }
                cache.order.push_back(path.into());
            }
            cache.values.insert(path.into(), probe.clone());
            cache.revision += 1;
        }
        Ok(probe)
    }
    fn inspect(&self, path: &Path) -> Result<crate::domain::diary::Recording, String> {
        let probe = self.probe(path)?;
        crate::domain::diary::Recording::new(
            path.into(),
            self.inner.capture_time_from_probe(path, &probe)?,
            probe.duration,
        )
    }
    fn capture_time(&self, path: &Path) -> Result<CaptureTime, String> {
        let probe = self.probe(path)?;
        self.inner.capture_time_from_probe(path, &probe)
    }
}
enum Event {
    Checked(PathBuf, Result<(), String>),
    Done,
}
struct Active {
    cancel: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Event>,
}
pub struct AudioPreparation {
    source: Arc<PreparedSource>,
    priority: Arc<WorkPriority>,
    active: Option<Active>,
    queued: Option<Vec<PathBuf>>,
    key: Option<String>,
    members: Vec<PathBuf>,
    errors: BTreeMap<PathBuf, String>,
    status_revision: u64,
    error: String,
}
impl AudioPreparation {
    pub fn new(source: Arc<dyn AudioSource>, priority: Arc<WorkPriority>) -> Self {
        Self {
            source: Arc::new(PreparedSource {
                inner: source,
                cache: Default::default(),
            }),
            priority,
            active: None,
            queued: None,
            key: None,
            members: Vec::new(),
            errors: Default::default(),
            status_revision: 0,
            error: String::new(),
        }
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn source(&self) -> Arc<dyn AudioSource> {
        self.source.clone()
    }
    pub fn version(&self) -> (u64, u64) {
        (
            self.source.cache.lock().unwrap().revision,
            self.status_revision,
        )
    }
    pub fn cancel(&mut self) {
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Relaxed);
        }
        self.queued = None;
        self.key = None;
        self.members.clear();
    }
    pub fn reset(&mut self) {
        self.cancel();
        self.source.clear();
        self.errors.clear();
        self.error.clear();
        self.status_revision += 1;
    }
    pub fn is_loading(&self) -> bool {
        self.active.is_some() || self.queued.is_some()
    }
    pub fn ensure(&mut self, key: &str, timeline: Option<&DiaryTimeline>, changed: bool) {
        if self.key.as_deref() == Some(key) && !changed {
            return;
        }
        let paths: Vec<_> = timeline
            .map(|t| t.clips().iter().map(|c| c.path().to_owned()).collect())
            .unwrap_or_default();
        if self.key.as_deref() == Some(key) && self.members == paths {
            return;
        }
        self.cancel();
        self.key = Some(key.into());
        self.members = paths.clone();
        self.errors.clear();
        self.error.clear();
        self.status_revision += 1;
        self.queued = Some(
            paths
                .into_iter()
                .filter(|p| self.source.cached_probe(p).is_none())
                .collect(),
        );
        self.launch();
    }
    fn launch(&mut self) {
        if self.active.is_some() {
            return;
        }
        let Some(paths) = self.queued.take() else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        let (source, priority) = (self.source.clone(), self.priority.clone());
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let (tx, receiver) = mpsc::sync_channel(16);
        self.active = Some(Active { cancel, receiver });
        std::thread::spawn(move || {
            for path in paths {
                if !priority.wait_background(&stop) {
                    break;
                }
                let result = source.probe(&path).map(|_| ());
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if tx.send(Event::Checked(path, result)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Event::Done);
        });
    }
    pub fn poll(&mut self) {
        let mut done = false;
        if let Some(active) = &self.active {
            for _ in 0..16 {
                match active.receiver.try_recv() {
                    Ok(Event::Checked(path, result)) => {
                        if !active.cancel.load(Ordering::Relaxed) {
                            self.status_revision += 1;
                            if let Err(error) = result {
                                if self.errors.len() >= CACHE_LIMIT {
                                    self.errors.pop_first();
                                }
                                self.errors.insert(path, error);
                            }
                        }
                    }
                    Ok(Event::Done) => {
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if !active.cancel.load(Ordering::Relaxed) {
                            self.error="Audio-Dauervorbereitung unerwartet beendet; Ansicht erneut öffnen oder aktualisieren".into();
                        }
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if done {
            self.active = None;
        }
        self.launch();
    }
    pub fn apply(&self, timeline: &mut DiaryTimeline) -> Result<(), String> {
        let updates = {
            let cache = self.source.cache.lock().unwrap();
            timeline
                .clips()
                .iter()
                .filter_map(|clip| {
                    cache
                        .values
                        .get(clip.path())
                        .filter(|probe| clip.duration() != Some(probe.duration))
                        .map(|probe| (clip.path().to_owned(), probe.duration))
                })
                .collect::<Vec<_>>()
        };
        if updates.is_empty() {
            return Ok(());
        }
        timeline.refine_durations(&updates)
    }
    pub fn progress(&self, timeline: Option<&DiaryTimeline>) -> VerificationProgress {
        let mut progress = VerificationProgress::default();
        if let Some(timeline) = timeline {
            progress.total = timeline.clips().len();
            progress.checked = timeline.clips().iter().filter(|c| c.is_verified()).count();
            for clip in timeline.clips() {
                if let Some(error) = self.errors.get(clip.path()) {
                    progress.errors.insert(clip.path().into(), error.clone());
                }
            }
        }
        progress
    }
}
impl Drop for AudioPreparation {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Source {
        probes: AtomicUsize,
        dates: AtomicUsize,
    }
    impl AudioSource for Source {
        fn probe(&self, path: &Path) -> Result<AudioProbe, String> {
            self.probes.fetch_add(1, Ordering::SeqCst);
            Ok(AudioProbe {
                duration: if path.to_string_lossy().contains("130000") {
                    3.
                } else {
                    1.
                },
                audio_only: true,
                file_created: None,
            })
        }
        fn capture_time(&self, path: &Path) -> Result<CaptureTime, String> {
            self.dates.fetch_add(1, Ordering::SeqCst);
            crate::domain::audio_date::date_from_filename(
                path.file_stem().unwrap().to_str().unwrap(),
            )
            .ok_or("date missing".into())
        }
    }
    fn timeline(paths: &[&str]) -> DiaryTimeline {
        DiaryTimeline::new(
            "2026".into(),
            paths
                .iter()
                .map(|p| crate::domain::diary_timeline::DiaryClip::from_filename((*p).into()))
                .collect(),
        )
        .unwrap()
    }
    fn drain(prep: &mut AudioPreparation) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while prep.is_loading() {
            prep.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn entering_view_prepares_proportions_without_dates_and_playback_reuses_probes() {
        let source = Arc::new(Source {
            probes: AtomicUsize::new(0),
            dates: AtomicUsize::new(0),
        });
        let mut prep = AudioPreparation::new(source.clone(), Default::default());
        let mut timeline = timeline(&["aud_20261113_120000.wav", "aud_20261113_130000.wav"]);
        prep.ensure("2026", Some(&timeline), true);
        drain(&mut prep);
        prep.apply(&mut timeline).unwrap();
        assert_eq!(
            timeline.clips()[1].weight() / timeline.clips()[0].weight(),
            3.
        );
        assert_eq!(source.probes.load(Ordering::SeqCst), 2);
        assert_eq!(source.dates.load(Ordering::SeqCst), 0);
        prep.source().inspect(timeline.clips()[0].path()).unwrap();
        assert_eq!(source.probes.load(Ordering::SeqCst), 2);
        assert_eq!(source.dates.load(Ordering::SeqCst), 1);
        prep.ensure("2026", Some(&timeline), false);
        assert!(!prep.is_loading());
        prep.ensure("photo/2008/09/a.jpg", Some(&timeline), true);
        drain(&mut prep);
        assert_eq!(source.probes.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn explicit_play_priority_prevents_background_probe_until_released_or_cancelled() {
        let priority = Arc::new(WorkPriority::default());
        let guard = priority.audio();
        let source = Arc::new(Source {
            probes: AtomicUsize::new(0),
            dates: AtomicUsize::new(0),
        });
        let mut prep = AudioPreparation::new(source.clone(), priority);
        prep.ensure("2026", Some(&timeline(&["aud_20261113_120000.wav"])), true);
        prep.cancel();
        drain(&mut prep);
        assert_eq!(source.probes.load(Ordering::SeqCst), 0);
        drop(guard);
    }
    struct Slow {
        calls: AtomicUsize,
        entered: mpsc::SyncSender<PathBuf>,
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl AudioSource for Slow {
        fn probe(&self, path: &Path) -> Result<AudioProbe, String> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            self.entered.send(path.into()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
            Ok(AudioProbe {
                duration: n as f64,
                audio_only: true,
                file_created: None,
            })
        }
        fn capture_time(&self, _: &Path) -> Result<CaptureTime, String> {
            panic!("duration preparation must not load dates")
        }
    }
    #[test]
    fn rapid_navigation_keeps_one_worker_and_latest_request_and_reset_rejects_stale_cache() {
        let (entered, events) = mpsc::sync_channel(4);
        let (release, gate) = mpsc::channel();
        let source = Arc::new(Slow {
            calls: AtomicUsize::new(0),
            entered,
            release: Mutex::new(gate),
        });
        let mut prep = AudioPreparation::new(source.clone(), Default::default());
        let mut a = timeline(&["aud_20261113_120000.wav"]);
        let b = timeline(&["aud_20261113_130000.wav"]);
        prep.ensure("a", Some(&a), true);
        assert_eq!(
            events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            a.clips()[0].path()
        );
        prep.ensure("b", Some(&b), true);
        prep.reset();
        prep.ensure("a", Some(&a), true);
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            prep.poll();
            if let Ok(path) = events.try_recv() {
                assert_eq!(path, a.clips()[0].path());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(source.calls.load(Ordering::SeqCst), 2);
        assert!(prep.source().cached_probe(a.clips()[0].path()).is_none());
        release.send(()).unwrap();
        drain(&mut prep);
        prep.apply(&mut a).unwrap();
        assert_eq!(a.clips()[0].duration(), Some(2.));
    }
    #[test]
    fn cache_has_a_fixed_limit_and_broken_worker_finishes_with_an_error() {
        let source = Arc::new(Source {
            probes: AtomicUsize::new(0),
            dates: AtomicUsize::new(0),
        });
        let mut prep = AudioPreparation::new(source, Default::default());
        for n in 0..=CACHE_LIMIT {
            prep.source
                .probe(Path::new(&format!("clip-{n}.wav")))
                .unwrap();
        }
        assert_eq!(prep.source.cache.lock().unwrap().values.len(), CACHE_LIMIT);
        assert!(prep.source.cached_probe(Path::new("clip-0.wav")).is_none());
        let (tx, receiver) = mpsc::sync_channel(1);
        drop(tx);
        prep.active = Some(Active {
            cancel: Default::default(),
            receiver,
        });
        prep.poll();
        assert!(!prep.is_loading());
        assert!(!prep.error().is_empty());
    }
}
