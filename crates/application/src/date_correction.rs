//! User-authored capture dates. Original media and recording timestamps are never changed.
use crate::domain::{CaptureTime, Media, tags::LibraryFile};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread::JoinHandle,
};
pub type Catalog = BTreeMap<LibraryFile, CaptureTime>;
pub trait Repository: Send + Sync + 'static {
    fn load(&self, root: &Path) -> Result<Catalog, String>;
    fn relocate(
        &self,
        _root: &Path,
        _file: &LibraryFile,
        _date: &CaptureTime,
        _expected: Option<&CaptureTime>,
    ) -> Result<LibraryFile, String> {
        Err("Verschieben wird von dieser Ablage nicht unterstützt".into())
    }
    fn save(
        &self,
        root: &Path,
        file: &LibraryFile,
        date: &CaptureTime,
        expected: Option<&CaptureTime>,
    ) -> Result<(), String>;
}
/// Editing one calendar component preserves the clock and all unchanged components.
pub fn replace_date(
    original: &CaptureTime,
    year: u32,
    month: u32,
    day: u32,
) -> Result<CaptureTime, String> {
    CaptureTime::parse(
        &format!("{year:04}:{month:02}:{day:02} {}", original.time_label()),
        "Korrigiertes Mediendatum",
    )
}
pub fn replace_timestamp(
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Result<CaptureTime, String> {
    CaptureTime::parse(
        &format!("{year:04}:{month:02}:{day:02} {hour:02}:{minute:02}:{second:02}"),
        "Korrigiertes Mediendatum",
    )
}
/// Canonical destination, without suffixes or fallback collision names.
pub fn relocation_target(file: &LibraryFile, date: &CaptureTime) -> Result<LibraryFile, String> {
    let path = Path::new(file.as_str());
    let group = path
        .parent()
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .into_owned();
    let media = Media::from_library_path(path.into(), group).ok_or("Ungültiges Medium")?;
    if media.is_audio() {
        return Err("Nur Fotos und Videos können einsortiert werden".into());
    }
    let target = date.target(Path::new(""), &media, 0);
    LibraryFile::new(
        target
            .to_str()
            .ok_or("Zielpfad ist nicht darstellbar")?
            .into(),
    )
}
pub struct Relocated {
    pub from: LibraryFile,
    pub to: LibraryFile,
}
struct Loaded {
    catalog: Catalog,
    relocated: Option<Relocated>,
}
pub fn project(root: &Path, catalog: &Catalog, media: Media) -> Media {
    let date = media
        .path()
        .strip_prefix(root)
        .ok()
        .and_then(Path::to_str)
        .and_then(|s| LibraryFile::new(s.into()).ok())
        .and_then(|file| catalog.get(&file))
        .cloned();
    media.with_capture_correction(date)
}
/// Expose logical destinations before the physical year inventory has arrived.
pub fn groups(catalog: &Catalog) -> BTreeMap<String, std::collections::BTreeSet<String>> {
    let mut groups = BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    for date in catalog.values() {
        groups
            .entry(date.year().into())
            .or_default()
            .insert(format!("{}/{}", date.year(), date.month()));
    }
    groups
}
/// Prioritize the actual folders when a logical corrected year is selected.
pub fn source_partitions(catalog: &Catalog, year: &str) -> std::collections::BTreeSet<String> {
    catalog
        .iter()
        .filter(|(_, date)| date.year() == year)
        .filter_map(|(file, _)| {
            file.as_str()
                .split(std::path::MAIN_SEPARATOR)
                .next()
                .map(str::to_owned)
        })
        .collect()
}
/// Keep the selected period if it still contains the media; otherwise follow at the same granularity.
pub fn destination_group(selected: Option<&str>, media: &Media) -> Option<String> {
    let selected = selected?;
    if crate::domain::matches_group(media.group(), Some(selected)) {
        return Some(selected.into());
    }
    let date = crate::domain::capture_from_filename(media)?;
    if crate::domain::diary::AudioScope::for_year(selected, crate::domain::diary::AudioKind::All)
        .is_some()
    {
        Some(date.year().into())
    } else {
        Some(format!("{}/{}", date.year(), date.month()))
    }
}
struct Worker {
    handle: JoinHandle<()>,
    receiver: mpsc::Receiver<Result<Loaded, String>>,
    generation: u64,
}
pub struct Session {
    repository: Option<Arc<dyn Repository>>,
    root: Option<PathBuf>,
    worker: Option<Worker>,
    generation: u64,
    pending_load: bool,
    pub catalog: Catalog,
    pub error: String,
    relocated: Option<Relocated>,
    pending_target: Option<LibraryFile>,
}
impl Session {
    pub fn new(repository: Option<Arc<dyn Repository>>) -> Self {
        Self {
            repository,
            root: None,
            worker: None,
            generation: 0,
            pending_load: false,
            catalog: Catalog::new(),
            error: String::new(),
            relocated: None,
            pending_target: None,
        }
    }
    pub fn open(&mut self, root: &Path) {
        self.generation += 1;
        self.root = Some(root.into());
        self.catalog.clear();
        self.relocated = None;
        self.pending_target = None;
        self.error.clear();
        self.pending_load = true;
        self.launch();
    }
    pub fn refresh(&mut self) {
        self.pending_load = true;
        self.launch();
    }
    fn launch(&mut self) {
        if self.worker.is_some() || !self.pending_load {
            return;
        }
        self.pending_load = false;
        let (Some(repository), Some(root)) = (self.repository.clone(), self.root.clone()) else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        let generation = self.generation;
        let handle = std::thread::spawn(move || {
            let _ = sender.send(repository.load(&root).map(|catalog| Loaded {
                catalog,
                relocated: None,
            }));
        });
        self.worker = Some(Worker {
            handle,
            receiver,
            generation,
        });
    }
    pub fn pending_target(&self) -> Option<&LibraryFile> {
        self.pending_target.as_ref()
    }
    pub fn busy(&self) -> bool {
        self.worker.is_some() || self.pending_load
    }
    pub fn save(&mut self, file: LibraryFile, date: CaptureTime) -> Result<(), String> {
        self.save_with_relocation(file, date, false)
    }
    pub fn take_relocated(&mut self) -> Option<Relocated> {
        self.relocated.take()
    }
    pub fn save_with_relocation(
        &mut self,
        file: LibraryFile,
        date: CaptureTime,
        relocate: bool,
    ) -> Result<(), String> {
        if self.busy() {
            return Err("Datumsänderung wird bereits verarbeitet".into());
        }
        let repository = self
            .repository
            .clone()
            .ok_or("Datumskorrektur nicht verfügbar")?;
        let root = self.root.clone().ok_or("Keine Bibliothek geöffnet")?;
        let expected = self.catalog.get(&file).cloned();
        let generation = self.generation;
        let (sender, receiver) = mpsc::channel();
        self.pending_target = if relocate {
            Some(relocation_target(&file, &date)?)
        } else {
            None
        };
        let mut catalog = self.catalog.clone();
        let handle = std::thread::spawn(move || {
            let result = (|| {
                let target = if relocate {
                    repository.relocate(&root, &file, &date, expected.as_ref())?
                } else {
                    repository.save(&root, &file, &date, expected.as_ref())?;
                    file.clone()
                };
                catalog.remove(&file);
                catalog.insert(target.clone(), date);
                let relocated = (target != file).then_some(Relocated {
                    from: file,
                    to: target,
                });
                Ok(Loaded { catalog, relocated })
            })();
            let _ = sender.send(result);
        });
        self.error.clear();
        self.worker = Some(Worker {
            handle,
            receiver,
            generation,
        });
        Ok(())
    }
    pub fn poll(&mut self) -> bool {
        let Some(worker) = &self.worker else {
            self.launch();
            return false;
        };
        let result = match worker.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(_) => Err("Datumsauftrag unerwartet beendet".into()),
        };
        let worker = self.worker.take().unwrap();
        let joined = worker.handle.join();
        let current = worker.generation == self.generation;
        if current {
            self.pending_target = None;
            match result {
                Ok(catalog) if joined.is_ok() => {
                    self.catalog = catalog.catalog;
                    self.relocated = catalog.relocated;
                    self.error.clear();
                }
                Ok(_) => self.error = "Datumsauftrag fehlgeschlagen".into(),
                Err(error) => self.error = error,
            }
        }
        self.launch();
        current
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.handle.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn correction_preserves_clock_and_validates_calendar() {
        let old = CaptureTime::parse("2024:02:29 03:58:11", "Test").unwrap();
        assert!(replace_date(&old, 2023, 2, 29).is_err());
        let new = replace_date(&old, 1980, 1, 2).unwrap();
        assert_eq!(new.time_label(), "03:58:11");
        assert_eq!(
            replace_date(&old, 2020, 2, 29).unwrap().stamp(),
            "20200229_035811"
        );
        assert_eq!(
            replace_date(&old, 2024, 1, 29).unwrap().stamp(),
            "20240129_035811"
        );
        assert_eq!(
            replace_date(&old, 2024, 2, 28).unwrap().stamp(),
            "20240228_035811"
        );
        let file = LibraryFile::new("2024/02/img_20240229_035811.jpg".into()).unwrap();
        let media =
            Media::from_library_path(Path::new("/library").join(file.as_str()), "2024/02".into())
                .unwrap();
        let mut catalog = Catalog::new();
        catalog.insert(file, new.clone());
        assert_eq!(groups(&catalog)["1980"], ["1980/01".into()].into());
        assert_eq!(source_partitions(&catalog, "1980"), ["2024".into()].into());
        assert!(source_partitions(&catalog, "2026").is_empty());
        let projected = project(Path::new("/library"), &catalog, media.clone());
        assert_eq!(projected.path(), media.path());
        assert_eq!(projected.group(), "1980/01");
        assert_eq!(crate::domain::capture_from_filename(&projected), Some(new));
        assert_eq!(
            crate::application::visible(std::slice::from_ref(&projected), "", Some("1980"), false),
            vec![0]
        );
        assert!(crate::application::visible(&[projected], "", Some("2024"), false).is_empty());
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;
    use std::sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    #[derive(Default)]
    struct Store {
        gate: (Mutex<bool>, Condvar),
        started: AtomicBool,
        writes: Mutex<Vec<PathBuf>>,
    }
    impl Repository for Store {
        fn load(&self, _: &Path) -> Result<Catalog, String> {
            Ok(Catalog::new())
        }
        fn save(
            &self,
            root: &Path,
            _: &LibraryFile,
            _: &CaptureTime,
            _: Option<&CaptureTime>,
        ) -> Result<(), String> {
            self.started.store(true, Ordering::Release);
            let (lock, wake) = &self.gate;
            let mut open = lock.lock().unwrap();
            while !*open {
                open = wake.wait(open).unwrap();
            }
            self.writes.lock().unwrap().push(root.into());
            Ok(())
        }
    }
    fn drain(session: &mut Session) {
        let limit = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while session.busy() {
            session.poll();
            assert!(std::time::Instant::now() < limit);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    #[test]
    fn library_switch_never_adopts_old_results_and_drop_finishes_accepted_saves() {
        let store = Arc::new(Store::default());
        let mut session = Session::new(Some(store.clone()));
        session.open(Path::new("/first"));
        drain(&mut session);
        let file = LibraryFile::new("2024/01/img_20240101_000000.jpg".into()).unwrap();
        let date = CaptureTime::parse("1980:01:01 00:00:00", "test").unwrap();
        session.save(file.clone(), date.clone()).unwrap();
        session.open(Path::new("/second"));
        *store.gate.0.lock().unwrap() = true;
        store.gate.1.notify_all();
        drain(&mut session);
        assert_eq!(session.root.as_deref(), Some(Path::new("/second")));
        assert!(session.catalog.is_empty());
        assert_eq!(*store.writes.lock().unwrap(), vec![PathBuf::from("/first")]);
        session.save(file, date).unwrap();
        drop(session);
        assert_eq!(
            store.writes.lock().unwrap().last().unwrap(),
            Path::new("/second")
        );
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    #[test]
    fn follow_keeps_all_and_existing_year_or_follows_year_month_and_custom_folder() {
        let media = Media::from_library_path(
            "/fixture/broken/img_20111111_124844.mp4".into(),
            "broken".into(),
        )
        .unwrap()
        .with_capture_correction(Some(
            CaptureTime::parse("1980:07:08 12:48:44", "test").unwrap(),
        ));
        for (from, to) in [
            (None, None),
            (Some("1980"), Some("1980")),
            (Some("2011"), Some("1980")),
            (Some("2011/11"), Some("1980/07")),
            (Some("broken"), Some("1980/07")),
        ] {
            assert_eq!(destination_group(from, &media).as_deref(), to);
        }
    }
}

#[cfg(test)]
mod relocation_plan_tests {
    use super::*;
    #[test]
    fn target_is_exact_and_clock_is_validated_without_any_collision_suffix() {
        let date = replace_timestamp(1980, 7, 8, 12, 48, 45).unwrap();
        let source = LibraryFile::new("broken/img_20111111_124844.mp4".into()).unwrap();
        assert_eq!(
            relocation_target(&source, &date).unwrap().as_str(),
            "1980/07/img_19800708_124845.mp4"
        );
        assert!(replace_timestamp(1980, 7, 8, 24, 0, 0).is_err());
        assert!(replace_timestamp(2023, 2, 29, 0, 0, 0).is_err());
        assert!(
            relocation_target(
                &LibraryFile::new("2026/audio/diary/aud_20261121_120000.mp4".into()).unwrap(),
                &date
            )
            .is_err()
        );
    }
}

pub mod follow_up;
