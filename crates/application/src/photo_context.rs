//! Lazy per-player photo index and cancellable, bounded preview loading.
use crate::domain::{
    CaptureTime,
    photo_context::{PhotoTimeline, TimedPhoto},
};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
};
pub trait ContextPhotoSource: Send + Sync + 'static {
    fn captured_at(&self, path: &Path) -> Result<CaptureTime, String>;
    /// Bounded, oriented PNG, generated off the UI thread.
    fn thumbnail(&self, path: &Path, edge: u32) -> Result<Vec<u8>, String>;
}
pub enum PhotoEvent {
    Failed {
        request: u64,
        message: String,
    },
    Selection {
        request: u64,
        photos: Box<[Option<TimedPhoto>; 5]>,
        unreadable: usize,
    },
    Image {
        request: u64,
        slot: usize,
        result: Result<Arc<Vec<u8>>, String>,
    },
}
struct Request {
    id: u64,
    civil: i64,
}
pub struct PhotoContext {
    sender: Option<mpsc::Sender<Request>>,
    receiver: mpsc::Receiver<PhotoEvent>,
    stop: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    ended: bool,
    displayed: Option<(u64, Box<[Option<TimedPhoto>; 5]>)>,
}
impl PhotoContext {
    pub fn new(source: Arc<dyn ContextPhotoSource>, candidates: Vec<PathBuf>) -> Self {
        let (sender, requests) = mpsc::channel::<Request>();
        let (events, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let generation = Arc::new(AtomicU64::new(0));
        let halted = stop.clone();
        let latest = generation.clone();
        std::thread::spawn(move || {
            // Waiting for the native player request is intentional: no photo I/O before playback.
            let Ok(mut request) = requests.recv() else {
                return;
            };
            let mut dated = Vec::new();
            let mut unreadable = 0;
            for path in candidates {
                if halted.load(Ordering::Relaxed) {
                    return;
                }
                match source.captured_at(&path) {
                    Ok(date) => dated.push(TimedPhoto::new(path, date)),
                    Err(_) => unreadable += 1,
                }
            }
            let timeline = PhotoTimeline::new(dated);
            let mut cache = HashMap::<PathBuf, Arc<Vec<u8>>>::new();
            let mut order = VecDeque::new();
            loop {
                while let Ok(newer) = requests.try_recv() {
                    request = newer;
                }
                if halted.load(Ordering::Relaxed) {
                    return;
                }
                let photos = timeline.around(request.civil);
                let id = request.id;
                if events
                    .send(PhotoEvent::Selection {
                        request: id,
                        photos: Box::new(photos.clone()),
                        unreadable,
                    })
                    .is_err()
                {
                    return;
                }
                let mut jobs = Vec::new();
                for (slot, photo) in photos.iter().enumerate() {
                    if let Some(photo) = photo {
                        if let Some(image) = cache.get(photo.path()) {
                            let _ = events.send(PhotoEvent::Image {
                                request: id,
                                slot,
                                result: Ok(image.clone()),
                            });
                        } else {
                            jobs.push((slot, photo.path().to_owned()));
                        }
                    }
                }
                let next = AtomicUsize::new(0);
                let (finished_tx, finished_rx) = mpsc::channel();
                std::thread::scope(|scope| {
                    for _ in 0..jobs.len().min(2) {
                        let next = &next;
                        let jobs = &jobs;
                        let source = &source;
                        let halted = &halted;
                        let latest = &latest;
                        let events = &events;
                        let finished = finished_tx.clone();
                        scope.spawn(move || {
                            loop {
                                if halted.load(Ordering::Relaxed)
                                    || latest.load(Ordering::Relaxed) != id
                                {
                                    break;
                                }
                                let Some((slot, path)) =
                                    jobs.get(next.fetch_add(1, Ordering::Relaxed))
                                else {
                                    break;
                                };
                                let result = source.thumbnail(path, 384).map(Arc::new);
                                if let Ok(image) = &result {
                                    let _ = finished.send((path.clone(), image.clone()));
                                }
                                if !halted.load(Ordering::Relaxed)
                                    && latest.load(Ordering::Relaxed) == id
                                {
                                    let _ = events.send(PhotoEvent::Image {
                                        request: id,
                                        slot: *slot,
                                        result,
                                    });
                                }
                            }
                        });
                    }
                });
                drop(finished_tx);
                for (path, image) in finished_rx {
                    if !cache.contains_key(&path) {
                        order.push_back(path.clone());
                    }
                    cache.insert(path, image);
                    while order.len() > 10 {
                        if let Some(old) = order.pop_front() {
                            cache.remove(&old);
                        }
                    }
                }
                let Ok(newer) = requests.recv() else {
                    return;
                };
                request = newer;
            }
        });
        Self {
            sender: Some(sender),
            receiver,
            stop,
            generation,
            ended: false,
            displayed: None,
        }
    }
    pub fn request(&mut self, id: u64, civil: f64) {
        if !civil.is_finite() || civil < i64::MIN as f64 || civil > i64::MAX as f64 {
            return;
        }
        self.ended = false;
        self.generation.store(id, Ordering::Relaxed);
        if let Some(sender) = &self.sender {
            let _ = sender.send(Request {
                id,
                civil: civil.floor() as i64,
            });
        }
    }
    /// Resolve a clicked slot only against the current published selection.
    pub fn photo_at(&self, request: u64, slot: usize) -> Option<PathBuf> {
        let (displayed_request, photos) = self.displayed.as_ref()?;
        if *displayed_request != request || self.generation.load(Ordering::Relaxed) != request {
            return None;
        }
        photos
            .get(slot)?
            .as_ref()
            .map(|photo| photo.path().to_owned())
    }
    pub fn poll(&mut self) -> Vec<PhotoEvent> {
        let mut events = Vec::new();
        for _ in 0..8 {
            match self.receiver.try_recv() {
                Ok(event) => {
                    let request = match &event {
                        PhotoEvent::Selection { request, .. }
                        | PhotoEvent::Image { request, .. }
                        | PhotoEvent::Failed { request, .. } => *request,
                    };
                    if request == self.generation.load(Ordering::Relaxed) {
                        if let PhotoEvent::Selection {
                            request, photos, ..
                        } = &event
                        {
                            self.displayed = Some((*request, photos.clone()));
                        }
                        events.push(event);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if !self.ended {
                        self.ended = true;
                        events.push(PhotoEvent::Failed {
                            request: self.generation.load(Ordering::Relaxed),
                            message: "Fotosuche unerwartet beendet".into(),
                        });
                    }
                    break;
                }
            }
        }
        events
    }
}
impl Drop for PhotoContext {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.sender.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[derive(Default)]
    struct Source {
        dates: AtomicUsize,
        images: AtomicUsize,
        active: AtomicUsize,
        maximum: AtomicUsize,
        blocked: AtomicBool,
        release: AtomicBool,
    }
    impl ContextPhotoSource for Source {
        fn captured_at(&self, path: &Path) -> Result<CaptureTime, String> {
            self.dates.fetch_add(1, Ordering::SeqCst);
            while self.blocked.load(Ordering::SeqCst) && !self.release.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            let second = path.file_stem().unwrap().to_str().unwrap();
            CaptureTime::parse(&format!("2023:12:27 22:51:{second:0>2}"), "test")
        }
        fn thumbnail(&self, _: &Path, _: u32) -> Result<Vec<u8>, String> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum.fetch_max(active, Ordering::SeqCst);
            self.images.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(5));
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(vec![1, 2, 3])
        }
    }
    fn paths() -> Vec<PathBuf> {
        (0..5).map(|i| format!("{i}.jpg").into()).collect()
    }
    fn time(second: u32) -> f64 {
        CaptureTime::parse(&format!("2023:12:27 22:51:{second:02}"), "test")
            .unwrap()
            .civil_seconds() as f64
    }
    fn wait(context: &mut PhotoContext, count: usize) -> Vec<PhotoEvent> {
        let until = Instant::now() + Duration::from_secs(3);
        let mut events = Vec::new();
        while events
            .iter()
            .filter(|event| matches!(event, PhotoEvent::Image { .. }))
            .count()
            < count
        {
            events.extend(context.poll());
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
        events
    }
    #[test]
    fn no_photo_io_before_request_and_index_and_thumbnails_are_reused_with_two_decoders() {
        let source = Arc::new(Source::default());
        let mut context = PhotoContext::new(source.clone(), paths());
        std::thread::sleep(Duration::from_millis(15));
        assert_eq!(source.dates.load(Ordering::SeqCst), 0);
        context.request(1, time(2));
        let events = wait(&mut context, 5);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, PhotoEvent::Selection { request: 1, .. }))
        );
        assert_eq!(source.dates.load(Ordering::SeqCst), 5);
        assert_eq!(source.images.load(Ordering::SeqCst), 5);
        assert!(source.maximum.load(Ordering::SeqCst) <= 2);
        context.request(2, time(3));
        wait(&mut context, 4);
        assert_eq!(source.dates.load(Ordering::SeqCst), 5);
        assert_eq!(source.images.load(Ordering::SeqCst), 5);
    }
    #[test]
    fn slow_search_is_nonblocking_and_obsolete_targets_never_reach_the_view() {
        let source = Arc::new(Source::default());
        source.blocked.store(true, Ordering::SeqCst);
        let mut context = PhotoContext::new(source.clone(), paths());
        context.request(1, time(0));
        let until = Instant::now() + Duration::from_secs(2);
        while source.dates.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
        assert!(context.poll().is_empty());
        context.request(2, time(4));
        source.release.store(true, Ordering::SeqCst);
        let events = wait(&mut context, 3);
        for event in events {
            match event {
                PhotoEvent::Selection { request, .. }
                | PhotoEvent::Image { request, .. }
                | PhotoEvent::Failed { request, .. } => assert_eq!(request, 2),
            }
        }
    }
    struct BrokenSource;
    impl ContextPhotoSource for BrokenSource {
        fn captured_at(&self, path: &Path) -> Result<CaptureTime, String> {
            if path == Path::new("2.jpg") {
                CaptureTime::parse("2023:12:27 22:51:02", "test")
            } else {
                Err("Kein Aufnahmedatum".into())
            }
        }
        fn thumbnail(&self, _: &Path, _: u32) -> Result<Vec<u8>, String> {
            Err("Beschädigtes Foto".into())
        }
    }
    #[test]
    fn missing_dates_and_decode_failure_produce_terminal_results() {
        let mut context = PhotoContext::new(Arc::new(BrokenSource), paths());
        context.request(1, time(2));
        let events = wait(&mut context, 1);
        assert!(events.iter().any(|event| matches!(event,
            PhotoEvent::Selection { photos, unreadable: 4, .. } if photos[2].is_some()
                && photos.iter().flatten().count() == 1)));
        assert!(events.iter().any(|event| matches!(
            event,
            PhotoEvent::Image {
                slot: 2,
                result: Err(_),
                ..
            }
        )));
    }
    struct CrashedSource;
    impl ContextPhotoSource for CrashedSource {
        fn captured_at(&self, _: &Path) -> Result<CaptureTime, String> {
            panic!("simulated adapter failure")
        }
        fn thumbnail(&self, _: &Path, _: u32) -> Result<Vec<u8>, String> {
            unreachable!()
        }
    }
    #[test]
    fn disconnected_worker_reports_failure_again_after_a_new_clip_request() {
        let mut context = PhotoContext::new(Arc::new(CrashedSource), paths());
        for id in [1, 2] {
            context.request(id, time(2));
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if context.poll().iter().any(|event| {
                    matches!(event,
                    PhotoEvent::Failed { request, .. } if *request == id)
                }) {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert!(context.poll().is_empty());
        }
    }
    #[test]
    fn photo_clicks_resolve_only_current_published_slots() {
        let mut context = PhotoContext::new(Arc::new(Source::default()), paths());
        assert!(context.photo_at(1, 2).is_none());
        context.request(1, time(2));
        wait(&mut context, 5);
        assert_eq!(context.photo_at(1, 2), Some(PathBuf::from("2.jpg")));
        assert!(context.photo_at(1, 5).is_none());
        context.request(2, time(4));
        assert!(context.photo_at(1, 2).is_none());
        assert!(context.photo_at(2, 2).is_none());
        wait(&mut context, 3);
        assert_eq!(context.photo_at(2, 2), Some(PathBuf::from("4.jpg")));
        assert!(context.photo_at(2, 4).is_none());
    }
}
