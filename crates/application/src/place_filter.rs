//! RAM-only location facets and a serial background projection reader.
use crate::{
    domain::{Media, tags::LibraryFile},
    media_geo::{LocatedPhoto, LocationIndex},
    work_priority::WorkPriority,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Selection {
    Country(String),
    Place(i64),
}
#[derive(Default)]
pub struct Catalog {
    entries: BTreeMap<LibraryFile, LocatedPhoto>,
}
impl Catalog {
    pub fn new(entries: Vec<LocatedPhoto>) -> Self {
        Self {
            entries: entries.into_iter().map(|e| (e.photo.clone(), e)).collect(),
        }
    }
    fn entry(&self, media: &Media, root: &Path) -> Option<&LocatedPhoto> {
        let path = media.path().strip_prefix(root).ok()?.to_str()?;
        self.entries.get(path)
    }
    /// An empty selection preserves photos without GPS. Otherwise all selected facets must match.
    pub fn filter(
        &self,
        items: &[Media],
        root: &Path,
        candidates: &[usize],
        selected: &BTreeSet<Selection>,
    ) -> Vec<usize> {
        if selected.is_empty() {
            return candidates.to_vec();
        }
        candidates
            .iter()
            .copied()
            .filter(|&i| {
                items
                    .get(i)
                    .and_then(|m| self.entry(m, root))
                    .is_some_and(|entry| {
                        selected.iter().all(|s| match s {
                            Selection::Country(c) => *c == entry.country,
                            Selection::Place(id) => *id == entry.place_id,
                        })
                    })
            })
            .collect()
    }
    /// Available choices follow the period, not the current tag/place selection.
    pub fn options(
        &self,
        items: &[Media],
        root: &Path,
        candidates: &[usize],
    ) -> Vec<(String, Vec<(i64, String)>)> {
        let mut countries: BTreeMap<String, BTreeMap<i64, String>> = BTreeMap::new();
        for i in candidates {
            if let Some(entry) = items.get(*i).and_then(|m| self.entry(m, root)) {
                countries
                    .entry(entry.country.clone())
                    .or_default()
                    .insert(entry.place_id, entry.place_name.clone());
            }
        }
        countries
            .into_iter()
            .map(|(country, places)| {
                let mut places: Vec<_> = places.into_iter().collect();
                places.sort_by(|a, b| {
                    a.1.to_lowercase()
                        .cmp(&b.1.to_lowercase())
                        .then(a.1.cmp(&b.1))
                        .then(a.0.cmp(&b.0))
                });
                (country, places)
            })
            .collect()
    }
}
struct Worker {
    receiver: mpsc::Receiver<Result<Vec<LocatedPhoto>, String>>,
    stop: Arc<AtomicBool>,
    root: PathBuf,
}
pub struct Session {
    repository: Arc<dyn LocationIndex>,
    priority: Arc<WorkPriority>,
    root: Option<PathBuf>,
    worker: Option<Worker>,
    catalog: Catalog,
    pub selected: BTreeSet<Selection>,
    error: String,
    last: Option<Instant>,
    dirty: bool,
    revision: u64,
}
impl Session {
    pub fn new(repository: Arc<dyn LocationIndex>, priority: Arc<WorkPriority>) -> Self {
        Self {
            repository,
            priority,
            root: None,
            worker: None,
            catalog: Catalog::default(),
            selected: BTreeSet::new(),
            error: String::new(),
            last: None,
            dirty: true,
            revision: 0,
        }
    }
    pub fn reset(&mut self) {
        if let Some(worker) = &self.worker {
            worker.stop.store(true, Ordering::Relaxed);
        }
        self.root = None;
        self.catalog = Catalog::default();
        self.revision = self.revision.wrapping_add(1);
        self.selected.clear();
        self.error.clear();
        self.last = None;
        self.dirty = true;
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn refresh(&mut self) {
        self.dirty = true;
    }
    pub fn pending(&self) -> bool {
        self.root.is_some() && (self.worker.is_some() || self.dirty)
    }
    pub fn poll(&mut self, root: Option<&Path>) {
        if self.root.as_deref() != root {
            self.reset();
            self.root = root.map(Path::to_owned);
        }
        if let Some(worker) = &self.worker {
            match worker.receiver.try_recv() {
                Ok(result) => {
                    if self.root.as_ref() == Some(&worker.root)
                        && !worker.stop.load(Ordering::Relaxed)
                    {
                        match result {
                            Ok(entries) => {
                                self.catalog = Catalog::new(entries);
                                self.revision = self.revision.wrapping_add(1);
                                self.error.clear();
                            }
                            Err(error) => self.error = error,
                        }
                    }
                    self.worker = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.root.as_ref() == Some(&worker.root)
                        && !worker.stop.load(Ordering::Relaxed)
                    {
                        self.error = "Ortsliste konnte nicht geladen werden".into();
                    }
                    self.worker = None;
                }
                Err(mpsc::TryRecvError::Empty) => return,
            }
        }
        if !self.dirty {
            return;
        }
        if self
            .last
            .is_some_and(|last| last.elapsed() < Duration::from_millis(500))
        {
            return;
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = stop.clone();
        let repository = self.repository.clone();
        let priority = self.priority.clone();
        let requested = root.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let value = if priority.wait_background(&cancel) {
                repository.locations(&requested, &cancel)
            } else {
                Err("Ortsliste abgebrochen".into())
            };
            let _ = sender.send(value);
        });
        self.worker = Some(Worker {
            receiver,
            stop,
            root,
        });
        self.last = Some(Instant::now());
        self.dirty = false;
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
    fn located(photo: &str, country: &str, id: i64, name: &str) -> LocatedPhoto {
        LocatedPhoto {
            photo: LibraryFile::new(photo.into()).unwrap(),
            country: country.into(),
            place_id: id,
            place_name: name.into(),
        }
    }
    #[test]
    fn cancelled_worker_panic_does_not_report_failure_for_the_new_library() {
        struct Index {
            entered: AtomicBool,
            release: AtomicBool,
        }
        impl LocationIndex for Index {
            fn locations(
                &self,
                root: &Path,
                stop: &AtomicBool,
            ) -> Result<Vec<LocatedPhoto>, String> {
                self.entered.store(true, Ordering::Relaxed);
                if root == Path::new("old") {
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::yield_now();
                    }
                    panic!("cancelled old worker");
                }
                while !self.release.load(Ordering::Relaxed) {
                    std::thread::yield_now();
                }
                Ok(vec![located("new.jpg", "DE", 1, "Berlin")])
            }
        }
        let index = Arc::new(Index {
            entered: AtomicBool::new(false),
            release: AtomicBool::new(false),
        });
        let mut session = Session::new(index.clone(), Arc::default());
        session.poll(Some(Path::new("old")));
        let deadline = Instant::now() + Duration::from_secs(3);
        while !index.entered.load(Ordering::Relaxed) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        loop {
            session.poll(Some(Path::new("new")));
            assert!(
                session.error().is_empty(),
                "stale error reached new library"
            );
            if session
                .worker
                .as_ref()
                .is_some_and(|w| w.root == Path::new("new"))
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        index.release.store(true, Ordering::Relaxed);
        while session.pending() {
            session.poll(Some(Path::new("new")));
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(session.catalog.entries.contains_key("new.jpg"));
    }
    #[test]
    fn failed_refresh_preserves_previous_projection_and_explicit_retry_recovers() {
        struct Index {
            fail: AtomicBool,
        }
        impl LocationIndex for Index {
            fn locations(&self, _: &Path, _: &AtomicBool) -> Result<Vec<LocatedPhoto>, String> {
                if self.fail.load(Ordering::Relaxed) {
                    Err("Datenträger nicht erreichbar".into())
                } else {
                    Ok(vec![located("photo.jpg", "DE", 1, "Berlin")])
                }
            }
        }
        let index = Arc::new(Index {
            fail: AtomicBool::new(false),
        });
        let mut session = Session::new(index.clone(), Arc::default());
        let root = Some(Path::new("library"));
        for fail in [false, true, false] {
            index.fail.store(fail, Ordering::Relaxed);
            session.refresh();
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                session.poll(root);
                if !session.pending() {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(session.catalog.entries.contains_key("photo.jpg"));
            assert_eq!(session.error().is_empty(), !fail);
        }
    }
    #[test]
    fn background_projection_rejects_cancelled_roots_and_does_not_poll_at_idle() {
        use std::sync::atomic::AtomicUsize;
        struct Reader {
            calls: AtomicUsize,
        }
        impl LocationIndex for Reader {
            fn locations(
                &self,
                root: &Path,
                stop: &AtomicBool,
            ) -> Result<Vec<LocatedPhoto>, String> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                if root == Path::new("old") {
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::yield_now();
                    }
                }
                Ok(vec![located(
                    if root == Path::new("old") {
                        "old.jpg"
                    } else {
                        "new.jpg"
                    },
                    "DE",
                    1,
                    "Berlin",
                )])
            }
        }
        let reader = Arc::new(Reader {
            calls: AtomicUsize::new(0),
        });
        let mut session = Session::new(reader.clone(), Arc::default());
        session.poll(Some(Path::new("old")));
        let deadline = Instant::now() + Duration::from_secs(3);
        while reader.calls.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        session.selected.insert(Selection::Country("FR".into()));
        loop {
            session.poll(Some(Path::new("new")));
            if !session.pending() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(session.selected.is_empty());
        assert!(session.catalog.entries.contains_key("new.jpg"));
        assert!(!session.catalog.entries.contains_key("old.jpg"));
        for _ in 0..20 {
            session.poll(Some(Path::new("new")));
        }
        assert_eq!(reader.calls.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn facets_sort_by_country_and_place_deduplicate_and_follow_period() {
        let root = Path::new("/photos");
        let items: Vec<_> = [
            "2020/01/a.jpg",
            "2020/02/b.jpg",
            "2021/01/c.jpg",
            "2020/01/d.jpg",
        ]
        .into_iter()
        .map(|p| Media::from_path(root.join(p), p[..7].into()).unwrap())
        .collect();
        let catalog = Catalog::new(vec![
            located("2020/01/a.jpg", "FR", 3, "Paris"),
            located("2020/02/b.jpg", "DE", 2, "Berlin"),
            located("2021/01/c.jpg", "DE", 1, "Aachen"),
            located("2020/01/d.jpg", "FR", 3, "Paris"),
        ]);
        assert_eq!(
            catalog.options(&items, root, &[0, 1, 2, 3]),
            vec![
                (
                    "DE".into(),
                    vec![(1, "Aachen".into()), (2, "Berlin".into())]
                ),
                ("FR".into(), vec![(3, "Paris".into())])
            ]
        );
        assert_eq!(
            catalog.options(&items, root, &[0, 3]),
            vec![("FR".into(), vec![(3, "Paris".into())])]
        );
    }
    #[test]
    fn empty_selection_keeps_unknown_media_otherwise_and_filters_combine_with_tags() {
        let root = Path::new("/photos");
        let items: Vec<_> = ["a.jpg", "b.jpg", "c.jpg", "unknown.jpg"]
            .into_iter()
            .map(|p| Media::from_path(root.join(p), "2020/01".into()).unwrap())
            .collect();
        let catalog = Catalog::new(vec![
            located("a.jpg", "DE", 1, "Berlin"),
            located("b.jpg", "DE", 2, "Bonn"),
            located("c.jpg", "FR", 3, "Paris"),
        ]);
        assert_eq!(
            catalog.filter(&items, root, &[0, 1, 2, 3], &BTreeSet::new()),
            vec![0, 1, 2, 3]
        );
        assert_eq!(
            catalog.filter(
                &items,
                root,
                &[0, 1, 2, 3],
                &[Selection::Country("DE".into())].into()
            ),
            vec![0, 1]
        );
        assert_eq!(
            catalog.filter(
                &items,
                root,
                &[0, 1, 2, 3],
                &[Selection::Country("DE".into()), Selection::Place(2)].into()
            ),
            vec![1]
        );
        assert!(
            catalog
                .filter(
                    &items,
                    root,
                    &[0, 1, 2, 3],
                    &[Selection::Place(1), Selection::Place(2)].into()
                )
                .is_empty()
        );
        let mut tags = crate::domain::tags::TagCatalog::default();
        tags.create("scan").unwrap();
        tags.assign("scan", LibraryFile::new("b.jpg".into()).unwrap(), true)
            .unwrap();
        let tagged = crate::organization::filter_all(
            &items,
            root,
            &tags,
            &[0, 1, 2, 3],
            &["scan".into()].into(),
        );
        assert_eq!(
            catalog.filter(
                &items,
                root,
                &tagged,
                &[Selection::Country("DE".into())].into()
            ),
            vec![1]
        );
    }
}
