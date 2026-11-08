//! Prepare one continuous year timeline in the background; native UI opens on the UI thread.
use crate::domain::{
    CaptureTime, Media,
    diary::{Recording, diary_year},
    diary_timeline::{DiaryClip, DiaryTimeline},
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
    /// Only a regular-file stat; no media decoding or metadata processes.
    fn file_size(&self, path: &Path) -> Result<u64, String>;
    fn inspect(&self, path: &Path) -> Result<Recording, String> {
        let probe = self.probe(path)?;
        if !probe.audio_only {
            return Err("Keine reine Audiodatei".into());
        }
        Recording::new(path.into(), self.capture_time(path)?, probe.duration)
    }
}
#[derive(Default)]
pub struct AudioPlayerUpdate {
    pub is_open: bool,
    pub photo_to_open: Option<PathBuf>,
}
#[derive(Clone, Copy, Default)]
pub struct AudioPlaybackState {
    pub is_open: bool,
    pub has_started: bool,
    pub failed: bool,
}
#[derive(Clone, Default)]
pub struct VerificationProgress {
    pub checked: usize,
    pub total: usize,
    pub errors: std::collections::BTreeMap<PathBuf, String>,
    pub cancelled: bool,
}
pub trait AudioPlayer: Send + Sync + 'static {
    /// Main thread, lightweight snapshot; only the selected clip may load asynchronously.
    fn open(
        &self,
        timeline: &DiaryTimeline,
        selected: &Path,
        photos: &[Media],
    ) -> Result<(), String>;
    /// Update layout in place without replacing/reseeking the current recording.
    fn revise(
        &self,
        timeline: &DiaryTimeline,
        progress: &VerificationProgress,
    ) -> Result<(), String>;
    fn state(&self) -> AudioPlaybackState {
        AudioPlaybackState::default()
    }
    /// Pump platform events on the UI thread; consume photo-open events.
    fn poll(&self) -> AudioPlayerUpdate {
        AudioPlayerUpdate::default()
    }
}
enum Event {
    Draft(Result<(DiaryTimeline, PathBuf, Vec<Media>), String>),
    Checked(PathBuf, Result<Recording, String>),
    Done,
}
struct Pending {
    receiver: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
    gate: Option<mpsc::Sender<()>>,
    opened: bool,
}
#[derive(Default)]
pub struct DiarySession {
    pending: Option<Pending>,
    timeline: Option<DiaryTimeline>,
    progress: VerificationProgress,
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
        if let Some(pending) = &mut self.pending {
            pending.cancel.store(true, Ordering::Relaxed);
            pending.gate.take();
            self.status = "Audio-Prüfung wird abgebrochen …".into();
        }
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
        let selected = selected.path().to_owned();
        if !paths.contains(&selected) {
            return Err("Aufnahme nicht im Jahresverlauf".into());
        }
        let photos = items.iter().filter(|m| m.is_photo()).cloned().collect();
        self.timeline = None;
        self.warnings.clear();
        self.progress = VerificationProgress {
            total: paths.len(),
            ..Default::default()
        };
        let (sender, receiver) = mpsc::sync_channel(2);
        let (go, gate) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        self.pending = Some(Pending {
            receiver,
            cancel,
            gate: Some(go),
            opened: false,
        });
        self.status = "Audioplayer wird mit Dateinamen und Dateigrößen geöffnet …".into();
        std::thread::spawn(move || {
            let draft = (|| {
                let mut clips = Vec::with_capacity(paths.len());
                for path in &paths {
                    if stop.load(Ordering::Relaxed) {
                        return Err("Abgebrochen".into());
                    }
                    clips.push(DiaryClip::from_file(
                        path.clone(),
                        source
                            .file_size(path)
                            .map_err(|e| format!("{}: {e}", path.display()))?,
                    ));
                }
                DiaryTimeline::new(year, clips)
            })();
            let Ok(timeline) = draft else {
                let _ = sender.send(Event::Draft(Err(draft.unwrap_err())));
                let _ = sender.send(Event::Done);
                return;
            };
            let mut order: Vec<_> = timeline
                .clips()
                .iter()
                .map(|c| c.path().to_owned())
                .collect();
            if let Some(index) = order.iter().position(|p| p == &selected) {
                order.swap(0, index);
            }
            if sender
                .send(Event::Draft(Ok((timeline, selected, photos))))
                .is_err()
            {
                return;
            }
            // Verification starts only after the selected clip has actually played.
            loop {
                if stop.load(Ordering::Relaxed) {
                    let _ = sender.send(Event::Done);
                    return;
                }
                match gate.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(()) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        let _ = sender.send(Event::Done);
                        return;
                    }
                }
            }
            for path in order {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let result = source.inspect(&path);
                if sender.send(Event::Checked(path, result)).is_err() {
                    return;
                }
            }
            let _ = sender.send(Event::Done);
        });
        Ok(())
    }
    pub fn poll(&mut self, player: &dyn AudioPlayer) {
        let Some(pending) = &mut self.pending else {
            return;
        };
        if pending.opened && pending.gate.is_some() && !pending.cancel.load(Ordering::Relaxed) {
            let state = player.state();
            if state.has_started {
                if let Some(gate) = pending.gate.take() {
                    let _ = gate.send(());
                }
                self.status = "Aufnahmedaten und Dauern werden im Hintergrund geprüft …".into();
            } else if !state.is_open || state.failed {
                pending.cancel.store(true, Ordering::Relaxed);
                pending.gate.take();
            }
        } else if pending.opened && !player.state().is_open {
            pending.cancel.store(true, Ordering::Relaxed);
        }
        let mut changed = false;
        let mut done = false;
        for _ in 0..8 {
            let event = match pending.receiver.try_recv() {
                Ok(event) => event,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = "Audio-Prüfung unerwartet beendet".into();
                    self.progress.cancelled = true;
                    changed = true;
                    done = true;
                    break;
                }
            };
            let cancelled = pending.cancel.load(Ordering::Relaxed);
            match event {
                Event::Draft(result) if !cancelled => match result {
                    Ok((timeline, selected, photos)) => match player
                        .open(&timeline, &selected, &photos)
                    {
                        Ok(()) => {
                            self.timeline = Some(timeline);
                            pending.opened = true;
                            self.status =
                                "Vorläufiger Jahresverlauf geöffnet · Wiedergabe startet".into();
                        }
                        Err(error) => {
                            self.status = format!("Diary nicht geöffnet: {error}");
                            pending.cancel.store(true, Ordering::Relaxed);
                            pending.gate.take();
                        }
                    },
                    Err(error) => {
                        self.status = format!("Diary nicht geöffnet: {error}");
                        pending.cancel.store(true, Ordering::Relaxed);
                        pending.gate.take();
                    }
                },
                Event::Draft(_) => {}
                Event::Checked(path, result) if !cancelled => {
                    self.progress.checked += 1;
                    changed = true;
                    let result = result.and_then(|recording| {
                        if recording.path() != path {
                            return Err("Prüfergebnis gehört zu einer anderen Aufnahme".into());
                        }
                        self.warnings.push(format!(
                            "{} · {}",
                            path.display(),
                            recording.captured().origin()
                        ));
                        self.timeline
                            .as_mut()
                            .ok_or("Jahresverlauf fehlt")?
                            .refine(recording)
                    });
                    if let Err(error) = result {
                        self.warnings.push(format!("{}: {error}", path.display()));
                        self.progress.errors.insert(path, error);
                    }
                    self.status = format!(
                        "Audio geprüft: {} / {} · {} Fehler",
                        self.progress.checked,
                        self.progress.total,
                        self.progress.errors.len()
                    );
                }
                Event::Checked(_, _) => {}
                Event::Done => {
                    done = true;
                    changed = true;
                    self.progress.cancelled = cancelled;
                    if cancelled && (pending.opened || self.status.starts_with("Audio-Prüfung")) {
                        self.status =
                            "Audio-Prüfung abgebrochen · ungeprüfte Abschnitte bleiben vorläufig"
                                .into();
                    }
                    break;
                }
            }
        }
        if changed
            && player.state().is_open
            && let Some(timeline) = &self.timeline
            && let Err(error) = player.revise(timeline, &self.progress)
        {
            self.status = format!("Jahresleiste konnte nicht aktualisiert werden: {error}");
            pending.cancel.store(true, Ordering::Relaxed);
            pending.gate.take();
        }
        if done {
            self.pending = None;
            self.timeline = None;
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
        fn file_size(&self, _: &Path) -> Result<u64, String> {
            Ok(16_000)
        }
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
    struct Player {
        opened: Mutex<Option<(DiaryTimeline, PathBuf)>>,
        revisions: Mutex<Vec<(DiaryTimeline, VerificationProgress)>>,
        started: AtomicBool,
        closed: AtomicBool,
    }
    impl AudioPlayer for Player {
        fn open(&self, t: &DiaryTimeline, selected: &Path, _: &[Media]) -> Result<(), String> {
            *self.opened.lock().unwrap() = Some((t.clone(), selected.into()));
            Ok(())
        }
        fn revise(&self, t: &DiaryTimeline, p: &VerificationProgress) -> Result<(), String> {
            self.revisions.lock().unwrap().push((t.clone(), p.clone()));
            Ok(())
        }
        fn state(&self) -> AudioPlaybackState {
            AudioPlaybackState {
                is_open: self.opened.lock().unwrap().is_some()
                    && !self.closed.load(Ordering::Relaxed),
                has_started: self.started.load(Ordering::Relaxed),
                failed: false,
            }
        }
    }
    fn until(
        session: &mut DiarySession,
        player: &Player,
        condition: impl Fn(&DiarySession, &Player) -> bool,
    ) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !condition(session, player) {
            session.poll(player);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
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
        player.started.store(true, Ordering::Relaxed);
        session.start(&items, &items[0], Arc::new(Source)).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while session.is_loading() {
            session.poll(&player);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let opened = player.opened.lock().unwrap();
        let (draft, selected) = opened.as_ref().unwrap();
        assert_eq!(selected, Path::new("b.wav"));
        assert_eq!(draft.clips().len(), 2);
        assert!(draft.clips().iter().all(|c| !c.is_verified()));
        let revisions = player.revisions.lock().unwrap();
        let (timeline, progress) = revisions.last().unwrap();
        assert!(timeline.clips().iter().all(|c| c.is_verified()));
        assert_eq!(timeline.clips()[0].duration(), Some(10.));
        assert_eq!(timeline.clips()[1].duration(), Some(30.));
        assert_eq!(progress.checked, 2);
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
        assert!(player.opened.lock().unwrap().is_none());
    }
    #[test]
    fn opens_without_media_reads_then_verifies_only_after_actual_playback() {
        use std::sync::atomic::AtomicUsize;
        struct SlowSource {
            calls: AtomicUsize,
            release: AtomicBool,
        }
        impl AudioSource for SlowSource {
            fn file_size(&self, _: &Path) -> Result<u64, String> {
                Ok(32_000)
            }
            fn probe(&self, _: &Path) -> Result<AudioProbe, String> {
                unreachable!()
            }
            fn capture_time(&self, _: &Path) -> Result<CaptureTime, String> {
                unreachable!()
            }
            fn inspect(&self, path: &Path) -> Result<Recording, String> {
                self.calls.fetch_add(1, Ordering::Relaxed);
                while !self.release.load(Ordering::Relaxed) {
                    std::thread::yield_now();
                }
                if path == Path::new("bad.wav") {
                    return Err("Nicht abspielbar".into());
                }
                Recording::new(
                    path.into(),
                    CaptureTime::parse("2026:01:01 00:00:00", "real metadata")?,
                    3.,
                )
            }
        }
        let items: Vec<_> = ["bad.wav", "2026-12-01_00-00-00.wav"]
            .iter()
            .map(|p| Media::from_path((*p).into(), "2026/diary".into()).unwrap())
            .collect();
        let source = Arc::new(SlowSource {
            calls: AtomicUsize::new(0),
            release: AtomicBool::new(false),
        });
        let player = Player::default();
        let mut session = DiarySession::default();
        session.start(&items, &items[1], source.clone()).unwrap();
        until(&mut session, &player, |_, p| {
            p.opened.lock().unwrap().is_some()
        });
        for _ in 0..5 {
            session.poll(&player);
        }
        assert_eq!(source.calls.load(Ordering::Relaxed), 0);
        assert_eq!(
            player.opened.lock().unwrap().as_ref().unwrap().0.clips()[0]
                .captured()
                .unwrap()
                .stamp(),
            "20261201_000000"
        );
        player.started.store(true, Ordering::Relaxed);
        until(&mut session, &player, |_, _| {
            source.calls.load(Ordering::Relaxed) > 0
        });
        assert!(session.is_loading());
        assert!(player.revisions.lock().unwrap().is_empty());
        source.release.store(true, Ordering::Relaxed);
        until(&mut session, &player, |s, _| !s.is_loading());
        let revisions = player.revisions.lock().unwrap();
        let (timeline, progress) = revisions.last().unwrap();
        assert_eq!(progress.checked, 2);
        assert_eq!(progress.errors.len(), 1);
        assert!(timeline.clips()[0].is_verified());
        assert_eq!(
            timeline.clips()[0].captured().unwrap().stamp(),
            "20260101_000000"
        );
        assert!(!timeline.clips()[1].is_verified());
    }
    #[test]
    fn closing_before_playback_cancels_waiting_verification_without_late_reopen() {
        let items = vec![Media::from_path("a.wav".into(), "2026/diary".into()).unwrap()];
        let mut session = DiarySession::default();
        let player = Player::default();
        session.start(&items, &items[0], Arc::new(Source)).unwrap();
        until(&mut session, &player, |_, p| {
            p.opened.lock().unwrap().is_some()
        });
        player.closed.store(true, Ordering::Relaxed);
        until(&mut session, &player, |s, _| !s.is_loading());
        assert!(session.warnings().is_empty());
        assert!(player.revisions.lock().unwrap().is_empty());
    }
    #[test]
    fn unreadable_size_and_unexpected_worker_exit_finish_without_opening() {
        struct BrokenSource(bool);
        impl AudioSource for BrokenSource {
            fn file_size(&self, _: &Path) -> Result<u64, String> {
                if self.0 {
                    panic!("simulated source failure")
                } else {
                    Err("Größe nicht lesbar".into())
                }
            }
            fn probe(&self, _: &Path) -> Result<AudioProbe, String> {
                unreachable!()
            }
            fn capture_time(&self, _: &Path) -> Result<CaptureTime, String> {
                unreachable!()
            }
        }
        let items = vec![Media::from_path("a.wav".into(), "2026/diary".into()).unwrap()];
        for crash in [false, true] {
            let player = Player::default();
            let mut session = DiarySession::default();
            session
                .start(&items, &items[0], Arc::new(BrokenSource(crash)))
                .unwrap();
            until(&mut session, &player, |s, _| !s.is_loading());
            assert!(player.opened.lock().unwrap().is_none());
            assert!(session.status().contains(if crash {
                "unerwartet"
            } else {
                "Größe nicht lesbar"
            }));
        }
    }
}
