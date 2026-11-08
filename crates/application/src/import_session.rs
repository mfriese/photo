//! Import lifecycle, structured completion and refresh acknowledgement, independent of UI.
use crate::import::{self, Event, FileDecision, ImportAdapter, Job};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering, mpsc},
};
#[derive(Default)]
pub struct ImportSession {
    job: Option<Job>,
    target: Option<PathBuf>,
    source: Option<PathBuf>,
    preview: bool,
    log: Vec<Event>,
    current: usize,
    total: usize,
    status: String,
    copied: bool,
    refresh: Option<PathBuf>,
}
impl ImportSession {
    pub fn running(&self) -> bool {
        self.job.is_some()
    }
    pub fn log(&self) -> &[Event] {
        &self.log
    }
    pub fn status(&self) -> &str {
        &self.status
    }
    pub fn progress(&self) -> (usize, usize) {
        (self.current, self.total)
    }
    pub fn take_refresh(&mut self) -> Option<PathBuf> {
        self.refresh.take()
    }
    pub fn start(
        &mut self,
        adapter: Arc<dyn ImportAdapter>,
        source: PathBuf,
        target: PathBuf,
        preview: bool,
    ) -> Result<(), String> {
        if self.running() {
            return Err("Ein Import läuft bereits".into());
        }
        self.log.clear();
        self.current = 0;
        self.total = 0;
        self.status.clear();
        self.copied = false;
        self.refresh = None;
        self.target = Some(target.clone());
        self.source = Some(source.clone());
        self.preview = preview;
        self.job = Some(import::start(adapter, source, target, preview));
        Ok(())
    }
    pub fn can_cleanup(&self) -> bool {
        !self.running() && !self.preview && self.log.iter().any(|e| e.file_report.is_some())
    }
    pub fn cleanup_request(&self) -> Result<crate::import_cleanup::CleanupRequest, String> {
        if self.running() || self.preview {
            return Err(
                "Nur nach einem tatsächlichen abgeschlossenen oder abgebrochenen Import verfügbar"
                    .into(),
            );
        }
        let reports: Vec<_> = self
            .log
            .iter()
            .filter_map(|event| event.file_report.clone())
            .collect();
        if reports.is_empty() {
            return Err("Keine protokollierten Quelldateien verfügbar".into());
        }
        Ok(crate::import_cleanup::CleanupRequest {
            source_root: self.source.clone().ok_or("Keine Importquelle")?,
            target_root: self.target.clone().ok_or("Kein Importziel")?,
            reports,
        })
    }
    pub fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn poll(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        let mut done = false;
        for _ in 0..500 {
            match job.events.try_recv() {
                Ok(event) => {
                    if event.total > 0 {
                        self.current = event.file;
                        self.total = event.total;
                    }
                    if let Some(report) = &event.file_report
                        && report.decision == FileDecision::Copied
                    {
                        self.copied = true;
                    }
                    if let Some(outcome) = &event.outcome {
                        self.status = outcome.message();
                        done = true;
                    }
                    self.log.push(event);
                    if done {
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status="Importauftrag unerwartet beendet. Bereits kopierte Dateien bleiben erhalten.".into();
                    done = true;
                    break;
                }
            }
        }
        if done {
            self.job = None;
            if self.copied {
                self.refresh = self.target.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{CaptureTime, Media},
        import::{CopyProgress, FileReport},
    };
    use std::{
        path::Path,
        sync::atomic::{AtomicBool, AtomicUsize},
        time::{Duration, Instant},
    };
    enum Behaviour {
        Normal,
        CrashAfterFirst,
        WaitForCancel,
    }
    struct Adapter {
        behaviour: Behaviour,
        copies: AtomicUsize,
    }
    impl ImportAdapter for Adapter {
        fn validate(&self, _: &Path, _: &Path) -> Result<(), String> {
            Ok(())
        }
        fn discover(&self, _: &Path) -> Result<Vec<Media>, String> {
            Ok(["a.jpg", "b.jpg"]
                .into_iter()
                .map(|p| Media::from_path(p.into(), String::new()).unwrap())
                .collect())
        }
        fn capture_time(&self, media: &Media) -> Result<CaptureTime, String> {
            if matches!(self.behaviour, Behaviour::CrashAfterFirst)
                && media.path() == Path::new("b.jpg")
            {
                panic!("simulated adapter exit after a completed copy");
            }
            CaptureTime::parse(
                if media.path() == Path::new("a.jpg") {
                    "2023:12:27 22:51:00"
                } else {
                    "2023:12:27 22:51:01"
                },
                "test",
            )
        }
        fn exists(&self, _: &Path) -> Result<bool, String> {
            Ok(false)
        }
        fn identical(&self, _: &Path, _: &Path) -> Result<bool, String> {
            unreachable!()
        }
        fn copy_verified(
            &self,
            _: &Path,
            _: &Path,
            cancel: &AtomicBool,
            _: &dyn Fn(CopyProgress),
        ) -> Result<(), String> {
            self.copies.fetch_add(1, Ordering::SeqCst);
            if matches!(self.behaviour, Behaviour::WaitForCancel) {
                while !cancel.load(Ordering::Relaxed) {
                    std::thread::yield_now();
                }
                return Err("Abgebrochen".into());
            }
            Ok(())
        }
    }
    fn finish(session: &mut ImportSession) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while session.running() {
            session.poll();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn committed_copy_is_acknowledged_once_even_if_the_worker_then_crashes() {
        let adapter = Arc::new(Adapter {
            behaviour: Behaviour::CrashAfterFirst,
            copies: AtomicUsize::new(0),
        });
        let mut session = ImportSession::default();
        session
            .start(adapter.clone(), "source".into(), "library".into(), false)
            .unwrap();
        assert!(
            session
                .start(adapter, "source".into(), "other".into(), false)
                .is_err()
        );
        finish(&mut session);
        assert!(session.status().contains("unerwartet"));
        let cleanup = session.cleanup_request().unwrap();
        assert_eq!(cleanup.source_root, PathBuf::from("source"));
        assert_eq!(cleanup.target_root, PathBuf::from("library"));
        assert_eq!(cleanup.reports.len(), 1);

        assert_eq!(session.take_refresh(), Some(PathBuf::from("library")));
        assert!(session.take_refresh().is_none());
        assert!(session.log().iter().any(|e| matches!(
            &e.file_report,
            Some(FileReport {
                decision: FileDecision::Copied,
                ..
            })
        )));
    }
    #[test]
    fn preview_never_acknowledges_a_write_and_cancellation_is_not_a_file_error() {
        let adapter = Arc::new(Adapter {
            behaviour: Behaviour::Normal,
            copies: AtomicUsize::new(0),
        });
        let mut session = ImportSession::default();
        session
            .start(adapter.clone(), "source".into(), "library".into(), true)
            .unwrap();
        finish(&mut session);
        assert_eq!(adapter.copies.load(Ordering::SeqCst), 0);
        assert!(session.take_refresh().is_none());
        assert!(!session.can_cleanup());
        assert!(session.cleanup_request().is_err());
        let adapter = Arc::new(Adapter {
            behaviour: Behaviour::WaitForCancel,
            copies: AtomicUsize::new(0),
        });
        session
            .start(adapter.clone(), "source".into(), "library".into(), false)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while adapter.copies.load(Ordering::SeqCst) == 0 {
            session.poll();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        session.cancel();
        finish(&mut session);
        assert!(
            matches!(&session.log().last().unwrap().outcome,Some(import::ImportOutcome::Completed(summary)) if summary.aborted && summary.failed==0)
        );
        assert!(session.take_refresh().is_none());
    }
}
