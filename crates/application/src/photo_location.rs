//! On-demand read-only photo location lookup. No filesystem, EXIF or SQL dependencies.
use crate::{
    domain::{geo::Coordinates, tags::LibraryFile},
    geodata::{Bounds, Place, PlaceIndex, Store},
    media_geo::{Position, Repository, ResolvedPlace, Snapshot},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
pub trait CoordinateSource: Send + Sync + 'static {
    fn coordinates(
        &self,
        root: &Path,
        photo: &LibraryFile,
        stop: &AtomicBool,
    ) -> Result<Option<Coordinates>, String>;
}
#[derive(Debug)]
pub enum Outcome {
    MissingCoordinates,
    DataUnavailable,
    Found {
        coordinates: Coordinates,
        place: Place,
        distance_meters: f64,
    },
    NoPlace {
        coordinates: Coordinates,
    },
}
#[derive(Clone, Copy)]
pub enum Phase {
    ReadingCoordinates,
    CheckingData,
    Searching,
}
pub fn nearest(
    index: &dyn PlaceIndex,
    database: &Path,
    coordinates: Coordinates,
    stop: &AtomicBool,
) -> Result<Option<(Place, f64)>, String> {
    let mut best: Option<(Place, f64)> = None;
    for radius in [
        1_000.,
        5_000.,
        25_000.,
        125_000.,
        625_000.,
        3_125_000.,
        15_625_000.,
        21_000_000.,
    ] {
        check(stop)?;
        for bounds in coordinates.search_boxes(radius)? {
            let mut invalid = None;
            index.visit_candidates(
                database,
                Bounds::new(bounds.west, bounds.south, bounds.east, bounds.north)?,
                stop,
                &mut |place| match Coordinates::new(place.latitude, place.longitude) {
                    Ok(point) => {
                        let distance = coordinates.distance_meters(point);
                        if best.as_ref().is_none_or(|(old, d)| {
                            distance < *d || (distance == *d && place.id < old.id)
                        }) {
                            best = Some((place, distance));
                        }
                    }
                    Err(error) => invalid = Some(error),
                },
            )?;
            if let Some(error) = invalid {
                return Err(error);
            }
        }
        // All places within this radius were considered. Anything outside cannot be closer.
        if best
            .as_ref()
            .is_some_and(|(_, distance)| *distance <= radius)
        {
            break;
        }
    }
    Ok(best)
}
fn check(stop: &AtomicBool) -> Result<(), String> {
    if stop.load(Ordering::Relaxed) {
        Err("Ortssuche abgebrochen".into())
    } else {
        Ok(())
    }
}
/// Prepared read-only dataset, borrowed for one lookup or an entire sweep.
pub struct PreparedSearch<'a> {
    index: &'a dyn PlaceIndex,
    database: PathBuf,
    hash: String,
}
impl<'a> PreparedSearch<'a> {
    pub fn prepare(
        store: &dyn Store,
        index: &'a dyn PlaceIndex,
        stop: &AtomicBool,
    ) -> Result<Option<Self>, String> {
        check(stop)?;
        let prepared = store.inspect(stop, &|_| {})?;
        check(stop)?;
        if !prepared.enabled || !prepared.ready {
            return Ok(None);
        }
        let hash = prepared
            .database
            .ok_or("Geodaten-Prüfnachweis fehlt")?
            .source_sha256;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Ungültiger Geodaten-Prüfnachweis".into());
        }
        Ok(Some(Self {
            index,
            database: store.locations().database,
            hash,
        }))
    }
    pub fn resolve(
        &self,
        source: &dyn Repository,
        root: &Path,
        photo: &LibraryFile,
        snapshot: &Snapshot,
        stop: &AtomicBool,
    ) -> Result<Option<ResolvedPlace>, String> {
        check(stop)?;
        let coordinates = match &snapshot.position {
            Position::Missing => return Ok(None),
            Position::Invalid(error) => return Err(error.clone()),
            Position::Present(point) => *point,
        };
        if let Some(cached) = snapshot
            .resolved
            .as_ref()
            .filter(|r| r.dataset_sha256 == self.hash)
        {
            return Ok(Some(cached.clone()));
        }
        let Some((place, distance_meters)) =
            nearest(self.index, &self.database, coordinates, stop)?
        else {
            return Ok(None);
        };
        let resolved = ResolvedPlace {
            place,
            distance_meters,
            dataset_sha256: self.hash.clone(),
        };
        check(stop)?;
        source.save_place(root, photo, &snapshot.fingerprint, &resolved, stop)?;
        Ok(Some(resolved))
    }
}
fn locate(
    source: &dyn Repository,
    store: &dyn Store,
    index: &dyn PlaceIndex,
    root: &Path,
    photo: &LibraryFile,
    stop: &AtomicBool,
    notify: impl Fn(Phase),
) -> Result<Outcome, String> {
    notify(Phase::ReadingCoordinates);
    check(stop)?;
    let snapshot = source.inspect(root, photo, stop)?;
    let coordinates = match snapshot.position {
        Position::Present(point) => point,
        Position::Missing => return Ok(Outcome::MissingCoordinates),
        Position::Invalid(error) => return Err(error),
    };
    notify(Phase::CheckingData);
    let Some(search) = PreparedSearch::prepare(store, index, stop)? else {
        return Ok(Outcome::DataUnavailable);
    };
    notify(Phase::Searching);
    match search.resolve(source, root, photo, &snapshot, stop)? {
        Some(resolved) => Ok(Outcome::Found {
            coordinates,
            place: resolved.place,
            distance_meters: resolved.distance_meters,
        }),
        None => Ok(Outcome::NoPlace { coordinates }),
    }
}

struct Request {
    root: PathBuf,
    photo: LibraryFile,
    generation: u64,
}
enum Event {
    Phase(Phase),
    Done(u64, Result<Outcome, String>),
}
struct Job {
    cancel: Arc<AtomicBool>,
    events: mpsc::Receiver<Event>,
}
pub struct Session {
    source: Arc<dyn Repository>,
    store: Arc<dyn Store>,
    index: Arc<dyn PlaceIndex>,
    job: Option<Job>,
    pending: Option<Request>,
    generation: u64,
    photo: Option<LibraryFile>,
    outcome: Option<Result<Outcome, String>>,
    phase: Phase,
}
impl Session {
    pub fn new(
        source: Arc<dyn Repository>,
        store: Arc<dyn Store>,
        index: Arc<dyn PlaceIndex>,
    ) -> Self {
        Self {
            source,
            store,
            index,
            job: None,
            pending: None,
            generation: 0,
            photo: None,
            outcome: None,
            phase: Phase::ReadingCoordinates,
        }
    }
    pub fn open(&mut self, root: PathBuf, photo: LibraryFile) {
        self.cancel();
        self.photo = Some(photo.clone());
        self.outcome = None;
        self.phase = Phase::ReadingCoordinates;
        self.pending = Some(Request {
            root,
            photo,
            generation: self.generation,
        });
        self.launch_pending();
    }
    pub fn photo(&self) -> Option<&LibraryFile> {
        self.photo.as_ref()
    }
    pub fn outcome(&self) -> Option<&Result<Outcome, String>> {
        self.outcome.as_ref()
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.pending.is_some()
    }
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.pending = None;
    }
    fn launch_pending(&mut self) {
        if self.job.is_some() {
            return;
        }
        let Some(request) = self.pending.take() else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let source = self.source.clone();
        let store = self.store.clone();
        let index = self.index.clone();
        let (tx, events) = mpsc::sync_channel(8);
        self.job = Some(Job { cancel, events });
        std::thread::spawn(move || {
            let result = locate(
                source.as_ref(),
                store.as_ref(),
                index.as_ref(),
                &request.root,
                &request.photo,
                &stop,
                |phase| {
                    let _ = tx.try_send(Event::Phase(phase));
                },
            );
            let _ = tx.send(Event::Done(request.generation, result));
        });
    }
    pub fn poll(&mut self) {
        let mut done = false;
        if let Some(job) = &self.job {
            for _ in 0..16 {
                match job.events.try_recv() {
                    Ok(Event::Phase(phase)) => {
                        if self.pending.is_none() {
                            self.phase = phase;
                        }
                    }
                    Ok(Event::Done(generation, result)) => {
                        if generation == self.generation {
                            self.outcome = Some(result);
                        }
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if self.pending.is_none() {
                            self.outcome =
                                Some(Err("Ortssuche unerwartet beendet; erneut öffnen".into()));
                        }
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if done {
            self.job = None;
            self.launch_pending();
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        geodata::{DatabaseInfo, Locations, Prepared, Progress},
        media_geo::Fingerprint,
    };
    use std::sync::atomic::AtomicUsize;
    #[derive(Default)]
    struct Ports {
        reads: AtomicUsize,
        saves: AtomicUsize,
        entered: AtomicBool,
        active: AtomicUsize,
        broken_store: AtomicBool,
        broken_search: AtomicBool,
        dataset_revision: AtomicUsize,
    }
    fn place(id: i64, latitude: f64, longitude: f64) -> Place {
        Place {
            id,
            name: format!("Ort {id}"),
            ascii_name: format!("Ort {id}"),
            latitude,
            longitude,
            country_code: "DE".into(),
            admin1_code: "16".into(),
        }
    }
    impl Repository for Ports {
        fn inspect(
            &self,
            _: &Path,
            photo: &LibraryFile,
            stop: &AtomicBool,
        ) -> Result<Snapshot, String> {
            assert_eq!(self.active.fetch_add(1, Ordering::SeqCst), 0);
            if photo.as_str() == "slow.jpg" {
                self.entered.store(true, Ordering::SeqCst);
                while !stop.load(Ordering::Relaxed) {
                    std::thread::yield_now();
                }
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            check(stop)?;
            let position = match photo.as_str() {
                "missing.jpg" => Position::Missing,
                "invalid.jpg" => Position::Invalid("GPS ungültig".into()),
                _ => Position::Present(Coordinates::new(0., 0.).unwrap()),
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
            stop: &AtomicBool,
        ) -> Result<(), String> {
            check(stop)?;
            self.saves.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    impl Store for Ports {
        fn locations(&self) -> Locations {
            Locations {
                archive: "/Global/cities.zip".into(),
                database: "/Global/places.sqlite".into(),
                preferences: "/Global/geo.plist".into(),
            }
        }
        fn inspect(
            &self,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<Prepared, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.broken_store.load(Ordering::Relaxed) {
                return Err("Ortsdaten beschädigt".into());
            }
            Ok(Prepared {
                enabled: true,
                ready: true,
                archive_ready: true,
                database: Some(DatabaseInfo {
                    places: 1,
                    source_sha256: if self.dataset_revision.load(Ordering::Relaxed) == 0 {
                        "a"
                    } else {
                        "b"
                    }
                    .repeat(64),
                    imported_at: "now".into(),
                }),
                ..Default::default()
            })
        }
        fn prepare(
            &self,
            _: bool,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<Prepared, String> {
            unreachable!()
        }
    }
    impl PlaceIndex for Ports {
        fn inspect(
            &self,
            _: &Path,
            _: Option<&str>,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<DatabaseInfo, String> {
            unreachable!()
        }
        fn prepare(
            &self,
            _: &Path,
            _: &Path,
            _: &str,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
            _: &(dyn Fn() -> Result<(), String> + Sync),
        ) -> Result<DatabaseInfo, String> {
            unreachable!()
        }
        fn candidates(&self, _: &Path, _: Bounds, _: usize) -> Result<Vec<Place>, String> {
            unreachable!()
        }
        fn visit_candidates(
            &self,
            _: &Path,
            bounds: Bounds,
            stop: &AtomicBool,
            visit: &mut dyn FnMut(Place),
        ) -> Result<(), String> {
            if self.broken_search.load(Ordering::Relaxed) {
                return Err("Ortsabfrage fehlgeschlagen".into());
            }
            check(stop)?;
            // Deliberately exceed the old 1000-row API limit; the nearest place must still be considered.
            for id in 1..=2001 {
                let p = place(id, 0.004, 0.004);
                if p.latitude >= bounds.south()
                    && p.latitude <= bounds.north()
                    && p.longitude >= bounds.west()
                    && p.longitude <= bounds.east()
                {
                    visit(p);
                }
            }
            visit(place(3000, 0., 0.));
            Ok(())
        }
    }
    #[test]
    fn resolution_reuses_only_matching_dataset_and_never_publishes_after_cancel() {
        let ports = Ports::default();
        let stop = AtomicBool::new(false);
        let root = Path::new("/lib");
        let file = LibraryFile::new("photo.jpg".into()).unwrap();
        let mut snapshot = Repository::inspect(&ports, root, &file, &stop).unwrap();
        let search = PreparedSearch::prepare(&ports, &ports, &stop)
            .unwrap()
            .unwrap();
        snapshot.resolved = search
            .resolve(&ports, root, &file, &snapshot, &stop)
            .unwrap();
        assert_eq!(ports.saves.load(Ordering::SeqCst), 1);
        ports.broken_search.store(true, Ordering::Relaxed);
        assert!(
            search
                .resolve(&ports, root, &file, &snapshot, &stop)
                .unwrap()
                .is_some()
        );
        assert_eq!(ports.saves.load(Ordering::SeqCst), 1);
        ports.dataset_revision.store(1, Ordering::Relaxed);
        let updated = PreparedSearch::prepare(&ports, &ports, &stop)
            .unwrap()
            .unwrap();
        assert!(
            updated
                .resolve(&ports, root, &file, &snapshot, &stop)
                .is_err()
        );
        ports.broken_search.store(false, Ordering::Relaxed);
        assert_eq!(
            updated
                .resolve(&ports, root, &file, &snapshot, &stop)
                .unwrap()
                .unwrap()
                .dataset_sha256,
            "b".repeat(64)
        );
        stop.store(true, Ordering::Relaxed);
        assert!(
            updated
                .resolve(&ports, root, &file, &snapshot, &stop)
                .is_err()
        );
        assert_eq!(ports.saves.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn sweep_exposes_dataset_errors_and_keeps_valid_gps_when_place_resolution_fails() {
        for broken_dataset in [true, false] {
            let ports = Arc::new(Ports::default());
            ports.broken_store.store(broken_dataset, Ordering::Relaxed);
            ports
                .broken_search
                .store(!broken_dataset, Ordering::Relaxed);
            let mut sweep = crate::geo_sweep::Session::new(ports.clone(), Arc::default())
                .with_places(ports.clone(), ports.clone());
            sweep.sync(
                Path::new("/lib"),
                vec![
                    LibraryFile::new("photo.jpg".into()).unwrap(),
                    LibraryFile::new("missing.jpg".into()).unwrap(),
                ],
            );
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !sweep.complete() {
                sweep.poll(true);
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            let p = sweep.progress();
            assert_eq!((p.checked, p.available, p.missing, p.failed), (2, 1, 1, 0));
            assert_eq!(p.available + p.missing + p.failed, p.checked);
            if broken_dataset {
                assert_eq!(p.data_warning.as_deref(), Some("Ortsdaten beschädigt"));
                assert_eq!(p.location_failed, 0);
            } else {
                assert!(p.data_warning.is_none());
                assert_eq!(p.location_failed, 1);
                assert_eq!(p.errors.len(), 1);
            }
            assert_eq!(ports.saves.load(Ordering::SeqCst), 0);
        }
    }
    #[test]
    fn background_sweep_prepares_places_without_opening_photo_popup() {
        let ports = Arc::new(Ports::default());
        let mut sweep = crate::geo_sweep::Session::new(ports.clone(), Arc::default())
            .with_places(ports.clone(), ports.clone());
        sweep.sync(
            Path::new("/lib"),
            vec![
                LibraryFile::new("photo.jpg".into()).unwrap(),
                LibraryFile::new("missing.jpg".into()).unwrap(),
            ],
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !sweep.complete() {
            sweep.poll(true);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(ports.saves.load(Ordering::SeqCst), 1);
        assert_eq!(
            ports.reads.load(Ordering::SeqCst),
            1,
            "prepare data only once per sweep"
        );
        assert_eq!(sweep.progress().available, 1);
        assert_eq!(sweep.progress().missing, 1);
    }
    #[test]
    fn missing_or_invalid_gps_is_reported_without_touching_geodata_and_dense_queries_are_not_truncated()
     {
        let ports = Ports::default();
        let stop = AtomicBool::new(false);
        assert!(matches!(
            locate(
                &ports,
                &ports,
                &ports,
                Path::new("/lib"),
                &LibraryFile::new("missing.jpg".into()).unwrap(),
                &stop,
                |_| {}
            )
            .unwrap(),
            Outcome::MissingCoordinates
        ));
        assert!(
            locate(
                &ports,
                &ports,
                &ports,
                Path::new("/lib"),
                &LibraryFile::new("invalid.jpg".into()).unwrap(),
                &stop,
                |_| {}
            )
            .is_err()
        );
        assert_eq!(ports.reads.load(Ordering::SeqCst), 0);
        let result = locate(
            &ports,
            &ports,
            &ports,
            Path::new("/lib"),
            &LibraryFile::new("photo.jpg".into()).unwrap(),
            &stop,
            |_| {},
        )
        .unwrap();
        assert!(
            matches!(result,Outcome::Found {place,distance_meters,..} if place.id==3000 && distance_meters==0.)
        );
        assert_eq!(ports.saves.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn rapid_photo_switches_keep_one_worker_and_ignore_old_results() {
        let ports = Arc::new(Ports::default());
        let mut session = Session::new(ports.clone(), ports.clone(), ports.clone());
        session.open("/lib".into(), LibraryFile::new("slow.jpg".into()).unwrap());
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !ports.entered.load(Ordering::SeqCst) {
            assert!(std::time::Instant::now() < until);
            std::thread::yield_now();
        }
        session.open(
            "/lib".into(),
            LibraryFile::new("missing.jpg".into()).unwrap(),
        );
        session.open("/lib".into(), LibraryFile::new("photo.jpg".into()).unwrap());
        while session.busy() {
            session.poll();
            assert!(std::time::Instant::now() < until);
            std::thread::yield_now();
        }
        assert_eq!(session.photo().unwrap().as_str(), "photo.jpg");
        assert!(
            matches!(session.outcome().unwrap(),Ok(Outcome::Found {place,..}) if place.id==3000)
        );
        assert_eq!(ports.saves.load(Ordering::SeqCst), 1);
    }
}
