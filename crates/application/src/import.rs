//! Import application service: naming and collision policy independent of GUI/storage.
use crate::domain::CaptureTime;
use crate::domain::Media;
pub use crate::domain::PhotoImportLayout;
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
    fn verify_audio(&self, _media: &Media) -> Result<(), String> {
        Err("Audioformat konnte nicht bestätigt werden".into())
    }
    fn file_dates(&self, _media: &Media) -> Result<crate::domain::import_date::FileDates, String> {
        Err("Dateisystemdaten nicht verfügbar".into())
    }
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
    pub tags: Vec<String>,
    pub tag_error: Option<String>,
    pub collision_renamed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Neutral,
    Success,
    Warning,
    Error,
}
impl FileReport {
    pub fn level(&self) -> LogLevel {
        if self.tag_error.is_some() || matches!(self.decision, FileDecision::Failed(_)) {
            LogLevel::Error
        } else if self.collision_renamed
            || self.captured.as_ref().is_some_and(CaptureTime::is_fallback)
            || matches!(
                self.decision,
                FileDecision::SkippedVideoName | FileDecision::Cancelled
            )
        {
            LogLevel::Warning
        } else {
            LogLevel::Success
        }
    }
}
/// Final file severity also colors its earlier detailed steps; no text parsing.
pub fn log_levels(events: &[Event]) -> Vec<LogLevel> {
    let files: std::collections::BTreeMap<_, _> = events
        .iter()
        .filter_map(|event| {
            event
                .file_report
                .as_ref()
                .map(|report| (event.file, report.level()))
        })
        .collect();
    events
        .iter()
        .map(|event| match &event.outcome {
            Some(ImportOutcome::Failed(_)) => LogLevel::Error,
            Some(ImportOutcome::Completed(summary))
                if summary.failed > 0
                    || summary.tag_index_error.is_some()
                    || files.values().any(|level| *level == LogLevel::Error) =>
            {
                LogLevel::Error
            }
            Some(ImportOutcome::Completed(summary))
                if summary.aborted || files.values().any(|level| *level == LogLevel::Warning) =>
            {
                LogLevel::Warning
            }
            Some(ImportOutcome::Completed(_)) => LogLevel::Success,
            None => files.get(&event.file).copied().unwrap_or(LogLevel::Neutral),
        })
        .collect()
}
#[derive(Clone, Debug)]
pub struct ImportSummary {
    pub copied: usize,
    pub skipped: usize,
    pub failed: usize,
    pub preview: bool,
    pub aborted: bool,
    pub tag_index_error: Option<String>,
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
                "{}: {} {}, {} übersprungen, {} Fehler{}{}",
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
                },
                s.tag_index_error.as_ref().map(|e|format!(" · Tagindex: {e}; Einzel-YAMLs bleiben erhalten. Tags aus Einzeldateien neu aufbauen.")).unwrap_or_default()
            ),
        }
    }
}
pub struct Job {
    pub events: mpsc::Receiver<Event>,
    pub cancel: Arc<AtomicBool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
pub fn start(
    adapter: Arc<dyn ImportAdapter>,
    source: PathBuf,
    target: PathBuf,
    preview: bool,
) -> Job {
    start_tagged(adapter, source, target, preview, None)
}
#[derive(Clone)]
pub struct TaggingOptions {
    pub repository: Arc<dyn crate::organization::CatalogRepository>,
    pub tags: Vec<String>,
}
pub fn start_tagged(
    adapter: Arc<dyn ImportAdapter>,
    source: PathBuf,
    target: PathBuf,
    preview: bool,
    tagging: Option<TaggingOptions>,
) -> Job {
    start_with_options(
        adapter,
        source,
        target,
        ImportOptions {
            preview,
            tagging,
            ..Default::default()
        },
    )
}
#[derive(Clone, Default)]
pub struct ImportOptions {
    pub preview: bool,
    pub tagging: Option<TaggingOptions>,
    pub photo_layout: PhotoImportLayout,
}
pub fn start_with_options(
    adapter: Arc<dyn ImportAdapter>,
    source: PathBuf,
    target: PathBuf,
    options: ImportOptions,
) -> Job {
    let ImportOptions {
        preview,
        tagging,
        photo_layout,
    } = options;
    let (tx, events) = mpsc::sync_channel(128);
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
        emit(
            0,
            0,
            match photo_layout {
                PhotoImportLayout::YearMonth => "Fotoablage: Jahr/Monat",
                PhotoImportLayout::Scans => "Fotoablage: Jahr/scans (ohne weitere Unterordner)",
            }
            .into(),
        );
        let result = (|| {
            adapter.validate(&source, &target)?;
            let names = tagging
                .as_ref()
                .map(|t| {
                    t.tags
                        .iter()
                        .map(|n| crate::domain::tags::normalize_tag_name(n))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            if !names.is_empty() {
                emit(
                    0,
                    0,
                    format!(
                        "{}Tags: {}",
                        if preview { "Geplante " } else { "Import-" },
                        names.join(", ")
                    ),
                );
            }
            let mut tag_batch = if !preview && !names.is_empty() {
                Some(crate::organization::index::ImportTags::new_with_cancel(
                    tagging.as_ref().unwrap().repository.clone(),
                    target.clone(),
                    &names,
                    &stop,
                )?)
            } else {
                None
            };
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
                    tags: Vec::new(),
                    tag_error: None,
                    collision_renamed: false,
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
                    let primary = adapter.capture_time(media);
                    let time = match primary {
                        Ok(date) if date.source() != crate::domain::DateSource::FileSystem => date,
                        previous => {
                            let verify_audio = media.is_audio() && previous.is_err();
                            let reason = match previous {
                                Ok(date) => date.origin().to_owned(),
                                Err(error) => error,
                            };
                            if verify_audio {
                                adapter.verify_audio(media).map_err(|error| {
                                    format!("{reason}; Audio nicht importierbar: {error}")
                                })?;
                            }
                            log(format!(
                                "Kein belegtes Aufnahmedatum: {reason}. Dateisystem-Fallback wird geprüft."
                            ));
                            let dates = adapter.file_dates(media).map_err(|e| {
                                format!("{reason}; Dateisystem-Fallback fehlgeschlagen: {e}")
                            })?;
                            dates.oldest().map_err(|e| format!("{reason}; {e}"))?
                        }
                    };
                    report.captured = Some(time.clone());
                    log(format!("Datum: {} ({})", time.stamp(), time.origin()));
                    let mut suffix = 0;
                    let destination = loop {
                        if stop.load(Ordering::Relaxed) {
                            return Err("Abgebrochen".into());
                        }
                        report.collision_renamed = suffix > 0;
                        let candidate =
                            time.target_with_layout(&target, media, suffix, photo_layout);
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
                let tag_eligible = matches!(
                    report.decision,
                    FileDecision::Copied | FileDecision::Planned
                );
                if tag_eligible && !names.is_empty() {
                    report.tags = names.clone();
                    if let (Some(batch), Some(destination)) = (&mut tag_batch, &report.target) {
                        match batch.assign(destination) {
                            Ok(()) => log(format!(
                                "Tags in Einzel-YAML gespeichert: {}",
                                names.join(", ")
                            )),
                            Err(error) => {
                                failed += 1;
                                report.tag_error = Some(error.clone());
                                log(format!(
                                    "Datei erhalten; Tags konnten nicht gespeichert werden: {error}"
                                ));
                            }
                        }
                    }
                }
                let _ = tx.send(Event {
                    file: i + 1,
                    total,
                    message: String::new(),
                    outcome: None,
                    file_report: Some(report),
                });
            }
            let mut tag_index_error = None;
            if let Some(batch) = &mut tag_batch
                && let Err(error) = batch.finish()
            {
                failed += 1;
                tag_index_error = Some(error.clone());
                emit(
                    0,
                    total,
                    format!(
                        "Tag-Einzeldateien bleiben erhalten; Tagindex nicht aktualisiert: {error}. Tags aus Einzeldateien neu aufbauen."
                    ),
                );
            }
            Ok::<_, String>(ImportSummary {
                copied,
                skipped,
                failed,
                preview,
                aborted: stop.load(Ordering::Relaxed),
                tag_index_error,
            })
        })();
        let outcome = match result {
            Ok(summary) => ImportOutcome::Completed(summary),
            Err(_) if stop.load(Ordering::Relaxed) => ImportOutcome::Completed(ImportSummary {
                copied: 0,
                skipped: 0,
                failed: 0,
                preview,
                aborted: true,
                tag_index_error: None,
            }),
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

#[cfg(test)]
mod severity_tests {
    use super::*;
    use crate::domain::DateSource;
    fn report() -> FileReport {
        FileReport {
            source: "photo.jpg".into(),
            target: None,
            captured: Some(CaptureTime::parse("2026:11:14 12:00:00", "EXIF").unwrap()),
            decision: FileDecision::Copied,
            tags: vec![],
            tag_error: None,
            collision_renamed: false,
        }
    }
    #[test]
    fn report_levels_distinguish_success_fallback_collision_skip_cancel_and_errors() {
        let mut value = report();
        assert_eq!(value.level(), LogLevel::Success);
        value.captured = Some(value.captured.unwrap().with_source(DateSource::FileSystem));
        assert_eq!(value.level(), LogLevel::Warning);
        value = report();
        value.collision_renamed = true;
        assert_eq!(value.level(), LogLevel::Warning);
        value = report();
        value.decision = FileDecision::SkippedVideoName;
        assert_eq!(value.level(), LogLevel::Warning);
        value.decision = FileDecision::SkippedIdentical;
        assert_eq!(value.level(), LogLevel::Success);
        value.decision = FileDecision::Cancelled;
        assert_eq!(value.level(), LogLevel::Warning);
        value.decision = FileDecision::Failed("failure".into());
        assert_eq!(value.level(), LogLevel::Error);
        value = report();
        value.tag_error = Some("tags failed".into());
        assert_eq!(value.level(), LogLevel::Error);
    }
    #[test]
    fn detailed_steps_inherit_typed_file_severity_without_parsing_messages() {
        let mut value = report();
        value.captured = Some(value.captured.unwrap().with_source(DateSource::FileSystem));
        let events = vec![
            Event {
                file: 1,
                total: 1,
                message: "any translated step".into(),
                outcome: None,
                file_report: None,
            },
            Event {
                file: 1,
                total: 1,
                message: String::new(),
                outcome: None,
                file_report: Some(value),
            },
        ];
        assert_eq!(
            log_levels(&events),
            vec![LogLevel::Warning, LogLevel::Warning]
        );
    }
}

#[cfg(test)]
mod audio_fallback_tests {
    use super::*;
    struct Adapter {
        valid: bool,
        copies: std::sync::atomic::AtomicUsize,
    }
    impl ImportAdapter for Adapter {
        fn validate(&self, _: &Path, _: &Path) -> Result<(), String> {
            Ok(())
        }
        fn discover(&self, _: &Path) -> Result<Vec<Media>, String> {
            Ok(vec![
                Media::from_audio_asset("recording.wav".into(), String::new()).unwrap(),
            ])
        }
        fn capture_time(&self, _: &Media) -> Result<CaptureTime, String> {
            Err("keine Metadaten".into())
        }
        fn verify_audio(&self, _: &Media) -> Result<(), String> {
            if self.valid {
                Ok(())
            } else {
                Err("ungültiges Audio".into())
            }
        }
        fn file_dates(&self, _: &Media) -> Result<crate::domain::import_date::FileDates, String> {
            assert!(self.valid, "invalid audio must not reach date fallback");
            Ok(crate::domain::import_date::FileDates {
                created: None,
                modified: Some(crate::domain::import_date::FileTimestamp {
                    unix_seconds: 1,
                    local: CaptureTime::parse("2020:01:02 03:04:05", "FALLBACK Inhaltsänderung")
                        .unwrap(),
                }),
            })
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
            _: &AtomicBool,
            _: &dyn Fn(CopyProgress),
        ) -> Result<(), String> {
            self.copies.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }
    #[test]
    fn filesystem_date_fallback_requires_valid_audio_and_never_bypasses_asset_validation() {
        for valid in [false, true] {
            let adapter = Arc::new(Adapter {
                valid,
                copies: Default::default(),
            });
            let job = start(adapter.clone(), "source".into(), "library".into(), false);
            let mut report = None;
            loop {
                let event = job
                    .events
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap();
                let done = event.is_terminal();
                if event.file_report.is_some() {
                    report = event.file_report;
                }
                if done {
                    break;
                }
            }
            assert_eq!(adapter.copies.load(Ordering::Relaxed), usize::from(valid));
            assert_eq!(
                report.unwrap().level(),
                if valid {
                    LogLevel::Warning
                } else {
                    LogLevel::Error
                }
            );
        }
    }
}
