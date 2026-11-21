//! Library-local YAML settings; only the current/recent library locator is stored on this Mac.
use crate::metadata_fs::{atomic_write, check_file};
use photosuite_application::preferences::{Preferences, PreferencesStore};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
const SETTINGS: &str = "yaml/settings.yaml";
const MAX_SETTINGS: u64 = 64 * 1024;
pub struct LibraryPreferences {
    locator: PathBuf,
}
impl LibraryPreferences {
    pub fn user_path() -> Result<PathBuf, String> {
        let home = std::env::var_os("HOME").ok_or("Benutzerverzeichnis nicht verfügbar")?;
        Ok(PathBuf::from(home).join("Library/Preferences/de.photosuite.PhotoSuite.library.plist"))
    }
    pub fn new(locator: PathBuf) -> Self {
        Self { locator }
    }
    fn local(&self) -> Result<Preferences, String> {
        match fs::symlink_metadata(&self.locator) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                let mut bytes = Vec::new();
                fs::File::open(&self.locator)
                    .map_err(|e| e.to_string())?
                    .take(MAX_SETTINGS + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() as u64 > MAX_SETTINGS {
                    return Err("Bibliotheks-Verweis überschreitet 64 KiB".into());
                }
                let value = plist::Value::from_reader(std::io::Cursor::new(bytes))
                    .map_err(|e| e.to_string())?;
                let dictionary = value
                    .as_dictionary()
                    .ok_or("Ungültiger Bibliotheks-Verweis")?;
                if dictionary
                    .get("version")
                    .and_then(plist::Value::as_unsigned_integer)
                    != Some(1)
                    || dictionary.keys().any(|k| {
                        !["version", "last_library", "recent_libraries"].contains(&k.as_str())
                    })
                {
                    return Err(
                        "Unbekanntes Format des Bibliotheks-Verweises; Datei bleibt unverändert"
                            .into(),
                    );
                }
                let last_library = dictionary
                    .get("last_library")
                    .map(|v| {
                        let path = v
                            .as_string()
                            .filter(|s| !s.is_empty())
                            .ok_or("Ungültiger Bibliothekspfad")?;
                        Ok::<_, String>(PathBuf::from(path))
                    })
                    .transpose()?;
                let recent = dictionary
                    .get("recent_libraries")
                    .map(|value| {
                        value
                            .as_array()
                            .ok_or("Ungültige Liste zuletzt geöffneter Bibliotheken")?
                            .iter()
                            .map(|value| {
                                value
                                    .as_string()
                                    .filter(|path| !path.is_empty())
                                    .map(PathBuf::from)
                                    .ok_or("Ungültiger Bibliothekspfad in Verlauf".to_owned())
                            })
                            .collect::<Result<Vec<_>, String>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                let mut preferences = Preferences {
                    last_library,
                    ..Default::default()
                };
                preferences.set_recent_libraries(recent);
                Ok(preferences)
            }
            Ok(_) => Err("Bibliotheks-Verweis ist keine reguläre Datei".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    fn initial(&self, explicit: Option<&Path>) -> Result<(Preferences, Option<String>), String> {
        let local = self.local()?;
        let Some(root) = explicit
            .map(Path::to_path_buf)
            .or_else(|| local.last_library.clone())
        else {
            return Ok((local, None));
        };
        match self.load_library(&root) {
            Ok(Some(mut settings)) => {
                settings.last_library = local.last_library.clone();
                settings.recent_libraries = local.recent_libraries.clone();
                settings.library_scope = Some(root);
                Ok((settings, None))
            }
            Ok(None) => Ok((local, None)),
            Err(error) => {
                let mut local = local;
                local.library_scope = None;
                Ok((
                    local,
                    Some(format!(
                        "Bibliotheks-Einstellungen konnten nicht geladen werden: {error}"
                    )),
                ))
            }
        }
    }
}
fn regular_root(root: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Einstellungsziel ist kein reguläres Bibliotheksverzeichnis".into());
    }
    Ok(())
}
fn read_settings_bytes(root: &Path) -> Result<Option<Vec<u8>>, String> {
    regular_root(root)?;
    let path = crate::metadata_fs::library_path(root, SETTINGS, false)?;
    if !check_file(&path)? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .map_err(|e| e.to_string())?
        .take(MAX_SETTINGS + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(Some(bytes))
}
fn decode_yaml(bytes: &[u8]) -> Result<Preferences, String> {
    if bytes.len() as u64 > MAX_SETTINGS {
        return Err("yaml/settings.yaml ist größer als 64 KiB; Datei bleibt unverändert".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let value: serde_json::Value =
        serde_saphyr::from_str(text).map_err(|e| format!("yaml/settings.yaml: {e}"))?;
    let object = value
        .as_object()
        .ok_or("yaml/settings.yaml muss eine Key-Value-Liste sein")?;
    if object.get("version").and_then(|v| v.as_u64()) != Some(1) {
        return Err("Unbekannte YAML-Einstellungsversion; Datei bleibt unverändert".into());
    }
    for key in object.keys() {
        if ![
            "version",
            "preview_size",
            "square_previews",
            "show_library",
            "videos_only",
            "write_exif",
            "reopen_library",
            "shortcuts",
        ]
        .contains(&key.as_str())
        {
            return Err(format!(
                "Unbekannter YAML-Eintrag {key}; Datei bleibt unverändert"
            ));
        }
    }
    let plist: plist::Value = serde_json::from_value(value).map_err(|e| e.to_string())?;
    crate::preferences_codec::decode(plist)
}
fn read_yaml(root: &Path) -> Result<Option<Preferences>, String> {
    read_settings_bytes(root)?
        .as_deref()
        .map(decode_yaml)
        .transpose()
}
fn yaml(preferences: &Preferences) -> Result<String, String> {
    let encoded = crate::preferences_codec::encode(preferences);
    crate::preferences_codec::decode(encoded.clone())?;
    let mut value = serde_json::to_value(encoded).map_err(|e| e.to_string())?;
    let object = value
        .as_object_mut()
        .ok_or("Einstellungen sind keine Key-Value-Liste")?;
    object.remove("last_library");
    object.remove("version");
    let text = serde_saphyr::to_string(&value).map_err(|e| e.to_string())?;
    Ok(format!(
        "# PhotoSuite – Einstellungen dieser Bibliothek\n# Der Bibliothekspfad wird ausschließlich lokal auf diesem Mac gespeichert.\nversion: 1\n{text}"
    ))
}
impl PreferencesStore for LibraryPreferences {
    fn load(&self) -> Result<Preferences, String> {
        self.initial(None).map(|(p, _)| p)
    }
    fn load_with_status(&self) -> Result<(Preferences, Option<String>), String> {
        self.initial(None)
    }
    fn load_for_startup(
        &self,
        explicit: Option<&Path>,
    ) -> Result<(Preferences, Option<String>), String> {
        self.initial(explicit)
    }
    fn startup_attempt(
        &self,
        explicit: Option<&Path>,
        progress: &dyn Fn(photosuite_application::preferences::StartupLocation),
    ) -> Result<photosuite_application::preferences::StartupRead, String> {
        use photosuite_application::preferences::{StartupLocation, StartupRead};
        let local = self.local()?;
        let root = explicit
            .map(Path::to_path_buf)
            .or_else(|| local.last_library.clone());
        progress(StartupLocation {
            preferences: local.clone(),
            library: root.clone(),
        });
        let Some(root) = root else {
            return Ok(StartupRead {
                preferences: local,
                library: None,
                ready: true,
                warning: None,
            });
        };
        // Actually open/read the directory, not just a possibly cached exists() check.
        let accessible = (|| {
            regular_root(&root)?;
            fs::read_dir(&root)
                .map_err(|e| e.to_string())?
                .next()
                .transpose()
                .map_err(|e| e.to_string())?;
            Ok::<_, String>(())
        })();
        if let Err(error) = accessible {
            return Ok(StartupRead {
                preferences: local,
                library: Some(root),
                ready: false,
                warning: Some(error),
            });
        }
        let bytes = match read_settings_bytes(&root) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ok(StartupRead {
                    preferences: local,
                    library: Some(root),
                    ready: false,
                    warning: Some(error),
                });
            }
        };
        match bytes.as_deref().map(decode_yaml).transpose() {
            Ok(settings) => {
                let mut settings = settings.unwrap_or_default();
                settings.last_library = local.last_library;
                settings.recent_libraries = local.recent_libraries;
                let open = explicit.is_some() || settings.reopen_library;
                settings.library_scope = Some(root.clone());
                Ok(StartupRead {
                    preferences: settings,
                    library: open.then_some(root),
                    ready: true,
                    warning: None,
                })
            }
            Err(error) => Ok(StartupRead {
                preferences: local,
                library: Some(root),
                ready: true,
                warning: Some(format!(
                    "Bibliotheks-Einstellungen konnten nicht geladen werden: {error}"
                )),
            }),
        }
    }
    fn load_library(&self, root: &Path) -> Result<Option<Preferences>, String> {
        Ok(Some(read_yaml(root)?.unwrap_or_default()))
    }

    fn save(&self, preferences: &Preferences) -> Result<(), String> {
        if let Some(root) = &preferences.library_scope {
            // Validate existing content before any replacement, including unknown versions/keys.
            read_yaml(root)?;
            let text = yaml(preferences)?;
            if text.len() as u64 > MAX_SETTINGS {
                return Err("Einstellungen überschreiten die Grenze von 64 KiB".into());
            }
            let path = crate::metadata_fs::library_path(root, SETTINGS, true)?;
            atomic_write(&path, text.as_bytes())
                .map_err(|e| format!("{}: {e}", root.join(SETTINGS).display()))
        } else {
            let previous = self.local()?;
            let mut location = preferences.clone();
            location.merge_library_history(&previous);
            let parent = self
                .locator
                .parent()
                .ok_or("Bibliotheks-Verweis ohne Verzeichnis")?;
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            let mut dictionary = plist::Dictionary::new();
            dictionary.insert("version".into(), plist::Value::Integer(1.into()));
            if let Some(path) = &preferences.last_library {
                if path.as_os_str().is_empty() {
                    return Err("Ungültiger leerer Bibliothekspfad".into());
                }
                dictionary.insert(
                    "last_library".into(),
                    plist::Value::String(path.to_string_lossy().into_owned()),
                );
            }
            if !location.recent_libraries.is_empty() {
                dictionary.insert(
                    "recent_libraries".into(),
                    plist::Value::Array(
                        location
                            .recent_libraries
                            .iter()
                            .map(|path| plist::Value::String(path.to_string_lossy().into_owned()))
                            .collect(),
                    ),
                );
            }
            let mut bytes = Vec::new();
            plist::Value::Dictionary(dictionary)
                .to_writer_xml(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_SETTINGS {
                return Err("Bibliotheks-Verweis überschreitet 64 KiB".into());
            }
            atomic_write(&self.locator, &bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store_at(path: PathBuf) -> LibraryPreferences {
        LibraryPreferences::new(path.with_extension("location.plist"))
    }
    use photosuite_application::preferences::Shortcut;
    #[test]
    fn version_one_locator_accepts_optional_history_and_keeps_it_out_of_library_settings() {
        let directory = tempfile::tempdir().unwrap();
        let locator = directory.path().join("library.plist");
        let root = directory.path().join("photos");
        fs::create_dir(&root).unwrap();
        let store = LibraryPreferences::new(locator.clone());
        let mut old = plist::Dictionary::new();
        old.insert("version".into(), plist::Value::Integer(1.into()));
        old.insert(
            "last_library".into(),
            plist::Value::String(root.to_str().unwrap().into()),
        );
        plist::Value::Dictionary(old).to_file_xml(&locator).unwrap();
        let mut preferences = store.load().unwrap();
        assert!(preferences.recent_libraries.is_empty());
        preferences.library_scope = Some(root.clone());
        store.save(&preferences).unwrap();
        let settings = fs::read(root.join(SETTINGS)).unwrap();
        preferences.library_scope = None;
        for index in 0..7 {
            preferences.remember_library(directory.path().join(format!("offline{index}")));
        }
        store.save(&preferences).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.last_library, preferences.last_library);
        assert_eq!(loaded.recent_libraries, preferences.recent_libraries);
        assert_eq!(loaded.recent_libraries.len(), 5);
        let value = plist::Value::from_file(&locator).unwrap();
        let dictionary = value.as_dictionary().unwrap();
        assert_eq!(dictionary.len(), 3);
        assert_eq!(dictionary["version"].as_unsigned_integer(), Some(1));
        assert_eq!(dictionary["recent_libraries"].as_array().unwrap().len(), 5);
        assert_eq!(
            fs::read(root.join(SETTINGS)).unwrap(),
            settings,
            "local history must not alter library YAML"
        );
        preferences.library_scope = Some(root.clone());
        store.save(&preferences).unwrap();
        assert_eq!(
            fs::read(root.join(SETTINGS)).unwrap(),
            settings,
            "history must not be serialized in library settings"
        );
        assert_eq!(
            fs::read_dir(directory.path()).unwrap().count(),
            2,
            "history must not create an extra settings file"
        );
        // A late navigation snapshot must retain older alternatives already stored locally.
        let late = Preferences {
            last_library: Some(directory.path().join("new")),
            ..Default::default()
        };
        store.save(&late).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(
            loaded.recent_libraries.first(),
            preferences.last_library.as_ref()
        );
        assert_eq!(loaded.recent_libraries.len(), 5);
    }
    #[test]
    fn malformed_recent_library_values_preserve_the_existing_locator() {
        let directory = tempfile::tempdir().unwrap();
        let locator = directory.path().join("library.plist");
        let store = LibraryPreferences::new(locator.clone());
        for value in [
            "<string>wrong type</string>",
            "<array><integer>3</integer></array>",
            "<array><string></string></array>",
        ] {
            let text = format!(
                "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>version</key><integer>1</integer><key>recent_libraries</key>{value}</dict></plist>"
            );
            fs::write(&locator, &text).unwrap();
            assert!(store.load().is_err());
            assert!(store.save(&Preferences::default()).is_err());
            assert_eq!(fs::read_to_string(&locator).unwrap(), text);
        }
    }
    #[test]
    fn catalogs_use_sqlite_settings_remain_yaml_and_old_locations_are_ignored() {
        use photosuite_application::{
            comments::{CommentCache, CommentRepository},
            organization::CatalogRepository,
        };
        let root = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        for name in [
            "photosuite.yaml",
            "photosuite-tags.yaml",
            "photosuite-comments.yaml",
        ] {
            fs::write(root.path().join(name), b"old-invalid: [").unwrap();
        }
        fs::create_dir_all(root.path().join("1980/audio/y/09")).unwrap();
        fs::write(
            root.path().join("1980/audio/y/09/photo.jpg.yaml"),
            b"old-invalid: [",
        )
        .unwrap();
        let settings = store_at(local.path().join("prefs.plist"));
        assert!(settings.load_library(root.path()).unwrap().is_some());
        let tags = crate::tag_store::SqliteTags::default();
        let loaded = tags.load(root.path()).unwrap();
        assert!(loaded.catalog.tags().is_empty());
        let comments = crate::comments_fs::SqliteComments {
            database: std::sync::Arc::default(),
            priority: Default::default(),
        };
        assert!(matches!(
            comments.read_cache(root.path()).unwrap(),
            CommentCache::Missing
        ));
        assert!(
            comments
                .scan_photo_records(
                    root.path(),
                    &std::sync::atomic::AtomicBool::new(false),
                    &|_| {}
                )
                .unwrap()
                .is_empty()
        );
        assert!(root.path().join(crate::library_db::FILE).is_file());
        settings
            .save(&Preferences {
                library_scope: Some(root.path().into()),
                ..Default::default()
            })
            .unwrap();
        tags.save(root.path(), &loaded.catalog, None).unwrap();
        comments
            .replace_index(
                root.path(),
                &crate::domain::comments::CommentCatalog::default(),
                None,
            )
            .unwrap();
        for name in ["settings.yaml", "library.sqlite"] {
            assert!(root.path().join("yaml").join(name).is_file());
        }
        for name in [
            "photosuite.yaml",
            "photosuite-tags.yaml",
            "photosuite-comments.yaml",
        ] {
            assert_eq!(fs::read(root.path().join(name)).unwrap(), b"old-invalid: [");
        }
    }
    #[cfg(unix)]
    #[test]
    fn every_yaml_store_rejects_a_symlinked_shared_directory() {
        use photosuite_application::{
            comments::CommentRepository, organization::CatalogRepository,
        };
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("settings.yaml"), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("yaml")).unwrap();
        let settings = store_at(local.path().join("prefs.plist"));
        assert!(settings.load_library(root.path()).is_err());
        assert!(
            settings
                .save(&Preferences {
                    library_scope: Some(root.path().into()),
                    ..Default::default()
                })
                .is_err()
        );
        let tags = crate::tag_store::SqliteTags::default();
        assert!(tags.load(root.path()).is_err());
        assert!(tags.save(root.path(), &Default::default(), None).is_err());
        let comments = crate::comments_fs::SqliteComments {
            database: std::sync::Arc::default(),
            priority: Default::default(),
        };
        assert!(comments.read_cache(root.path()).is_err());
        assert!(
            comments
                .replace_index(root.path(), &Default::default(), None)
                .is_err()
        );
        assert!(
            comments
                .scan_photo_records(
                    root.path(),
                    &std::sync::atomic::AtomicBool::new(false),
                    &|_| {}
                )
                .is_err()
        );
        assert_eq!(
            fs::read(outside.path().join("settings.yaml")).unwrap(),
            b"outside"
        );
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
    }
    #[test]
    fn explicit_startup_path_uses_its_settings_even_if_remembered_library_is_missing() {
        let directory = tempfile::tempdir().unwrap();
        let store = store_at(directory.path().join("prefs.plist"));
        let old = directory.path().join("unavailable");
        let chosen = directory.path().join("chosen");
        fs::create_dir(&chosen).unwrap();
        store
            .save(&Preferences {
                last_library: Some(old.clone()),
                ..Default::default()
            })
            .unwrap();
        store
            .save(&Preferences {
                library_scope: Some(chosen.clone()),
                preview_size: 4,
                ..Default::default()
            })
            .unwrap();
        let (loaded, warning) = store.load_for_startup(Some(&chosen)).unwrap();
        assert!(warning.is_none());
        assert_eq!(loaded.preview_size, 4);
        assert_eq!(loaded.last_library, Some(old));
        assert_eq!(loaded.library_scope, Some(chosen));
    }
    #[test]
    fn yaml_roundtrip_keeps_settings_in_library_and_only_locator_locally() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("prefs.plist");
        let root = directory.path().join("library");
        fs::create_dir(&root).unwrap();
        let mut legacy = Preferences {
            preview_size: 4,
            square_previews: false,
            videos_only: true,
            write_exif: true,
            show_library: false,
            reopen_library: false,
            last_library: Some(root.clone()),
            ..Default::default()
        };
        legacy.shortcuts.insert("rotate_left".into(), None);
        legacy.shortcuts.insert(
            "rotate_right".into(),
            Some(Shortcut {
                key: "R".into(),
                alt: true,
                ctrl: false,
                shift: true,
                command: false,
                mac_cmd: true,
            }),
        );
        fs::write(&legacy_path, b"unsupported old preferences").unwrap();
        let original = fs::read(&legacy_path).unwrap();
        let store = store_at(legacy_path.clone());
        let mut settings = legacy.clone();
        settings.library_scope = Some(root.clone());
        store.save(&settings).unwrap();
        store
            .save(&Preferences {
                last_library: Some(root.clone()),
                ..Default::default()
            })
            .unwrap();
        let (mut loaded, warning) = store.load_with_status().unwrap();
        assert!(warning.is_none());
        assert_eq!(loaded.preview_size, 4);
        assert_eq!(loaded.library_scope.as_ref(), Some(&root));
        store.save(&loaded).unwrap();
        let text = fs::read_to_string(root.join(SETTINGS)).unwrap();
        assert!(text.contains("preview_size: 4"));
        assert!(text.contains("square_previews: false"));
        assert!(!text.contains("last_library"));
        assert!(!text.contains("library_scope"));
        assert!(!text.contains(root.to_str().unwrap()));
        loaded.library_scope = None;
        store.save(&loaded).unwrap();
        let locator = plist::Value::from_file(&store.locator)
            .unwrap()
            .into_dictionary()
            .unwrap();
        assert_eq!(locator.len(), 2);
        assert!(locator.contains_key("last_library"));
        assert!(!locator.contains_key("preview_size"));
        let restored = store.load().unwrap();
        legacy.library_scope = Some(root.clone());
        assert_eq!(restored, legacy);
        assert!(!restored.reopen_library);
        assert!(restored.startup_library([]).is_none());
        assert_eq!(fs::read(&legacy_path).unwrap(), original);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }
    #[test]
    fn two_libraries_keep_independent_settings_and_scope_does_not_move_locator() {
        let directory = tempfile::tempdir().unwrap();
        let store = store_at(directory.path().join("prefs.plist"));
        let a = directory.path().join("a");
        let b = directory.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let location = Preferences {
            last_library: Some(b.clone()),
            ..Default::default()
        };
        store.save(&location).unwrap();
        let first = Preferences {
            preview_size: 0,
            library_scope: Some(a.clone()),
            last_library: Some(a.clone()),
            ..Default::default()
        };
        let second = Preferences {
            preview_size: 4,
            square_previews: false,
            library_scope: Some(b.clone()),
            last_library: Some(b.clone()),
            ..Default::default()
        };
        store.save(&second).unwrap();
        store.save(&first).unwrap();
        assert_eq!(store.load().unwrap().last_library, Some(b.clone()));
        assert_eq!(store.load_library(&a).unwrap().unwrap().preview_size, 0);
        assert_eq!(store.load_library(&b).unwrap().unwrap().preview_size, 4);
        assert_eq!(fs::read_dir(a).unwrap().count(), 1);
        assert_eq!(fs::read_dir(b).unwrap().count(), 1);
    }
    #[test]
    fn malformed_future_and_unknown_settings_are_preserved_and_location_survives() {
        let directory = tempfile::tempdir().unwrap();
        let store = store_at(directory.path().join("prefs.plist"));
        let root = directory.path().join("library");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("yaml")).unwrap();
        let location = Preferences {
            last_library: Some(root.clone()),
            ..Default::default()
        };
        store.save(&location).unwrap();
        fs::write(root.join("original.jpg"), b"unchanged original").unwrap();
        for text in [
            "bad: [",
            "version: 2\npreview_size: 2\n",
            "version: 1\npreview_size: 99\n",
            "version: 1\ncustom: true\n",
            "version: 1\nshow_library: invalid\n",
        ] {
            fs::write(root.join(SETTINGS), text).unwrap();
            let (loaded, warning) = store.load_with_status().unwrap();
            assert!(warning.is_some());
            assert_eq!(loaded.last_library, Some(root.clone()));
            assert!(loaded.library_scope.is_none());
            let settings = Preferences {
                library_scope: Some(root.clone()),
                ..Default::default()
            };
            assert!(store.save(&settings).is_err());
            assert_eq!(fs::read_to_string(root.join(SETTINGS)).unwrap(), text);
        }
        assert_eq!(
            fs::read(root.join("original.jpg")).unwrap(),
            b"unchanged original"
        );
    }
    #[test]
    fn read_only_library_reports_write_failure_without_modifying_media() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("library");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("photo.jpg"), b"original").unwrap();
        let store = store_at(directory.path().join("prefs.plist"));
        let permissions = fs::metadata(&root).unwrap().permissions();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
        let result = store.save(&Preferences {
            library_scope: Some(root.clone()),
            ..Default::default()
        });
        fs::set_permissions(&root, permissions).unwrap();
        assert!(result.is_err());
        assert!(!root.join(SETTINGS).exists());
        assert_eq!(fs::read(root.join("photo.jpg")).unwrap(), b"original");
    }
    #[test]
    fn symlink_or_missing_library_is_never_written_and_empty_files_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("library");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("yaml")).unwrap();
        let store = store_at(directory.path().join("prefs.plist"));
        let outside = directory.path().join("outside.yaml");
        fs::write(&outside, b"outside").unwrap();
        std::os::unix::fs::symlink(&outside, root.join(SETTINGS)).unwrap();
        let settings = Preferences {
            library_scope: Some(root.clone()),
            ..Default::default()
        };
        assert!(store.save(&settings).is_err());
        assert_eq!(fs::read(outside).unwrap(), b"outside");
        let missing = directory.path().join("missing");
        assert!(
            store
                .save(&Preferences {
                    library_scope: Some(missing.clone()),
                    ..Default::default()
                })
                .is_err()
        );
        assert!(!missing.exists());
        fs::remove_file(root.join(SETTINGS)).unwrap();
        fs::write(root.join(SETTINGS), b"").unwrap();
        assert!(store.save(&settings).is_err());
        assert!(fs::read(root.join(SETTINGS)).unwrap().is_empty());
    }
}

#[cfg(test)]
mod locator_validation_tests {
    use super::*;
    #[test]
    fn unsupported_or_oversized_local_locator_is_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let locator = directory.path().join("library.plist");
        let store = LibraryPreferences::new(locator.clone());
        for dictionary in [
            "<dict><key>last_library</key><string>/Fotos</string></dict>",
            "<dict><key>version</key><integer>1</integer><key>preview_size</key><integer>4</integer></dict>",
            "<dict><key>version</key><integer>2</integer></dict>",
            "<dict><key>version</key><integer>1</integer><key>last_library</key><string></string></dict>",
        ] {
            let bytes =
                format!("<?xml version=\"1.0\"?><plist version=\"1.0\">{dictionary}</plist>")
                    .into_bytes();
            fs::write(&locator, &bytes).unwrap();
            assert!(store.load().is_err());
            assert!(store.save(&Preferences::default()).is_err());
            assert_eq!(fs::read(&locator).unwrap(), bytes);
        }
        let bytes = vec![b' '; MAX_SETTINGS as usize + 1];
        fs::write(&locator, &bytes).unwrap();
        assert!(store.load().unwrap_err().contains("64 KiB"));
        assert!(store.save(&Preferences::default()).is_err());
        assert_eq!(fs::read(locator).unwrap(), bytes);
    }
    #[test]
    fn unknown_shortcut_fields_are_not_silently_discarded() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("yaml")).unwrap();
        let path = root.path().join(SETTINGS);
        let bytes =
            b"version: 1\nshortcuts:\n  rotate_left:\n    key: L\n    future_modifier: true\n";
        fs::write(&path, bytes).unwrap();
        let store = LibraryPreferences::new(root.path().join("library.plist"));
        assert!(store.load_library(root.path()).is_err());
        assert!(
            store
                .save(&Preferences {
                    library_scope: Some(root.path().into()),
                    ..Default::default()
                })
                .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[cfg(test)]
mod startup_readiness_tests {
    use super::*;
    #[test]
    fn startup_access_retries_missing_storage_but_preserves_invalid_settings_and_locator() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("offline");
        let locator = directory.path().join("library.plist");
        let store = LibraryPreferences::new(locator.clone());
        store
            .save(&Preferences {
                last_library: Some(root.clone()),
                ..Default::default()
            })
            .unwrap();
        let before = fs::read(&locator).unwrap();
        let progress = std::sync::Mutex::new(None);
        let read = store
            .startup_attempt(None, &|location| {
                *progress.lock().unwrap() = location.library;
            })
            .unwrap();
        assert!(!read.ready);
        assert_eq!(read.library, Some(root.clone()));
        assert_eq!(*progress.lock().unwrap(), Some(root.clone()));
        assert!(!root.exists());
        assert_eq!(fs::read(&locator).unwrap(), before);
        fs::create_dir(&root).unwrap();
        let read = store.startup_attempt(None, &|_| {}).unwrap();
        assert!(read.ready);
        assert_eq!(read.library, Some(root.clone()));
        assert!(!root.join("yaml").exists());
        fs::create_dir(root.join("yaml")).unwrap();
        let bytes = b"invalid: [";
        fs::write(root.join(SETTINGS), bytes).unwrap();
        let read = store.startup_attempt(None, &|_| {}).unwrap();
        assert!(read.ready);
        assert!(read.warning.is_some());
        assert_eq!(fs::read(root.join(SETTINGS)).unwrap(), bytes);
        assert_eq!(fs::read(locator).unwrap(), before);
    }
}
