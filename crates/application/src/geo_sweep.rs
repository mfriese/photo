//! Library-wide, resumable GPS preflight. Persistence stays behind the geo repository.
use crate::{
    domain::tags::LibraryFile,
    media_geo::{Position, Repository},
    work_priority::WorkPriority,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};
#[derive(Clone, Default, Debug)]
pub struct Progress {
    pub checked: usize,
    pub total: usize,
    pub available: usize,
    pub missing: usize,
    pub failed: usize,
    pub current: Option<LibraryFile>,
    /// Bounded recent failures; the total failure count remains exact.
    pub errors: Vec<(LibraryFile, String)>,
}
struct Worker {
    cancel: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
    receiver: mpsc::Receiver<Progress>,
    generation: u64,
}
pub struct Session {
    repository: Arc<dyn Repository>,
    priority: Arc<WorkPriority>,
    root: Option<PathBuf>,
    photos: Vec<LibraryFile>,
    generation: u64,
    worker: Option<Worker>,
    progress: Progress,
    complete: bool,
    error: String,
    places: Option<(
        Arc<dyn crate::geodata::Store>,
        Arc<dyn crate::geodata::PlaceIndex>,
    )>,
}
impl Session {
    pub fn new(repository: Arc<dyn Repository>, priority: Arc<WorkPriority>) -> Self {
        Self {
            repository,
            priority,
            root: None,
            photos: Vec::new(),
            generation: 0,
            worker: None,
            progress: Progress::default(),
            complete: false,
            error: String::new(),
            places: None,
        }
    }
    pub fn with_places(
        mut self,
        store: Arc<dyn crate::geodata::Store>,
        index: Arc<dyn crate::geodata::PlaceIndex>,
    ) -> Self {
        self.places = Some((store, index));
        self
    }
    pub fn reset(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel.store(true, Ordering::Relaxed);
        }
        self.generation = self.generation.wrapping_add(1);
        self.root = None;
        self.photos.clear();
        self.progress = Progress::default();
        self.complete = false;
        self.error.clear();
    }
    /// Called when the complete library inventory changes, never for view filters.
    pub fn sync(&mut self, root: &Path, mut photos: Vec<LibraryFile>) {
        photos.sort();
        photos.dedup();
        if self.root.as_deref() == Some(root) && self.photos == photos {
            return;
        }
        self.reset();
        self.root = Some(root.to_owned());
        self.progress.total = photos.len();
        self.photos = photos;
    }
    pub fn retry(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel.store(true, Ordering::Relaxed);
        }
        self.generation = self.generation.wrapping_add(1);
        self.complete = false;
        self.error.clear();
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }
    pub fn progress(&self) -> &Progress {
        &self.progress
    }
    /// Pause cancels cooperatively. Resume reuses the durable per-photo fingerprints.
    pub fn poll(&mut self, allowed: bool) {
        if let Some(worker) = &self.worker {
            if !allowed {
                worker.cancel.store(true, Ordering::Relaxed);
            }
            for progress in worker.receiver.try_iter() {
                if worker.generation == self.generation {
                    self.progress = progress;
                }
            }
            if !worker.handle.is_finished() {
                return;
            }
            let worker = self.worker.take().unwrap();
            let succeeded = worker.handle.join().is_ok();
            for progress in worker.receiver.try_iter() {
                if worker.generation == self.generation {
                    self.progress = progress;
                }
            }
            if worker.generation == self.generation && !succeeded {
                self.error = "GPS-Prüfung unerwartet beendet; Dateien erneut prüfen".into();
            }
            if worker.generation == self.generation
                && succeeded
                && !worker.cancel.load(Ordering::Relaxed)
            {
                self.complete = self.progress.checked == self.photos.len();
            }
        }
        if !allowed || self.complete || self.root.is_none() || !self.error.is_empty() {
            return;
        }
        let root = self.root.clone().unwrap();
        let photos = self.photos.clone();
        let repo = self.repository.clone();
        let priority = self.priority.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let (sender, receiver) = mpsc::sync_channel(2);
        self.progress = Progress {
            total: photos.len(),
            ..Progress::default()
        };
        let places = self.places.clone();
        let handle = thread::spawn(move || {
            if !priority.wait_background(&stop) {
                return;
            }
            let prepared = places.as_ref().and_then(|(store, index)| {
                store
                    .inspect(&stop, &|_| {})
                    .ok()
                    .filter(|p| p.enabled && p.ready)
                    .and_then(|p| p.database)
                    .map(|db| (store.locations().database, db.source_sha256, index.clone()))
            });
            let mut progress = Progress {
                total: photos.len(),
                ..Progress::default()
            };
            for photo in photos {
                if !priority.wait_background(&stop) {
                    return;
                }
                progress.current = Some(photo.clone());
                let _ = sender.try_send(progress.clone());
                let outcome = repo.inspect(&root, &photo, &stop).and_then(|snapshot| {
                    if let (Position::Present(coordinates), Some((database, hash, index))) =
                        (&snapshot.position, &prepared)
                        && snapshot
                            .resolved
                            .as_ref()
                            .is_none_or(|r| r.dataset_sha256 != *hash)
                        && let Some((place, distance_meters)) = crate::photo_location::nearest(
                            index.as_ref(),
                            database,
                            *coordinates,
                            &stop,
                        )?
                    {
                        repo.save_place(
                            &root,
                            &photo,
                            &snapshot.fingerprint,
                            &crate::media_geo::ResolvedPlace {
                                place,
                                distance_meters,
                                dataset_sha256: hash.clone(),
                            },
                            &stop,
                        )?;
                    }
                    Ok(snapshot)
                });
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let error = match outcome {
                    Ok(snapshot) => match snapshot.position {
                        Position::Present(_) => {
                            progress.available += 1;
                            None
                        }
                        Position::Missing => {
                            progress.missing += 1;
                            None
                        }
                        Position::Invalid(error) => Some(error),
                    },
                    Err(error) => Some(error),
                };
                if let Some(error) = error {
                    progress.failed += 1;
                    if progress.errors.len() == 16 {
                        progress.errors.remove(0);
                    }
                    progress
                        .errors
                        .push((photo, error.chars().take(2048).collect()));
                }
                progress.checked += 1;
                let _ = sender.try_send(progress.clone());
            }
            progress.current = None;
            // Bounded channel; cancellation never waits for an unpolled UI.
            loop {
                match sender.try_send(progress) {
                    Ok(()) | Err(mpsc::TrySendError::Disconnected(_)) => break,
                    Err(mpsc::TrySendError::Full(value)) => {
                        progress = value;
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
            }
        });
        self.worker = Some(Worker {
            cancel,
            handle,
            receiver,
            generation: self.generation,
        });
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::geo::Coordinates,
        media_geo::{Fingerprint, ResolvedPlace, Snapshot},
    };
    use std::{
        sync::atomic::AtomicUsize,
        time::{Duration, Instant},
    };
    struct Repo {
        calls: AtomicUsize,
        block: AtomicBool,
        active: AtomicUsize,
        peak: AtomicUsize,
    }
    impl Repository for Repo {
        fn inspect(
            &self,
            _: &Path,
            photo: &LibraryFile,
            stop: &AtomicBool,
        ) -> Result<Snapshot, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(active, Ordering::SeqCst);
            while self.block.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(1));
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            let position = if photo.as_str().contains("missing") {
                Position::Missing
            } else if photo.as_str().contains("invalid") {
                Position::Invalid("EXIF defekt".into())
            } else {
                Position::Present(Coordinates::new(0., 0.).unwrap())
            };
            Ok(Snapshot {
                fingerprint: Fingerprint {
                    bytes: 1,
                    modified: "1".into(),
                },
                position,
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
    fn repo() -> Arc<Repo> {
        Arc::new(Repo {
            calls: AtomicUsize::new(0),
            block: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        })
    }
    fn file(name: &str) -> LibraryFile {
        LibraryFile::new(name.into()).unwrap()
    }
    fn until(session: &mut Session, test: impl Fn(&Session) -> bool) {
        let start = Instant::now();
        while !test(session) {
            assert!(start.elapsed() < Duration::from_secs(3));
            session.poll(true);
            thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn all_photos_finish_including_missing_invalid_and_duplicates_without_repeated_work() {
        let repo = repo();
        let mut s = Session::new(repo.clone(), Arc::default());
        s.sync(
            Path::new("library"),
            vec![
                file("valid.jpg"),
                file("missing.jpg"),
                file("invalid.jpg"),
                file("valid.jpg"),
            ],
        );
        assert!(!s.complete());
        until(&mut s, |s| s.complete());
        assert_eq!(
            (
                s.progress.checked,
                s.progress.available,
                s.progress.missing,
                s.progress.failed
            ),
            (3, 1, 1, 1)
        );
        s.sync(
            Path::new("library"),
            vec![file("invalid.jpg"), file("missing.jpg"), file("valid.jpg")],
        );
        for _ in 0..5 {
            s.poll(true);
        }
        assert_eq!(repo.calls.load(Ordering::SeqCst), 3);
        s.retry();
        until(&mut s, |s| s.complete());
        assert_eq!(repo.calls.load(Ordering::SeqCst), 6);
    }
    #[test]
    fn pause_switch_and_audio_priority_never_spawn_overlapping_workers_or_accept_stale_results() {
        let repo = repo();
        repo.block.store(true, Ordering::Relaxed);
        let priority = Arc::new(WorkPriority::default());
        let mut s = Session::new(repo.clone(), priority.clone());
        s.sync(Path::new("first"), vec![file("invalid.jpg")]);
        s.poll(false);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
        s.poll(true);
        let start = Instant::now();
        while repo.calls.load(Ordering::SeqCst) == 0 {
            assert!(start.elapsed() < Duration::from_secs(2));
            thread::yield_now();
        }
        s.poll(false);
        s.sync(Path::new("second"), vec![file("valid.jpg")]);
        let audio = priority.audio();
        repo.block.store(false, Ordering::Relaxed);
        for _ in 0..20 {
            s.poll(true);
            thread::sleep(Duration::from_millis(1));
        }
        assert!(!s.complete());
        assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
        drop(audio);
        until(&mut s, |s| s.complete());
        assert_eq!(s.progress.available, 1);
        assert_eq!(s.progress.failed, 0);
        assert_eq!(repo.peak.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn errors_are_bounded_and_empty_library_finishes() {
        let repo = repo();
        let mut s = Session::new(repo, Arc::default());
        s.sync(Path::new("empty"), vec![]);
        until(&mut s, |s| s.complete());
        s.sync(
            Path::new("many"),
            (0..50).map(|i| file(&format!("invalid{i}.jpg"))).collect(),
        );
        until(&mut s, |s| s.complete());
        assert_eq!(s.progress.failed, 50);
        assert_eq!(s.progress.errors.len(), 16);
    }
}
