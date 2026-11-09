//! Bounded mailbox: at most one active operation, latest save, and latest load.
use super::{CatalogRepository, TagCatalog};
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
};
pub(super) enum Request {
    Load {
        root: PathBuf,
        generation: u64,
    },
    Save {
        root: PathBuf,
        generation: u64,
        revision: u64,
        catalog: Arc<TagCatalog>,
    },
}
pub(super) enum Event {
    Loaded {
        generation: u64,
        result: Result<TagCatalog, String>,
    },
    Saved {
        generation: u64,
        revision: u64,
        result: Result<(), String>,
    },
}
#[derive(Default)]
struct Mailbox {
    load: Option<Request>,
    save: Option<Request>,
    stopping: bool,
}
struct Loaded {
    root: PathBuf,
    generation: u64,
    version: Option<Vec<u8>>,
}
pub(super) struct Worker {
    inbox: Arc<(Mutex<Mailbox>, Condvar)>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn new(repository: Arc<dyn CatalogRepository>) -> (Self, mpsc::Receiver<Event>) {
        let inbox = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let shared = inbox.clone();
        let (events, receiver) = mpsc::sync_channel(16);
        let handle = thread::spawn(move || {
            let mut loaded: Option<Loaded> = None;
            loop {
                let request = {
                    let (lock, wake) = &*shared;
                    let mut mailbox = lock.lock().unwrap();
                    while mailbox.save.is_none() && mailbox.load.is_none() && !mailbox.stopping {
                        mailbox = wake.wait(mailbox).unwrap();
                    }
                    if let Some(save) = mailbox.save.take() {
                        save
                    } else if mailbox.stopping {
                        break;
                    } else {
                        mailbox.load.take().unwrap()
                    }
                };
                match request {
                    Request::Load { root, generation } => {
                        let result = repository.load(&root).map(|stored| {
                            loaded = Some(Loaded {
                                root,
                                generation,
                                version: stored.revision,
                            });
                            stored.catalog
                        });
                        if result.is_err() {
                            loaded = None;
                        }
                        let _ = events.send(Event::Loaded { generation, result });
                    }
                    Request::Save {
                        root,
                        generation,
                        revision,
                        catalog,
                    } => {
                        let result = match &mut loaded {
                            Some(active)
                                if active.root == root && active.generation == generation =>
                            {
                                repository
                                    .save(&root, &catalog, active.version.as_deref())
                                    .map(|new| active.version = Some(new))
                            }
                            _ => Err("Tag-Bibliothek ist nicht geladen".into()),
                        };
                        let _ = events.send(Event::Saved {
                            generation,
                            revision,
                            result,
                        });
                    }
                }
            }
        });
        (
            Self {
                inbox,
                handle: Some(handle),
            },
            receiver,
        )
    }
    pub fn submit(&self, request: Request) -> Result<(), String> {
        if self
            .handle
            .as_ref()
            .is_none_or(thread::JoinHandle::is_finished)
        {
            return Err("Tag-Dienst unerwartet beendet".into());
        }
        let (lock, wake) = &*self.inbox;
        let mut mailbox = lock.lock().map_err(|_| "Tag-Dienst wurde unterbrochen")?;
        if mailbox.stopping {
            return Err("Tag-Dienst beendet".into());
        }
        match &request {
            Request::Load { .. } => mailbox.load = Some(request),
            Request::Save { .. } => mailbox.save = Some(request),
        };
        wake.notify_one();
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.inbox;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.stopping = true;
            mailbox.load = None;
            wake.notify_one();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
