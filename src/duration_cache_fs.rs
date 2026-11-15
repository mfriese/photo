//! Disposable per-year display cache. All access is on the preparation worker.
use photosuite_application::duration_cache::{DurationCache, Fingerprint, Length, Snapshot};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};
pub struct YamlDurations;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    last_full_verified: Option<String>,
    files: BTreeMap<String, Entry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    seconds: f64,
    size: u64,
    modified: String,
}
fn parts(group: &Path) -> Result<(&Path, &str), String> {
    let year = group
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Ungültiges Cachejahr")?;
    if year.len() != 4 || !year.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Ungültiges Cachejahr".into());
    }
    Ok((group.parent().ok_or("Bibliothekswurzel fehlt")?, year))
}
fn cache_path(group: &Path, create: bool) -> Result<PathBuf, String> {
    let (root, year) = parts(group)?;
    crate::metadata_fs::library_path(root, &format!("yaml/{year}/audio-durations.yaml"), create)
}
fn fingerprint(metadata: &fs::Metadata) -> Result<Fingerprint, String> {
    let modified = metadata
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    Ok(Fingerprint {
        size: metadata.len(),
        modified: format!("{}.{:09}", modified.as_secs(), modified.subsec_nanos()),
    })
}
fn inventory(group: &Path, cancel: &AtomicBool) -> Result<BTreeMap<PathBuf, Fingerprint>, String> {
    let (root, year) = parts(group)?;
    let year_meta = fs::symlink_metadata(group).map_err(|e| e.to_string())?;
    if !year_meta.is_dir() || year_meta.file_type().is_symlink() {
        return Err("Audiojahr ist kein reguläres Verzeichnis".into());
    }
    let mut files = BTreeMap::new();
    for kind in ["diary", "comments"] {
        let marker =
            crate::metadata_fs::library_path(root, &format!("{year}/audio/{kind}/.scan"), false)?;
        let directory = marker.parent().ok_or("Audioverzeichnis fehlt")?;
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return Err("Abgebrochen".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path
                .file_name()
                .is_some_and(|s| s.to_string_lossy().starts_with("._"))
                || crate::domain::Media::from_audio_asset(path.clone(), String::new()).is_none()
            {
                continue;
            }
            let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if metadata.file_type().is_symlink() {
                return Err("Symbolische Audiodatei; Cacheabgleich nicht vollständig".into());
            }
            if metadata.is_file() {
                files.insert(path, fingerprint(&metadata)?);
            }
        }
    }
    Ok(files)
}
impl DurationCache for YamlDurations {
    fn groups(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        paths
            .iter()
            .filter_map(|p| {
                let kind = p.parent()?;
                if !matches!(kind.file_name()?.to_str()?, "diary" | "comments") {
                    return None;
                }
                let audio = kind.parent()?;
                if audio.file_name()? != "audio" {
                    return None;
                }
                let group = audio.parent()?;
                parts(group).ok()?;
                Some(group.to_owned())
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    fn load(&self, group: &Path, cancel: &AtomicBool) -> Result<Snapshot, String> {
        if cancel.load(Ordering::Relaxed) {
            return Err("Abgebrochen".into());
        }
        let (root, year) = parts(group)?;
        let path = cache_path(group, false)?;
        let mut entries = BTreeMap::new();
        let mut last_full_verified = None;
        if crate::metadata_fs::check_file(&path)? {
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .map_err(|e| e.to_string())?
                .take(32 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 32 * 1024 * 1024 {
                return Err("Audiolängen-Cache überschreitet 32 MiB".into());
            }
            // This derived, disposable cache has no user-authored metadata to migrate.
            if let Ok(document) = serde_saphyr::from_slice::<Document>(&bytes)
                && document.version == 1
            {
                last_full_verified = document.last_full_verified;
                for (relative, entry) in document.files {
                    let p = Path::new(&relative);
                    if p.components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_)))
                    {
                        continue;
                    }
                    let prefix = format!("{year}/audio/");
                    if !relative.starts_with(&prefix)
                        || !entry.seconds.is_finite()
                        || entry.seconds <= 0.
                    {
                        continue;
                    }
                    let full = root.join(p);
                    if self.groups(std::slice::from_ref(&full)) != [group.to_owned()] {
                        continue;
                    }
                    entries.insert(
                        full,
                        Length {
                            seconds: entry.seconds,
                            fingerprint: Fingerprint {
                                size: entry.size,
                                modified: entry.modified,
                            },
                        },
                    );
                }
            }
        }
        Ok(Snapshot {
            today: crate::file_dates_fs::today()?,
            last_full_verified,
            entries,
            files: BTreeMap::new(),
        })
    }
    fn inventory(
        &self,
        group: &Path,
        cancel: &AtomicBool,
    ) -> Result<BTreeMap<PathBuf, Fingerprint>, String> {
        inventory(group, cancel)
    }
    fn save(&self, group: &Path, snapshot: &Snapshot) -> Result<(), String> {
        let (root, _) = parts(group)?;
        let files = snapshot
            .entries
            .iter()
            .map(|(path, entry)| {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("Nicht-UTF8-Audiopfad")?
                    .to_owned();
                Ok((
                    relative,
                    Entry {
                        seconds: entry.seconds,
                        size: entry.fingerprint.size,
                        modified: entry.fingerprint.modified.clone(),
                    },
                ))
            })
            .collect::<Result<_, String>>()?;
        let text = serde_saphyr::to_string(&Document {
            version: 1,
            last_full_verified: snapshot.last_full_verified.clone(),
            files,
        })
        .map_err(|e| e.to_string())?;
        if text.len() > 32 * 1024 * 1024 {
            return Err("Audiolängen-Cache überschreitet 32 MiB".into());
        }
        crate::metadata_fs::atomic_write(&cache_path(group, true)?, text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photosuite_application::duration_cache::reconcile;
    #[test]
    fn year_cache_round_trip_daily_prune_and_unreachable_storage_preserve_media_and_cache() {
        let temp = tempfile::tempdir().unwrap();
        let group = temp.path().join("2026");
        let directory = group.join("audio/diary");
        fs::create_dir_all(&directory).unwrap();
        let audio = directory.join("aud_20261115_120000.wav");
        fs::write(&audio, b"test audio unchanged").unwrap();
        let store = YamlDurations;
        let stop = AtomicBool::new(false);
        assert_eq!(
            store.groups(std::slice::from_ref(&audio)),
            std::slice::from_ref(&group)
        );
        let mut snapshot = store.load(&group, &stop).unwrap();
        assert!(snapshot.entries.is_empty());
        assert!(!temp.path().join("yaml").exists());
        snapshot.entries.insert(
            audio.clone(),
            Length {
                seconds: 12.5,
                fingerprint: store.inventory(&group, &stop).unwrap()[&audio].clone(),
            },
        );
        snapshot.last_full_verified = Some(snapshot.today.clone());
        store.save(&group, &snapshot).unwrap();
        let saved = store.load(&group, &stop).unwrap();
        assert_eq!(saved.entries[&audio].seconds, 12.5);
        reconcile(
            &store,
            &group,
            &stop,
            |h| assert_eq!(h[&audio].seconds, 12.5),
            |_| panic!("same-day hit must not probe"),
        )
        .unwrap();
        assert_eq!(fs::read(&audio).unwrap(), b"test audio unchanged");
        // Replacing the year with an unavailable mount/path must not publish an empty inventory.
        let cache = cache_path(&group, false).unwrap();
        let before = fs::read(&cache).unwrap();
        fs::rename(&group, temp.path().join("offline")).unwrap();
        assert!(reconcile(&store, &group, &stop, |_| {}, |_| Ok(1.)).is_err());
        assert_eq!(fs::read(&cache).unwrap(), before);
        fs::rename(temp.path().join("offline"), &group).unwrap();
        let mut snapshot = store.load(&group, &stop).unwrap();
        snapshot.last_full_verified = Some("20000101".into());
        store.save(&group, &snapshot).unwrap();
        fs::remove_file(&audio).unwrap();
        reconcile(
            &store,
            &group,
            &stop,
            |_| {},
            |_| panic!("no present media"),
        )
        .unwrap();
        assert!(store.load(&group, &stop).unwrap().entries.is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_audio_or_metadata_never_prunes_or_writes_through_links() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let group = temp.path().join("2026");
        fs::create_dir_all(group.join("audio/diary")).unwrap();
        let external = temp.path().join("external.wav");
        fs::write(&external, b"original").unwrap();
        symlink(&external, group.join("audio/diary/a.wav")).unwrap();
        assert!(
            YamlDurations
                .inventory(&group, &AtomicBool::new(false))
                .is_err()
        );
        assert!(!temp.path().join("yaml").exists());
        fs::remove_file(group.join("audio/diary/a.wav")).unwrap();
        fs::create_dir(temp.path().join("elsewhere")).unwrap();
        symlink(temp.path().join("elsewhere"), temp.path().join("yaml")).unwrap();
        assert!(YamlDurations.load(&group, &AtomicBool::new(false)).is_err());
        assert_eq!(fs::read(external).unwrap(), b"original");
    }
}
