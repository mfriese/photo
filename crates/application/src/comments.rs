//! Recording and reference persistence orchestration. No GUI, native or filesystem dependencies.
use crate::domain::{
    CaptureTime, Media,
    comments::{Comment, CommentCatalog},
    tags::LibraryFile,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};
#[derive(Clone, Debug)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CapturePhase {
    #[default]
    Idle,
    Preparing,
    Recording,
    Paused,
    Finishing,
    Ready,
    Error,
}
#[derive(Clone, Debug)]
pub struct CapturedTake {
    pub id: String,
    pub path: PathBuf,
    pub recorded_at: String,
}
#[derive(Clone, Debug, Default)]
pub struct CaptureSnapshot {
    pub phase: CapturePhase,
    pub seconds: f64,
    pub level: f32,
    pub peak: f32,
    pub wave: Vec<f32>,
    pub playing: bool,
    pub message: String,
    pub take: Option<CapturedTake>,
    pub id: String,
}
#[derive(Clone, Copy, Debug)]
pub enum CaptureCommand {
    Pause,
    Resume,
    Finish,
    Preview,
    Discard,
}
pub trait CommentRecorder: Send + Sync + 'static {
    fn devices(&self) -> Result<Vec<InputDevice>, String>;
    fn start(&self, device: &str) -> Result<(), String>;
    fn command(&self, command: CaptureCommand) -> Result<(), String>;
    fn snapshot(&self) -> CaptureSnapshot;
}
#[derive(Clone, Debug)]
pub struct PhotoTarget {
    pub root: PathBuf,
    pub file: LibraryFile,
    pub time: Option<CaptureTime>,
}
pub struct StoredComments {
    pub catalog: CommentCatalog,
    pub revision: Option<Vec<u8>>,
}
pub trait CommentRepository: Send + Sync + 'static {
    fn load(&self, root: &Path) -> Result<StoredComments, String>;
    fn save(
        &self,
        root: &Path,
        catalog: &CommentCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String>;
}
/// Technical operations only. Naming, collision selection and Comment construction
/// are application/domain decisions, never a filesystem adapter's workflow.
pub trait CommentMediaStore: Send + Sync + 'static {
    fn validate(&self, target: &PhotoTarget, take: &CapturedTake) -> Result<(), String>;
    fn photo_time(&self, target: &PhotoTarget) -> Result<CaptureTime, String>;
    fn exists(&self, path: &Path) -> Result<bool, String>;
    /// Exclusive, byte-verified publication; must never overwrite an existing file.
    fn copy_verified(&self, source: &Path, target: &Path) -> Result<(), String>;
}
mod publication;
pub use publication::publish_comment;

enum Outcome {
    Loaded(PathBuf, Result<StoredComments, String>),
    Saved {
        published: Option<Box<Comment>>,
        result: Result<StoredComments, String>,
    },
}
pub struct CommentSession {
    repository: Arc<dyn CommentRepository>,
    recorder: Arc<dyn CommentRecorder>,
    media: Arc<dyn CommentMediaStore>,
    root: Option<PathBuf>,
    catalog: CommentCatalog,
    revision: Option<Vec<u8>>,
    request: Option<PathBuf>,
    worker: Option<mpsc::Receiver<Outcome>>,
    device_worker: Option<mpsc::Receiver<Result<Vec<InputDevice>, String>>>,
    devices: Vec<InputDevice>,
    device: String,
    snapshot: CaptureSnapshot,
    target: Option<PhotoTarget>,
    open: bool,
    error: String,
    message: String,
    saving: bool,
    loaded: bool,
    capture_owned: bool,
    previous_id: String,
    pending_start: bool,
    published: Option<Comment>,
    saved: bool,
    changed: bool,
}
impl CommentSession {
    pub fn new(
        repository: Arc<dyn CommentRepository>,
        recorder: Arc<dyn CommentRecorder>,
        media: Arc<dyn CommentMediaStore>,
    ) -> Self {
        Self {
            repository,
            recorder,
            media,
            root: None,
            catalog: CommentCatalog::default(),
            revision: None,
            request: None,
            worker: None,
            device_worker: None,
            devices: Vec::new(),
            device: String::new(),
            snapshot: CaptureSnapshot::default(),
            target: None,
            open: false,
            error: String::new(),
            message: String::new(),
            saving: false,
            loaded: false,
            capture_owned: false,
            previous_id: String::new(),
            pending_start: false,
            published: None,
            saved: false,
            changed: false,
        }
    }
    pub fn devices(&self) -> &[InputDevice] {
        &self.devices
    }
    pub fn device(&self) -> &str {
        &self.device
    }
    pub fn snapshot(&self) -> &CaptureSnapshot {
        &self.snapshot
    }
    pub fn target(&self) -> Option<&PhotoTarget> {
        self.target.as_ref()
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn is_saving(&self) -> bool {
        self.saving
    }
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
    pub fn error(&self) -> &str {
        &self.error
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn report_error(&mut self, error: String) {
        self.error = error;
    }
    pub fn show(&mut self) {
        if self.target.is_some() {
            self.open = true;
        }
    }
    pub fn can_start(&self) -> bool {
        self.open
            && self.loaded
            && !self.saving
            && self.published.is_none()
            && matches!(
                self.snapshot.phase,
                CapturePhase::Idle | CapturePhase::Ready | CapturePhase::Error
            )
    }
    pub fn set_device(&mut self, device: String) -> Result<(), String> {
        if self.saving
            || !matches!(
                self.snapshot.phase,
                CapturePhase::Idle | CapturePhase::Ready | CapturePhase::Error
            )
        {
            return Err("Mikrofon kann während einer Aufnahme nicht gewechselt werden".into());
        }
        if !device.is_empty() && !self.devices.iter().any(|d| d.id == device) {
            return Err("Mikrofon gehört nicht zur verfügbaren Geräteliste".into());
        }
        self.device = device;
        Ok(())
    }
    pub fn load(&mut self, root: PathBuf) {
        if self.has_unsaved() || self.saving {
            return;
        }
        if self.root.as_ref() != Some(&root) {
            self.discard();
        }
        self.root = Some(root.clone());
        self.catalog = CommentCatalog::default();
        self.revision = None;
        self.loaded = false;
        self.error.clear();
        self.request = Some(root);
        self.changed = true;
    }
    pub fn reload(&mut self) {
        if let Some(root) = self.root.clone() {
            self.load(root);
        }
    }
    pub fn catalog(&self) -> &CommentCatalog {
        &self.catalog
    }
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }
    pub fn is_loading(&self) -> bool {
        !self.loaded && (self.request.is_some() || self.worker.is_some())
    }
    pub fn has_unsaved(&self) -> bool {
        self.capture_owned && !self.saved
    }
    pub fn busy(&self) -> bool {
        self.saving || self.is_loading() || self.device_worker.is_some()
    }
    pub fn project(&self, media: Media) -> Media {
        let comment = self
            .root
            .as_ref()
            .and_then(|root| media.path().strip_prefix(root).ok())
            .and_then(|p| p.to_str())
            .and_then(|p| LibraryFile::new(p.into()).ok())
            .and_then(|p| self.catalog.by_audio(&p));
        media.place_comment(comment)
    }
    pub fn comment_media(&self) -> Vec<Media> {
        let Some(root) = &self.root else {
            return Vec::new();
        };
        self.catalog
            .entries()
            .values()
            .filter_map(|comment| {
                let path = root.join(comment.audio().as_str());
                let group = Path::new(comment.audio().as_str())
                    .parent()?
                    .to_string_lossy()
                    .into_owned();
                Media::from_audio_asset(path, group).map(|media| media.place_comment(Some(comment)))
            })
            .collect()
    }
    pub fn for_photo(&self, photo: &LibraryFile) -> Vec<&Comment> {
        self.catalog.for_photo(photo)
    }
    pub fn open_photo(&mut self, target: PhotoTarget) -> Result<(), String> {
        if self.has_unsaved() || self.saving {
            return Err("Bitte die aktuelle Aufnahme zuerst speichern oder verwerfen".into());
        }
        if self.root.as_ref() != Some(&target.root) {
            return Err("Kommentar gehört nicht zur geöffneten Bibliothek".into());
        }
        if !Media::from_path(PathBuf::from(target.file.as_str()), String::new())
            .is_some_and(|m| m.is_photo())
        {
            return Err("Audiokommentare können nur einem Foto zugeordnet werden".into());
        }
        self.recorder.command(CaptureCommand::Discard)?;
        self.snapshot = CaptureSnapshot::default();
        self.target = Some(target);
        self.open = true;
        self.saved = false;
        self.capture_owned = false;
        self.published = None;
        self.message.clear();
        self.refresh_devices();
        Ok(())
    }
    pub fn refresh_devices(&mut self) {
        if self.device_worker.is_some() {
            return;
        }
        let recorder = self.recorder.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.device_worker = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(recorder.devices());
        });
    }
    pub fn start(&mut self, player: &dyn crate::diary::AudioPlayer) -> Result<(), String> {
        if !self.can_start() || self.target.is_none() {
            return Err("Aufnahme ist noch nicht bereit".into());
        }
        if player.transport().playing {
            player.control(crate::diary::AudioControl::Toggle)?;
        }
        self.previous_id = self.recorder.snapshot().id;
        self.recorder.start(&self.device)?;
        self.pending_start = true;
        self.capture_owned = true;
        self.saved = false;
        self.error.clear();
        self.message.clear();
        self.snapshot = CaptureSnapshot {
            phase: CapturePhase::Preparing,
            ..Default::default()
        };
        Ok(())
    }
    pub fn command(&mut self, command: CaptureCommand) {
        if matches!(command, CaptureCommand::Discard) {
            self.discard();
            return;
        }
        let valid = !self.saving
            && self.capture_owned
            && match command {
                CaptureCommand::Pause => self.snapshot.phase == CapturePhase::Recording,
                CaptureCommand::Resume => self.snapshot.phase == CapturePhase::Paused,
                CaptureCommand::Finish => matches!(
                    self.snapshot.phase,
                    CapturePhase::Recording | CapturePhase::Paused
                ),
                CaptureCommand::Preview => self.snapshot.phase == CapturePhase::Ready,
                CaptureCommand::Discard => unreachable!(),
            };
        if !valid {
            self.error = "Befehl im aktuellen Aufnahmezustand nicht möglich".into();
            return;
        }
        if let Err(error) = self.recorder.command(command) {
            self.error = error;
        }
    }
    pub fn discard(&mut self) {
        if self.saving {
            return;
        }
        if let Err(error) = self.recorder.command(CaptureCommand::Discard) {
            self.error = error;
            return;
        }
        self.capture_owned = false;
        self.pending_start = false;
        self.open = false;
        self.target = None;
        self.snapshot = Default::default();
        if self.published.is_some() {
            self.message =
                "Audiodatei bleibt erhalten; die Zuordnung wurde nicht gespeichert".into();
        }
        self.published = None;
    }
    pub fn save(&mut self) -> Result<(), String> {
        if self.saving || self.worker.is_some() || !self.loaded {
            return Err("Speicherauftrag noch nicht möglich".into());
        }
        let target = self.target.clone().ok_or("Foto fehlt")?;
        let take = self
            .snapshot
            .take
            .clone()
            .ok_or("Aufnahme bitte zuerst beenden")?;
        if !self.capture_owned || self.saved || self.snapshot.phase != CapturePhase::Ready {
            return Err("Keine fertige neue Aufnahme".into());
        }
        if self.snapshot.playing {
            self.command(CaptureCommand::Preview);
        }
        let repository = self.repository.clone();
        let media = self.media.clone();
        let capture_lifetime = self.recorder.clone();
        let existing = self.published.clone();
        let retry = existing.is_some();
        let mut catalog = self.catalog.clone();
        let mut revision = self.revision.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.worker = Some(rx);
        self.saving = true;
        self.error.clear();
        self.message = "Aufnahme prüfen, kopieren und zuordnen …".into();
        std::thread::spawn(move || {
            // Keep native staging alive even if the owning window is unexpectedly dropped.
            let _capture_lifetime = capture_lifetime;
            let published = match existing {
                Some(comment) => Ok(comment),
                None => publish_comment(media.as_ref(), &target, &take),
            };
            let outcome = match published {
                Err(error) => Outcome::Saved {
                    published: None,
                    result: Err(error),
                },
                Ok(comment) => {
                    let result = (|| {
                        if retry {
                            let current = repository.load(&target.root)?;
                            catalog = current.catalog;
                            revision = current.revision;
                        }
                        catalog.insert(comment.clone())?;
                        let revision =
                            repository.save(&target.root, &catalog, revision.as_deref())?;
                        Ok(StoredComments {
                            catalog,
                            revision: Some(revision),
                        })
                    })();
                    Outcome::Saved {
                        published: Some(Box::new(comment)),
                        result,
                    }
                }
            };
            let _ = tx.send(outcome);
        });
        Ok(())
    }
    pub fn poll(&mut self) {
        if self.open && self.capture_owned {
            let snapshot = self.recorder.snapshot();
            if !self.pending_start
                || snapshot.id != self.previous_id
                || snapshot.phase == CapturePhase::Error
            {
                self.pending_start = false;
                self.snapshot = snapshot;
            }
        }
        if let Some(worker) = &self.device_worker {
            match worker.try_recv() {
                Ok(result) => {
                    self.device_worker = None;
                    match result {
                        Ok(devices) => self.devices = devices,
                        Err(error) => self.error = error,
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.device_worker = None;
                    self.error = "Geräteabfrage unerwartet beendet".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(worker) = &self.worker {
            match worker.try_recv() {
                Ok(outcome) => {
                    self.worker = None;
                    match outcome {
                        Outcome::Loaded(root, result) if self.root.as_ref() == Some(&root) => {
                            match result {
                                Ok(stored) => {
                                    self.catalog = stored.catalog;
                                    self.revision = stored.revision;
                                    self.loaded = true;
                                    self.error.clear();
                                    self.changed = true;
                                }
                                Err(error) => {
                                    self.error = error;
                                    self.loaded = false;
                                }
                            }
                        }
                        Outcome::Loaded(_, _) => {}
                        Outcome::Saved { published, result } => {
                            self.saving = false;
                            self.published = published.map(|comment| *comment);
                            match result {
                                Ok(stored) => {
                                    self.catalog = stored.catalog;
                                    self.revision = stored.revision;
                                    self.saved = true;
                                    self.capture_owned = false;
                                    self.changed = true;
                                    self.message =
                                        "Audiokommentar gespeichert und mit dem Foto verknüpft"
                                            .into();
                                    self.recorder.command(CaptureCommand::Discard).ok();
                                    self.snapshot = Default::default();
                                    self.published = None;
                                }
                                Err(error) => {
                                    self.error = if self.published.is_some() {
                                        format!(
                                            "Audiodatei gespeichert, Zuordnung noch nicht gespeichert: {error}. Erneut speichern ist möglich."
                                        )
                                    } else {
                                        error
                                    };
                                }
                            }
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.worker = None;
                    self.saving = false;
                    self.error =
                        "Kommentarauftrag unerwartet beendet; vorhandene Dateien bleiben erhalten"
                            .into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.worker.is_none()
            && let Some(root) = self.request.take()
        {
            let repository = self.repository.clone();
            let (tx, rx) = mpsc::sync_channel(1);
            self.worker = Some(rx);
            std::thread::spawn(move || {
                let result = repository.load(&root);
                let _ = tx.send(Outcome::Loaded(root, result));
            });
        }
    }
}
impl Drop for CommentSession {
    fn drop(&mut self) {
        if !self.saving {
            self.recorder.command(CaptureCommand::Discard).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Default)]
    struct Fake {
        snapshot: Mutex<CaptureSnapshot>,
        catalog: Mutex<CommentCatalog>,
        copies: AtomicUsize,
        saves: AtomicUsize,
        commands: AtomicUsize,
    }
    impl crate::diary::AudioPlayer for Fake {
        fn revise(
            &self,
            _: &crate::domain::diary_timeline::DiaryTimeline,
            _: &crate::diary::VerificationProgress,
        ) -> Result<(), String> {
            Ok(())
        }
        fn open(
            &self,
            _: &crate::domain::diary_timeline::DiaryTimeline,
            _: &Path,
            _: &[Media],
        ) -> Result<(), String> {
            Err("unused".into())
        }
    }
    impl CommentRecorder for Fake {
        fn devices(&self) -> Result<Vec<InputDevice>, String> {
            Ok(Vec::new())
        }
        fn start(&self, _: &str) -> Result<(), String> {
            self.snapshot.lock().unwrap().id = "new".into();
            Ok(())
        }
        fn command(&self, _: CaptureCommand) -> Result<(), String> {
            self.commands.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn snapshot(&self) -> CaptureSnapshot {
            self.snapshot.lock().unwrap().clone()
        }
    }
    impl CommentRepository for Fake {
        fn load(&self, _: &Path) -> Result<StoredComments, String> {
            Ok(StoredComments {
                catalog: self.catalog.lock().unwrap().clone(),
                revision: None,
            })
        }
        fn save(
            &self,
            _: &Path,
            catalog: &CommentCatalog,
            _: Option<&[u8]>,
        ) -> Result<Vec<u8>, String> {
            if self.saves.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err("external revision".into());
            }
            *self.catalog.lock().unwrap() = catalog.clone();
            Ok(vec![1])
        }
    }
    impl CommentMediaStore for Fake {
        fn validate(&self, _: &PhotoTarget, _: &CapturedTake) -> Result<(), String> {
            Ok(())
        }
        fn photo_time(&self, _: &PhotoTarget) -> Result<CaptureTime, String> {
            Err("unknown".into())
        }
        fn exists(&self, _: &Path) -> Result<bool, String> {
            Ok(false)
        }
        fn copy_verified(&self, _: &Path, _: &Path) -> Result<(), String> {
            self.copies.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    fn wait(session: &mut CommentSession) {
        for _ in 0..1000 {
            session.poll();
            if !session.busy() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("worker did not finish");
    }
    #[test]
    fn failed_reference_save_retries_without_copying_or_losing_take() {
        let fake = Arc::new(Fake::default());
        let mut session = CommentSession::new(fake.clone(), fake.clone(), fake.clone());
        session.load("/library".into());
        wait(&mut session);
        session
            .open_photo(PhotoTarget {
                root: "/library".into(),
                file: LibraryFile::new("2008/09/a.jpg".into()).unwrap(),
                time: None,
            })
            .unwrap();
        wait(&mut session);
        session.start(fake.as_ref()).unwrap();
        *fake.snapshot.lock().unwrap() = CaptureSnapshot {
            phase: CapturePhase::Ready,
            id: "new".into(),
            take: Some(CapturedTake {
                id: "12345678-1234-1234-1234-123456789abc".into(),
                path: "/tmp/own.m4a".into(),
                recorded_at: "2026-11-12T15:30:00+01:00".into(),
            }),
            ..Default::default()
        };
        session.poll();
        session.save().unwrap();
        session.discard();
        assert!(session.open);
        wait(&mut session);
        assert!(session.error.contains("Zuordnung noch nicht"));
        assert!(session.has_unsaved());
        session.load("/different".into());
        assert_eq!(session.root.as_deref(), Some(Path::new("/library")));
        session.save().unwrap();
        wait(&mut session);
        assert!(!session.has_unsaved());
        assert_eq!(fake.copies.load(Ordering::SeqCst), 1);
        assert_eq!(session.catalog.entries().len(), 1);
        assert_eq!(session.comment_media()[0].group(), "2008/audio");
    }
    #[test]
    fn root_changes_discard_stale_load_results() {
        let fake = Arc::new(Fake::default());
        let mut session = CommentSession::new(fake.clone(), fake.clone(), fake);
        session.load("/a".into());
        session.poll();
        session.load("/b".into());
        wait(&mut session);
        assert_eq!(session.root.as_deref(), Some(Path::new("/b")));
        assert!(session.loaded);
    }
    #[test]
    fn workflow_guards_are_enforced_without_ui_and_reject_nonphoto_targets() {
        let fake = Arc::new(Fake::default());
        let mut session = CommentSession::new(fake.clone(), fake.clone(), fake.clone());
        assert!(session.start(fake.as_ref()).is_err());
        let baseline = fake.commands.load(Ordering::SeqCst);
        session.command(CaptureCommand::Preview);
        assert_eq!(fake.commands.load(Ordering::SeqCst), baseline);
        session.load("/library".into());
        wait(&mut session);
        assert!(
            session
                .open_photo(PhotoTarget {
                    root: "/library".into(),
                    file: LibraryFile::new("2008/audio/a.wav".into()).unwrap(),
                    time: None
                })
                .is_err()
        );
        assert!(!session.is_open());
        session
            .open_photo(PhotoTarget {
                root: "/library".into(),
                file: LibraryFile::new("2008/09/a.jpg".into()).unwrap(),
                time: None,
            })
            .unwrap();
        session.start(fake.as_ref()).unwrap();
        assert!(session.set_device("changed".into()).is_err());
        assert!(!session.can_start());
        let baseline = fake.commands.load(Ordering::SeqCst);
        session.command(CaptureCommand::Resume);
        assert_eq!(fake.commands.load(Ordering::SeqCst), baseline);
        session.command(CaptureCommand::Discard);
        assert!(!session.has_unsaved());
        assert!(!session.is_open());
    }
    struct TraceStore(Mutex<Vec<String>>);
    impl CommentMediaStore for TraceStore {
        fn validate(&self, _: &PhotoTarget, _: &CapturedTake) -> Result<(), String> {
            self.0.lock().unwrap().push("validate".into());
            Ok(())
        }
        fn photo_time(&self, _: &PhotoTarget) -> Result<CaptureTime, String> {
            self.0.lock().unwrap().push("date".into());
            Err("unknown photo date".into())
        }
        fn exists(&self, path: &Path) -> Result<bool, String> {
            self.0
                .lock()
                .unwrap()
                .push(path.file_name().unwrap().to_string_lossy().into_owned());
            Ok(!path.to_string_lossy().ends_with("_002.m4a"))
        }
        fn copy_verified(&self, _: &Path, target: &Path) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .push(format!("copy:{}", target.display()));
            Ok(())
        }
    }
    #[test]
    fn publication_policy_validates_before_io_and_selects_suffix_in_application() {
        let store = TraceStore(Mutex::new(Vec::new()));
        let target = PhotoTarget {
            root: "/library".into(),
            file: LibraryFile::new("2008/09/a.jpg".into()).unwrap(),
            time: None,
        };
        let mut take = CapturedTake {
            id: "invalid".into(),
            path: "/tmp/take.m4a".into(),
            recorded_at: "2026-11-12T15:30:00+01:00".into(),
        };
        assert!(publish_comment(&store, &target, &take).is_err());
        assert!(store.0.lock().unwrap().is_empty());
        take.id = "12345678-1234-1234-1234-123456789abc".into();
        let comment = publish_comment(&store, &target, &take).unwrap();
        assert_eq!(
            comment.audio().as_str(),
            "2026/audio/comment_20261112_153000_002.m4a"
        );
        assert!(comment.photo_time().is_none());
        assert_eq!(comment.logical_year(), "2008");
        let trace = store.0.lock().unwrap();
        assert_eq!(trace[0], "validate");
        assert_eq!(trace[1], "date");
        assert_eq!(trace.len(), 6);
        assert!(trace[5].starts_with("copy:"));
    }
    struct BlockingStore {
        entered: mpsc::SyncSender<()>,
        release: Mutex<mpsc::Receiver<()>>,
        fake: Arc<Fake>,
    }
    impl CommentMediaStore for BlockingStore {
        fn validate(&self, _: &PhotoTarget, _: &CapturedTake) -> Result<(), String> {
            Ok(())
        }
        fn photo_time(&self, _: &PhotoTarget) -> Result<CaptureTime, String> {
            Err("unknown".into())
        }
        fn exists(&self, _: &Path) -> Result<bool, String> {
            Ok(false)
        }
        fn copy_verified(&self, _: &Path, _: &Path) -> Result<(), String> {
            self.entered.send(()).map_err(|e| e.to_string())?;
            self.release
                .lock()
                .unwrap()
                .recv()
                .map_err(|e| e.to_string())?;
            self.fake.copies.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    #[test]
    fn saving_rejects_commands_and_dropping_owner_does_not_discard_staging_midcopy() {
        let fake = Arc::new(Fake::default());
        let (entered_tx, entered) = mpsc::sync_channel(1);
        let (release, release_rx) = mpsc::channel();
        let media = Arc::new(BlockingStore {
            entered: entered_tx,
            release: Mutex::new(release_rx),
            fake: fake.clone(),
        });
        let mut session = CommentSession::new(fake.clone(), fake.clone(), media);
        session.load("/library".into());
        wait(&mut session);
        session
            .open_photo(PhotoTarget {
                root: "/library".into(),
                file: LibraryFile::new("2008/09/a.jpg".into()).unwrap(),
                time: None,
            })
            .unwrap();
        wait(&mut session);
        session.start(fake.as_ref()).unwrap();
        *fake.snapshot.lock().unwrap() = CaptureSnapshot {
            phase: CapturePhase::Ready,
            id: "new".into(),
            take: Some(CapturedTake {
                id: "12345678-1234-1234-1234-123456789abc".into(),
                path: "/tmp/take.m4a".into(),
                recorded_at: "2026-11-12T15:30:00+01:00".into(),
            }),
            ..Default::default()
        };
        session.poll();
        session.save().unwrap();
        entered
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let commands = fake.commands.load(Ordering::SeqCst);
        session.command(CaptureCommand::Preview);
        session.command(CaptureCommand::Discard);
        assert!(session.is_saving());
        assert!(session.has_unsaved());
        assert!(session.is_open());
        assert_eq!(fake.commands.load(Ordering::SeqCst), commands);
        drop(session);
        assert_eq!(fake.commands.load(Ordering::SeqCst), commands);
        release.send(()).unwrap();
        for _ in 0..1000 {
            if fake.copies.load(Ordering::SeqCst) == 1 {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("copy did not finish after owner drop");
    }
}
