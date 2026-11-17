//! Local, cross-library model weights and strictly separate macOS transcription preferences.
use crate::metadata_fs::{atomic_write_if_current, check_file, library_path};
use photosuite_application::transcription::{
    ModelStore, ModelTransport, Prepared, Progress, Settings, Stage,
    catalog::{self, Model},
};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
const PREFS: &str = "Library/Preferences/de.photosuite.PhotoSuite.transcription.plist";
const MODELS: &str = "Library/Application Support/PhotoSuite/Whisper/models";
pub struct WhisperModels {
    home: PathBuf,
    transport: Arc<dyn ModelTransport>,
}
impl WhisperModels {
    pub fn new(home: PathBuf, transport: Arc<dyn ModelTransport>) -> Self {
        Self { home, transport }
    }
    fn model_path(&self, model: &Model, create: bool) -> Result<PathBuf, String> {
        library_path(&self.home, &format!("{MODELS}/{}", model.filename), create)
    }
    fn preferences(&self) -> Result<(Settings, Option<Vec<u8>>), String> {
        let path = library_path(&self.home, PREFS, false)?;
        if !check_file(&path)? {
            return Ok((Settings::default(), None));
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 65536 {
            return Err("Transkriptions-Preferences überschreiten 64 KiB".into());
        }
        let value = plist::Value::from_reader(std::io::Cursor::new(&bytes))
            .map_err(|e| format!("Transkriptions-Preferences bleiben unverändert: {e}"))?;
        let d = value
            .as_dictionary()
            .ok_or("Ungültige Transkriptions-Preferences")?;
        if d.get("version").and_then(plist::Value::as_unsigned_integer) != Some(1)
            || d.keys()
                .any(|k| !["version", "model", "language"].contains(&k.as_str()))
        {
            return Err("Unbekannte Transkriptions-Preferences; Datei bleibt unverändert".into());
        }
        let model = d
            .get("model")
            .and_then(plist::Value::as_string)
            .ok_or("Modellauswahl fehlt")?;
        let language = d
            .get("language")
            .and_then(plist::Value::as_string)
            .ok_or("Sprachauswahl fehlt")?;
        Ok((Settings::new(model, language)?, Some(bytes)))
    }
    fn inventory(&self) -> Result<BTreeSet<String>, String> {
        let mut available = BTreeSet::new();
        for model in catalog::MODELS {
            let path = self.model_path(model, false)?;
            if check_file(&path)?
                && fs::metadata(path).map_err(|e| e.to_string())?.len() == model.bytes
            {
                available.insert(model.id.into());
            }
        }
        Ok(available)
    }
    fn verify(
        &self,
        path: &Path,
        model: &Model,
        cancel: &AtomicBool,
        progress: &(dyn Fn(Progress) + Sync),
    ) -> Result<bool, String> {
        if !check_file(path)? {
            return Ok(false);
        }
        let before = fs::metadata(path).map_err(|e| e.to_string())?;
        if before.len() != model.bytes {
            return Ok(false);
        }
        progress(Progress {
            stage: Stage::Checking,
            bytes: 0,
            total: model.bytes,
        });
        let actual = self.transport.sha256(path, cancel, progress)?;
        let after = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !after.is_file()
            || after.file_type().is_symlink()
            || after.len() != before.len()
            || after.modified().ok() != before.modified().ok()
        {
            return Err("Modell wurde während der Prüfung verändert".into());
        }
        Ok(actual == model.sha256)
    }
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Abgebrochen; bisherige Auswahl bleibt erhalten".into())
    } else {
        Ok(())
    }
}
impl ModelStore for WhisperModels {
    fn locations(&self) -> photosuite_application::transcription::Locations {
        photosuite_application::transcription::Locations {
            models: self.home.join(MODELS),
            preferences: self.home.join(PREFS),
        }
    }
    fn inspect(
        &self,
        cancel: &AtomicBool,
        progress: &(dyn Fn(Progress) + Sync),
    ) -> Result<Prepared, String> {
        cancelled(cancel)?;
        let (settings, _) = self.preferences()?;
        let path = self.model_path(settings.model(), false)?;
        let available = self.inventory()?;
        let valid = self.verify(&path, settings.model(), cancel, progress)?;
        let warning = if !valid && check_file(&path)? {
            "Vorhandenes Modell ist unvollständig oder seine Prüfsumme stimmt nicht. Beim Bestätigen wird ein geprüftes Modell neu heruntergeladen.".into()
        } else {
            String::new()
        };
        Ok(Prepared {
            settings,
            available,
            ready_path: valid.then_some(path),
            warning,
        })
    }
    fn prepare(
        &self,
        settings: &Settings,
        cancel: &AtomicBool,
        progress: &(dyn Fn(Progress) + Sync),
    ) -> Result<Prepared, String> {
        cancelled(cancel)?;
        let (_, revision) = self.preferences()?;
        let model = settings.model();
        let path = self.model_path(model, false)?;
        if !self.verify(&path, model, cancel, progress)? {
            let existing = if check_file(&path)? {
                let m = fs::metadata(&path).map_err(|e| e.to_string())?;
                Some((m.len(), m.modified().ok()))
            } else {
                None
            };
            let target = self.model_path(model, true)?;
            let parent = target.parent().ok_or("Modellverzeichnis fehlt")?;
            let temp = tempfile::Builder::new()
                .prefix(".photosuite-model-")
                .suffix(".tmp")
                .tempfile_in(parent)
                .map_err(|e| e.to_string())?;
            progress(Progress {
                stage: Stage::Downloading,
                bytes: 0,
                total: model.bytes,
            });
            self.transport
                .download(&model.url(), temp.path(), model.bytes, cancel, progress)?;
            cancelled(cancel)?;
            if !self.verify(temp.path(), model, cancel, progress)? {
                return Err("Downloadprüfung fehlgeschlagen: Größe oder SHA-256 stimmt nicht; bisheriges Modell bleibt erhalten".into());
            }
            temp.as_file().sync_all().map_err(|e| e.to_string())?;
            cancelled(cancel)?;
            // Do not overwrite files that appeared/changed while downloading.
            static COMMIT: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _guard = COMMIT
                .lock()
                .map_err(|_| "Modell-Schreibschutz unerwartet beendet")?;
            let current = if check_file(&target)? {
                let m = fs::metadata(&target).map_err(|e| e.to_string())?;
                Some((m.len(), m.modified().ok()))
            } else {
                None
            };
            if current != existing {
                return Err(
                    "Modell wurde während des Downloads extern verändert; kein Überschreiben"
                        .into(),
                );
            }
            if existing.is_none() {
                temp.persist_noclobber(&target).map_err(|e| e.to_string())?;
            } else {
                temp.persist(&target).map_err(|e| e.to_string())?;
            }
        }
        cancelled(cancel)?;
        progress(Progress {
            stage: Stage::Saving,
            bytes: model.bytes,
            total: model.bytes,
        });
        let available = self.inventory()?;
        let value = plist::Value::Dictionary(
            [
                ("version".to_owned(), plist::Value::Integer(1.into())),
                ("model".to_owned(), plist::Value::String(model.id.into())),
                (
                    "language".to_owned(),
                    plist::Value::String(settings.language().into()),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let mut bytes = Vec::new();
        value.to_writer_xml(&mut bytes).map_err(|e| e.to_string())?;
        atomic_write_if_current(
            &library_path(&self.home, PREFS, true)?,
            &bytes,
            revision.as_deref(),
        )
        .map_err(|e| {
            format!(
                "Modell ist geprüft vorhanden; die Auswahl konnte nicht gespeichert werden: {e}"
            )
        })?;
        Ok(Prepared {
            settings: settings.clone(),
            available,
            ready_path: Some(path),
            warning: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    const ID: &str = "tiny-q5_1";
    #[derive(Default)]
    struct Transfer {
        downloads: AtomicUsize,
        wrong_hash: AtomicBool,
        short: AtomicBool,
        stop: AtomicBool,
        mutate: std::sync::Mutex<Option<(PathBuf, Vec<u8>)>>,
    }
    impl ModelTransport for Transfer {
        fn download(
            &self,
            url: &str,
            path: &Path,
            expected: u64,
            cancel: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<(), String> {
            assert_eq!(url, catalog::model(ID).unwrap().url());
            self.downloads.fetch_add(1, Ordering::SeqCst);
            let file = fs::OpenOptions::new().write(true).open(path).unwrap();
            file.set_len(expected - u64::from(self.short.load(Ordering::SeqCst)))
                .unwrap();
            if let Some((path, bytes)) = self.mutate.lock().unwrap().take() {
                fs::write(path, bytes).unwrap();
            }
            if self.stop.load(Ordering::SeqCst) {
                cancel.store(true, Ordering::Relaxed);
                return Err("Abgebrochen".into());
            }
            Ok(())
        }
        fn download_bounded(
            &self,
            _: &str,
            _: &std::path::Path,
            _: u64,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<(), String> {
            Err("unused".into())
        }
        fn sha256(
            &self,
            _: &Path,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<String, String> {
            Ok(if self.wrong_hash.load(Ordering::SeqCst) {
                "0".repeat(64)
            } else {
                catalog::model(ID).unwrap().sha256.into()
            })
        }
    }
    #[test]
    fn global_model_is_shared_verified_and_reused_without_writing_any_library() {
        let home = tempfile::tempdir().unwrap();
        let transport = Arc::new(Transfer::default());
        let store = WhisperModels::new(home.path().into(), transport.clone());
        let stop = AtomicBool::new(false);
        let choice = Settings::new(ID, "de").unwrap();
        assert!(store.inspect(&stop, &|_| {}).unwrap().ready_path.is_none());
        assert_eq!(
            fs::read_dir(home.path()).unwrap().count(),
            0,
            "inspect never creates folders"
        );
        let p = store.prepare(&choice, &stop, &|_| {}).unwrap();
        assert!(p.ready_path.unwrap().is_file());
        let pref = home.path().join(PREFS);
        let (restored, _) = store.preferences().unwrap();
        assert_eq!(restored, choice);
        let library_a = home.path().join("library-a");
        let library_b = home.path().join("library-b");
        fs::create_dir(&library_a).unwrap();
        fs::create_dir(&library_b).unwrap();
        store
            .prepare(&Settings::new(ID, "fr").unwrap(), &stop, &|_| {})
            .unwrap();
        assert_eq!(transport.downloads.load(Ordering::SeqCst), 1);
        assert!(pref.is_file());
        assert!(fs::read_dir(library_a).unwrap().next().is_none());
        assert!(fs::read_dir(library_b).unwrap().next().is_none());
        let text = fs::read_to_string(pref).unwrap();
        assert!(text.contains("<string>fr</string>"));
        assert!(!text.contains("last_library"));
    }
    #[test]
    fn incomplete_wrong_hash_and_cancelled_downloads_never_publish_or_change_preferences() {
        for mode in 0..3 {
            let home = tempfile::tempdir().unwrap();
            let transfer = Arc::new(Transfer::default());
            transfer.short.store(mode == 0, Ordering::SeqCst);
            transfer.wrong_hash.store(mode == 1, Ordering::SeqCst);
            transfer.stop.store(mode == 2, Ordering::SeqCst);
            let store = WhisperModels::new(home.path().into(), transfer);
            let stop = AtomicBool::new(false);
            let choice = Settings::new(ID, "de").unwrap();
            assert!(store.prepare(&choice, &stop, &|_| {}).is_err());
            assert!(!home.path().join(PREFS).exists());
            assert!(!store.model_path(choice.model(), false).unwrap().exists());
            let directory = home.path().join(MODELS);
            assert!(
                fs::read_dir(directory).unwrap().next().is_none(),
                "owned temporary download cleaned"
            );
        }
    }
    #[test]
    fn damaged_preferences_symlinks_and_external_changes_are_preserved() {
        let home = tempfile::tempdir().unwrap();
        let transfer = Arc::new(Transfer::default());
        let store = WhisperModels::new(home.path().into(), transfer.clone());
        let pref = library_path(home.path(), PREFS, true).unwrap();
        fs::write(&pref, b"not a plist").unwrap();
        assert!(
            store
                .prepare(
                    &Settings::new(ID, "de").unwrap(),
                    &AtomicBool::new(false),
                    &|_| {}
                )
                .is_err()
        );
        assert_eq!(fs::read(&pref).unwrap(), b"not a plist");
        assert_eq!(transfer.downloads.load(Ordering::SeqCst), 0);
        fs::remove_file(&pref).unwrap();
        *transfer.mutate.lock().unwrap() = Some((pref.clone(), b"external change".to_vec()));
        assert!(
            store
                .prepare(
                    &Settings::new(ID, "de").unwrap(),
                    &AtomicBool::new(false),
                    &|_| {}
                )
                .is_err()
        );
        assert_eq!(fs::read(pref).unwrap(), b"external change");
        #[cfg(unix)]
        {
            let home = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(outside.path(), home.path().join("Library")).unwrap();
            let store = WhisperModels::new(home.path().into(), Arc::new(Transfer::default()));
            assert!(
                store
                    .prepare(
                        &Settings::new(ID, "de").unwrap(),
                        &AtomicBool::new(false),
                        &|_| {}
                    )
                    .is_err()
            );
            assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
        }
    }
    #[test]
    fn failed_repair_preserves_old_model_and_only_verified_repair_replaces_it() {
        let home = tempfile::tempdir().unwrap();
        let transfer = Arc::new(Transfer::default());
        let store = WhisperModels::new(home.path().into(), transfer.clone());
        let choice = Settings::new(ID, "de").unwrap();
        let target = store.model_path(choice.model(), true).unwrap();
        fs::write(&target, b"partial older model").unwrap();
        transfer.wrong_hash.store(true, Ordering::SeqCst);
        assert!(
            store
                .prepare(&choice, &AtomicBool::new(false), &|_| {})
                .is_err()
        );
        assert_eq!(fs::read(&target).unwrap(), b"partial older model");
        assert!(!home.path().join(PREFS).exists());
        transfer.wrong_hash.store(false, Ordering::SeqCst);
        let prepared = store
            .prepare(&choice, &AtomicBool::new(false), &|_| {})
            .unwrap();
        assert_eq!(prepared.ready_path, Some(target.clone()));
        assert_eq!(fs::metadata(target).unwrap().len(), choice.model().bytes);
    }
    #[test]
    fn externally_appearing_model_is_never_overwritten_and_cancel_preserves_existing_preferences() {
        let home = tempfile::tempdir().unwrap();
        let transfer = Arc::new(Transfer::default());
        let store = WhisperModels::new(home.path().into(), transfer.clone());
        let choice = Settings::new(ID, "de").unwrap();
        let target = store.model_path(choice.model(), true).unwrap();
        *transfer.mutate.lock().unwrap() = Some((target.clone(), b"external model".to_vec()));
        assert!(
            store
                .prepare(&choice, &AtomicBool::new(false), &|_| {})
                .is_err()
        );
        assert_eq!(fs::read(&target).unwrap(), b"external model");
        fs::remove_file(target).unwrap();
        store
            .prepare(&choice, &AtomicBool::new(false), &|_| {})
            .unwrap();
        let pref = home.path().join(PREFS);
        let before = fs::read(&pref).unwrap();
        let stop = AtomicBool::new(true);
        assert!(
            store
                .prepare(&Settings::new(ID, "fr").unwrap(), &stop, &|_| {})
                .is_err()
        );
        assert_eq!(fs::read(pref).unwrap(), before);
    }
}
