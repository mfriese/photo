//! Guided duplicate-review state machine; UI only supplies choices and renders state.
use crate::domain::duplicates::CollisionGroup;
use crate::{
    duplicates::{self, DuplicateImages, Event, Job, Report},
    ports::MediaRepository,
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering, mpsc},
};
#[derive(Default)]
pub struct ReviewSession {
    job: Option<Job>,
    library: Option<PathBuf>,
    groups: Vec<CollisionGroup>,
    index: usize,
    report: Option<Report>,
    status: String,
    log: Vec<String>,
    refresh: bool,
    aborted: bool,
}
impl ReviewSession {
    pub fn running(&self) -> bool {
        self.job.is_some() || self.report.is_some()
    }
    pub fn waiting(&self) -> bool {
        self.job.is_some()
    }
    pub fn report(&self) -> Option<&Report> {
        self.report.as_ref()
    }
    pub fn position(&self) -> (usize, usize) {
        (self.index + 1, self.groups.len())
    }
    pub fn status(&self) -> &str {
        &self.status
    }
    pub fn log(&self) -> &[String] {
        &self.log
    }
    pub fn take_refresh(&mut self) -> bool {
        std::mem::take(&mut self.refresh)
    }
    pub fn start(
        &mut self,
        library: PathBuf,
        repo: Arc<dyn MediaRepository>,
    ) -> Result<(), String> {
        if self.running() {
            return Err("Eine Prüfung läuft bereits".into());
        }
        self.library = Some(library.clone());
        self.groups.clear();
        self.index = 0;
        self.log.clear();
        self.aborted = false;
        self.status = "Bibliothek nach Kollisionsgruppen durchsuchen …".into();
        self.job = Some(duplicates::discover(repo, library));
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.aborted = true;
        self.report = None;
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.status =
            "Abgebrochen. Bereits verschobene Dateien bleiben im Papierkorb wiederherstellbar."
                .into();
    }
    fn next(&mut self, adapter: Arc<dyn DuplicateImages>) {
        self.report = None;
        if self.aborted {
            return;
        }
        if let Some(group) = self.groups.get(self.index) {
            self.status = format!(
                "Gruppe {} / {} wird verglichen …",
                self.index + 1,
                self.groups.len()
            );
            self.job = Some(duplicates::compare(adapter, group.clone()));
        } else {
            self.status = "Duplikatsuche abgeschlossen. Entscheidungen stehen im Protokoll.".into();
        }
    }
    pub fn keep_all(&mut self, adapter: Arc<dyn DuplicateImages>) -> Result<(), String> {
        if self.report.is_none() {
            return Err("Es liegt kein abgeschlossener Gruppenvergleich vor".into());
        }
        self.log
            .push(format!("Gruppe {}: alle Dateien behalten", self.index + 1));
        self.index += 1;
        self.next(adapter);
        Ok(())
    }
    pub fn remove_selected(
        &mut self,
        adapter: Arc<dyn DuplicateImages>,
        keepers: Vec<usize>,
    ) -> Result<(), String> {
        let report = self
            .report
            .as_ref()
            .ok_or("Es liegt kein abgeschlossener Gruppenvergleich vor")?;
        report.removal_plan(&keepers)?;
        let library = self.library.clone().ok_or("Keine Bibliothek gewählt")?;
        self.job = Some(duplicates::remove(
            adapter,
            library,
            self.report.take().unwrap(),
            keepers,
        ));
        Ok(())
    }
    pub fn poll(&mut self, adapter: Arc<dyn DuplicateImages>) {
        let mut events = Vec::new();
        let mut disconnected = false;
        if let Some(job) = &self.job {
            loop {
                match job.receiver.try_recv() {
                    Ok(e) => events.push(e),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        let terminal = events.iter().any(|e| !matches!(e, Event::Progress(_)));
        for event in events {
            match event {
                Event::Groups(groups) => {
                    self.job = None;
                    if !self.aborted {
                        self.groups = groups;
                        self.index = 0;
                        self.log
                            .push(format!("{} Kollisionsgruppen gefunden", self.groups.len()));
                        self.next(adapter.clone());
                    }
                }
                Event::Progress(message) => {
                    if !self.aborted {
                        self.status = message;
                    }
                }
                Event::Report(report) => {
                    self.job = None;
                    if !self.aborted {
                        self.report = Some(report);
                        self.status = "Vergleich abgeschlossen. Bitte eine Option wählen.".into();
                    }
                }
                Event::Failed(error) => {
                    self.job = None;
                    self.status = error;
                }
                Event::Removed(results) => {
                    self.job = None;
                    for (path, result) in results {
                        match result {
                            Ok(()) => {
                                self.refresh = true;
                                self.log.push(format!("Papierkorb: {}", path.display()));
                            }
                            Err(error) => self
                                .log
                                .push(format!("BEHALTEN: {} · {error}", path.display())),
                        }
                    }
                    if !self.aborted {
                        self.index += 1;
                        self.next(adapter.clone());
                    }
                }
            }
        }
        if disconnected && !terminal && self.job.is_some() {
            self.job = None;
            if !self.aborted {
                self.status = "Auftrag unerwartet beendet. Keine weitere Gruppe gestartet.".into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{domain::Media, duplicates::Comparison};
    use std::sync::atomic::AtomicUsize;
    struct Adapter {
        removed: AtomicUsize,
    }
    impl MediaRepository for Adapter {
        fn scan(&self, _: &std::path::Path) -> (Vec<Media>, usize) {
            (
                [
                    "img_20080916_035811.jpg",
                    "img_20080916_035811_x.jpg",
                    "img_20080917_035811.jpg",
                    "img_20080917_035811_y.jpg",
                ]
                .into_iter()
                .map(|p| Media::from_path(p.into(), String::new()).unwrap())
                .collect(),
                0,
            )
        }
    }
    impl DuplicateImages for Adapter {
        fn compare(&self, _: &std::path::Path, _: &std::path::Path) -> Result<Comparison, String> {
            Ok(Comparison::Identical)
        }
        fn trash_identical(
            &self,
            _: &std::path::Path,
            _: &std::path::Path,
            _: &std::path::Path,
            _: &AtomicBool,
        ) -> Result<(), String> {
            self.removed.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }
    use std::sync::atomic::AtomicBool;
    fn wait(session: &mut ReviewSession, adapter: Arc<Adapter>) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while session.waiting() {
            session.poll(adapter.clone());
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn reviews_exactly_one_group_then_advances_after_decision() {
        let adapter = Arc::new(Adapter {
            removed: AtomicUsize::new(0),
        });
        let mut session = ReviewSession::default();
        session.start("library".into(), adapter.clone()).unwrap();
        wait(&mut session, adapter.clone());
        assert_eq!(session.position(), (1, 2));
        assert!(session.report().is_some());
        assert_eq!(adapter.removed.load(Ordering::Relaxed), 0);
        assert!(session.remove_selected(adapter.clone(), vec![100]).is_err());
        assert!(session.report().is_some());
        session.remove_selected(adapter.clone(), vec![0]).unwrap();
        wait(&mut session, adapter.clone());
        assert_eq!(session.position(), (2, 2));
        assert_eq!(adapter.removed.load(Ordering::Relaxed), 1);
        assert!(session.take_refresh());
        session.keep_all(adapter.clone()).unwrap();
        assert!(!session.running());
    }
    #[test]
    fn cancellation_during_review_never_starts_next_group() {
        let adapter = Arc::new(Adapter {
            removed: AtomicUsize::new(0),
        });
        let mut session = ReviewSession::default();
        session.start("library".into(), adapter.clone()).unwrap();
        wait(&mut session, adapter.clone());
        session.cancel();
        session.poll(adapter.clone());
        assert!(!session.running());
        assert_eq!(session.position(), (1, 2));
        assert!(session.keep_all(adapter.clone()).is_err());
        assert_eq!(adapter.removed.load(Ordering::Relaxed), 0);
    }
}
