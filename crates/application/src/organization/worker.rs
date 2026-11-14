//! Bounded mailbox: at most one active operation, latest save, and latest load.
use super::{CatalogRepository, TagCatalog};
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
};
pub(super) enum Request {
    Rebuild {
        root: PathBuf,
        generation: u64,
    },
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
    load: Option<(Request, Arc<std::sync::atomic::AtomicBool>)>,
    active_read: Option<Arc<std::sync::atomic::AtomicBool>>,
    save: Option<Request>,
    stopping: bool,
}
struct Loaded {
    root: PathBuf,
    generation: u64,
    version: Option<Vec<u8>>,
    catalog: TagCatalog,
    seeded: bool,
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
                let (request, cancel) = {
                    let (lock, wake) = &*shared;
                    let mut mailbox = lock.lock().unwrap();
                    while mailbox.save.is_none() && mailbox.load.is_none() && !mailbox.stopping {
                        mailbox = wake.wait(mailbox).unwrap();
                    }
                    if let Some(save) = mailbox.save.take() {
                        (save, Arc::new(std::sync::atomic::AtomicBool::new(false)))
                    } else if mailbox.stopping {
                        break;
                    } else {
                        let (request, cancel) = mailbox.load.take().unwrap();
                        mailbox.active_read = Some(cancel.clone());
                        (request, cancel)
                    }
                };
                let reading = !matches!(&request, Request::Save { .. });
                let rebuild = matches!(&request, Request::Rebuild { .. });
                match request {
                    Request::Load { root, generation } | Request::Rebuild { root, generation } => {
                        let result = if rebuild {
                            super::index::rebuild_cancel(repository.as_ref(), &root, &cancel)
                        } else {
                            super::index::load_with_backfill_cancel(
                                repository.as_ref(),
                                &root,
                                &cancel,
                            )
                        }
                        .map(|stored| {
                            loaded = Some(Loaded {
                                root,
                                generation,
                                version: stored.revision,
                                catalog: stored.catalog.clone(),
                                seeded: true,
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
                                super::index::save_cancel(
                                    repository.as_ref(),
                                    &root,
                                    &active.catalog,
                                    &catalog,
                                    active.version.as_deref(),
                                    !active.seeded,
                                    &std::sync::atomic::AtomicBool::new(false),
                                )
                                .map(|new| {
                                    active.version = Some(new);
                                    active.catalog = (*catalog).clone();
                                    active.seeded = true;
                                })
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
                if reading {
                    let (lock, _) = &*shared;
                    lock.lock().unwrap().active_read = None;
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
    pub fn cancel_read(&self) -> Result<(), String> {
        let (lock, _) = &*self.inbox;
        let mailbox = lock.lock().map_err(|_| "Tag-Dienst wurde unterbrochen")?;
        if let Some(active) = &mailbox.active_read {
            active.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if let Some((_, queued)) = &mailbox.load {
            queued.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
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
            Request::Load { .. } | Request::Rebuild { .. } => {
                if let Some(active) = &mailbox.active_read {
                    active.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                if let Some((_, queued)) = &mailbox.load {
                    queued.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                mailbox.load = Some((request, Arc::new(std::sync::atomic::AtomicBool::new(false))));
            }
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
            if let Some(active) = &mailbox.active_read {
                active.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            mailbox.stopping = true;
            mailbox.load = None;
            wake.notify_one();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
