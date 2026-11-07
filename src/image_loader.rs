//! UI image adapter with a bounded queue and two decoding workers.
use eframe::egui::{
    self, ColorImage,
    load::{ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
};
type Entry = Option<Result<Arc<ColorImage>, String>>;
struct CacheEntry {
    generation: u64,
    state: Entry,
}
type Cache = Arc<Mutex<HashMap<String, CacheEntry>>>;
struct Task {
    generation: u64,
    key: String,
    path: String,
    turns: u8,
    ctx: egui::Context,
}
pub struct OrientedLoader {
    cache: Cache,
    sender: mpsc::SyncSender<Task>,
    generation: std::sync::atomic::AtomicU64,
}
impl OrientedLoader {
    pub fn new(decoder: Arc<dyn crate::ports::ImageDecoder>) -> Self {
        let cache: Cache = Default::default();
        let (sender, receiver) = mpsc::sync_channel::<Task>(32);
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..2 {
            let receiver = receiver.clone();
            let cache = cache.clone();
            let decoder = decoder.clone();
            std::thread::Builder::new()
                .name(format!("photosuite-images-{index}"))
                .spawn(move || {
                    loop {
                        let task = { receiver.lock().unwrap().recv() };
                        let Ok(task) = task else {
                            break;
                        };
                        if !cache
                            .lock()
                            .unwrap()
                            .get(&task.key)
                            .is_some_and(|entry| entry.generation == task.generation)
                        {
                            continue;
                        }
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            decoder.decode(std::path::Path::new(&task.path), task.turns)
                        }))
                        .unwrap_or_else(|_| Err("Bilddecoder wurde unerwartet beendet".into()))
                        .map(|image| {
                            Arc::new(ColorImage::from_rgba_unmultiplied(
                                [image.width as usize, image.height as usize],
                                &image.rgba,
                            ))
                        });
                        publish(&cache, &task.key, task.generation, result);
                        task.ctx.request_repaint();
                    }
                })
                .expect("image worker could not start");
        }
        Self {
            cache,
            sender,
            generation: std::sync::atomic::AtomicU64::new(0),
        }
    }
}
fn publish(cache: &Cache, key: &str, generation: u64, result: Result<Arc<ColorImage>, String>) {
    if let Some(entry) = cache.lock().unwrap().get_mut(key)
        && entry.generation == generation
    {
        entry.state = Some(result);
    }
}
impl ImageLoader for OrientedLoader {
    fn id(&self) -> &str {
        "photosuite.oriented"
    }
    fn load(&self, ctx: &egui::Context, uri: &str, _: SizeHint) -> ImageLoadResult {
        let rest = uri
            .strip_prefix("oriented://")
            .ok_or(LoadError::NotSupported)?;
        let (turns, path) = rest.split_once('/').ok_or(LoadError::NotSupported)?;
        let turns: u8 = turns.parse().map_err(|_| LoadError::NotSupported)?;
        let mut cache = self.cache.lock().unwrap();
        if let Some(entry) = cache.get(uri) {
            return match &entry.state {
                None => Ok(ImagePoll::Pending { size: None }),
                Some(Ok(image)) => Ok(ImagePoll::Ready {
                    image: image.clone(),
                }),
                Some(Err(error)) => Err(LoadError::Loading(error.clone())),
            };
        }
        let generation = self
            .generation
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let task = Task {
            generation,
            key: uri.into(),
            path: path.into(),
            turns,
            ctx: ctx.clone(),
        };
        match self.sender.try_send(task) {
            Ok(()) => {
                cache.insert(
                    uri.into(),
                    CacheEntry {
                        generation,
                        state: None,
                    },
                );
            }
            Err(mpsc::TrySendError::Full(_)) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(30))
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                return Err(LoadError::Loading("Bildlade-Dienst nicht verfügbar".into()));
            }
        }
        Ok(ImagePoll::Pending { size: None })
    }
    fn forget(&self, uri: &str) {
        self.cache.lock().unwrap().remove(uri);
    }
    fn forget_all(&self) {
        self.cache.lock().unwrap().clear();
    }
    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .unwrap()
            .values()
            .filter_map(|e| e.state.as_ref())
            .map(|e| match e {
                Ok(i) => i.pixels.len() * 4,
                Err(e) => e.len(),
            })
            .sum()
    }
}
impl Drop for OrientedLoader {
    fn drop(&mut self) {
        self.cache.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Condvar,
        atomic::{AtomicUsize, Ordering},
    };
    struct Decoder {
        active: AtomicUsize,
        maximum: AtomicUsize,
        gate: (Mutex<bool>, Condvar),
    }
    impl crate::ports::ImageDecoder for Decoder {
        fn decode(&self, _: &std::path::Path, _: u8) -> Result<crate::ports::DecodedImage, String> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum.fetch_max(active, Ordering::SeqCst);
            let (lock, cv) = &self.gate;
            let mut open = lock.lock().unwrap();
            while !*open {
                open = cv.wait(open).unwrap();
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(crate::ports::DecodedImage {
                width: 1,
                height: 1,
                rgba: vec![1, 2, 3, 255],
            })
        }
    }
    #[test]
    fn load_burst_has_bounded_parallelism_and_nonblocking_queue() {
        let decoder = Arc::new(Decoder {
            active: AtomicUsize::new(0),
            maximum: AtomicUsize::new(0),
            gate: (Mutex::new(false), Condvar::new()),
        });
        let loader = OrientedLoader::new(decoder.clone());
        let ctx = egui::Context::default();
        for i in 0..100 {
            assert!(matches!(
                loader.load(&ctx, &format!("oriented://0/{i}.jpg"), SizeHint::default()),
                Ok(ImagePoll::Pending { .. })
            ));
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while decoder.active.load(Ordering::SeqCst) < 2 {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(decoder.maximum.load(Ordering::SeqCst), 2);
        assert!(loader.cache.lock().unwrap().len() <= 34);
        {
            let (lock, cv) = &decoder.gate;
            *lock.lock().unwrap() = true;
            cv.notify_all();
        }
    }
    #[test]
    fn forgotten_generation_cannot_overwrite_reloaded_image() {
        let cache: Cache = Default::default();
        cache.lock().unwrap().insert(
            "image".into(),
            CacheEntry {
                generation: 2,
                state: None,
            },
        );
        publish(&cache, "image", 1, Err("old".into()));
        assert!(cache.lock().unwrap()["image"].state.is_none());
        publish(&cache, "image", 2, Err("new".into()));
        publish(&cache, "image", 1, Err("old".into()));
        assert_eq!(
            cache.lock().unwrap()["image"]
                .state
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap_err(),
            "new"
        );
    }
}
