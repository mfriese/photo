//! Six-frame video overview use case; sequential decoding, canceled on receiver drop.
use crate::ports::{VideoFrame, VideoPreviewSource};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
pub const FRAME_COUNT: usize = 6;
pub fn sample_times(duration: f64) -> Result<[f64; FRAME_COUNT], String> {
    if !duration.is_finite() || duration <= 0. {
        return Err("Videodauer ist nicht verfügbar".into());
    }
    Ok(std::array::from_fn(|i| {
        duration * (i as f64 + 0.5) / FRAME_COUNT as f64
    }))
}
pub enum Event {
    Started(f64),
    Frame(usize, Result<VideoFrame, String>),
    Failed(String),
    Done,
}
struct Request {
    source: Arc<dyn VideoPreviewSource>,
    path: PathBuf,
}
struct Active {
    receiver: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
}
#[derive(Default)]
pub struct StoryboardSession {
    active: Option<Active>,
    queued: Option<Request>,
}
impl StoryboardSession {
    pub fn request(&mut self, source: Arc<dyn VideoPreviewSource>, path: PathBuf) {
        self.cancel();
        self.queued = Some(Request { source, path });
        self.launch();
    }
    pub fn cancel(&mut self) {
        self.queued = None;
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn is_loading(&self) -> bool {
        self.active.is_some()
    }
    fn launch(&mut self) {
        if self.active.is_some() {
            return;
        }
        let Some(Request { source, path }) = self.queued.take() else {
            return;
        };
        // One request emits at most FRAME_COUNT+3 events; no unbounded stream of pixels.
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        self.active = Some(Active { receiver, cancel });
        std::thread::spawn(move || {
            let emit = |event| !stop.load(Ordering::Relaxed) && sender.send(event).is_ok();
            let result = (|| {
                if stop.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let duration = source.duration(&path)?;
                let times = sample_times(duration)?;
                if !emit(Event::Started(duration)) {
                    return Ok(());
                }
                for (index, seconds) in times.into_iter().enumerate() {
                    if stop.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    if !emit(Event::Frame(index, source.frame_at(&path, seconds))) {
                        return Ok(());
                    }
                }
                Ok::<(), String>(())
            })();
            if let Err(error) = result {
                emit(Event::Failed(error));
            }
            emit(Event::Done);
        });
    }
    pub fn poll(&mut self) -> Vec<Event> {
        let Some(active) = &self.active else {
            return Vec::new();
        };
        let obsolete = active.cancel.load(Ordering::Relaxed);
        let mut events = Vec::new();
        let mut finished = false;
        loop {
            match active.receiver.try_recv() {
                Ok(event) => {
                    if matches!(event, Event::Done) {
                        finished = true;
                    }
                    if !obsolete {
                        events.push(event);
                    }
                    if finished {
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    finished = true;
                    if !obsolete {
                        events.push(Event::Failed("Videovorschau unerwartet beendet".into()));
                        events.push(Event::Done);
                    }
                    break;
                }
            }
        }
        if finished {
            self.active = None;
            self.launch();
        }
        events
    }
}
impl Drop for StoryboardSession {
    fn drop(&mut self) {
        self.cancel();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn six_samples_are_even_and_inside_video() {
        assert_eq!(sample_times(120.).unwrap(), [10., 30., 50., 70., 90., 110.]);
        for d in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(sample_times(d).is_err());
        }
        let short = sample_times(0.1).unwrap();
        assert!(short.windows(2).all(|v| v[0] < v[1]));
        assert!(short[5] < 0.1);
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use std::{
        path::Path,
        sync::Mutex,
        time::{Duration, Instant},
    };
    #[derive(Default)]
    struct Source {
        calls: Mutex<Vec<PathBuf>>,
        release: AtomicBool,
    }
    impl VideoPreviewSource for Source {
        fn duration(&self, path: &Path) -> Result<f64, String> {
            self.calls.lock().unwrap().push(path.into());
            if path == Path::new("old") {
                while !self.release.load(Ordering::SeqCst) {
                    std::thread::yield_now();
                }
            }
            Ok(12.)
        }
        fn frame_at(&self, path: &Path, seconds: f64) -> Result<VideoFrame, String> {
            assert_eq!(
                path,
                Path::new("new"),
                "obsolete request must not decode frames"
            );
            Ok(VideoFrame {
                image: crate::ports::DecodedImage::new(1, 1, vec![1, 2, 3, 255])?,
                seconds,
            })
        }
    }
    #[test]
    fn replacement_waits_for_inflight_io_and_publishes_only_latest_frames() {
        let source = Arc::new(Source::default());
        let mut session = StoryboardSession::default();
        session.request(source.clone(), "old".into());
        let deadline = Instant::now() + Duration::from_secs(3);
        while source.calls.lock().unwrap().is_empty() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        session.request(source.clone(), "middle".into());
        session.request(source.clone(), "new".into());
        assert!(session.poll().is_empty());
        assert_eq!(source.calls.lock().unwrap().len(), 1);
        source.release.store(true, Ordering::SeqCst);
        let mut events = Vec::new();
        while session.is_loading() {
            events.extend(session.poll());
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Frame(_, Ok(_))))
                .count(),
            FRAME_COUNT
        );
        assert_eq!(
            *source.calls.lock().unwrap(),
            vec![PathBuf::from("old"), PathBuf::from("new")]
        );
    }
}
