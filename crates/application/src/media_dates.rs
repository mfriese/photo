//! Lazy read-only gallery dates. Filename fast path; one bounded metadata worker.
use crate::{
    domain::{CaptureTime, Media},
    work_priority::WorkPriority,
};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
pub trait MediaDateSource: Send + Sync + 'static {
    fn captured_at(&self, media: &Media) -> Result<CaptureTime, String>;
}
const LIMIT: usize = 8192;
struct Request {
    media: Media,
    generation: u64,
    cancel: Arc<AtomicBool>,
}
struct Reply {
    path: PathBuf,
    generation: u64,
    date: Result<CaptureTime, String>,
}
struct Worker {
    sender: mpsc::SyncSender<Request>,
    receiver: mpsc::Receiver<Reply>,
}
pub struct MediaDates {
    source: Arc<dyn MediaDateSource>,
    priority: Arc<WorkPriority>,
    worker: Option<Worker>,
    generation: u64,
    cancel: Arc<AtomicBool>,
    pending: usize,
    retry: bool,
    entries: HashMap<PathBuf, Option<Result<CaptureTime, String>>>,
    order: VecDeque<PathBuf>,
}
impl MediaDates {
    pub fn new(source: Arc<dyn MediaDateSource>, priority: Arc<WorkPriority>) -> Self {
        Self {
            source,
            priority,
            worker: None,
            generation: 0,
            pending: 0,
            retry: false,
            cancel: Arc::new(AtomicBool::new(false)),
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }
    pub fn clear(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation = self.generation.wrapping_add(1);
        self.entries.clear();
        self.pending = 0;
        self.retry = false;
        self.order.clear();
    }
    fn insert(&mut self, path: PathBuf, value: Option<Result<CaptureTime, String>>) {
        if self.entries.len() >= LIMIT
            && let Some(old) = self.order.pop_front()
            && self
                .entries
                .remove(&old)
                .is_some_and(|entry| entry.is_none())
        {
            self.pending -= 1;
        }
        if value.is_none() {
            self.pending += 1;
        }
        self.order.push_back(path.clone());
        self.entries.insert(path, value);
    }
    pub fn get(&mut self, media: &Media) -> Option<&Result<CaptureTime, String>> {
        if !self.entries.contains_key(media.path()) {
            if let Some(date) = crate::domain::capture_from_filename(media) {
                self.insert(media.path().into(), Some(Ok(date)));
            } else {
                if self.worker.is_none() {
                    self.start_worker();
                }
                let request = Request {
                    media: media.clone(),
                    generation: self.generation,
                    cancel: self.cancel.clone(),
                };
                match self.worker.as_ref().unwrap().sender.try_send(request) {
                    Ok(()) => self.insert(media.path().into(), None),
                    Err(mpsc::TrySendError::Full(_)) => {
                        self.retry = true;
                        return None;
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => self.insert(
                        media.path().into(),
                        Some(Err("Datumsauftrag beendet".into())),
                    ),
                }
            }
        }
        self.entries
            .get(media.path())
            .and_then(|value| value.as_ref())
    }
    fn start_worker(&mut self) {
        let (sender, requests) = mpsc::sync_channel::<Request>(32);
        let (results, receiver) = mpsc::sync_channel(32);
        let source = self.source.clone();
        let priority = self.priority.clone();
        std::thread::spawn(move || {
            for request in requests {
                if !priority.wait_background(&request.cancel) {
                    continue;
                }
                let date = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    source.captured_at(&request.media)
                }))
                .unwrap_or_else(|_| Err("Datumsprüfung unerwartet fehlgeschlagen".into()));
                if request.cancel.load(Ordering::Relaxed) {
                    continue;
                }
                if results
                    .send(Reply {
                        path: request.media.path().into(),
                        generation: request.generation,
                        date,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        self.worker = Some(Worker { sender, receiver });
    }
    pub fn poll(&mut self) {
        self.retry = false;
        if let Some(worker) = &self.worker {
            for reply in worker.receiver.try_iter().take(32) {
                if reply.generation == self.generation
                    && let Some(entry) = self.entries.get_mut(&reply.path)
                {
                    if entry.is_none() {
                        self.pending -= 1;
                    }
                    *entry = Some(reply.date);
                }
            }
        }
    }
    pub fn is_loading(&self) -> bool {
        self.pending > 0 || self.retry
    }
}
impl Drop for MediaDates {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Source(AtomicUsize);
    impl MediaDateSource for Source {
        fn captured_at(&self, _: &Media) -> Result<CaptureTime, String> {
            self.0.fetch_add(1, Ordering::Relaxed);
            CaptureTime::parse("2008:09:16 03:58:11", "test EXIF")
        }
    }
    fn media(name: &str) -> Media {
        Media::from_path(name.into(), "2008/09".into()).unwrap()
    }
    #[test]
    fn names_need_no_metadata_and_unknown_files_are_loaded_once() {
        let source = Arc::new(Source(AtomicUsize::new(0)));
        let mut dates = MediaDates::new(source.clone(), Arc::default());
        let named = media("img_20080916_035811.jpg");
        assert_eq!(
            dates.get(&named).unwrap().as_ref().unwrap().date_label(),
            "16.09.2008"
        );
        assert_eq!(source.0.load(Ordering::Relaxed), 0);
        let old = media("holiday.jpg");
        assert!(dates.get(&old).is_none());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while dates.is_loading() {
            dates.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        for _ in 0..10 {
            assert_eq!(
                dates.get(&old).unwrap().as_ref().unwrap().time_label(),
                "03:58:11"
            );
        }
        assert_eq!(source.0.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn cancellation_respects_audio_priority_and_backpressure_is_nonblocking() {
        let source = Arc::new(Source(AtomicUsize::new(0)));
        let priority = Arc::new(WorkPriority::default());
        let audio = priority.audio();
        let mut dates = MediaDates::new(source.clone(), priority);
        for n in 0..100 {
            assert!(dates.get(&media(&format!("old{n}.jpg"))).is_none());
        }
        assert!(dates.pending <= 33);
        assert_eq!(source.0.load(Ordering::Relaxed), 0);
        dates.clear();
        drop(audio);
        for _ in 0..100 {
            dates.poll();
            std::thread::yield_now();
        }
        assert!(!dates.is_loading());
        assert!(dates.entries.is_empty());
    }
    #[test]
    fn named_cache_is_bounded_and_refresh_releases_it() {
        let source = Arc::new(Source(AtomicUsize::new(0)));
        let mut dates = MediaDates::new(source, Arc::default());
        for index in 0..LIMIT + 50 {
            dates.get(&media(&format!("img_20080916_035811_{index}.jpg")));
        }
        assert_eq!(dates.entries.len(), LIMIT);
        assert!(!dates.is_loading());
        dates.clear();
        assert!(dates.entries.is_empty());
    }
    #[test]
    fn late_metadata_from_a_previous_library_cannot_replace_the_new_date() {
        struct Blocking {
            started: mpsc::SyncSender<()>,
            release: std::sync::Mutex<mpsc::Receiver<()>>,
            calls: AtomicUsize,
        }
        impl MediaDateSource for Blocking {
            fn captured_at(&self, _: &Media) -> Result<CaptureTime, String> {
                let first = self.calls.fetch_add(1, Ordering::Relaxed) == 0;
                if first {
                    self.started.send(()).unwrap();
                    self.release.lock().unwrap().recv().unwrap();
                }
                CaptureTime::parse(
                    if first {
                        "2008:09:16 03:58:11"
                    } else {
                        "2012:01:02 12:34:56"
                    },
                    "test",
                )
            }
        }
        let (started, waiting) = mpsc::sync_channel(1);
        let (release, gate) = mpsc::channel();
        let source = Arc::new(Blocking {
            started,
            release: std::sync::Mutex::new(gate),
            calls: AtomicUsize::new(0),
        });
        let mut dates = MediaDates::new(source, Arc::default());
        let photo = media("same-name.jpg");
        assert!(dates.get(&photo).is_none());
        waiting
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        dates.clear();
        assert!(dates.get(&photo).is_none());
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while dates.is_loading() {
            dates.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(
            dates.get(&photo).unwrap().as_ref().unwrap().stamp(),
            "20120102_123456"
        );
    }
}
