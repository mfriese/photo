//! Incremental read-only collection loading, one worker with user-selected priority.
use crate::{domain::Media, ports::MediaRepository};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
struct Request {
    repository: Arc<dyn MediaRepository>,
    root: PathBuf,
}
enum Event {
    Index(Vec<String>),
    Batch(Vec<Media>, usize),
    Failed(String),
    Complete,
}
#[derive(Default)]
struct Priority {
    next: Option<String>,
    current: Option<String>,
    completed: std::collections::BTreeSet<String>,
}
struct Active {
    root: PathBuf,
    receiver: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
    scan_cancel: Arc<AtomicBool>,
    priority: Arc<Mutex<Priority>>,
}
pub struct Collection {
    pub root: PathBuf,
    pub items: Vec<Media>,
    pub unreadable: usize,
}
#[derive(Default)]
pub struct CollectionSession {
    active: Option<Active>,
    request: Option<Request>,
    years: Vec<String>,
    groups: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    unreadable: usize,
}
impl CollectionSession {
    pub fn groups(
        &self,
    ) -> &std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
        &self.groups
    }
    pub fn years(&self) -> &[String] {
        &self.years
    }
    pub fn partition_loaded(&self, year: &str) -> bool {
        self.active
            .as_ref()
            .is_none_or(|active| active.priority.lock().unwrap().completed.contains(year))
    }
    pub fn prioritize(&mut self, year: &str) {
        if !self.years.iter().any(|y| y == year) {
            return;
        }
        if let Some(active) = &self.active {
            let mut priority = active.priority.lock().unwrap();
            if priority.completed.contains(year)
                || (priority.current.as_deref() == Some(year)
                    && !active.scan_cancel.load(Ordering::Relaxed))
            {
                return;
            }
            priority.next = Some(year.into());
            active.scan_cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn request(&mut self, repository: Arc<dyn MediaRepository>, root: PathBuf) {
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Relaxed);
            active.scan_cancel.store(true, Ordering::Relaxed);
        }
        self.years.clear();
        self.groups.clear();
        self.unreadable = 0;
        self.request = Some(Request { repository, root });
        self.launch();
    }
    fn launch(&mut self) {
        if self.active.is_some() {
            return;
        }
        let Some(request) = self.request.take() else {
            return;
        };
        let (sender, receiver) = mpsc::sync_channel(2);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let scan_cancel = Arc::new(AtomicBool::new(false));
        let scan_stop = scan_cancel.clone();
        let priority = Arc::new(Mutex::new(Priority::default()));
        let next = priority.clone();
        self.active = Some(Active {
            root: request.root.clone(),
            receiver,
            cancel,
            scan_cancel,
            priority,
        });
        std::thread::spawn(move || {
            let root = request.root;
            let indexed = match request.repository.partitions(&root) {
                Ok(value) => value,
                Err(error) => {
                    let _ = sender.send(Event::Failed(error));
                    return;
                }
            };
            let Some(mut remaining) = indexed else {
                let (items, errors) = request.repository.scan_cancellable(&root, &scan_stop);
                let _ = sender.send(Event::Batch(items, errors));
                let _ = sender.send(Event::Complete);
                return;
            };
            remaining.sort();
            remaining.dedup();
            remaining.reverse();
            if sender.send(Event::Index(remaining.clone())).is_err() {
                return;
            }
            remaining.push(String::new()); // Files directly in the library stay discoverable.
            while !remaining.is_empty() && !stop.load(Ordering::Relaxed) {
                let partition = {
                    let mut priority = next.lock().unwrap();
                    scan_stop.store(false, Ordering::Relaxed);
                    let index = priority
                        .next
                        .take()
                        .and_then(|name| remaining.iter().position(|p| *p == name))
                        .unwrap_or(0);
                    let partition = remaining.remove(index);
                    priority.current = Some(partition.clone());
                    partition
                };
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let (items, errors) = request
                    .repository
                    .scan_partition(&root, &partition, &scan_stop);
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                {
                    let mut priority = next.lock().unwrap();
                    priority.current = None;
                    if scan_stop.load(Ordering::Relaxed) {
                        remaining.push(partition);
                        continue;
                    }
                    priority.completed.insert(partition);
                }
                if sender.send(Event::Batch(items, errors)).is_err() {
                    return;
                }
            }
            let _ = sender.send(Event::Complete);
        });
    }
    pub fn is_loading(&self) -> bool {
        self.active.is_some()
    }
    pub fn poll(&mut self) -> Option<Result<Collection, String>> {
        let active = self.active.as_ref()?;
        let event = active.receiver.try_recv();
        if matches!(event, Err(mpsc::TryRecvError::Empty)) {
            return None;
        }
        if active.cancel.load(Ordering::Relaxed) {
            // Drain obsolete bounded events until the old worker has actually finished.
            if matches!(
                event,
                Ok(Event::Complete) | Err(mpsc::TryRecvError::Disconnected) | Ok(Event::Failed(_))
            ) {
                self.active.take();
                self.launch();
            }
            return None;
        }
        let root = active.root.clone();
        match event {
            Ok(Event::Index(years)) => {
                for year in &years {
                    self.groups.entry(year.clone()).or_default();
                }
                self.years = years;
                None
            }
            Ok(Event::Batch(items, errors)) => {
                for item in &items {
                    let group = item.group();
                    let year = group
                        .split(std::path::MAIN_SEPARATOR)
                        .next()
                        .unwrap_or(group);
                    self.groups
                        .entry(year.into())
                        .or_default()
                        .insert(group.into());
                }
                self.unreadable += errors;
                Some(Ok(Collection {
                    root,
                    items,
                    unreadable: self.unreadable,
                }))
            }
            Ok(Event::Complete) => {
                self.active.take();
                Some(Ok(Collection {
                    root,
                    items: Vec::new(),
                    unreadable: self.unreadable,
                }))
            }
            Ok(Event::Failed(error)) => {
                self.active.take();
                Some(Err(error))
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.active.take();
                Some(Err(
                    "Sammlung konnte nicht geladen werden: Hintergrundauftrag unerwartet beendet"
                        .into(),
                ))
            }
            Err(mpsc::TryRecvError::Empty) => unreachable!(),
        }
    }
}
impl Drop for CollectionSession {
    fn drop(&mut self) {
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Relaxed);
            active.scan_cancel.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::Path,
        sync::{Mutex, atomic::AtomicUsize},
        time::{Duration, Instant},
    };
    #[test]
    fn shallow_index_precedes_contents_and_selected_year_interrupts_background_scan() {
        struct Indexed {
            calls: Mutex<Vec<String>>,
            release: AtomicBool,
        }
        impl MediaRepository for Indexed {
            fn scan(&self, _: &Path) -> (Vec<Media>, usize) {
                panic!("must use partitions")
            }
            fn partitions(&self, _: &Path) -> Result<Option<Vec<String>>, String> {
                Ok(Some(vec!["2023".into(), "2025".into(), "2024".into()]))
            }
            fn scan_partition(
                &self,
                root: &Path,
                year: &str,
                cancel: &AtomicBool,
            ) -> (Vec<Media>, usize) {
                self.calls.lock().unwrap().push(year.into());
                if year != "2023" {
                    while !self.release.load(Ordering::SeqCst) && !cancel.load(Ordering::Relaxed) {
                        std::thread::yield_now();
                    }
                }
                (
                    vec![
                        Media::from_path(root.join(year).join("a.jpg"), format!("{year}/01"))
                            .unwrap(),
                    ],
                    0,
                )
            }
        }
        let repository = Arc::new(Indexed {
            calls: Mutex::new(Vec::new()),
            release: AtomicBool::new(false),
        });
        let mut session = CollectionSession::default();
        session.request(repository.clone(), "library".into());
        let deadline = Instant::now() + Duration::from_secs(3);
        while session.years().is_empty() || repository.calls.lock().unwrap().is_empty() {
            assert!(session.poll().is_none());
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(session.years(), ["2025", "2024", "2023"]);
        assert!(session.is_loading());
        session.prioritize("2023");
        let batch = loop {
            if let Some(result) = session.poll() {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(batch.items.len(), 1);
        assert_eq!(batch.items[0].group(), "2023/01");
        assert_eq!(&repository.calls.lock().unwrap()[..2], ["2025", "2023"]);
        // Completed years do not interrupt the next scan.
        session.prioritize("2023");
        repository.release.store(true, Ordering::SeqCst);
        let mut paths: Vec<_> = batch
            .items
            .into_iter()
            .map(|m| m.path().to_owned())
            .collect();
        while session.is_loading() {
            if let Some(result) = session.poll() {
                paths.extend(
                    result
                        .unwrap()
                        .items
                        .into_iter()
                        .map(|m| m.path().to_owned()),
                );
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(paths.len(), 4); // No partial results or repeated batches.
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), 4); // Three directories plus root files.
        assert_eq!(
            repository
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|y| *y == "2023")
                .count(),
            1
        );
    }

    #[derive(Default)]
    struct Repository {
        calls: Mutex<Vec<PathBuf>>,
        release: AtomicBool,
        active: AtomicUsize,
        maximum: AtomicUsize,
    }
    impl MediaRepository for Repository {
        fn scan(&self, root: &Path) -> (Vec<Media>, usize) {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum.fetch_max(active, Ordering::SeqCst);
            self.calls.lock().unwrap().push(root.into());
            if root == Path::new("old") {
                while !self.release.load(Ordering::SeqCst) {
                    std::thread::yield_now();
                }
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            (
                vec![Media::from_path(root.join("a.jpg"), "2023/01".into()).unwrap()],
                0,
            )
        }
    }
    #[test]
    fn rapid_switching_is_single_flight_and_only_latest_collection_is_published() {
        let repo = Arc::new(Repository::default());
        let mut session = CollectionSession::default();
        session.request(repo.clone(), "old".into());
        let deadline = Instant::now() + Duration::from_secs(3);
        while repo.calls.lock().unwrap().is_empty() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        session.request(repo.clone(), "middle".into());
        session.request(repo.clone(), "new".into());
        assert!(session.poll().is_none());
        assert_eq!(repo.calls.lock().unwrap().len(), 1);
        repo.release.store(true, Ordering::SeqCst);
        let collection = loop {
            if let Some(result) = session.poll() {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(collection.root, PathBuf::from("new"));
        assert_eq!(collection.items[0].path(), Path::new("new/a.jpg"));
        assert_eq!(
            *repo.calls.lock().unwrap(),
            vec![PathBuf::from("old"), PathBuf::from("new")]
        );
        assert_eq!(repo.maximum.load(Ordering::SeqCst), 1);
        while session.is_loading() {
            session.poll();
            assert!(Instant::now() < deadline);
        }
    }
    #[test]
    fn worker_failure_is_terminal_and_a_later_request_recovers() {
        struct Broken;
        impl MediaRepository for Broken {
            fn scan(&self, _: &Path) -> (Vec<Media>, usize) {
                panic!("simulated repository failure")
            }
        }
        let mut session = CollectionSession::default();
        session.request(Arc::new(Broken), "bad".into());
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(result) = session.poll() {
                assert!(result.is_err());
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(!session.is_loading());
        session.request(Arc::new(Repository::default()), "new".into());
        loop {
            if let Some(result) = session.poll() {
                assert_eq!(result.unwrap().root, PathBuf::from("new"));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
}
