//! Explicit gallery removal: reversible, per-file results and cooperative cancellation.
use crate::domain::{Media, tags::LibraryFile};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
pub trait MediaTrash: Send + Sync + 'static {
    /// Validate containment, regular file/ancestors and move only this media to the OS Trash.
    /// Never remove metadata or fall back to permanent deletion.
    fn trash(&self, root: &Path, file: &LibraryFile) -> Result<(), String>;
}
pub struct Outcome {
    pub root: PathBuf,
    pub requested: usize,
    pub files: Vec<(LibraryFile, Result<(), String>)>,
}
#[derive(Default)]
pub struct TrashSession {
    receiver: Option<mpsc::Receiver<Outcome>>,
    cancel: Arc<AtomicBool>,
}
impl TrashSession {
    pub fn running(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn start(
        &mut self,
        port: Arc<dyn MediaTrash>,
        root: PathBuf,
        files: Vec<LibraryFile>,
    ) -> Result<(), String> {
        if self.running() {
            return Err("Papierkorbaktion läuft bereits".into());
        }
        let files: std::collections::BTreeSet<_> = files.into_iter().collect();
        if files.is_empty() {
            return Err("Keine Medien ausgewählt".into());
        }
        for file in &files {
            if !Media::from_path(file.as_str().into(), String::new())
                .is_some_and(|m| m.is_photo() || m.is_video())
            {
                return Err("Diese Aktion unterstützt nur Fotos und Videos".into());
            }
        }
        let (tx, rx) = mpsc::sync_channel(1);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            let mut outcome = Outcome {
                requested: files.len(),
                root,
                files: vec![],
            };
            for file in files {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let result = port.trash(&outcome.root, &file);
                outcome.files.push((file, result));
            }
            let _ = tx.send(outcome);
        });
        self.receiver = Some(rx);
        Ok(())
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn poll(&mut self) -> Option<Result<Outcome, String>> {
        match self.receiver.as_ref()?.try_recv() {
            Ok(outcome) => {
                self.receiver = None;
                Some(Ok(outcome))
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.receiver = None;
                Some(Err(
                    "Papierkorbauftrag unerwartet beendet; Bibliothek bitte aktualisieren".into(),
                ))
            }
        }
    }
}
impl Drop for TrashSession {
    fn drop(&mut self) {
        self.cancel();
    }
}
/// Project confirmed removals into the current in-memory collection, never rescan the library.
pub struct Removal {
    pub removed: std::collections::BTreeSet<PathBuf>,
    pub selected: Option<usize>,
}
pub fn apply_outcome(
    items: &mut Vec<Media>,
    selected: Option<usize>,
    root: &Path,
    outcome: &Outcome,
) -> Removal {
    if root != outcome.root {
        return Removal {
            removed: Default::default(),
            selected,
        };
    }
    let selected_path = selected
        .and_then(|index| items.get(index))
        .map(|media| media.path().to_owned());
    let removed = outcome
        .files
        .iter()
        .filter(|(_, result)| result.is_ok())
        .map(|(file, _)| root.join(file.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    items.retain(|media| !removed.contains(media.path()));
    let selected =
        selected_path.and_then(|path| items.iter().position(|media| media.path() == path));
    Removal { removed, selected }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(std::sync::Mutex<Vec<LibraryFile>>);
    impl MediaTrash for Fake {
        fn trash(&self, _: &Path, file: &LibraryFile) -> Result<(), String> {
            self.0.lock().unwrap().push(file.clone());
            if file.as_str().contains("failed") {
                Err("unavailable".into())
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn batch_deduplicates_and_reports_partial_failure_without_permanent_removal() {
        let port = Arc::new(Fake(Default::default()));
        let a = LibraryFile::new("2026/11/a.jpg".into()).unwrap();
        let b = LibraryFile::new("2026/11/failed.jpg".into()).unwrap();
        let mut session = TrashSession::default();
        session
            .start(port.clone(), "/library".into(), vec![a.clone(), a, b])
            .unwrap();
        assert!(
            session
                .start(port.clone(), "/other".into(), vec![])
                .is_err()
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let result = loop {
            if let Some(outcome) = session.poll() {
                break outcome.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(result.files.len(), 2);
        assert_eq!(result.files.iter().filter(|(_, r)| r.is_ok()).count(), 1);
        assert_eq!(port.0.lock().unwrap().len(), 2);
        assert!(!session.running());
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    struct Blocking {
        started: std::sync::Mutex<mpsc::Sender<()>>,
        release: std::sync::Mutex<mpsc::Receiver<()>>,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl MediaTrash for Blocking {
        fn trash(&self, _: &Path, _: &LibraryFile) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.started.lock().unwrap().send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
            Ok(())
        }
    }
    #[test]
    fn cancel_preserves_completed_acknowledgement_and_starts_no_next_file() {
        let (started, ready) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let port = Arc::new(Blocking {
            started: std::sync::Mutex::new(started),
            release: std::sync::Mutex::new(wait),
            calls: Default::default(),
        });
        let mut session = TrashSession::default();
        session
            .start(
                port.clone(),
                "/library".into(),
                vec![
                    LibraryFile::new("a.jpg".into()).unwrap(),
                    LibraryFile::new("b.jpg".into()).unwrap(),
                ],
            )
            .unwrap();
        ready
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        session.cancel();
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let outcome = loop {
            if let Some(result) = session.poll() {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(outcome.requested, 2);
        assert_eq!(outcome.files.len(), 1);
        assert!(outcome.files[0].1.is_ok());
        assert_eq!(port.calls.load(Ordering::Relaxed), 1);
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    #[test]
    fn only_successes_are_removed_and_surviving_viewer_indices_are_remapped() {
        let root = Path::new("/memory-only");
        let mut items = ["a.jpg", "b.jpg", "c.jpg"]
            .map(|name| {
                Media::from_library_path(root.join("2026/11").join(name), "2026/11".into()).unwrap()
            })
            .to_vec();
        let outcome = Outcome {
            root: root.into(),
            requested: 3,
            files: vec![
                (LibraryFile::new("2026/11/a.jpg".into()).unwrap(), Ok(())),
                (
                    LibraryFile::new("2026/11/c.jpg".into()).unwrap(),
                    Err("test failure".into()),
                ),
            ],
        };
        let ignored = apply_outcome(&mut items, Some(1), Path::new("/other"), &outcome);
        assert!(ignored.removed.is_empty());
        assert_eq!(items.len(), 3);
        let result = apply_outcome(&mut items, Some(1), root, &outcome);
        assert_eq!(result.selected, Some(0));
        assert_eq!(items.len(), 2);
        assert!(items[0].path().ends_with("b.jpg"));
        assert!(items[1].path().ends_with("c.jpg"));
        assert_eq!(result.removed, [root.join("2026/11/a.jpg")].into());
        let result = apply_outcome(&mut items, Some(3928), root, &outcome);
        assert!(result.selected.is_none());
    }
}
