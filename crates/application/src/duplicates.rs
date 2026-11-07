//! Guided review. Compare one family completely, await a user decision, then continue.
use crate::{domain::duplicates::CollisionGroup, ports::MediaRepository};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Comparison {
    Identical,
    Different(String),
    Uncertain(String),
}
pub trait DuplicateImages: Send + Sync + 'static {
    fn compare(&self, a: &Path, b: &Path) -> Result<Comparison, String>;
    /// Revalidate pixels immediately before moving a candidate to the OS Trash.
    fn trash_identical(
        &self,
        library: &Path,
        keeper: &Path,
        candidate: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), String>;
}
#[derive(Clone, Debug)]
pub struct Pair {
    pub a: usize,
    pub b: usize,
    pub result: Comparison,
}
#[derive(Clone, Debug)]
pub struct Report {
    pub group: CollisionGroup,
    pub pairs: Vec<Pair>,
}
impl Report {
    fn equal(&self, a: usize, b: usize) -> bool {
        a == b
            || self.pairs.iter().any(|p| {
                ((p.a == a && p.b == b) || (p.a == b && p.b == a))
                    && p.result == Comparison::Identical
            })
    }
    pub fn classes(&self) -> Vec<Vec<usize>> {
        let mut classes: Vec<Vec<usize>> = Vec::new();
        for i in 0..self.group.files.len() {
            if let Some(class) = classes
                .iter_mut()
                .find(|class| class.iter().all(|&j| self.equal(i, j)))
            {
                class.push(i);
            } else {
                classes.push(vec![i]);
            }
        }
        classes
    }
    pub fn removal_plan(&self, keepers: &[usize]) -> Result<Vec<(PathBuf, PathBuf)>, String> {
        let classes = self.classes();
        if keepers.len() != classes.len() {
            return Err("Für jede Bildklasse muss eine Datei erhalten bleiben".into());
        }
        let mut plan = Vec::new();
        for (class, &keep) in classes.iter().zip(keepers) {
            if !class.contains(&keep) {
                return Err("Ungültige Auswahl".into());
            }
            for &i in class {
                if i != keep {
                    plan.push((self.group.files[keep].clone(), self.group.files[i].clone()));
                }
            }
        }
        Ok(plan)
    }
}
pub enum Event {
    Groups(Vec<CollisionGroup>),
    Progress(String),
    Report(Report),
    Removed(Vec<(PathBuf, Result<(), String>)>),
    Failed(String),
}
pub struct Job {
    pub receiver: mpsc::Receiver<Event>,
    pub cancel: Arc<AtomicBool>,
}
pub fn discover(repo: Arc<dyn MediaRepository>, library: PathBuf) -> Job {
    job(move |tx, cancel| {
        let (items, errors) = repo.scan(&library);
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let event = if errors > 0 {
            Event::Failed(format!("Scan unvollständig: {errors} Lesefehler"))
        } else {
            Event::Groups(crate::domain::duplicates::collision_groups(&items))
        };
        let _ = tx.send(event);
    })
}
pub fn compare(adapter: Arc<dyn DuplicateImages>, group: CollisionGroup) -> Job {
    job(move |tx, cancel| {
        let mut pairs = Vec::new();
        let total = group.files.len() * group.files.len().saturating_sub(1) / 2;
        for a in 0..group.files.len() {
            for b in a + 1..group.files.len() {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                if tx
                    .send(Event::Progress(format!(
                        "Vergleich {} / {total}: {} ↔ {}",
                        pairs.len() + 1,
                        group.files[a].display(),
                        group.files[b].display()
                    )))
                    .is_err()
                {
                    return;
                }
                let result = adapter
                    .compare(&group.files[a], &group.files[b])
                    .unwrap_or_else(Comparison::Uncertain);
                pairs.push(Pair { a, b, result });
            }
        }
        if !cancel.load(Ordering::Relaxed) {
            let _ = tx.send(Event::Report(Report { group, pairs }));
        }
    })
}
pub fn remove(
    adapter: Arc<dyn DuplicateImages>,
    library: PathBuf,
    report: Report,
    keepers: Vec<usize>,
) -> Job {
    job(move |tx, cancel| {
        let plan = match report.removal_plan(&keepers) {
            Ok(p) => p,
            Err(e) => {
                let _ = tx.send(Event::Failed(e));
                return;
            }
        };
        let mut results = Vec::new();
        for (keeper, candidate) in plan {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let _ = tx.send(Event::Progress(format!(
                "Erneut prüfen und in Papierkorb: {}",
                candidate.display()
            )));
            let result = adapter.trash_identical(&library, &keeper, &candidate, &cancel);
            results.push((candidate, result));
        }
        let _ = tx.send(Event::Removed(results));
    })
}
fn job(task: impl FnOnce(mpsc::Sender<Event>, Arc<AtomicBool>) + Send + 'static) -> Job {
    let (tx, receiver) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    std::thread::spawn(move || task(tx, stop));
    Job { receiver, cancel }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct Adapter(Mutex<Vec<(PathBuf, PathBuf)>>);
    impl DuplicateImages for Adapter {
        fn compare(&self, a: &Path, b: &Path) -> Result<Comparison, String> {
            self.0.lock().unwrap().push((a.into(), b.into()));
            Ok(
                if a.file_name().unwrap() == "c.jpg" || b.file_name().unwrap() == "c.jpg" {
                    Comparison::Different("different pixels".into())
                } else {
                    Comparison::Identical
                },
            )
        }
        fn trash_identical(
            &self,
            _: &Path,
            _: &Path,
            _: &Path,
            _: &AtomicBool,
        ) -> Result<(), String> {
            panic!("comparison must never remove files")
        }
    }
    #[test]
    fn every_pair_is_compared_and_review_is_explicit() {
        let adapter = Arc::new(Adapter(Mutex::new(vec![])));
        let group = CollisionGroup {
            name: "group".into(),
            files: vec!["a.jpg".into(), "b.jpg".into(), "c.jpg".into()],
        };
        let job = compare(adapter.clone(), group);
        let report = loop {
            if let Event::Report(report) = job
                .receiver
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
            {
                break report;
            }
        };
        assert_eq!(adapter.0.lock().unwrap().len(), 3);
        assert_eq!(report.classes(), vec![vec![0, 1], vec![2]]);
        assert_eq!(
            report.removal_plan(&[0, 2]).unwrap(),
            vec![(PathBuf::from("a.jpg"), PathBuf::from("b.jpg"))]
        );
        assert!(report.removal_plan(&[0, 0]).is_err());
    }
    #[test]
    fn uncertain_comparisons_do_not_allow_removal() {
        let r = Report {
            group: CollisionGroup {
                name: "group".into(),
                files: vec!["a.jpg".into(), "b.jpg".into()],
            },
            pairs: vec![Pair {
                a: 0,
                b: 1,
                result: Comparison::Uncertain("color".into()),
            }],
        };
        assert!(r.removal_plan(&[0, 1]).unwrap().is_empty());
    }
}
