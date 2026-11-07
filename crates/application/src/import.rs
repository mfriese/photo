//! Import application service: naming and collision policy independent of GUI/storage.
use crate::domain::CaptureTime;
use crate::domain::Media;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
#[derive(Clone, Debug)]
pub enum CopyProgress {
    Bytes(u64, u64),
    Verify,
    Publish,
}
pub trait ImportAdapter: Send + Sync + 'static {
    fn validate(&self, source: &Path, target: &Path) -> Result<(), String>;
    fn discover(&self, source: &Path) -> Result<Vec<Media>, String>;
    fn capture_time(&self, media: &Media) -> Result<CaptureTime, String>;
    fn exists(&self, path: &Path) -> Result<bool, String>;
    fn identical(&self, source: &Path, target: &Path) -> Result<bool, String>;
    fn copy_verified(
        &self,
        source: &Path,
        target: &Path,
        cancel: &AtomicBool,
        progress: &dyn Fn(CopyProgress),
    ) -> Result<(), String>;
}
#[derive(Clone, Debug)]
pub struct Event {
    pub file: usize,
    pub total: usize,
    pub message: String,
    pub outcome: Option<ImportOutcome>,
    pub file_report: Option<FileReport>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileDecision {
    Planned,
    Copied,
    SkippedIdentical,
    SkippedVideoName,
    Failed(String),
    Cancelled,
}
#[derive(Clone, Debug)]
pub struct FileReport {
    pub source: PathBuf,
    pub target: Option<PathBuf>,
    pub captured: Option<CaptureTime>,
    pub decision: FileDecision,
}
#[derive(Clone, Debug)]
pub struct ImportSummary {
    pub copied: usize,
    pub skipped: usize,
    pub failed: usize,
    pub preview: bool,
    pub aborted: bool,
}
#[derive(Clone, Debug)]
pub enum ImportOutcome {
    Completed(ImportSummary),
    Failed(String),
}
impl Event {
    pub fn is_terminal(&self) -> bool {
        self.outcome.is_some()
    }
}
impl ImportOutcome {
    pub fn message(&self) -> String {
        match self {
            Self::Failed(error) => format!("Import nicht möglich: {error}"),
            Self::Completed(s) => format!(
                "{}: {} {}, {} übersprungen, {} Fehler{}",
                if s.aborted {
                    "Abgebrochen"
                } else {
                    "Abgeschlossen"
                },
                s.copied,
                if s.preview { "geplant" } else { "kopiert" },
                s.skipped,
                s.failed,
                if s.preview {
                    " · Prüflauf verändert keine Dateien; Import prüft erneut"
                } else {
                    ""
                }
            ),
        }
    }
}
pub struct Job {
    pub events: mpsc::Receiver<Event>,
    pub cancel: Arc<AtomicBool>,
}
pub fn start(
    adapter: Arc<dyn ImportAdapter>,
    source: PathBuf,
    target: PathBuf,
    preview: bool,
) -> Job {
    let (tx, events) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    std::thread::spawn(move || {
        let emit = |file, total, message| {
            let _ = tx.send(Event {
                file,
                total,
                message,
                outcome: None,
                file_report: None,
            });
        };
        emit(
            0,
            0,
            format!(
                "{}: Quelle {} → Bibliothek {}",
                if preview { "Prüflauf" } else { "Import" },
                source.display(),
                target.display()
            ),
        );
        let result = (|| {
            adapter.validate(&source, &target)?;
            emit(
                0,
                0,
                "Verzeichnisse geprüft; Quelle bleibt unverändert. Suche Medien …".into(),
            );
            let items = adapter.discover(&source)?;
            let total = items.len();
            let mut planned = std::collections::HashMap::<PathBuf, PathBuf>::new();
            let (mut copied, mut skipped, mut failed) = (0, 0, 0);
            for (i, media) in items.iter().enumerate() {
                if stop.load(Ordering::Relaxed) {
                    emit(
                        i,
                        total,
                        "Abgebrochen. Bereits abgeschlossene Kopien bleiben erhalten.".into(),
                    );
                    break;
                }
                let log = |message| emit(i + 1, total, message);
                log(format!("Quelle: {}", media.path().display()));
                let mut report = FileReport {
                    source: media.path().into(),
                    target: None,
                    captured: None,
                    decision: if preview {
                        FileDecision::Planned
                    } else {
                        FileDecision::Copied
                    },
                };
                let action = (|| {
                    log("Aufnahmedatum aus Metadaten lesen …".into());
                    let time = adapter.capture_time(media)?;
                    report.captured = Some(time.clone());
                    log(format!("Datum: {} ({})", time.stamp(), time.origin()));
                    let mut suffix = 0;
                    let destination = loop {
                        if stop.load(Ordering::Relaxed) {
                            return Err("Abgebrochen".into());
                        }
                        let candidate = time.target(&target, media, suffix);
                        report.target = Some(candidate.clone());
                        log(format!("Ziel prüfen: {}", candidate.display()));
                        if !planned.contains_key(&candidate) && !adapter.exists(&candidate)? {
                            break candidate;
                        }
                        if media.is_video() {
                            report.decision = FileDecision::SkippedVideoName;
                            log("Übersprungen: Videoname existiert bereits (kein Inhaltsvergleich).".into());
                            skipped += 1;
                            return Ok(());
                        }
                        log("Namenskollision: Dateiinhalt bytegenau vergleichen …".into());
                        if adapter.identical(
                            media.path(),
                            planned.get(&candidate).unwrap_or(&candidate),
                        )? {
                            report.decision = FileDecision::SkippedIdentical;
                            log("Übersprungen: identischer Dateiinhalt.".into());
                            skipped += 1;
                            return Ok(());
                        }
                        log("Unterschiedlicher Inhalt: nächsten Nummernzusatz prüfen.".into());
                        suffix += 1;
                    };
                    if preview {
                        planned.insert(destination.clone(), media.path().to_path_buf());
                        log(format!(
                            "Geplant: {} → {}",
                            media.path().display(),
                            destination.display()
                        ));
                    } else {
                        log("Kopieren in temporäre Datei …".into());
                        adapter.copy_verified(media.path(), &destination, &stop, &|progress| {
                            log(match progress {
                                CopyProgress::Bytes(done, total) => {
                                    format!("Kopiert: {done} / {total} Bytes")
                                }
                                CopyProgress::Verify => {
                                    "Kopie bytegenau gegen Quelle verifizieren …".into()
                                }
                                CopyProgress::Publish => {
                                    "Verifizierte Kopie ohne Überschreiben an Ziel übernehmen …"
                                        .into()
                                }
                            })
                        })?;
                        log(format!(
                            "Verifiziert und veröffentlicht: {}",
                            destination.display()
                        ));
                    }
                    copied += 1;
                    Ok::<(), String>(())
                })();
                if let Err(e) = action {
                    if !stop.load(Ordering::Relaxed) {
                        failed += 1;
                    }
                    report.decision = if stop.load(Ordering::Relaxed) {
                        FileDecision::Cancelled
                    } else {
                        FileDecision::Failed(e.clone())
                    };
                    log(format!("Nicht importiert: {e}"));
                }
                let _ = tx.send(Event {
                    file: i + 1,
                    total,
                    message: String::new(),
                    outcome: None,
                    file_report: Some(report),
                });
            }
            Ok::<_, String>(ImportSummary {
                copied,
                skipped,
                failed,
                preview,
                aborted: stop.load(Ordering::Relaxed),
            })
        })();
        let outcome = match result {
            Ok(summary) => ImportOutcome::Completed(summary),
            Err(error) => ImportOutcome::Failed(error),
        };
        let _ = tx.send(Event {
            file: 0,
            total: 0,
            message: outcome.message(),
            outcome: Some(outcome),
            file_report: None,
        });
    });
    Job { events, cancel }
}

#[cfg(test)]
mod use_case_tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    struct MemoryAdapter {
        files: Mutex<HashMap<PathBuf, Vec<u8>>>,
    }
    impl ImportAdapter for MemoryAdapter {
        fn validate(&self, _: &Path, _: &Path) -> Result<(), String> {
            Ok(())
        }
        fn discover(&self, _: &Path) -> Result<Vec<Media>, String> {
            Ok(["a.jpg", "b.jpg", "c.jpg"]
                .into_iter()
                .map(|p| Media::from_path(p.into(), String::new()).unwrap())
                .collect())
        }
        fn capture_time(&self, _: &Media) -> Result<CaptureTime, String> {
            CaptureTime::parse("2008:09:16 03:58:11", "EXIF")
        }
        fn exists(&self, path: &Path) -> Result<bool, String> {
            Ok(self.files.lock().unwrap().contains_key(path))
        }
        fn identical(&self, source: &Path, target: &Path) -> Result<bool, String> {
            let files = self.files.lock().unwrap();
            Ok(files.get(source) == files.get(target))
        }
        fn copy_verified(
            &self,
            source: &Path,
            target: &Path,
            _: &AtomicBool,
            _: &dyn Fn(CopyProgress),
        ) -> Result<(), String> {
            let mut files = self.files.lock().unwrap();
            assert!(!files.contains_key(target));
            let value = files[source].clone();
            files.insert(target.into(), value);
            Ok(())
        }
    }
    #[test]
    fn import_policy_runs_without_filesystem_and_returns_typed_summary() {
        let adapter = Arc::new(MemoryAdapter {
            files: Mutex::new(
                [("a.jpg", vec![1]), ("b.jpg", vec![1]), ("c.jpg", vec![2])]
                    .into_iter()
                    .map(|(p, b)| (p.into(), b))
                    .collect(),
            ),
        });
        for preview in [true, false] {
            let job = start(adapter.clone(), "source".into(), "library".into(), preview);
            let mut reports = Vec::new();
            let outcome = loop {
                let event = job
                    .events
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap();
                if let Some(report) = event.file_report {
                    reports.push(report);
                }
                if let Some(outcome) = event.outcome {
                    break outcome;
                }
            };
            let ImportOutcome::Completed(summary) = outcome else {
                panic!("expected completion")
            };
            assert_eq!(reports.len(), 3);
            assert_eq!(reports[1].decision, FileDecision::SkippedIdentical);
            assert!(
                reports
                    .iter()
                    .all(|report| report.captured.is_some() && report.target.is_some())
            );
            assert_eq!(
                reports[2].target.as_deref(),
                Some(Path::new("library/2008/09/img_20080916_035811_001.jpg"))
            );
            assert_eq!(
                reports[0].decision,
                if preview {
                    FileDecision::Planned
                } else {
                    FileDecision::Copied
                }
            );
            assert_eq!((summary.copied, summary.skipped, summary.failed), (2, 1, 0));
            assert_eq!(summary.preview, preview);
            assert!(!summary.aborted);
            assert_eq!(
                adapter.files.lock().unwrap().len(),
                if preview { 3 } else { 5 }
            );
        }
        assert!(
            adapter
                .files
                .lock()
                .unwrap()
                .contains_key(Path::new("library/2008/09/img_20080916_035811_001.jpg"))
        );
    }
}
