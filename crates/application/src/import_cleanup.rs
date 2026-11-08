//! Explicit two-phase source cleanup. Import itself never removes its sources.
use crate::import::{FileDecision, FileReport};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileVersion {
    pub device: u64,
    pub inode: u64,
    pub size: u64,
    pub modified: (i64, i64),
    pub changed: (i64, i64),
}
#[derive(Clone, Debug)]
pub struct Proof {
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    pub source: PathBuf,
    pub target: PathBuf,
    pub source_version: FileVersion,
    pub target_version: FileVersion,
    pub source_root_id: (u64, u64),
    pub target_root_id: (u64, u64),
}
pub trait Trash: Send + Sync + 'static {
    fn move_to_trash(&self, path: &Path) -> Result<(), String>;
}
pub trait CleanupPort: Send + Sync + 'static {
    fn check(
        &self,
        source_root: &Path,
        target_root: &Path,
        source: &Path,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<Proof, String>;
    /// Must recheck the exact reviewed versions and full bytes immediately before Trash.
    fn trash_checked(&self, proof: &Proof, cancel: &AtomicBool) -> Result<(), String>;
}
#[derive(Clone)]
pub struct CleanupRequest {
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    pub reports: Vec<FileReport>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Pending,
    Ready,
    Blocked(String),
    Trashed,
    Moving,
    Unknown(String),
    Kept(String),
}
pub struct Row {
    pub source: PathBuf,
    pub target: Option<PathBuf>,
    pub decision: FileDecision,
    pub status: Status,
}
enum Event {
    Checked(usize, Result<Box<Proof>, String>),
    Removing(usize),
    Removed(usize, Result<(), String>),
    Done,
}
struct Job {
    receiver: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
#[derive(Default)]
pub struct CleanupSession {
    rows: Vec<Row>,
    proofs: Vec<Option<Proof>>,
    job: Option<Job>,
    reviewed: bool,
    confirmed: bool,
    cancelled: bool,
    status: String,
}
impl CleanupSession {
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    pub fn running(&self) -> bool {
        self.job.is_some()
    }
    pub fn status(&self) -> &str {
        &self.status
    }
    pub fn ready_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| r.status == Status::Ready)
            .count()
    }
    pub fn can_confirm(&self) -> bool {
        self.reviewed
            && !self.confirmed
            && !self.cancelled
            && !self.running()
            && self.ready_count() > 0
    }
    pub fn start(
        &mut self,
        request: CleanupRequest,
        port: Arc<dyn CleanupPort>,
    ) -> Result<(), String> {
        if self.running() {
            return Err("Eine Quellenprüfung läuft bereits".into());
        }
        if request.reports.is_empty() {
            return Err("Keine protokollierten Quelldateien verfügbar".into());
        }
        self.rows = request
            .reports
            .iter()
            .map(|r| Row {
                source: r.source.clone(),
                target: r.target.clone(),
                decision: r.decision.clone(),
                status: Status::Pending,
            })
            .collect();
        self.proofs = vec![None; self.rows.len()];
        self.reviewed = false;
        self.confirmed = false;
        self.cancelled = false;
        self.status = "Quelle und Ziel werden pro Datei bytegenau geprüft …".into();
        let (sender, receiver) = mpsc::sync_channel(16);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        self.job = Some(Job { receiver, cancel });
        std::thread::spawn(move || {
            for (index, report) in request.reports.into_iter().enumerate() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let result = if !matches!(
                    report.decision,
                    FileDecision::Copied
                        | FileDecision::SkippedIdentical
                        | FileDecision::SkippedVideoName
                ) {
                    Err(
                        "Nicht erfolgreich importiert oder nur geplant; Quelle bleibt erhalten"
                            .into(),
                    )
                } else if let Some(target) = &report.target {
                    port.check(
                        &request.source_root,
                        &request.target_root,
                        &report.source,
                        target,
                        &stop,
                    )
                } else {
                    Err("Kein bestätigter Zielpfad vorhanden".into())
                };
                if sender
                    .send(Event::Checked(index, result.map(Box::new)))
                    .is_err()
                {
                    return;
                }
            }
            let _ = sender.send(Event::Done);
        });
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.reviewed = false;
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.status = "Abgebrochen. Noch nicht verschobene Quellen bleiben erhalten.".into();
    }
    pub fn confirm(&mut self, port: Arc<dyn CleanupPort>) -> Result<(), String> {
        if !self.can_confirm() {
            return Err("Zuerst eine vollständige aktuelle Prüfung abschließen".into());
        }
        let plan: Vec<_> = self
            .proofs
            .iter()
            .enumerate()
            .filter_map(|(index, p)| {
                if self.rows[index].status == Status::Ready {
                    p.clone().map(|proof| (index, proof))
                } else {
                    None
                }
            })
            .collect();
        self.confirmed = true;
        self.status =
            "Freigegebene Quellen erneut prüfen und in den Papierkorb verschieben …".into();
        let (sender, receiver) = mpsc::sync_channel(16);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        self.job = Some(Job { receiver, cancel });
        std::thread::spawn(move || {
            for (index, proof) in plan {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if sender.send(Event::Removing(index)).is_err() {
                    return;
                }
                let result = port.trash_checked(&proof, &stop);
                if sender.send(Event::Removed(index, result)).is_err() {
                    return;
                }
            }
            let _ = sender.send(Event::Done);
        });
        Ok(())
    }
    pub fn poll(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        let mut done = false;
        for _ in 0..64 {
            match job.receiver.try_recv() {
                Ok(Event::Checked(index, result)) => match result {
                    Ok(proof) => {
                        self.rows[index].status = Status::Ready;
                        self.proofs[index] = Some(*proof);
                    }
                    Err(error) => self.rows[index].status = Status::Blocked(error),
                },
                Ok(Event::Removing(index)) => self.rows[index].status = Status::Moving,
                Ok(Event::Removed(index, result)) => {
                    self.rows[index].status = match result {
                        Ok(()) => Status::Trashed,
                        Err(error) => Status::Kept(error),
                    };
                }
                Ok(Event::Done) => {
                    done = true;
                    self.reviewed = !self.cancelled && !self.confirmed;
                    if !self.cancelled {
                        self.status = if self.confirmed {
                            format!(
                                "Abgeschlossen: {} Quellen im Papierkorb; übrige Quellen erhalten",
                                self.rows
                                    .iter()
                                    .filter(|r| r.status == Status::Trashed)
                                    .count()
                            )
                        } else {
                            format!(
                                "Prüfung abgeschlossen: {} von {} Quellen freigegeben. Bitte Ergebnisse prüfen und ausdrücklich bestätigen.",
                                self.ready_count(),
                                self.rows.len()
                            )
                        };
                    }
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    done = true;
                    self.reviewed = false;
                    self.status="Auftrag unerwartet beendet. Bestätigte Papierkorbaktionen sind protokolliert; unklare Zeilen bitte erneut prüfen.".into();
                    break;
                }
            }
        }
        if done {
            self.job = None;
            for row in &mut self.rows {
                if self.confirmed && row.status == Status::Moving {
                    row.status = Status::Unknown(
                        "Vorgang ohne Abschlussquittung; Quelle und Papierkorb erneut prüfen"
                            .into(),
                    );
                }
                if self.confirmed && row.status == Status::Ready {
                    row.status = Status::Kept("Nicht ausgeführt; Quelle bleibt erhalten".into());
                }
                if row.status == Status::Pending {
                    row.status = Status::Blocked("Nicht geprüft; Quelle bleibt erhalten".into());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Mutex, atomic::AtomicUsize},
        time::{Duration, Instant},
    };
    #[derive(Default)]
    struct Port {
        checks: AtomicUsize,
        moved: Mutex<Vec<PathBuf>>,
        gate: AtomicBool,
        release: AtomicBool,
        entered: AtomicBool,
        crash: AtomicBool,
    }
    fn proof(source: &Path, target: &Path) -> Proof {
        let version = FileVersion {
            device: 1,
            inode: 1,
            size: 1,
            modified: (1, 0),
            changed: (1, 0),
        };
        Proof {
            source_root: "source".into(),
            target_root: "library".into(),
            source: source.into(),
            target: target.into(),
            source_version: version.clone(),
            target_version: version,
            source_root_id: (1, 1),
            target_root_id: (1, 2),
        }
    }
    impl CleanupPort for Port {
        fn check(
            &self,
            _: &Path,
            _: &Path,
            source: &Path,
            target: &Path,
            _: &AtomicBool,
        ) -> Result<Proof, String> {
            self.checks.fetch_add(1, Ordering::SeqCst);
            Ok(proof(source, target))
        }
        fn trash_checked(&self, proof: &Proof, _: &AtomicBool) -> Result<(), String> {
            self.entered.store(true, Ordering::SeqCst);
            while self.gate.load(Ordering::SeqCst) && !self.release.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            self.moved.lock().unwrap().push(proof.source.clone());
            assert!(
                !self.crash.load(Ordering::SeqCst),
                "simulated failure after an unacknowledged operation"
            );
            Ok(())
        }
    }
    fn request(decisions: &[FileDecision]) -> CleanupRequest {
        CleanupRequest {
            source_root: "source".into(),
            target_root: "library".into(),
            reports: decisions
                .iter()
                .enumerate()
                .map(|(i, decision)| FileReport {
                    source: format!("source/{i}.wav").into(),
                    target: Some(format!("library/{i}.wav").into()),
                    captured: None,
                    decision: decision.clone(),
                })
                .collect(),
        }
    }
    fn finish(session: &mut CleanupSession) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while session.running() {
            session.poll();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn review_never_moves_and_only_explicit_confirmation_processes_eligible_rows() {
        let port = Arc::new(Port::default());
        let mut session = CleanupSession::default();
        session
            .start(
                request(&[
                    FileDecision::Copied,
                    FileDecision::SkippedIdentical,
                    FileDecision::SkippedVideoName,
                    FileDecision::Failed("copy failed".into()),
                    FileDecision::Planned,
                ]),
                port.clone(),
            )
            .unwrap();
        assert!(session.confirm(port.clone()).is_err());
        finish(&mut session);
        assert_eq!(session.ready_count(), 3);
        assert_eq!(port.checks.load(Ordering::SeqCst), 3);
        assert!(port.moved.lock().unwrap().is_empty());
        session.confirm(port.clone()).unwrap();
        assert!(session.confirm(port.clone()).is_err());
        finish(&mut session);
        assert_eq!(port.moved.lock().unwrap().len(), 3);
        assert_eq!(
            session
                .rows()
                .iter()
                .filter(|r| r.status == Status::Trashed)
                .count(),
            3
        );
        assert!(!session.can_confirm());
    }
    #[test]
    fn cancellation_preserves_inflight_success_and_never_starts_the_next_file() {
        let port = Arc::new(Port::default());
        let mut session = CleanupSession::default();
        session
            .start(
                request(&[FileDecision::Copied, FileDecision::Copied]),
                port.clone(),
            )
            .unwrap();
        finish(&mut session);
        port.gate.store(true, Ordering::SeqCst);
        session.confirm(port.clone()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !port.entered.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        session.cancel();
        port.release.store(true, Ordering::SeqCst);
        finish(&mut session);
        assert_eq!(port.moved.lock().unwrap().len(), 1);
        assert_eq!(session.rows()[0].status, Status::Trashed);
        assert!(matches!(session.rows()[1].status, Status::Kept(_)));
        assert!(!session.can_confirm());
    }
    #[test]
    fn unacknowledged_operation_is_reported_as_unknown_and_cannot_be_retried_blindly() {
        let port = Arc::new(Port::default());
        let mut session = CleanupSession::default();
        session
            .start(request(&[FileDecision::Copied]), port.clone())
            .unwrap();
        finish(&mut session);
        port.crash.store(true, Ordering::SeqCst);
        session.confirm(port).unwrap();
        finish(&mut session);
        assert!(matches!(session.rows()[0].status, Status::Unknown(_)));
        assert!(!session.can_confirm());
        assert!(session.status().contains("unerwartet"));
    }
    #[test]
    fn cancelling_the_review_forbids_confirmation() {
        let port = Arc::new(Port::default());
        let mut session = CleanupSession::default();
        session
            .start(request(&[FileDecision::Copied]), port.clone())
            .unwrap();
        finish(&mut session);
        session.cancel();
        assert!(session.confirm(port.clone()).is_err());
        assert!(port.moved.lock().unwrap().is_empty());
    }
}
