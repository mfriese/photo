//! Recording and reference persistence orchestration. No GUI, native or filesystem dependencies.
use crate::domain::{
    Media,
    comments::{Comment, CommentCatalog},
    tags::LibraryFile,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};
/// Bounded telemetry window: 30 envelope measurements/sec over approximately 32 sec.
pub const CAPTURE_WAVE_SAMPLES: usize = 960;
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
    pub position: f64,
    pub edit_failed: bool,
    pub wave_start: f64,
    pub wave_span: f64,
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
    Seek(f64),
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
}
pub struct StoredComments {
    pub catalog: CommentCatalog,
    pub revision: Option<Vec<u8>>,
}
pub struct RebuildBase {
    pub revision: Option<Vec<u8>>,
    pub warning: String,
}
#[derive(Clone, Debug, Default)]
pub struct RebuildProgress {
    pub checked: usize,
    pub phase: String,
}
/// Technical cache state; obsolete data never carries legacy reference values.
pub enum CommentCache {
    Current(StoredComments),
    Missing,
    Obsolete,
}
pub trait CommentRepository: Send + Sync + 'static {
    fn prepare_metadata_layout(
        &self,
        _root: &Path,
        _cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), String> {
        Ok(())
    }
    fn read_cache(&self, root: &Path) -> Result<CommentCache, String>;
    /// Return the complete authoritative photo record after the revision-guarded update.
    fn write_photo_record(&self, root: &Path, comment: &Comment) -> Result<CommentCatalog, String>;
    fn rebuild_base(&self, root: &Path) -> Result<RebuildBase, String> {
        let revision = match self.read_cache(root)? {
            CommentCache::Current(stored) => stored.revision,
            CommentCache::Missing => None,
            CommentCache::Obsolete => {
                return Err("Adapter benötigt die rohe Indexrevision zum Neuaufbau".into());
            }
        };
        Ok(RebuildBase {
            revision,
            warning: String::new(),
        })
    }
    fn scan_photo_records(
        &self,
        _root: &Path,
        _cancel: &std::sync::atomic::AtomicBool,
        _progress: &dyn Fn(RebuildProgress),
    ) -> Result<Vec<Comment>, String> {
        Err("Indexaufbau nicht verfügbar".into())
    }
    fn replace_index(
        &self,
        root: &Path,
        catalog: &CommentCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        self.save(root, catalog, expected)
    }
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

    fn exists(&self, path: &Path) -> Result<bool, String>;
    /// Exclusive, byte-verified publication; must never overwrite an existing file.
    fn copy_verified(&self, source: &Path, target: &Path) -> Result<(), String>;
}
mod index;
pub use index::{commit as commit_comment, load as load_catalog};
mod publication;
pub use publication::publish_comment;

enum Outcome {
    Rebuilt {
        root: PathBuf,
        generation: u64,
        result: Result<(StoredComments, String), String>,
    },
    Loaded {
        root: PathBuf,
        generation: u64,
        result: Result<StoredComments, String>,
    },
    Saved {
        published: Option<Box<Comment>>,
        result: Result<StoredComments, String>,
    },
}
#[derive(Clone, Copy)]
enum SaveIntent {
    Finish,
    Ready,
}
/// In-memory projection for the current photo; never probes audio on the UI thread.
#[derive(Default)]
pub struct PhotoAudio {
    pub items: Vec<Media>,
    pub group: String,
    pub year: String,
}
pub struct CommentSession {
    repository: Arc<dyn CommentRepository>,
    recorder: Arc<dyn CommentRecorder>,
    media: Arc<dyn CommentMediaStore>,
    root: Option<PathBuf>,
    catalog: CommentCatalog,
    revision: Option<Vec<u8>>,
    request: Option<PathBuf>,
    generation: u64,
    load_cancel: Arc<std::sync::atomic::AtomicBool>,
    worker: Option<mpsc::Receiver<Outcome>>,
    refresh_pending: bool,
    rebuilding: bool,
    rebuild_cancel: Arc<std::sync::atomic::AtomicBool>,
    rebuild_progress: RebuildProgress,
    rebuild_updates: Option<mpsc::Receiver<RebuildProgress>>,
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
    save_intent: Option<SaveIntent>,
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
            generation: 0,
            load_cancel: Default::default(),
            worker: None,
            refresh_pending: false,
            rebuilding: false,
            rebuild_cancel: Default::default(),
            rebuild_progress: Default::default(),
            rebuild_updates: None,
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
            save_intent: None,
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
            && !self.is_save_pending()
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
        self.rebuild_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.load_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.load_cancel = Default::default();
        self.generation += 1;
        self.refresh_pending = false;
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
    /// Reconcile individual records after the ordinary reload completes, on the same bounded worker.
    pub fn refresh_index(&mut self) {
        if self.root.is_some() && !self.has_unsaved() && !self.saving {
            self.refresh_pending = true;
        }
    }
    /// An incomplete/failed load must never erase derived tag memberships.
    pub fn authoritative_catalog(&self) -> Option<(&Path, &CommentCatalog)> {
        (self.loaded && !self.refresh_pending && !self.rebuilding)
            .then(|| (self.root.as_deref().unwrap(), &self.catalog))
    }
    pub fn is_rebuilding(&self) -> bool {
        self.rebuilding
    }
    pub fn can_rebuild(&self) -> bool {
        self.root.is_some()
            && !self.has_unsaved()
            && !self.saving
            && self.worker.is_none()
            && self.request.is_none()
            && !self.rebuilding
    }
    pub fn rebuild_progress(&self) -> &RebuildProgress {
        &self.rebuild_progress
    }
    pub fn cancel_rebuild(&mut self) {
        self.rebuild_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn rebuild(&mut self) -> Result<(), String> {
        if !self.can_rebuild() {
            return Err("Bitte laufende Aufnahme oder Kommentarauftrag zuerst abschließen".into());
        }
        let root = self.root.clone().ok_or("Keine Bibliothek geöffnet")?;
        let repository = self.repository.clone();
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.rebuild_cancel = cancel.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let (progress, updates) = mpsc::sync_channel(16);
        self.worker = Some(rx);
        self.rebuild_updates = Some(updates);
        self.rebuilding = true;
        self.error.clear();
        self.rebuild_progress = RebuildProgress {
            checked: 0,
            phase: "Kommentar-YAMLs werden vorbereitet …".into(),
        };
        let generation = self.generation;
        std::thread::spawn(move || {
            let result = index::rebuild(repository.as_ref(), &root, &cancel, &|update| {
                let _ = progress.try_send(update);
            });
            let _ = tx.send(Outcome::Rebuilt {
                root,
                generation,
                result,
            });
        });
        Ok(())
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
        self.refresh_pending
            || self.rebuilding
            || self.saving
            || self.is_loading()
            || self.device_worker.is_some()
    }
    pub fn project(&self, media: Media) -> Media {
        let comment = self
            .root
            .as_ref()
            .and_then(|root| media.path().strip_prefix(root).ok())
            .and_then(|p| p.to_str())
            .and_then(|p| LibraryFile::new(p.into()).ok())
            .and_then(|p| self.catalog.by_audio(&p));
        media.with_comment_recording(comment)
    }
    pub fn audio_for_photo(&self, path: &Path) -> PhotoAudio {
        let Some(root) = &self.root else {
            return PhotoAudio::default();
        };
        let Some(file) = path
            .strip_prefix(root)
            .ok()
            .and_then(Path::to_str)
            .and_then(|p| LibraryFile::new(p.into()).ok())
        else {
            return PhotoAudio::default();
        };
        let comments = self.catalog.for_photo(&file);
        let year = comments
            .first()
            .and_then(|c| c.reference_time())
            .map(|d| d.year().to_owned())
            .unwrap_or_else(|| {
                file.as_str()
                    .split(std::path::MAIN_SEPARATOR)
                    .next()
                    .unwrap_or("")
                    .to_owned()
            });
        let group = format!("photo/{}", file.as_str());
        let items = comments
            .into_iter()
            .filter_map(|comment| {
                let path = root.join(comment.audio().as_str());
                let group = Path::new(comment.audio().as_str())
                    .parent()?
                    .to_string_lossy()
                    .into_owned();
                Media::from_audio_asset(path, group)
                    .map(|m| m.with_comment_recording(Some(comment)))
            })
            .collect();
        PhotoAudio { items, group, year }
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
                Media::from_audio_asset(path, group)
                    .map(|media| media.with_comment_recording(Some(comment)))
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
            && !self.is_save_pending()
            && self.capture_owned
            && match command {
                CaptureCommand::Pause => self.snapshot.phase == CapturePhase::Recording,
                CaptureCommand::Resume => {
                    self.snapshot.phase == CapturePhase::Paused && self.published.is_none()
                }
                CaptureCommand::Finish => matches!(
                    self.snapshot.phase,
                    CapturePhase::Recording | CapturePhase::Paused
                ),
                CaptureCommand::Preview => {
                    matches!(
                        self.snapshot.phase,
                        CapturePhase::Paused | CapturePhase::Ready
                    ) && self.snapshot.seconds > 0.
                }
                CaptureCommand::Seek(position) => {
                    matches!(
                        self.snapshot.phase,
                        CapturePhase::Paused | CapturePhase::Ready
                    ) && crate::domain::audio_edit::AudioCursor::new(
                        position,
                        self.snapshot.seconds,
                    )
                    .is_ok()
                }
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
        self.save_intent = None;
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
    pub fn is_save_pending(&self) -> bool {
        self.save_intent.is_some()
    }
    /// Explicit save from the close confirmation: finalize first, then publish.
    pub fn request_save(&mut self) -> Result<(), String> {
        if self.saving || self.is_save_pending() {
            return Ok(());
        }
        if !self.has_unsaved() {
            return Err("Keine ungespeicherte Aufnahme vorhanden".into());
        }
        if self.snapshot.edit_failed {
            return Err("Der neue Abschnitt wurde nicht übernommen. Erneut aufnehmen oder mit Stop den bisherigen Stand bestätigen.".into());
        }
        match self.snapshot.phase {
            CapturePhase::Ready => self.save(),
            CapturePhase::Preparing => {
                self.save_intent = Some(SaveIntent::Finish);
                Ok(())
            }
            CapturePhase::Recording | CapturePhase::Paused => {
                self.recorder.command(CaptureCommand::Finish)?;
                self.save_intent = Some(SaveIntent::Ready);
                Ok(())
            }
            CapturePhase::Finishing => {
                self.save_intent = Some(SaveIntent::Ready);
                Ok(())
            }
            _ => Err("Aufnahme kann noch nicht gespeichert werden".into()),
        }
    }
    pub fn save(&mut self) -> Result<(), String> {
        if self.saving || self.is_save_pending() || self.worker.is_some() || !self.loaded {
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
                            let current = index::load(
                                repository.as_ref(),
                                &target.root,
                                &std::sync::atomic::AtomicBool::new(false),
                            )?;
                            catalog = current.catalog;
                            revision = current.revision;
                        }
                        catalog.insert(comment.clone())?;
                        index::commit(
                            repository.as_ref(),
                            &target.root,
                            &comment,
                            &catalog,
                            revision.as_deref(),
                        )
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
        if self.save_intent.is_some() && self.snapshot.edit_failed {
            self.save_intent = None;
            self.error = self.snapshot.message.clone();
        }
        if let Some(intent) = self.save_intent {
            match (intent, self.snapshot.phase) {
                (SaveIntent::Finish, CapturePhase::Recording | CapturePhase::Paused) => {
                    match self.recorder.command(CaptureCommand::Finish) {
                        Ok(()) => self.save_intent = Some(SaveIntent::Ready),
                        Err(error) => {
                            self.error = error;
                            self.save_intent = None;
                        }
                    }
                }
                (SaveIntent::Ready, CapturePhase::Paused) => {
                    if let Err(error) = self.recorder.command(CaptureCommand::Finish) {
                        self.error = error;
                        self.save_intent = None;
                    }
                }
                (_, CapturePhase::Ready) => {
                    self.save_intent = None;
                    if let Err(error) = self.save() {
                        self.error = error;
                    }
                }
                (_, CapturePhase::Error) => {
                    self.save_intent = None;
                    self.error = if self.snapshot.message.is_empty() {
                        "Aufnahme konnte nicht abgeschlossen werden".into()
                    } else {
                        self.snapshot.message.clone()
                    };
                }
                _ => {}
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
        if let Some(updates) = &self.rebuild_updates {
            for update in updates.try_iter().take(16) {
                self.rebuild_progress = update;
            }
        }
        if let Some(worker) = &self.worker {
            match worker.try_recv() {
                Ok(outcome) => {
                    self.worker = None;
                    match outcome {
                        Outcome::Rebuilt {
                            root,
                            generation,
                            result,
                        } => {
                            self.rebuilding = false;
                            self.rebuild_updates = None;
                            if self.root.as_ref() == Some(&root) && generation == self.generation {
                                match result {
                                    Ok((stored, message)) => {
                                        self.catalog = stored.catalog;
                                        self.revision = stored.revision;
                                        self.loaded = true;
                                        self.changed = true;
                                        self.message = message;
                                        self.error.clear();
                                    }
                                    Err(error) => {
                                        self.error = error;
                                    }
                                }
                            }
                        }
                        Outcome::Loaded {
                            root,
                            generation,
                            result,
                        } if self.root.as_ref() == Some(&root) && generation == self.generation => {
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
                        Outcome::Loaded { .. } => {}
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
                    self.refresh_pending = false;
                    self.saving = false;
                    self.rebuilding = false;
                    self.rebuild_updates = None;
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
            let generation = self.generation;
            let cancel = self.load_cancel.clone();
            std::thread::spawn(move || {
                let result = index::load(repository.as_ref(), &root, &cancel);
                let _ = tx.send(Outcome::Loaded {
                    root,
                    generation,
                    result,
                });
            });
        }
        if self.refresh_pending && self.request.is_none() && self.can_rebuild() {
            self.refresh_pending = false;
            if let Err(error) = self.rebuild() {
                self.error = error;
            }
        }
    }
}
impl Drop for CommentSession {
    fn drop(&mut self) {
        self.load_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.rebuild_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
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
        scans: AtomicUsize,
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
        fn scan_photo_records(
            &self,
            _: &Path,
            _: &std::sync::atomic::AtomicBool,
            _: &dyn Fn(RebuildProgress),
        ) -> Result<Vec<Comment>, String> {
            self.scans.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .catalog
                .lock()
                .unwrap()
                .entries()
                .values()
                .cloned()
                .collect())
        }
        fn replace_index(
            &self,
            _: &Path,
            catalog: &CommentCatalog,
            _: Option<&[u8]>,
        ) -> Result<Vec<u8>, String> {
            *self.catalog.lock().unwrap() = catalog.clone();
            Ok(vec![1])
        }
        fn write_photo_record(
            &self,
            _: &Path,
            comment: &Comment,
        ) -> Result<CommentCatalog, String> {
            let current = self.catalog.lock().unwrap();
            let mut record = CommentCatalog::default();
            for c in current.for_photo(comment.photo()) {
                record.insert(c.clone())?;
            }
            record.insert(comment.clone())?;
            Ok(record)
        }
        fn read_cache(&self, _: &Path) -> Result<CommentCache, String> {
            Ok(CommentCache::Current(StoredComments {
                catalog: self.catalog.lock().unwrap().clone(),
                revision: None,
            }))
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
    fn same_library_reload_rejects_old_results_and_serializes_workers() {
        struct Slow {
            calls: AtomicUsize,
            entered: mpsc::SyncSender<usize>,
            release: Mutex<mpsc::Receiver<()>>,
        }
        impl CommentRepository for Slow {
            fn read_cache(&self, _: &Path) -> Result<CommentCache, String> {
                let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
                self.entered.send(n).unwrap();
                self.release.lock().unwrap().recv().unwrap();
                Ok(CommentCache::Current(StoredComments {
                    catalog: Default::default(),
                    revision: Some(vec![n as u8]),
                }))
            }
            fn write_photo_record(&self, _: &Path, _: &Comment) -> Result<CommentCatalog, String> {
                panic!("load must not write sources")
            }
            fn save(
                &self,
                _: &Path,
                _: &CommentCatalog,
                _: Option<&[u8]>,
            ) -> Result<Vec<u8>, String> {
                panic!("current cache must not be rewritten")
            }
        }
        let (entered, events) = mpsc::sync_channel(2);
        let (release, gate) = mpsc::channel();
        let repository = Arc::new(Slow {
            calls: AtomicUsize::new(0),
            entered,
            release: Mutex::new(gate),
        });
        let fake = Arc::new(Fake::default());
        let mut session = CommentSession::new(repository, fake.clone(), fake);
        session.load("/library".into());
        assert!(!session.can_rebuild());
        session.poll();
        assert_eq!(
            events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            1
        );
        session.load("/library".into());
        session.take_changed();
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            session.poll();
            if let Ok(n) = events.try_recv() {
                assert_eq!(n, 2);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(!session.loaded);
        assert!(session.error().is_empty());
        assert!(!session.take_changed());
        release.send(()).unwrap();
        wait(&mut session);
        assert!(session.loaded);
        assert_eq!(session.revision, Some(vec![2]));
        assert!(session.error().is_empty());
    }
    #[test]
    fn photo_audio_projects_only_linked_comments_without_probing() {
        let fake = Arc::new(Fake::default());
        let photo = LibraryFile::new("2008/09/img_20080916_035811.jpg".into()).unwrap();
        let other = LibraryFile::new("2008/09/img_20080917_035811.jpg".into()).unwrap();
        for n in 1..=3 {
            let comment = Comment::new(
                format!("12345678-1234-1234-1234-{n:012}"),
                LibraryFile::new(format!(
                    "{}/audio/comments/com_{}1113_120000_{n:03}.m4a",
                    2024 + n,
                    2024 + n
                ))
                .unwrap(),
                if n < 3 { photo.clone() } else { other.clone() },
            )
            .unwrap();
            fake.catalog.lock().unwrap().insert(comment).unwrap();
        }
        let mut session = CommentSession::new(fake.clone(), fake.clone(), fake.clone());
        session.load("/library".into());
        wait(&mut session);
        let commands_before = fake.commands.load(Ordering::SeqCst);
        let audio = session.audio_for_photo(&Path::new("/library").join(photo.as_str()));
        assert_eq!(audio.year, "2008");
        assert_eq!(audio.group, format!("photo/{}", photo.as_str()));
        assert_eq!(audio.items.len(), 2);
        assert!(
            audio
                .items
                .iter()
                .all(|m| m.group().ends_with("/audio/comments") && m.has_recording_time())
        );
        let mut draft = crate::diary::GalleryAudio::default();
        draft.ensure(&audio.items, Path::new("/library"), &audio.group);
        assert_eq!(draft.timeline().unwrap().clips().len(), 2);
        let next = session.audio_for_photo(&Path::new("/library").join(other.as_str()));
        assert_eq!(next.items.len(), 1);
        assert!(!audio.items.iter().any(|a| a.path() == next.items[0].path()));
        let empty = session.audio_for_photo(Path::new("/library/2008/09/no-comments.jpg"));
        assert!(empty.items.is_empty());
        assert_eq!(empty.year, "2008");
        assert!(
            session
                .audio_for_photo(Path::new("/other/2008/a.jpg"))
                .items
                .is_empty()
        );
        assert_eq!(fake.commands.load(Ordering::SeqCst), commands_before);
    }
    #[test]
    fn refresh_waits_for_load_then_rebuilds_once_and_never_publishes_an_empty_loading_catalog() {
        let fake = Arc::new(Fake::default());
        let mut session = CommentSession::new(fake.clone(), fake.clone(), fake.clone());
        session.load("/library".into());
        assert!(session.authoritative_catalog().is_none());
        session.refresh_index();
        wait(&mut session);
        assert_eq!(fake.scans.load(Ordering::SeqCst), 1);
        assert!(session.authoritative_catalog().is_some());
        for _ in 0..5 {
            session.poll();
        }
        assert_eq!(fake.scans.load(Ordering::SeqCst), 1);
        session.load("/other".into());
        assert!(session.authoritative_catalog().is_none());
        wait(&mut session);
        assert_eq!(fake.scans.load(Ordering::SeqCst), 1);
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
        assert_eq!(session.comment_media()[0].group(), "2026/audio/comments");
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
                    file: LibraryFile::new("2008/audio/a.wav".into()).unwrap()
                })
                .is_err()
        );
        assert!(!session.is_open());
        session
            .open_photo(PhotoTarget {
                root: "/library".into(),
                file: LibraryFile::new("2008/09/a.jpg".into()).unwrap(),
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

        fn exists(&self, path: &Path) -> Result<bool, String> {
            self.0
                .lock()
                .unwrap()
                .push(path.file_name().unwrap().to_string_lossy().into_owned());
            Ok(path.extension().is_some_and(|ext| ext == "m4a")
                && !path.to_string_lossy().ends_with("_002.m4a"))
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
            "2026/audio/comments/com_20261112_153000_002.m4a"
        );

        assert_eq!(comment.photo_year(), "2008");
        let trace = store.0.lock().unwrap();
        assert_eq!(trace[0], "validate");
        assert_eq!(trace[1], "com_20261112_153000.m4a");
        assert_eq!(trace.len(), 5);
        assert!(trace[4].starts_with("copy:"));
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
    #[test]
    fn close_save_finishes_a_paused_take_and_preserves_retry_on_storage_failure() {
        let fake = Arc::new(Fake::default());
        let mut session = CommentSession::new(fake.clone(), fake.clone(), fake.clone());
        session.load("/library".into());
        wait(&mut session);
        session
            .open_photo(PhotoTarget {
                root: "/library".into(),
                file: LibraryFile::new("2008/09/a.jpg".into()).unwrap(),
            })
            .unwrap();
        wait(&mut session);
        session.start(fake.as_ref()).unwrap();
        *fake.snapshot.lock().unwrap() = CaptureSnapshot {
            phase: CapturePhase::Paused,
            id: "new".into(),
            ..Default::default()
        };
        session.poll();
        session.request_save().unwrap();
        assert!(session.is_save_pending());
        assert!(!session.can_start());
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
        wait(&mut session);
        assert!(session.has_unsaved());
        assert!(!session.is_save_pending());
        assert!(session.error().contains("Zuordnung noch nicht"));
        session.request_save().unwrap();
        wait(&mut session);
        assert!(!session.has_unsaved());
        assert_eq!(fake.copies.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn seek_and_preview_are_only_allowed_on_valid_paused_positions() {
        let fake = Arc::new(Fake::default());
        let mut session = CommentSession::new(fake.clone(), fake.clone(), fake.clone());
        session.load("/library".into());
        wait(&mut session);
        session
            .open_photo(PhotoTarget {
                root: "/library".into(),
                file: LibraryFile::new("2008/09/a.jpg".into()).unwrap(),
            })
            .unwrap();
        wait(&mut session);
        session.start(fake.as_ref()).unwrap();
        *fake.snapshot.lock().unwrap() = CaptureSnapshot {
            phase: CapturePhase::Paused,
            id: "new".into(),
            seconds: 10.,
            ..Default::default()
        };
        session.poll();
        let initial = fake.commands.load(Ordering::SeqCst);
        session.command(CaptureCommand::Seek(5.));
        session.command(CaptureCommand::Preview);
        assert_eq!(fake.commands.load(Ordering::SeqCst), initial + 2);
        for value in [-1., 11., f64::NAN] {
            session.command(CaptureCommand::Seek(value));
        }
        assert_eq!(fake.commands.load(Ordering::SeqCst), initial + 2);
        *fake.snapshot.lock().unwrap() = CaptureSnapshot {
            phase: CapturePhase::Recording,
            id: "new".into(),
            seconds: 10.,
            ..Default::default()
        };
        session.poll();
        session.command(CaptureCommand::Seek(5.));
        assert_eq!(fake.commands.load(Ordering::SeqCst), initial + 2);
    }
    #[test]
    fn failed_replacement_cancels_auto_save_until_the_user_confirms_old_version() {
        let fake = Arc::new(Fake::default());
        let mut session = CommentSession::new(fake.clone(), fake.clone(), fake.clone());
        session.load("/library".into());
        wait(&mut session);
        session
            .open_photo(PhotoTarget {
                root: "/library".into(),
                file: LibraryFile::new("2008/09/a.jpg".into()).unwrap(),
            })
            .unwrap();
        wait(&mut session);
        session.start(fake.as_ref()).unwrap();
        *fake.snapshot.lock().unwrap() = CaptureSnapshot {
            phase: CapturePhase::Finishing,
            id: "new".into(),
            seconds: 10.,
            ..Default::default()
        };
        session.poll();
        session.request_save().unwrap();
        *fake.snapshot.lock().unwrap() = CaptureSnapshot {
            phase: CapturePhase::Paused,
            id: "new".into(),
            seconds: 10.,
            edit_failed: true,
            message: "merge failed".into(),
            ..Default::default()
        };
        session.poll();
        assert!(!session.is_save_pending());
        assert!(session.request_save().is_err());
        assert_eq!(fake.copies.load(Ordering::SeqCst), 0);
        assert!(session.has_unsaved());
    }
}
