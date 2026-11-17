//! Shared GeoNames archive and separate appwide plist; no library or original media writes.
use crate::metadata_fs::{atomic_write_if_current, check_file, library_path};
use photosuite_application::{
    downloads::{AssetTransport, Progress, Stage},
    geodata::{Locations, MAX_ARCHIVE_BYTES, Prepared, SOURCE, Store},
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
const ARCHIVE: &str = "Library/Application Support/PhotoSuite/GeoNames/cities500.zip";
const PREFERENCES: &str = "Library/Preferences/de.photosuite.PhotoSuite.geodata.plist";
const MAX_EXPANDED: u64 = 256 * 1024 * 1024;
pub struct GeoData {
    home: PathBuf,
    transport: Arc<dyn AssetTransport>,
}
#[derive(Default)]
struct Preferences {
    enabled: bool,
    sha256: Option<String>,
}
impl GeoData {
    pub fn new(home: PathBuf, transport: Arc<dyn AssetTransport>) -> Self {
        Self { home, transport }
    }
    fn preferences(&self) -> Result<(Preferences, Option<Vec<u8>>), String> {
        let path = library_path(&self.home, PREFERENCES, false)?;
        if !check_file(&path)? {
            return Ok((Preferences::default(), None));
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 65536 {
            return Err("Geodaten-Preferences überschreiten 64 KiB".into());
        }
        let value = plist::Value::from_reader(std::io::Cursor::new(&bytes))
            .map_err(|e| format!("Geodaten-Preferences bleiben unverändert: {e}"))?;
        let d = value
            .as_dictionary()
            .ok_or("Ungültige Geodaten-Preferences")?;
        if d.get("version").and_then(plist::Value::as_unsigned_integer) != Some(1)
            || d.keys()
                .any(|k| !["version", "enabled", "sha256"].contains(&k.as_str()))
        {
            return Err("Unbekannte Geodaten-Preferences; Datei bleibt unverändert".into());
        }
        let enabled = d
            .get("enabled")
            .and_then(plist::Value::as_boolean)
            .ok_or("Geodaten-Aktivierung fehlt")?;
        let sha256 = match d.get("sha256") {
            None => None,
            Some(plist::Value::String(s))
                if s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()) =>
            {
                Some(s.clone())
            }
            _ => return Err("Ungültige Geodaten-Prüfsumme".into()),
        };
        Ok((Preferences { enabled, sha256 }, Some(bytes)))
    }
    fn verify(
        &self,
        path: &Path,
        stop: &AtomicBool,
        notify: &(dyn Fn(Progress) + Sync),
    ) -> Result<(u64, String), String> {
        cancelled(stop)?;
        if !check_file(path)? {
            return Err("Noch keine Geodaten heruntergeladen".into());
        }
        let before = fingerprint(path)?;
        if before.0 == 0 || before.0 > MAX_ARCHIVE_BYTES {
            return Err("Geodaten-ZIP hat eine ungültige Größe".into());
        }
        notify(Progress {
            stage: Stage::Checking,
            bytes: 0,
            total: before.0,
        });
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut zip =
            zip::ZipArchive::new(file).map_err(|e| format!("Ungültige Geodaten-ZIP: {e}"))?;
        if zip.is_empty() || zip.len() > 8 {
            return Err("Unerwarteter Geodaten-ZIP-Inhalt".into());
        }
        let mut found = false;
        let mut total = 0u64;
        let mut buffer = [0u8; 65536];
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().map_err(|e| e.to_string())?;
            if name == "cities500.txt" {
                if found || entry.size() == 0 {
                    return Err("Ungültige cities500.txt".into());
                }
                found = true;
            }
            if entry.size() > MAX_EXPANDED || entry.is_symlink() {
                return Err("Ungültiger Geodaten-ZIP-Eintrag".into());
            }
            loop {
                cancelled(stop)?;
                let n = entry
                    .read(&mut buffer)
                    .map_err(|e| format!("Geodaten-ZIP-Prüfung fehlgeschlagen: {e}"))?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > MAX_EXPANDED {
                    return Err("Geodaten überschreiten das entpackte Größenlimit".into());
                }
            }
        }
        if !found {
            return Err("cities500.txt fehlt im ZIP".into());
        }
        let sha256 = self.transport.sha256(path, stop, notify)?;
        if fingerprint(path)? != before {
            return Err("Geodaten während der Prüfung verändert".into());
        }
        Ok((before.0, sha256))
    }
}
fn cancelled(stop: &AtomicBool) -> Result<(), String> {
    if stop.load(Ordering::Relaxed) {
        Err("Abgebrochen; bisherige Einstellung bleibt erhalten".into())
    } else {
        Ok(())
    }
}
fn fingerprint(path: &Path) -> Result<(u64, std::time::SystemTime), String> {
    if !check_file(path)? {
        return Err("Geodaten-Datei fehlt".into());
    }
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    Ok((m.len(), m.modified().map_err(|e| e.to_string())?))
}
impl Store for GeoData {
    fn locations(&self) -> Locations {
        Locations {
            archive: self.home.join(ARCHIVE),
            preferences: self.home.join(PREFERENCES),
        }
    }
    fn inspect(
        &self,
        stop: &AtomicBool,
        notify: &(dyn Fn(Progress) + Sync),
    ) -> Result<Prepared, String> {
        cancelled(stop)?;
        let (prefs, _) = self.preferences()?;
        let path = library_path(&self.home, ARCHIVE, false)?;
        if !check_file(&path)? {
            return Ok(Prepared {
                enabled: prefs.enabled,
                ..Default::default()
            });
        }
        match self.verify(&path,stop,notify) {
            Ok((bytes,sha)) if prefs.sha256.as_ref().is_none_or(|expected| expected==&sha) => Ok(Prepared {enabled:prefs.enabled,ready:true,bytes,warning:String::new()}),
            Ok(_) => Ok(Prepared {enabled:prefs.enabled,warning:"Lokale Prüfsumme stimmt nicht. Erneut bestätigen, um die Datei neu herunterzuladen.".into(),..Default::default()}),
            Err(error) => {cancelled(stop)?; Ok(Prepared {enabled:prefs.enabled,warning:error,..Default::default()})}
        }
    }
    fn prepare(
        &self,
        enabled: bool,
        stop: &AtomicBool,
        notify: &(dyn Fn(Progress) + Sync),
    ) -> Result<Prepared, String> {
        cancelled(stop)?;
        let (prefs, revision) = self.preferences()?;
        let path = library_path(&self.home, ARCHIVE, false)?;
        let mut sha = prefs.sha256.clone();
        let mut prepared = if enabled {
            Prepared::default()
        } else {
            self.inspect(stop, notify)?
        };
        if enabled {
            let valid = self
                .verify(&path, stop, notify)
                .ok()
                .filter(|(_, hash)| sha.as_ref().is_none_or(|expected| expected == hash));
            if let Some((bytes, hash)) = valid {
                prepared.ready = true;
                prepared.bytes = bytes;
                sha = Some(hash);
            } else {
                cancelled(stop)?;
                let previous = if check_file(&path)? {
                    Some(fingerprint(&path)?)
                } else {
                    None
                };
                let target = library_path(&self.home, ARCHIVE, true)?;
                let temp = tempfile::Builder::new()
                    .prefix(".photosuite-geodata-")
                    .suffix(".tmp")
                    .tempfile_in(target.parent().ok_or("Geodatenverzeichnis fehlt")?)
                    .map_err(|e| e.to_string())?;
                notify(Progress {
                    stage: Stage::Downloading,
                    bytes: 0,
                    total: 0,
                });
                self.transport.download_bounded(
                    SOURCE,
                    temp.path(),
                    MAX_ARCHIVE_BYTES,
                    stop,
                    notify,
                )?;
                cancelled(stop)?;
                let (bytes, hash) = self.verify(temp.path(), stop, notify)?;
                prepared.ready = true;
                prepared.bytes = bytes;
                temp.as_file().sync_all().map_err(|e| e.to_string())?;
                cancelled(stop)?;
                static COMMIT: std::sync::Mutex<()> = std::sync::Mutex::new(());
                let _guard = COMMIT
                    .lock()
                    .map_err(|_| "Geodaten-Schreibschutz beendet")?;
                library_path(&self.home, ARCHIVE, false)?;
                let current = if check_file(&target)? {
                    Some(fingerprint(&target)?)
                } else {
                    None
                };
                if current != previous {
                    return Err("Geodaten extern verändert; kein Überschreiben".into());
                }
                if previous.is_none() {
                    temp.persist_noclobber(&target).map_err(|e| e.to_string())?;
                } else {
                    temp.persist(&target).map_err(|e| e.to_string())?;
                }
                sha = Some(hash);
            }
        }
        cancelled(stop)?;
        notify(Progress {
            stage: Stage::Saving,
            bytes: 0,
            total: 0,
        });
        let mut d = plist::Dictionary::new();
        d.insert("version".into(), 1u64.into());
        d.insert("enabled".into(), enabled.into());
        if let Some(sha) = sha {
            d.insert("sha256".into(), sha.into());
        }
        let mut bytes = Vec::new();
        plist::Value::Dictionary(d)
            .to_writer_xml(&mut bytes)
            .map_err(|e| e.to_string())?;
        let preferences = library_path(&self.home, PREFERENCES, true)?;
        atomic_write_if_current(&preferences, &bytes, revision.as_deref())?;
        prepared.enabled = enabled;
        Ok(prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, sync::atomic::AtomicUsize};
    fn archive() -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file(
            "cities500.txt",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(b"2950159\tBerlin\tBerlin\t\t52.52437\t13.41053\tP\tPPLC\tDE\t\t16\t\t\t\t3426354\t\t74\tEurope/Berlin\t2026-01-01\n").unwrap();
        zip.finish().unwrap().into_inner()
    }
    #[derive(Default)]
    struct Transfer {
        count: AtomicUsize,
        mode: AtomicUsize,
        home: PathBuf,
    }
    impl AssetTransport for Transfer {
        fn download(
            &self,
            _: &str,
            _: &Path,
            _: u64,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<(), String> {
            unreachable!()
        }
        fn download_bounded(
            &self,
            url: &str,
            path: &Path,
            limit: u64,
            stop: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<(), String> {
            assert_eq!(url, SOURCE);
            assert_eq!(limit, MAX_ARCHIVE_BYTES);
            self.count.fetch_add(1, Ordering::SeqCst);
            match self.mode.load(Ordering::SeqCst) {
                1 => {
                    fs::write(path, b"<html>Error</html>").unwrap();
                }
                2 => {
                    fs::write(path, b"partial").unwrap();
                    stop.store(true, Ordering::SeqCst);
                    return Err("Abgebrochen".into());
                }
                3 => {
                    let p = library_path(&self.home, ARCHIVE, true).unwrap();
                    fs::write(p, b"external").unwrap();
                    fs::write(path, archive()).unwrap();
                }
                4 => {
                    let p = library_path(&self.home, PREFERENCES, true).unwrap();
                    fs::write(p, b"external preference").unwrap();
                    fs::write(path, archive()).unwrap();
                }
                _ => fs::write(path, archive()).unwrap(),
            }
            Ok(())
        }
        fn sha256(
            &self,
            path: &Path,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<String, String> {
            // Deterministic fake checksum; native SHA-256 is tested independently.
            Ok(format!(
                "{:064x}",
                fs::read(path)
                    .map_err(|e| e.to_string())?
                    .iter()
                    .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(*b as u64))
            ))
        }
    }
    #[test]
    fn global_data_is_opt_in_reused_across_libraries_and_disabling_keeps_archive() {
        let home = tempfile::tempdir().unwrap();
        let transport = Arc::new(Transfer::default());
        let store = GeoData::new(home.path().into(), transport.clone());
        let stop = AtomicBool::new(false);
        assert!(!store.inspect(&stop, &|_| {}).unwrap().enabled);
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
        let p = store.prepare(true, &stop, &|_| {}).unwrap();
        assert!(p.enabled && p.ready);
        assert_eq!(transport.count.load(Ordering::SeqCst), 1);
        let bytes = fs::read(store.locations().archive).unwrap();
        store.prepare(true, &stop, &|_| {}).unwrap();
        assert_eq!(transport.count.load(Ordering::SeqCst), 1);
        assert!(!store.prepare(false, &stop, &|_| {}).unwrap().enabled);
        assert_eq!(fs::read(store.locations().archive).unwrap(), bytes);
        let other = GeoData::new(home.path().into(), transport.clone());
        assert!(other.inspect(&stop, &|_| {}).unwrap().ready);
        assert_eq!(transport.count.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn invalid_downloads_cancellation_and_external_changes_preserve_existing_files() {
        for mode in [1, 2, 3, 4] {
            let home = tempfile::tempdir().unwrap();
            let transport = Arc::new(Transfer {
                mode: AtomicUsize::new(mode),
                home: home.path().into(),
                ..Default::default()
            });
            let store = GeoData::new(home.path().into(), transport);
            let stop = AtomicBool::new(false);
            assert!(store.prepare(true, &stop, &|_| {}).is_err());
            let location = store.locations();
            if mode == 3 {
                assert_eq!(fs::read(&location.archive).unwrap(), b"external");
            } else if mode != 4 {
                assert!(!location.archive.exists());
            }
            if mode == 4 {
                assert_eq!(
                    fs::read(&location.preferences).unwrap(),
                    b"external preference"
                );
            } else {
                assert!(!location.preferences.exists());
            }
            assert!(
                !fs::read_dir(location.archive.parent().unwrap())
                    .unwrap()
                    .any(|p| p.unwrap().file_name().to_string_lossy().ends_with(".tmp"))
            );
        }
    }
    #[test]
    fn damaged_zip_and_preferences_are_preserved_and_zip_crc_is_checked() {
        let home = tempfile::tempdir().unwrap();
        let store = GeoData::new(home.path().into(), Arc::new(Transfer::default()));
        let stop = AtomicBool::new(false);
        store.prepare(true, &stop, &|_| {}).unwrap();
        let location = store.locations();
        let prefs = fs::read(&location.preferences).unwrap();
        let mut bytes = fs::read(&location.archive).unwrap();
        let offset = bytes.windows(6).position(|p| p == b"Berlin").unwrap();
        bytes[offset] = b'X';
        fs::write(&location.archive, &bytes).unwrap();
        assert!(!store.inspect(&stop, &|_| {}).unwrap().ready);
        assert_eq!(fs::read(&location.archive).unwrap(), bytes);
        assert!(store.prepare(true, &stop, &|_| {}).unwrap().ready);
        fs::write(&location.preferences, b"bad plist").unwrap();
        assert!(store.prepare(false, &stop, &|_| {}).is_err());
        assert_eq!(fs::read(&location.preferences).unwrap(), b"bad plist");
        assert!(!prefs.is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_data_and_preference_paths_are_rejected() {
        use std::os::unix::fs::symlink;
        for relative in [ARCHIVE, PREFERENCES] {
            let home = tempfile::tempdir().unwrap();
            let target = home.path().join("untouched");
            fs::write(&target, b"original").unwrap();
            let path = library_path(home.path(), relative, true).unwrap();
            symlink(&target, &path).unwrap();
            let store = GeoData::new(home.path().into(), Arc::new(Transfer::default()));
            assert!(
                store
                    .prepare(true, &AtomicBool::new(false), &|_| {})
                    .is_err()
            );
            assert_eq!(fs::read(target).unwrap(), b"original");
        }
    }
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "Explicit network smoke test: downloads only cities500.zip into a temporary home"]
    fn real_geonames_archive_downloads_validates_and_reuses_without_global_preferences() {
        let home = tempfile::tempdir().unwrap();
        let store = GeoData::new(
            home.path().into(),
            Arc::new(crate::whisper_models_native::NativeModelTransport),
        );
        let p = store
            .prepare(true, &AtomicBool::new(false), &|_| {})
            .unwrap();
        assert!(p.ready && p.bytes > 1_000_000);
        assert!(
            store
                .inspect(&AtomicBool::new(false), &|_| {})
                .unwrap()
                .ready
        );
        println!(
            "GeoNames HTTPS and ZIP/CRC/SHA-256 verification OK: {} bytes (temporary files only)",
            p.bytes
        );
    }
}
