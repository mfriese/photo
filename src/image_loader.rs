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
const CACHE_BYTES: usize = 256 * 1024 * 1024;
const CACHE_ENTRIES: usize = 1024;
struct CacheEntry {
    generation: u64,
    last_used: u64,
    state: Entry,
}
type Cache = Arc<Mutex<HashMap<String, CacheEntry>>>;
struct Task {
    generation: u64,
    key: String,
    path: String,
    turns: u8,
    max_edge: u32,
    ctx: egui::Context,
}
pub struct OrientedLoader {
    cache: Cache,
    sender: mpsc::SyncSender<Task>,
    generation: Arc<std::sync::atomic::AtomicU64>,
}
impl OrientedLoader {
    pub fn new(decoder: Arc<dyn crate::ports::ImageDecoder>) -> Self {
        Self::with_budget(decoder, CACHE_BYTES)
    }
    fn with_budget(decoder: Arc<dyn crate::ports::ImageDecoder>, budget: usize) -> Self {
        let cache: Cache = Default::default();
        let generation = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (sender, receiver) = mpsc::sync_channel::<Task>(32);
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..2 {
            let receiver = receiver.clone();
            let cache = cache.clone();
            let decoder = decoder.clone();
            let clock = generation.clone();
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
                            decoder
                                .decode_preview(
                                    std::path::Path::new(&task.path),
                                    task.turns,
                                    task.max_edge,
                                )
                                .and_then(|image| {
                                    if image.width() > task.max_edge
                                        || image.height() > task.max_edge
                                    {
                                        return Err(
                                            "Bildadapter überschreitet die Vorschauauflösung"
                                                .into(),
                                        );
                                    }
                                    Ok(image)
                                })
                        }))
                        .unwrap_or_else(|_| Err("Bilddecoder wurde unerwartet beendet".into()))
                        .map(|image| {
                            Arc::new(ColorImage::from_rgba_unmultiplied(
                                [image.width() as usize, image.height() as usize],
                                image.rgba(),
                            ))
                        });
                        let victims = publish(
                            &cache,
                            &task.key,
                            task.generation,
                            result,
                            clock.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                            budget,
                        );
                        // Never call egui while holding the cache lock: forget reenters this loader.
                        for victim in victims {
                            task.ctx.forget_image(&victim);
                        }
                        task.ctx.request_repaint();
                    }
                })
                .expect("image worker could not start");
        }
        Self {
            cache,
            sender,
            generation,
        }
    }
}
fn publish(
    cache: &Cache,
    key: &str,
    generation: u64,
    result: Result<Arc<ColorImage>, String>,
    used: u64,
    budget: usize,
) -> Vec<String> {
    let mut cache = cache.lock().unwrap();
    if let Some(entry) = cache.get_mut(key)
        && entry.generation == generation
    {
        entry.state = Some(result);
        entry.last_used = used;
    } else {
        return Vec::new();
    }
    let cost = |entry: &CacheEntry| match &entry.state {
        Some(Ok(image)) => image.pixels.len() * 4,
        Some(Err(error)) => error.len(),
        None => 0,
    };
    let mut bytes: usize = cache.values().map(cost).sum();
    let mut victims = Vec::new();
    while bytes > budget || cache.len() > CACHE_ENTRIES {
        let Some(oldest) = cache
            .iter()
            .filter(|(_, e)| e.state.is_some())
            .min_by_key(|(_, e)| e.last_used)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        if let Some(entry) = cache.remove(&oldest) {
            bytes -= cost(&entry);
            victims.push(oldest);
        }
    }
    victims
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
        let (turns, edge) = turns.split_once(':').unwrap_or((turns, "4096"));
        let turns: u8 = turns.parse().map_err(|_| LoadError::NotSupported)?;
        let max_edge: u32 = edge.parse().map_err(|_| LoadError::NotSupported)?;
        if turns > 3 || ![256, 512, 1024, 4096].contains(&max_edge) {
            return Err(LoadError::NotSupported);
        }
        let mut cache = self.cache.lock().unwrap();
        if let Some(entry) = cache.get_mut(uri) {
            entry.last_used = self
                .generation
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
            max_edge,
            ctx: ctx.clone(),
        };
        match self.sender.try_send(task) {
            Ok(()) => {
                cache.insert(
                    uri.into(),
                    CacheEntry {
                        generation,
                        last_used: generation,
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
            crate::ports::DecodedImage::new(1, 1, vec![1, 2, 3, 255])
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
                last_used: 2,
                state: None,
            },
        );
        publish(&cache, "image", 1, Err("old".into()), 1, CACHE_BYTES);
        assert!(cache.lock().unwrap()["image"].state.is_none());
        publish(&cache, "image", 2, Err("new".into()), 2, CACHE_BYTES);
        publish(&cache, "image", 1, Err("old".into()), 1, CACHE_BYTES);
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
    #[test]
    fn cache_budget_evicts_old_results_and_preserves_pending_requests() {
        let cache: Cache = Default::default();
        for (key, generation) in [("old", 1), ("recent", 2), ("new", 3), ("pending", 4)] {
            cache.lock().unwrap().insert(
                key.into(),
                CacheEntry {
                    generation,
                    last_used: generation,
                    state: None,
                },
            );
        }
        let pixel = || {
            Ok(Arc::new(ColorImage::new(
                [1, 1],
                vec![egui::Color32::WHITE],
            )))
        };
        assert!(publish(&cache, "old", 1, pixel(), 1, 8).is_empty());
        assert!(publish(&cache, "recent", 2, pixel(), 2, 8).is_empty());
        let evicted = publish(&cache, "new", 3, pixel(), 3, 8);
        assert_eq!(evicted, vec!["old"]);
        let cache = cache.lock().unwrap();
        assert!(
            cache.contains_key("recent")
                && cache.contains_key("new")
                && cache["pending"].state.is_none()
        );
    }
    #[test]
    fn eviction_releases_egui_texture_cache_without_lock_reentry_deadlock() {
        struct Immediate;
        impl crate::ports::ImageDecoder for Immediate {
            fn decode(
                &self,
                _: &std::path::Path,
                _: u8,
            ) -> Result<crate::ports::DecodedImage, String> {
                crate::ports::DecodedImage::new(1, 1, vec![1, 2, 3, 255])
            }
        }
        let ctx = egui::Context::default();
        let loader = Arc::new(OrientedLoader::with_budget(Arc::new(Immediate), 8));
        ctx.add_image_loader(loader.clone());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        for i in 0..3 {
            loop {
                match ctx
                    .try_load_texture(
                        &format!("oriented://0:256/{i}.jpg"),
                        egui::TextureOptions::LINEAR,
                        SizeHint::default(),
                    )
                    .unwrap()
                {
                    egui::load::TexturePoll::Ready { .. } => break,
                    egui::load::TexturePoll::Pending { .. } => {
                        assert!(std::time::Instant::now() < deadline);
                        std::thread::yield_now();
                    }
                }
            }
        }
        assert!(loader.byte_size() <= 8);
        assert!(matches!(
            ctx.try_load_texture(
                "oriented://0:256/0.jpg",
                egui::TextureOptions::LINEAR,
                SizeHint::default()
            )
            .unwrap(),
            egui::load::TexturePoll::Pending { .. }
        ));
    }
}
