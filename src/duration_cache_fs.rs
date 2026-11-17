//! Disposable per-year display cache. All access is on the preparation worker.
use photosuite_application::duration_cache::{DurationCache, Fingerprint, Length, Snapshot};

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};
#[cfg_attr(test, derive(Default))]
pub struct SqliteDurations {
    pub database: std::sync::Arc<crate::library_db::Database>,
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
impl DurationCache for SqliteDurations {
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
        self.database.transaction(root,|tx|{
            let last_full_verified=crate::library_db::optional(tx.query_row("SELECT last_full_verified FROM audio_verifications WHERE year=?1",[year],|r|r.get::<_,Option<String>>(0)))?.flatten();
            let mut entries=BTreeMap::new();
            let mut stmt=tx.prepare("SELECT media_path,seconds,source_bytes,source_modified FROM audio_durations WHERE year=?1 ORDER BY media_path")?;
            for row in stmt.query_map([year],|r|Ok((r.get::<_,String>(0)?,r.get::<_,f64>(1)?,r.get::<_,u64>(2)?,r.get::<_,String>(3)?)))? {
                let (relative,seconds,size,modified)=row?;
                let file=crate::domain::tags::LibraryFile::new(relative).map_err(crate::library_db::invalid)?;
                let path=root.join(file.as_str());
                if self.groups(std::slice::from_ref(&path))!=[group.to_owned()] {return Err(crate::library_db::invalid("Audiocache gehört zu einem anderen Jahr"));}
                entries.insert(path,Length {seconds,fingerprint:Fingerprint {size,modified}});
            }
            Ok(Snapshot {today:crate::file_dates_fs::today().map_err(crate::library_db::invalid)?,last_full_verified,entries,files:BTreeMap::new()})
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
        let (root, year) = parts(group)?;
        self.database.transaction(root,|tx| {
            tx.execute("DELETE FROM audio_durations WHERE year=?1",[year])?;
            for (path,entry) in &snapshot.entries {
                let file=crate::domain::tags::LibraryFile::new(path.strip_prefix(root).map_err(|e|crate::library_db::invalid(e.to_string()))?.to_str().ok_or_else(||crate::library_db::invalid("Nicht-UTF8-Audiopfad"))?.into()).map_err(crate::library_db::invalid)?;
                if self.groups(std::slice::from_ref(path))!=[group.to_owned()] || !entry.seconds.is_finite() || entry.seconds<=0. {return Err(crate::library_db::invalid("Ungültiger Audiocache"));}
                crate::library_db::media(tx,&file)?;
                tx.execute("INSERT INTO audio_durations(media_path,year,seconds,source_bytes,source_modified) VALUES(?1,?2,?3,?4,?5)",rusqlite::params![file.as_str(),year,entry.seconds,entry.fingerprint.size,entry.fingerprint.modified])?;
            }
            tx.execute("INSERT INTO audio_verifications(year,last_full_verified) VALUES(?1,?2) ON CONFLICT(year) DO UPDATE SET last_full_verified=excluded.last_full_verified",(year,&snapshot.last_full_verified))?;
            tx.execute("UPDATE index_state SET change_sequence=change_sequence+1 WHERE id=1",[])?;Ok(())
        })
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
        let store = SqliteDurations::default();
        let stop = AtomicBool::new(false);
        assert_eq!(
            store.groups(std::slice::from_ref(&audio)),
            std::slice::from_ref(&group)
        );
        let mut snapshot = store.load(&group, &stop).unwrap();
        assert!(snapshot.entries.is_empty());
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
        let cache = temp.path().join(crate::library_db::FILE);
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
            SqliteDurations::default()
                .inventory(&group, &AtomicBool::new(false))
                .is_err()
        );
        assert!(!temp.path().join("yaml").exists());
        fs::remove_file(group.join("audio/diary/a.wav")).unwrap();
        fs::create_dir(temp.path().join("elsewhere")).unwrap();
        symlink(temp.path().join("elsewhere"), temp.path().join("yaml")).unwrap();
        assert!(
            SqliteDurations::default()
                .load(&group, &AtomicBool::new(false))
                .is_err()
        );
        assert_eq!(fs::read(external).unwrap(), b"original");
    }
}
