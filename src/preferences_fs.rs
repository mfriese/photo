//! XML property-list adapter stored in the user's macOS Preferences directory.
use crate::preferences_codec::{decode, encode};
use photosuite_application::preferences::{Preferences, PreferencesStore};
use plist::Value;
use std::{io::Write, path::PathBuf};
pub struct PlistPreferences {
    path: PathBuf,
    legacy: Option<PathBuf>,
}
impl PlistPreferences {
    pub fn at(path: PathBuf) -> Self {
        let legacy = path
            .file_name()
            .filter(|name| *name == "de.photosuite.PhotoSuite.settings.plist")
            .and_then(|_| path.parent())
            .map(|parent| parent.join("de.photosuite.PhotoSuite.plist"));
        Self { path, legacy }
    }
    pub fn user_path() -> Result<PathBuf, String> {
        let home = std::env::var_os("HOME").ok_or("Benutzerverzeichnis nicht verfügbar")?;
        Ok(PathBuf::from(home).join("Library/Preferences/de.photosuite.PhotoSuite.settings.plist"))
    }
}
impl PreferencesStore for PlistPreferences {
    fn load(&self) -> Result<Preferences, String> {
        let opened = std::fs::File::open(&self.path).or_else(|error| {
            if error.kind() == std::io::ErrorKind::NotFound
                && let Some(legacy) = &self.legacy
            {
                std::fs::File::open(legacy)
            } else {
                Err(error)
            }
        });
        match opened {
            Ok(file) => decode(Value::from_reader(file).map_err(|e| e.to_string())?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    fn save(&self, p: &Preferences) -> Result<(), String> {
        // Do not silently replace malformed or newer files.
        self.load()?;
        let parent = self
            .path
            .parent()
            .ok_or("Preferences-Pfad ohne Verzeichnis")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        encode(p)
            .to_writer_xml(&mut file)
            .map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(&self.path).map_err(|e| e.to_string())?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use photosuite_application::preferences::Shortcut;
    #[test]
    fn legacy_library_is_migrated_without_modifying_macos_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let old = directory.path().join("de.photosuite.PhotoSuite.plist");
        let new = directory
            .path()
            .join("de.photosuite.PhotoSuite.settings.plist");
        let original = Preferences {
            last_library: Some("/Fotos/Familie".into()),
            ..Default::default()
        };
        PlistPreferences::at(old.clone()).save(&original).unwrap();
        let bytes = std::fs::read(&old).unwrap();
        let store = PlistPreferences::at(new.clone());
        assert_eq!(store.load().unwrap(), original);
        let mut persistence = photosuite_application::preferences::Persistence::new(
            std::sync::Arc::new(store),
            original.clone(),
        );
        persistence.flush_current();
        drop(persistence);
        assert_eq!(PlistPreferences::at(new.clone()).load().unwrap(), original);
        assert_eq!(std::fs::read(&old).unwrap(), bytes);
        std::fs::write(old, b"changed by macOS").unwrap();
        assert_eq!(
            PlistPreferences::at(new).load().unwrap().last_library,
            original.last_library
        );
    }
    #[test]
    fn plist_roundtrip_and_removed_shortcuts() {
        let dir = tempfile::tempdir().unwrap();
        let store = PlistPreferences::at(dir.path().join("prefs.plist"));
        let mut p = Preferences {
            preview_size: 4,
            write_exif: true,
            last_library: Some("/Fotos/Urlaub & Familie".into()),
            ..Default::default()
        };
        p.shortcuts.insert("rotate_left".into(), None);
        p.shortcuts.insert(
            "rotate_right".into(),
            Some(Shortcut {
                key: "R".into(),
                alt: true,
                ctrl: false,
                shift: true,
                command: false,
                mac_cmd: false,
            }),
        );
        store.save(&p).unwrap();
        assert_eq!(store.load().unwrap(), p);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn corrupted_preferences_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.plist");
        std::fs::write(&path, b"broken").unwrap();
        let store = PlistPreferences::at(path.clone());
        assert!(store.load().is_err());
        assert!(store.save(&Preferences::default()).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"broken");
    }
}
