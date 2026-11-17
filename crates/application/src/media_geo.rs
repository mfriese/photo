//! Derived photo GPS cache. Originals and the GeoNames source are sufficient to reconstruct it.
use crate::{
    domain::{geo::Coordinates, tags::LibraryFile},
    geodata::Place,
    work_priority::WorkPriority,
};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub bytes: u64,
    pub modified: String,
}
#[derive(Clone, Debug)]
pub enum Position {
    Missing,
    Invalid(String),
    Present(Coordinates),
}
#[derive(Clone, Debug)]
pub struct ResolvedPlace {
    pub place: Place,
    pub distance_meters: f64,
    pub dataset_sha256: String,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub fingerprint: Fingerprint,
    pub position: Position,
    pub resolved: Option<ResolvedPlace>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdministrativeRegion {
    pub code: String,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocatedPhoto {
    pub photo: LibraryFile,
    pub country: String,
    pub country_name: String,
    pub region: Option<AdministrativeRegion>,
    pub place_id: i64,
    pub place_name: String,
}
/// Read projection for navigation; independent of EXIF inspection and cache publication.
pub trait LocationIndex: Send + Sync + 'static {
    fn locations(&self, root: &Path, stop: &AtomicBool) -> Result<Vec<LocatedPhoto>, String>;
}
pub trait Repository: Send + Sync + 'static {
    fn inspect(
        &self,
        root: &Path,
        photo: &LibraryFile,
        stop: &AtomicBool,
    ) -> Result<Snapshot, String>;
    fn save_place(
        &self,
        root: &Path,
        photo: &LibraryFile,
        expected: &Fingerprint,
        resolved: &ResolvedPlace,
        stop: &AtomicBool,
    ) -> Result<(), String>;
}
struct Request {
    root: PathBuf,
    photo: LibraryFile,
    key: PathBuf,
    generation: u64,
    cancel: Arc<AtomicBool>,
}
struct Reply {
    key: PathBuf,
    generation: u64,
    value: Result<Snapshot, String>,
}
struct Worker {
    sender: mpsc::SyncSender<Request>,
    receiver: mpsc::Receiver<Reply>,
}
pub struct Catalog {
    repository: Arc<dyn Repository>,
    priority: Arc<WorkPriority>,
    worker: Option<Worker>,
    generation: u64,
    cancel: Arc<AtomicBool>,
    entries: HashMap<PathBuf, Option<Result<Snapshot, String>>>,
    order: VecDeque<PathBuf>,
    pending: usize,
    retry: bool,
}
impl Catalog {
    pub fn new(repository: Arc<dyn Repository>, priority: Arc<WorkPriority>) -> Self {
        Self {
            repository,
            priority,
            worker: None,
            generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            entries: HashMap::new(),
            order: VecDeque::new(),
            pending: 0,
            retry: false,
        }
    }
    pub fn clear(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation = self.generation.wrapping_add(1);
        self.entries.clear();
        self.order.clear();
        self.pending = 0;
        self.retry = false;
    }
    pub fn loading(&self) -> bool {
        self.pending > 0 || self.retry
    }
    pub fn get(&mut self, root: &Path, photo: &LibraryFile) -> Option<&Result<Snapshot, String>> {
        let key = root.join(photo.as_str());
        if !self.entries.contains_key(&key) {
            if self.worker.is_none() {
                self.start();
            }
            let request = Request {
                root: root.into(),
                photo: photo.clone(),
                key: key.clone(),
                generation: self.generation,
                cancel: self.cancel.clone(),
            };
            match self.worker.as_ref().unwrap().sender.try_send(request) {
                Ok(()) => {
                    if self.entries.len() >= 8192
                        && let Some(old) = self.order.pop_front()
                        && self
                            .entries
                            .remove(&old)
                            .is_some_and(|entry| entry.is_none())
                    {
                        self.pending = self.pending.saturating_sub(1);
                    }
                    self.entries.insert(key.clone(), None);
                    self.order.push_back(key.clone());
                    self.pending += 1;
                }
                Err(mpsc::TrySendError::Full(_)) => {
                    self.retry = true;
                    return None;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.worker = None;
                    return None;
                }
            }
        }
        self.entries.get(&key).and_then(|entry| entry.as_ref())
    }
    fn start(&mut self) {
        let (sender, requests) = mpsc::sync_channel::<Request>(32);
        let (replies, receiver) = mpsc::sync_channel(32);
        let repository = self.repository.clone();
        let priority = self.priority.clone();
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let value = if request.cancel.load(Ordering::Relaxed)
                    || !priority.wait_background(&request.cancel)
                {
                    Err("GPS-Prüfung abgebrochen".into())
                } else {
                    repository.inspect(&request.root, &request.photo, &request.cancel)
                };
                if replies
                    .send(Reply {
                        key: request.key,
                        generation: request.generation,
                        value,
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
            for _ in 0..64 {
                match worker.receiver.try_recv() {
                    Ok(reply) if reply.generation == self.generation => {
                        if let Some(entry) = self.entries.get_mut(&reply.key) {
                            if entry.is_none() {
                                self.pending = self.pending.saturating_sub(1);
                            }
                            *entry = Some(reply.value);
                        }
                    }
                    Ok(_) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.worker = None;
                        for entry in self.entries.values_mut() {
                            if entry.is_none() {
                                *entry = Some(Err("GPS-Dienst unerwartet beendet".into()));
                            }
                        }
                        self.pending = 0;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
    }
}
impl Drop for Catalog {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Repo(AtomicUsize);
    impl Repository for Repo {
        fn inspect(&self, _: &Path, _: &LibraryFile, _: &AtomicBool) -> Result<Snapshot, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Snapshot {
                fingerprint: Fingerprint {
                    bytes: 1,
                    modified: "1".into(),
                },
                position: Position::Present(Coordinates::new(0., 0.).unwrap()),
                resolved: None,
            })
        }
        fn save_place(
            &self,
            _: &Path,
            _: &LibraryFile,
            _: &Fingerprint,
            _: &ResolvedPlace,
            _: &AtomicBool,
        ) -> Result<(), String> {
            unreachable!()
        }
    }
    #[test]
    fn visible_photo_requests_are_nonblocking_reused_and_respect_audio_priority() {
        let repository = Arc::new(Repo(AtomicUsize::new(0)));
        let priority = Arc::new(WorkPriority::default());
        let audio = priority.audio();
        let mut catalog = Catalog::new(repository.clone(), priority);
        let file = LibraryFile::new("photo.jpg".into()).unwrap();
        assert!(catalog.get(Path::new("/lib"), &file).is_none());
        assert_eq!(repository.0.load(Ordering::SeqCst), 0);
        drop(audio);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while catalog.get(Path::new("/lib"), &file).is_none() {
            catalog.poll();
            assert!(std::time::Instant::now() < until);
            std::thread::yield_now();
        }
        assert_eq!(repository.0.load(Ordering::SeqCst), 1);
        catalog.get(Path::new("/lib"), &file);
        assert_eq!(repository.0.load(Ordering::SeqCst), 1);
        catalog.clear();
        assert!(catalog.get(Path::new("/other"), &file).is_none());
    }
}
