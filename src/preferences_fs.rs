//! XML property-list adapter stored in the user's macOS Preferences directory.
use photosuite_application::preferences::{Preferences, PreferencesStore, Shortcut};
use plist::{Dictionary, Value};
use std::{io::Write, path::PathBuf};
pub struct PlistPreferences {
    path: PathBuf,
}
impl PlistPreferences {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn user_path() -> Result<PathBuf, String> {
        let home = std::env::var_os("HOME").ok_or("Benutzerverzeichnis nicht verfügbar")?;
        Ok(PathBuf::from(home).join("Library/Preferences/de.photosuite.PhotoSuite.plist"))
    }
}
fn encode(p: &Preferences) -> Value {
    let mut d = Dictionary::new();
    d.insert("version".into(), Value::Integer(1.into()));
    for (key, value) in [
        ("square_previews", p.square_previews),
        ("show_library", p.show_library),
        ("videos_only", p.videos_only),
        ("write_exif", p.write_exif),
        ("reopen_library", p.reopen_library),
    ] {
        d.insert(key.into(), Value::Boolean(value));
    }
    d.insert(
        "preview_size".into(),
        Value::Integer((p.preview_size as u64).into()),
    );
    if let Some(path) = &p.last_library {
        d.insert(
            "last_library".into(),
            Value::String(path.to_string_lossy().into_owned()),
        );
    }
    let mut bindings = Dictionary::new();
    for (command, shortcut) in &p.shortcuts {
        let value = if let Some(s) = shortcut {
            let mut b = Dictionary::new();
            b.insert("key".into(), Value::String(s.key.clone()));
            for (k, v) in [
                ("alt", s.alt),
                ("ctrl", s.ctrl),
                ("shift", s.shift),
                ("command", s.command),
                ("mac_cmd", s.mac_cmd),
            ] {
                b.insert(k.into(), Value::Boolean(v));
            }
            Value::Dictionary(b)
        } else {
            Value::Boolean(false)
        };
        bindings.insert(command.clone(), value);
    }
    d.insert("shortcuts".into(), Value::Dictionary(bindings));
    Value::Dictionary(d)
}
fn decode(value: Value) -> Result<Preferences, String> {
    let d = value
        .into_dictionary()
        .ok_or("Preferences müssen eine Key-Value-Liste sein")?;
    if let Some(version) = d.get("version")
        && version.as_unsigned_integer() != Some(1)
    {
        return Err("Unbekannte Preferences-Version; Datei bleibt unverändert".into());
    }
    let mut p = Preferences::default();
    for (k, dest) in [
        ("square_previews", &mut p.square_previews),
        ("show_library", &mut p.show_library),
        ("videos_only", &mut p.videos_only),
        ("write_exif", &mut p.write_exif),
        ("reopen_library", &mut p.reopen_library),
    ] {
        if let Some(v) = d.get(k) {
            *dest = v
                .as_boolean()
                .ok_or_else(|| format!("Ungültiger Wert für {k}"))?;
        }
    }
    if let Some(v) = d.get("preview_size") {
        p.preview_size = v
            .as_unsigned_integer()
            .filter(|n| *n <= 4)
            .ok_or("preview_size muss zwischen 0 und 4 liegen")? as usize;
    }
    if let Some(v) = d.get("last_library") {
        p.last_library = Some(PathBuf::from(
            v.as_string().ok_or("Ungültiger Bibliothekspfad")?,
        ));
    }
    if let Some(v) = d.get("shortcuts") {
        for (command, value) in v.as_dictionary().ok_or("Ungültige Tastenkürzelliste")? {
            if value.as_boolean() == Some(false) {
                p.shortcuts.insert(command.clone(), None);
                continue;
            }
            let b = value.as_dictionary().ok_or("Ungültiges Tastenkürzel")?;
            let flag = |k: &str| -> Result<bool, String> {
                b.get(k)
                    .map(|v| {
                        v.as_boolean()
                            .ok_or_else(|| format!("Ungültiger Modifier {k}"))
                    })
                    .unwrap_or(Ok(false))
            };
            p.shortcuts.insert(
                command.clone(),
                Some(Shortcut {
                    key: b
                        .get("key")
                        .and_then(Value::as_string)
                        .ok_or("Taste fehlt")?
                        .into(),
                    alt: flag("alt")?,
                    ctrl: flag("ctrl")?,
                    shift: flag("shift")?,
                    command: flag("command")?,
                    mac_cmd: flag("mac_cmd")?,
                }),
            );
        }
    }
    Ok(p)
}
impl PreferencesStore for PlistPreferences {
    fn load(&self) -> Result<Preferences, String> {
        match std::fs::File::open(&self.path) {
            Ok(file) => decode(Value::from_reader(file).map_err(|e| e.to_string())?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    fn save(&self, p: &Preferences) -> Result<(), String> {
        // Do not silently replace malformed or newer files.
        if self.path.exists() {
            self.load()?;
        }
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
