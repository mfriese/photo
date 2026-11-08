//! Prepare one continuous year timeline in the background; native UI opens on the UI thread.
use crate::domain::{
    CaptureTime, Media,
    diary::{Recording, YearTimeline, diary_year},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
#[derive(Clone, Debug)]
pub struct AudioProbe {
    pub duration: f64,
    pub audio_only: bool,
    pub file_created: Option<String>,
}
pub trait AudioSource: Send + Sync + 'static {
    fn probe(&self, path: &Path) -> Result<AudioProbe, String>;
    fn capture_time(&self, path: &Path) -> Result<CaptureTime, String>;
}
pub trait AudioPlayer: Send + Sync + 'static {
    /// Called on the main thread. Composition loading must stay asynchronous.
    fn open(&self, timeline: &YearTimeline, start_seconds: f64) -> Result<(), String>;
}
enum Event {
    Progress(String),
    Ready(Result<(YearTimeline, f64), String>),
}
#[derive(Default)]
pub struct DiarySession {
    pending: Option<(mpsc::Receiver<Event>, Arc<AtomicBool>)>,
    status: String,
    warnings: Vec<String>,
}
impl DiarySession {
    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }
    pub fn status(&self) -> &str {
        &self.status
    }
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    pub fn cancel(&mut self) {
        if let Some((_, cancel)) = &self.pending {
            cancel.store(true, Ordering::Relaxed);
            self.status = "Audio-Vorbereitung wird abgebrochen …".into();
        } else {
            self.status.clear();
        }
        self.warnings.clear();
    }
    pub fn start(
        &mut self,
        items: &[Media],
        selected: &Media,
        source: Arc<dyn AudioSource>,
    ) -> Result<(), String> {
        if self.pending.is_some() {
            return Err(
                "Eine Audio-Vorbereitung läuft noch; bitte Abschluss oder Abbruch abwarten".into(),
            );
        }
        let year = diary_year(selected.group())
            .ok_or("Audio bitte aus einem Jahr/diary-Ordner öffnen")?
            .to_owned();
        let paths: Vec<PathBuf> = items
            .iter()
            .filter(|m| m.is_audio() && diary_year(m.group()) == Some(&year))
            .map(|m| m.path().into())
            .collect();
        let chosen = selected.path().to_owned();
        self.cancel();
        self.warnings.clear();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.pending = Some((rx, cancel.clone()));
        self.status = "Jahresverlauf wird vorbereitet …".into();
        std::thread::spawn(move || {
            let result = (|| {
                let mut recordings = Vec::new();
                for (i, path) in paths.iter().enumerate() {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("Abgebrochen".into());
                    }
                    let _ = tx.send(Event::Progress(format!(
                        "Audio {} / {}: {}",
                        i + 1,
                        paths.len(),
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )));
                    let probe = source
                        .probe(path)
                        .map_err(|e| format!("{}: {e}", path.display()))?;
                    if !probe.audio_only {
                        return Err(format!("Keine reine Audiodatei: {}", path.display()));
                    }
                    recordings.push(Recording::new(
                        path.clone(),
                        source
                            .capture_time(path)
                            .map_err(|e| format!("{}: {e}", path.display()))?,
                        probe.duration,
                    )?);
                }
                let timeline = YearTimeline::new(year, recordings)?;
                let index = timeline
                    .recordings()
                    .iter()
                    .position(|r| r.path() == chosen)
                    .ok_or("Aufnahme nicht im Jahresverlauf")?;
                let start = timeline.offset(index).unwrap();
                Ok((timeline, start))
            })();
            let _ = tx.send(Event::Ready(result));
        });
        Ok(())
    }
    pub fn poll(&mut self, player: &dyn AudioPlayer) {
        let Some((rx, cancel)) = &self.pending else {
            return;
        };
        let cancelled = cancel.load(Ordering::Relaxed);
        let mut ready = None;
        loop {
            match rx.try_recv() {
                Ok(Event::Progress(message)) => {
                    if !cancelled {
                        self.status = message;
                    }
                }
                Ok(Event::Ready(result)) => {
                    ready = Some(result);
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    ready = Some(Err("Audioauftrag unerwartet beendet".into()));
                    break;
                }
            }
        }
        if let Some(result) = ready {
            self.pending = None;
            if cancelled {
                self.status = "Audio-Vorbereitung abgebrochen".into();
                return;
            }
            let result = result.and_then(|(timeline, start)| {
                self.warnings = timeline
                    .recordings()
                    .iter()
                    .map(|r| {
                        format!(
                            "{} · {}",
                            r.path().file_name().unwrap_or_default().to_string_lossy(),
                            r.captured().origin()
                        )
                    })
                    .collect();
                player.open(&timeline, start)
            });
            self.status = match result {
                Ok(()) => "Jahresverlauf im Audioplayer geöffnet".into(),
                Err(e) => format!("Diary nicht geöffnet: {e}"),
            };
        }
    }
}
impl Drop for DiarySession {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Read-only dates for the current diary grid. One worker, including cancelled work.
enum CatalogEvent {
    Date(PathBuf, Result<CaptureTime, String>),
    Done,
}
#[derive(Default)]
pub struct DiaryCatalog {
    key: Option<(String, Vec<PathBuf>)>,
    dates: std::collections::BTreeMap<PathBuf, Result<CaptureTime, String>>,
    pending: Option<(mpsc::Receiver<CatalogEvent>, Arc<AtomicBool>)>,
}
impl DiaryCatalog {
    pub fn date(&self, path: &Path) -> Option<&Result<CaptureTime, String>> {
        self.dates.get(path)
    }
    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }
    pub fn cancel(&mut self) {
        if let Some((_, cancel)) = &self.pending {
            cancel.store(true, Ordering::Relaxed);
        }
        self.key = None;
        self.dates.clear();
    }
    pub fn ensure(&mut self, items: &[Media], year: &str, source: Arc<dyn AudioSource>) {
        let paths: Vec<_> = items
            .iter()
            .filter(|m| m.is_audio() && diary_year(m.group()) == Some(year))
            .map(|m| m.path().to_owned())
            .collect();
        let key = (year.to_owned(), paths.clone());
        if self.key.as_ref() == Some(&key) {
            return;
        }
        if self.pending.is_some() {
            self.cancel();
            return;
        }
        self.key = Some(key);
        self.dates.clear();
        if paths.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.pending = Some((rx, cancel.clone()));
        std::thread::spawn(move || {
            for path in paths {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let result = source.capture_time(&path);
                if tx.send(CatalogEvent::Date(path, result)).is_err() {
                    return;
                }
            }
            let _ = tx.send(CatalogEvent::Done);
        });
    }
    pub fn poll(&mut self) {
        let Some((rx, cancel)) = &self.pending else {
            return;
        };
        let cancelled = cancel.load(Ordering::Relaxed);
        let mut done = false;
        let mut disconnected = false;
        loop {
            match rx.try_recv() {
                Ok(CatalogEvent::Date(path, result)) => {
                    if !cancelled {
                        self.dates.insert(path, result);
                    }
                }
                Ok(CatalogEvent::Done) => {
                    done = true;
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    done = true;
                    disconnected = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if done {
            self.pending = None;
            if disconnected
                && !cancelled
                && let Some((_, paths)) = &self.key
            {
                for path in paths {
                    self.dates
                        .entry(path.clone())
                        .or_insert_with(|| Err("Datumsauftrag unerwartet beendet".into()));
                }
            }
            if cancelled {
                self.key = None;
                self.dates.clear();
            }
        }
    }
}
impl Drop for DiaryCatalog {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct Source;
    impl AudioSource for Source {
        fn probe(&self, path: &Path) -> Result<AudioProbe, String> {
            Ok(AudioProbe {
                duration: if path.file_name().unwrap() == "a.wav" {
                    10.
                } else {
                    30.
                },
                audio_only: true,
                file_created: None,
            })
        }
        fn capture_time(&self, path: &Path) -> Result<CaptureTime, String> {
            CaptureTime::parse(
                if path.file_name().unwrap() == "a.wav" {
                    "2026:01:01 10:00:00"
                } else {
                    "2026:09:01 10:00:00"
                },
                "test",
            )
        }
    }
    #[derive(Default)]
    struct Player(Mutex<Option<(usize, f64, f64)>>);
    impl AudioPlayer for Player {
        fn open(&self, t: &YearTimeline, start: f64) -> Result<(), String> {
            *self.0.lock().unwrap() = Some((t.recordings().len(), t.duration(), start));
            Ok(())
        }
    }
    #[test]
    fn catalog_reads_dates_in_background_and_reuses_the_same_year() {
        let items = vec![Media::from_path("a.wav".into(), "2026/diary".into()).unwrap()];
        let mut catalog = DiaryCatalog::default();
        catalog.ensure(&items, "2026", Arc::new(Source));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while catalog.is_loading() {
            catalog.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(
            catalog
                .date(Path::new("a.wav"))
                .unwrap()
                .as_ref()
                .unwrap()
                .date_label(),
            "01.01.2026"
        );
        catalog.ensure(&items, "2026", Arc::new(Source));
        assert!(!catalog.is_loading());
        catalog.cancel();
        assert!(catalog.date(Path::new("a.wav")).is_none());
    }
    #[test]
    fn whole_year_is_prepared_in_order_and_selected_file_sets_global_start() {
        let items: Vec<_> = [
            ("b.wav", "2026/diary"),
            ("a.wav", "2026/diary"),
            ("outside.wav", "2025/diary"),
        ]
        .into_iter()
        .map(|(p, g)| Media::from_path(p.into(), g.into()).unwrap())
        .collect();
        let mut session = DiarySession::default();
        let player = Player::default();
        session.start(&items, &items[0], Arc::new(Source)).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while session.is_loading() {
            session.poll(&player);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(*player.0.lock().unwrap(), Some((2, 40., 10.)));
        assert_eq!(session.warnings().len(), 2);
    }
    #[test]
    fn cancel_never_opens_a_late_player() {
        let items = vec![Media::from_path("a.wav".into(), "2026/diary".into()).unwrap()];
        let mut session = DiarySession::default();
        let player = Player::default();
        session.start(&items, &items[0], Arc::new(Source)).unwrap();
        assert!(session.start(&items, &items[0], Arc::new(Source)).is_err());
        session.cancel();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while session.is_loading() {
            session.poll(&player);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(*player.0.lock().unwrap(), None);
    }
}
