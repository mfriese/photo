//! Disposable display durations. Media inventory, never this cache, defines playback membership.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub size: u64,
    pub modified: String,
}
#[derive(Clone, Debug)]
pub struct Length {
    pub seconds: f64,
    pub fingerprint: Fingerprint,
}
pub struct Snapshot {
    pub today: String,
    pub last_full_verified: Option<String>,
    pub entries: BTreeMap<PathBuf, Length>,
    /// Complete successful inventory of the canonical year's audio directories.
    pub files: BTreeMap<PathBuf, Fingerprint>,
}
pub trait DurationCache: Send + Sync + 'static {
    fn groups(&self, paths: &[PathBuf]) -> Vec<PathBuf>;
    fn load(&self, group: &Path, cancel: &AtomicBool) -> Result<Snapshot, String>;
    fn inventory(
        &self,
        group: &Path,
        cancel: &AtomicBool,
    ) -> Result<BTreeMap<PathBuf, Fingerprint>, String>;
    fn save(&self, group: &Path, snapshot: &Snapshot) -> Result<(), String>;
}
/// Publish display hints first, then reconcile missing/changed entries and the daily full check.
/// A failed or cancelled verification never advances the full-verification date.
pub fn reconcile(
    store: &dyn DurationCache,
    group: &Path,
    cancel: &AtomicBool,
    hints: impl FnOnce(&BTreeMap<PathBuf, Length>),
    measure: impl FnMut(&Path) -> Result<f64, String>,
) -> Result<(), String> {
    let snapshot = store.load(group, cancel)?;
    hints(&snapshot.entries);
    reconcile_loaded(store, group, snapshot, cancel, measure)
}
pub fn reconcile_loaded(
    store: &dyn DurationCache,
    group: &Path,
    mut snapshot: Snapshot,
    cancel: &AtomicBool,
    mut measure: impl FnMut(&Path) -> Result<f64, String>,
) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    snapshot.files = store.inventory(group, cancel)?;
    let full = snapshot.last_full_verified.as_ref() != Some(&snapshot.today);
    let mut errors = Vec::new();
    let mut ordered = snapshot.files.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(path, fingerprint)| {
        snapshot
            .entries
            .get(*path)
            .is_some_and(|e| &e.fingerprint == *fingerprint)
    });
    for (path, fingerprint) in ordered {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let changed = snapshot
            .entries
            .get(path)
            .is_none_or(|e| &e.fingerprint != fingerprint);
        if full || changed {
            match measure(path) {
                Ok(seconds) if seconds.is_finite() && seconds > 0. => {
                    snapshot.entries.insert(
                        path.clone(),
                        Length {
                            seconds,
                            fingerprint: fingerprint.clone(),
                        },
                    );
                }
                result => {
                    snapshot.entries.remove(path);
                    if errors.len() < 16 {
                        errors.push(format!(
                            "{}: {}",
                            path.display(),
                            result
                                .err()
                                .unwrap_or_else(|| "Ungültige Audiodauer".into())
                        ));
                    }
                }
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    if full {
        snapshot
            .entries
            .retain(|p, _| snapshot.files.contains_key(p));
        if errors.is_empty() {
            snapshot.last_full_verified = Some(snapshot.today.clone());
        }
    }
    store.save(group, &snapshot)?;
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct Store {
        full: Option<String>,
        saves: Mutex<Vec<Snapshot>>,
    }
    fn fp(size: u64) -> Fingerprint {
        Fingerprint {
            size,
            modified: "stamp".into(),
        }
    }
    fn length(size: u64, seconds: f64) -> Length {
        Length {
            seconds,
            fingerprint: fp(size),
        }
    }
    impl DurationCache for Store {
        fn groups(&self, _: &[PathBuf]) -> Vec<PathBuf> {
            vec!["year".into()]
        }
        fn load(&self, _: &Path, _: &AtomicBool) -> Result<Snapshot, String> {
            Ok(Snapshot {
                today: "20261115".into(),
                last_full_verified: self.full.clone(),
                entries: [
                    ("a".into(), length(1, 10.)),
                    ("changed".into(), length(1, 20.)),
                    ("gone".into(), length(1, 30.)),
                ]
                .into(),
                files: [
                    ("a".into(), fp(1)),
                    ("changed".into(), fp(2)),
                    ("new".into(), fp(1)),
                ]
                .into(),
            })
        }
        fn inventory(
            &self,
            group: &Path,
            cancel: &AtomicBool,
        ) -> Result<BTreeMap<PathBuf, Fingerprint>, String> {
            Ok(self.load(group, cancel)?.files)
        }
        fn save(&self, _: &Path, s: &Snapshot) -> Result<(), String> {
            self.saves.lock().unwrap().push(Snapshot {
                today: s.today.clone(),
                last_full_verified: s.last_full_verified.clone(),
                entries: s.entries.clone(),
                files: s.files.clone(),
            });
            Ok(())
        }
    }
    #[test]
    fn today_reuses_lengths_and_updates_missing_and_changed_without_advancing_full_date() {
        let store = Store {
            full: Some("20261115".into()),
            saves: Mutex::default(),
        };
        let calls = Mutex::new(Vec::new());
        reconcile(
            &store,
            Path::new("year"),
            &AtomicBool::new(false),
            |hints| {
                assert_eq!(hints[Path::new("a")].seconds, 10.);
                assert!(
                    calls.lock().unwrap().is_empty(),
                    "publish hints before probing"
                );
            },
            |p| {
                calls.lock().unwrap().push(p.to_owned());
                Ok(42.)
            },
        )
        .unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            [PathBuf::from("changed"), PathBuf::from("new")]
        );
        let saves = store.saves.lock().unwrap();
        let s = &saves[0];
        assert_eq!(s.entries[Path::new("a")].seconds, 10.);
        assert_eq!(s.entries[Path::new("new")].seconds, 42.);
        assert_eq!(s.last_full_verified.as_deref(), Some("20261115"));
    }
    #[test]
    fn previous_day_rechecks_every_present_file_prunes_stale_entries_and_records_full_success() {
        let store = Store {
            full: Some("20261114".into()),
            saves: Mutex::default(),
        };
        let mut calls = Vec::new();
        reconcile(
            &store,
            Path::new("year"),
            &AtomicBool::new(false),
            |_| {},
            |p| {
                calls.push(p.to_owned());
                Ok(7.)
            },
        )
        .unwrap();
        assert_eq!(calls.len(), 3);
        let saves = store.saves.lock().unwrap();
        let s = &saves[0];
        assert!(!s.entries.contains_key(Path::new("gone")));
        assert_eq!(s.last_full_verified.as_deref(), Some("20261115"));
    }
    #[test]
    fn failed_verification_does_not_claim_full_success_and_cancellation_never_publishes() {
        let store = Store {
            full: Some("20261114".into()),
            saves: Mutex::default(),
        };
        assert!(
            reconcile(
                &store,
                Path::new("year"),
                &AtomicBool::new(false),
                |_| {},
                |_| Err("unreadable".into())
            )
            .is_err()
        );
        assert_eq!(
            store.saves.lock().unwrap()[0].last_full_verified.as_deref(),
            Some("20261114")
        );
        store.saves.lock().unwrap().clear();
        let stop = AtomicBool::new(false);
        reconcile(
            &store,
            Path::new("year"),
            &stop,
            |_| {},
            |_| {
                stop.store(true, Ordering::Relaxed);
                Ok(2.)
            },
        )
        .unwrap();
        assert!(store.saves.lock().unwrap().is_empty());
    }
}
