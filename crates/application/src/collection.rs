//! One active collection scan; newer requests replace only the pending request.
use crate::{domain::Media, ports::MediaRepository};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
struct Request {
    repository: Arc<dyn MediaRepository>,
    root: PathBuf,
}
struct Active {
    root: PathBuf,
    receiver: mpsc::Receiver<(Vec<Media>, usize)>,
    cancel: Arc<AtomicBool>,
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
}
impl CollectionSession {
    pub fn request(&mut self, repository: Arc<dyn MediaRepository>, root: PathBuf) {
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Relaxed);
        }
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
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let root = request.root;
        self.active = Some(Active {
            root: root.clone(),
            receiver,
            cancel,
        });
        std::thread::spawn(move || {
            let _ = sender.send(request.repository.scan_cancellable(&root, &stop));
        });
    }
    pub fn is_loading(&self) -> bool {
        self.active.is_some()
    }
    pub fn poll(&mut self) -> Option<Result<Collection, String>> {
        let active = self.active.as_ref()?;
        let result = match active.receiver.try_recv() {
            Ok(value) => Ok(value),
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err(
                "Sammlung konnte nicht geladen werden: Hintergrundauftrag unerwartet beendet"
                    .into(),
            ),
        };
        let obsolete = active.cancel.load(Ordering::Relaxed);
        let root = self.active.take().unwrap().root;
        if obsolete {
            self.launch();
            return None;
        }
        Some(result.map(|(items, unreadable)| Collection {
            root,
            items,
            unreadable,
        }))
    }
}
impl Drop for CollectionSession {
    fn drop(&mut self) {
        if let Some(active) = &self.active {
            active.cancel.store(true, Ordering::Relaxed);
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
        assert!(!session.is_loading());
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
