//! In-memory tags/albums, a bounded serial persistence worker and explicit failure state.
use crate::domain::{
    Media,
    tags::{LibraryFile, TagCatalog},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};
pub struct StoredCatalog {
    pub catalog: TagCatalog,
    pub revision: Option<Vec<u8>>,
}
pub trait CatalogRepository: Send + Sync + 'static {
    fn load(&self, root: &Path) -> Result<StoredCatalog, String>;
    /// Compare the current file with the revision before replacement; never touch media.
    fn save(
        &self,
        root: &Path,
        catalog: &TagCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String>;
}
pub enum Action {
    Create {
        name: String,
        file: Option<LibraryFile>,
    },
    Rename {
        old: String,
        new: String,
    },
    Remove(String),
    Assign {
        tag: String,
        file: LibraryFile,
        present: bool,
    },
    Album {
        tag: String,
        enabled: bool,
    },
}
mod worker;
use worker::{Event, Request, Worker};
#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Created(String),
    Renamed { old: String, new: String },
    Removed(String),
    Updated,
}
/// One current catalog and one latest pending snapshot; intermediate saves are coalesced.
pub struct OrganizationSession {
    worker: Worker,
    receiver: Option<mpsc::Receiver<Event>>,
    root: Option<PathBuf>,
    generation: u64,
    revision: u64,
    acknowledged: u64,
    catalog: Option<Arc<TagCatalog>>,
    loading: bool,
    saving: bool,
    error: String,
}
impl OrganizationSession {
    pub fn new(repository: Arc<dyn CatalogRepository>) -> Self {
        let (worker, receiver) = Worker::new(repository);
        Self {
            worker,
            receiver: Some(receiver),
            root: None,
            generation: 0,
            revision: 0,
            acknowledged: 0,
            catalog: None,
            loading: false,
            saving: false,
            error: String::new(),
        }
    }
    pub fn open(&mut self, root: PathBuf) -> Result<(), String> {
        if self.dirty() || self.saving() {
            return Err("Tags werden noch gespeichert oder konnten nicht gespeichert werden. Erneut speichern oder ungespeicherte Änderungen verwerfen, bevor die Bibliothek gewechselt wird.".into());
        }
        let generation = self.generation + 1;
        self.worker.submit(Request::Load {
            root: root.clone(),
            generation,
        })?;
        self.root = Some(root);
        self.generation = generation;
        self.revision = 0;
        self.acknowledged = 0;
        self.catalog = None;
        self.loading = true;
        self.saving = false;
        self.error.clear();
        Ok(())
    }
    pub fn catalog(&self) -> Option<&TagCatalog> {
        self.catalog.as_deref()
    }
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn loading(&self) -> bool {
        self.loading
    }
    pub fn saving(&self) -> bool {
        self.saving
    }
    pub fn dirty(&self) -> bool {
        self.revision != self.acknowledged
    }
    pub fn tags_for(&self, path: &Path) -> Vec<&str> {
        let Ok(file) = self.file(path) else {
            return Vec::new();
        };
        self.catalog()
            .map(|catalog| catalog.tags_for(&file))
            .unwrap_or_default()
    }
    pub fn file(&self, path: &Path) -> Result<LibraryFile, String> {
        let relative = path
            .strip_prefix(self.root.as_deref().ok_or("Keine Bibliothek geöffnet")?)
            .map_err(|_| "Datei liegt außerhalb der Bibliothek")?;
        LibraryFile::new(
            relative
                .to_str()
                .ok_or("Dateipfad ist kein gültiger Unicode-Text")?
                .to_owned(),
        )
    }
    pub fn apply(&mut self, action: Action) -> Result<Change, String> {
        let mut catalog = self
            .catalog
            .as_deref()
            .ok_or("Tags werden noch geladen oder konnten nicht geladen werden")?
            .clone();
        let change = match action {
            Action::Create { name, file } => {
                let name = catalog.create(&name)?;
                if let Some(file) = file {
                    catalog.assign(&name, file, true)?;
                }
                Change::Created(name)
            }
            Action::Rename { old, new } => {
                let new = catalog.rename(&old, &new)?;
                Change::Renamed { old, new }
            }
            Action::Remove(tag) => {
                catalog.remove(&tag)?;
                Change::Removed(tag)
            }
            Action::Assign { tag, file, present } => {
                catalog.assign(&tag, file, present)?;
                Change::Updated
            }
            Action::Album { tag, enabled } => {
                catalog.set_album(&tag, enabled)?;
                Change::Updated
            }
        };
        if self.catalog.as_deref() == Some(&catalog) {
            return Ok(change);
        }
        let catalog = Arc::new(catalog);
        let revision = self.revision + 1;
        self.worker.submit(Request::Save {
            root: self.root.clone().ok_or("Keine Bibliothek geöffnet")?,
            generation: self.generation,
            revision,
            catalog: catalog.clone(),
        })?;
        self.catalog = Some(catalog);
        self.revision = revision;
        self.saving = true;
        Ok(change)
    }
    pub fn retry(&mut self) -> Result<(), String> {
        if self.saving() {
            return Err("Ein Tag-Speichervorgang läuft bereits".into());
        }
        if !self.dirty() {
            return Ok(());
        }
        let catalog = self.catalog.clone().ok_or("Keine Tags geladen")?;
        self.worker.submit(Request::Save {
            root: self.root.clone().ok_or("Keine Bibliothek geöffnet")?,
            generation: self.generation,
            revision: self.revision,
            catalog,
        })?;
        self.saving = true;
        Ok(())
    }
    pub fn discard(&mut self) -> Result<(), String> {
        if self.saving() {
            return Err("Bitte laufenden Speichervorgang abwarten".into());
        }
        let root = self.root.clone().ok_or("Keine Bibliothek geöffnet")?;
        let generation = self.generation + 1;
        self.worker.submit(Request::Load { root, generation })?;
        self.generation = generation;
        self.revision = 0;
        self.acknowledged = 0;
        self.catalog = None;
        self.loading = true;
        self.error.clear();
        Ok(())
    }
    pub fn poll(&mut self) {
        for _ in 0..32 {
            let event = match self.receiver.as_ref().unwrap().try_recv() {
                Ok(event) => event,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = false;
                    self.saving = false;
                    self.error="Tag-Dienst unerwartet beendet; nicht bestätigte Änderungen bleiben in dieser Sitzung. Vor erneutem Laden den Dateistand prüfen.".into();
                    break;
                }
            };
            match event {
                Event::Loaded { generation, result } if generation == self.generation => {
                    self.loading = false;
                    match result {
                        Ok(catalog) => {
                            self.catalog = Some(Arc::new(catalog));
                            self.error.clear();
                        }
                        Err(error) => self.error = error,
                    }
                }
                Event::Saved {
                    generation,
                    revision,
                    result,
                } if generation == self.generation => {
                    if revision == self.revision {
                        self.saving = false;
                    }
                    match result {
                        Ok(()) => {
                            self.acknowledged = self.acknowledged.max(revision);
                            if !self.dirty() {
                                self.error.clear();
                            }
                        }
                        Err(error) => {
                            if revision == self.revision {
                                self.error = format!("Tags nicht gespeichert: {error}");
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
impl Drop for OrganizationSession {
    fn drop(&mut self) {
        self.receiver.take();
    }
}
pub fn visible(
    items: &[Media],
    query: &str,
    videos_only: bool,
    root: &Path,
    catalog: &TagCatalog,
    tag: &str,
) -> Vec<usize> {
    let query = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, media)| {
            (!videos_only || media.is_video())
                && media
                    .path()
                    .strip_prefix(root)
                    .ok()
                    .and_then(Path::to_str)
                    .is_some_and(|relative| catalog.contains(tag, relative))
                && (query.is_empty()
                    || media
                        .path()
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query))
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::{
        sync::{
            Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };
    #[derive(Default)]
    struct Memory {
        catalog: Mutex<TagCatalog>,
        loads: AtomicUsize,
        saves: Mutex<Vec<TagCatalog>>,
        fail: AtomicBool,
    }
    impl CatalogRepository for Memory {
        fn load(&self, _: &Path) -> Result<StoredCatalog, String> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            Ok(StoredCatalog {
                catalog: self.catalog.lock().unwrap().clone(),
                revision: None,
            })
        }
        fn save(
            &self,
            _: &Path,
            catalog: &TagCatalog,
            _: Option<&[u8]>,
        ) -> Result<Vec<u8>, String> {
            if self.fail.load(Ordering::SeqCst) {
                return Err("read only".into());
            }
            *self.catalog.lock().unwrap() = catalog.clone();
            self.saves.lock().unwrap().push(catalog.clone());
            Ok(vec![1])
        }
    }
    fn wait(session: &mut OrganizationSession, condition: impl Fn(&OrganizationSession) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !condition(session) {
            session.poll();
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
    }
    #[test]
    fn slow_writer_coalesces_many_changes_and_flushes_only_the_latest_pending_snapshot() {
        struct Slow {
            entered: AtomicBool,
            release: AtomicBool,
            saves: Mutex<Vec<TagCatalog>>,
        }
        impl CatalogRepository for Slow {
            fn load(&self, _: &Path) -> Result<StoredCatalog, String> {
                Ok(StoredCatalog {
                    catalog: Default::default(),
                    revision: None,
                })
            }
            fn save(
                &self,
                _: &Path,
                catalog: &TagCatalog,
                _: Option<&[u8]>,
            ) -> Result<Vec<u8>, String> {
                self.saves.lock().unwrap().push(catalog.clone());
                self.entered.store(true, Ordering::SeqCst);
                while !self.release.load(Ordering::SeqCst) {
                    thread::yield_now();
                }
                Ok(vec![1])
            }
        }
        let repo = Arc::new(Slow {
            entered: AtomicBool::new(false),
            release: AtomicBool::new(false),
            saves: Mutex::new(Vec::new()),
        });
        let mut session = OrganizationSession::new(repo.clone());
        session.open("a".into()).unwrap();
        wait(&mut session, |s| s.catalog().is_some());
        assert_eq!(
            session
                .apply(Action::Create {
                    name: "  Familie  ".into(),
                    file: None
                })
                .unwrap(),
            Change::Created("Familie".into())
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while !repo.entered.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        for i in 0..100 {
            session
                .apply(Action::Create {
                    name: format!("tag{i}"),
                    file: None,
                })
                .unwrap();
        }
        assert!(session.saving());
        assert!(session.retry().is_err());
        repo.release.store(true, Ordering::SeqCst);
        wait(&mut session, |s| !s.dirty());
        assert!(!session.saving());
        let saves = repo.saves.lock().unwrap();
        assert_eq!(saves.len(), 2);
        assert_eq!(saves.last().unwrap().tags().len(), 101);
    }
    #[test]
    fn rapid_library_requests_load_only_the_latest_waiting_library() {
        struct Slow {
            entered: AtomicBool,
            release: AtomicBool,
            roots: Mutex<Vec<PathBuf>>,
        }
        impl CatalogRepository for Slow {
            fn load(&self, root: &Path) -> Result<StoredCatalog, String> {
                self.roots.lock().unwrap().push(root.into());
                if root == Path::new("a") {
                    self.entered.store(true, Ordering::SeqCst);
                    while !self.release.load(Ordering::SeqCst) {
                        thread::yield_now();
                    }
                }
                Ok(StoredCatalog {
                    catalog: Default::default(),
                    revision: None,
                })
            }
            fn save(&self, _: &Path, _: &TagCatalog, _: Option<&[u8]>) -> Result<Vec<u8>, String> {
                unreachable!()
            }
        }
        let repo = Arc::new(Slow {
            entered: AtomicBool::new(false),
            release: AtomicBool::new(false),
            roots: Mutex::new(Vec::new()),
        });
        let mut session = OrganizationSession::new(repo.clone());
        session.open("a".into()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !repo.entered.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        session.open("b".into()).unwrap();
        session.open("c".into()).unwrap();
        repo.release.store(true, Ordering::SeqCst);
        wait(&mut session, |s| s.catalog().is_some());
        assert_eq!(session.root(), Some(Path::new("c")));
        assert_eq!(
            *repo.roots.lock().unwrap(),
            vec![PathBuf::from("a"), PathBuf::from("c")]
        );
    }
    #[test]
    fn edits_are_immediate_in_memory_and_filter_all_media_without_reloading() {
        let repository = Arc::new(Memory::default());
        let mut session = OrganizationSession::new(repository.clone());
        session.open("/photos".into()).unwrap();
        wait(&mut session, |s| s.catalog().is_some());
        let file = session.file(Path::new("/photos/2023/01/a.jpg")).unwrap();
        session
            .apply(Action::Create {
                name: "Urlaub".into(),
                file: Some(file),
            })
            .unwrap();
        session
            .apply(Action::Album {
                tag: "Urlaub".into(),
                enabled: true,
            })
            .unwrap();
        session
            .apply(Action::Assign {
                tag: "Urlaub".into(),
                file: session.file(Path::new("/photos/2023/diary/a.wav")).unwrap(),
                present: true,
            })
            .unwrap();
        assert!(session.catalog().unwrap().albums().contains("Urlaub"));
        let items = vec![
            Media::from_path("/photos/2023/01/a.jpg".into(), "2023/01".into()).unwrap(),
            Media::from_path("/photos/2024/01/a.jpg".into(), "2024/01".into()).unwrap(),
            Media::from_path("/photos/2023/diary/a.wav".into(), "2023/diary".into()).unwrap(),
        ];
        assert_eq!(
            visible(
                &items,
                "",
                false,
                Path::new("/photos"),
                session.catalog().unwrap(),
                "Urlaub"
            ),
            vec![0, 2]
        );
        assert!(
            visible(
                &items,
                "",
                true,
                Path::new("/photos"),
                session.catalog().unwrap(),
                "Urlaub"
            )
            .is_empty()
        );
        assert!(session.file(Path::new("/outside/a.jpg")).is_err());
        wait(&mut session, |s| !s.dirty());
        assert_eq!(repository.loads.load(Ordering::SeqCst), 1);
        let saves = repository.saves.lock().unwrap();
        assert!(!saves.is_empty() && saves.len() <= 3);
        assert_eq!(saves.last(), Some(&*repository.catalog.lock().unwrap()));
    }
    #[test]
    fn failed_save_retains_changes_and_blocks_library_switch_until_retry_or_discard() {
        let repository = Arc::new(Memory::default());
        let mut session = OrganizationSession::new(repository.clone());
        session.open("a".into()).unwrap();
        wait(&mut session, |s| s.catalog().is_some());
        repository.fail.store(true, Ordering::SeqCst);
        session
            .apply(Action::Create {
                name: "keep".into(),
                file: None,
            })
            .unwrap();
        wait(&mut session, |s| !s.saving());
        assert!(session.dirty());
        assert!(session.error().contains("read only"));
        assert!(session.open("b".into()).is_err());
        assert!(session.catalog().unwrap().tags().contains_key("keep"));
        repository.fail.store(false, Ordering::SeqCst);
        session.retry().unwrap();
        wait(&mut session, |s| !s.dirty());
        assert!(session.error().is_empty());
        repository.fail.store(true, Ordering::SeqCst);
        session
            .apply(Action::Create {
                name: "discard".into(),
                file: None,
            })
            .unwrap();
        wait(&mut session, |s| !s.saving());
        session.discard().unwrap();
        wait(&mut session, |s| !s.loading());
        assert!(!session.catalog().unwrap().tags().contains_key("discard"));
        assert!(session.catalog().unwrap().tags().contains_key("keep"));
        session.open("b".into()).unwrap();
    }
    #[test]
    fn drop_flushes_queued_catalog_changes() {
        let repository = Arc::new(Memory::default());
        let mut session = OrganizationSession::new(repository.clone());
        session.open("a".into()).unwrap();
        wait(&mut session, |s| s.catalog().is_some());
        session
            .apply(Action::Create {
                name: "a".into(),
                file: None,
            })
            .unwrap();
        session
            .apply(Action::Album {
                tag: "a".into(),
                enabled: true,
            })
            .unwrap();
        drop(session);
        assert!(repository.catalog.lock().unwrap().albums().contains("a"));
    }
    #[test]
    fn worker_failure_finishes_loading_and_never_enables_editing() {
        struct Broken;
        impl CatalogRepository for Broken {
            fn load(&self, _: &Path) -> Result<StoredCatalog, String> {
                panic!("simulated catalog failure")
            }
            fn save(&self, _: &Path, _: &TagCatalog, _: Option<&[u8]>) -> Result<Vec<u8>, String> {
                unreachable!()
            }
        }
        let mut session = OrganizationSession::new(Arc::new(Broken));
        session.open("a".into()).unwrap();
        wait(&mut session, |s| !s.loading());
        assert!(session.error().contains("unerwartet"));
        assert!(
            session
                .apply(Action::Create {
                    name: "a".into(),
                    file: None
                })
                .is_err()
        );
    }
}
