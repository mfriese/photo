//! Settings codec used by the current library YAML adapter.
use photosuite_application::preferences::{Preferences, Shortcut};
use plist::{Dictionary, Value};
pub(crate) fn encode(p: &Preferences) -> Value {
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
pub(crate) fn decode(value: Value) -> Result<Preferences, String> {
    let d = value
        .into_dictionary()
        .ok_or("Preferences müssen eine Key-Value-Liste sein")?;
    if d.get("version").and_then(Value::as_unsigned_integer) != Some(1) {
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
    if let Some(v) = d.get("shortcuts") {
        for (command, value) in v.as_dictionary().ok_or("Ungültige Tastenkürzelliste")? {
            if !photosuite_domain::commands::ALL
                .iter()
                .any(|known| known.id() == command)
            {
                return Err(format!("Unbekannter Befehl in Tastenkürzeln: {command}"));
            }
            if value.as_boolean() == Some(false) {
                p.shortcuts.insert(command.clone(), None);
                continue;
            }
            let b = value.as_dictionary().ok_or("Ungültiges Tastenkürzel")?;
            if b.keys().any(|key| {
                !["key", "alt", "ctrl", "shift", "command", "mac_cmd"].contains(&key.as_str())
            }) {
                return Err("Unbekanntes Feld im Tastenkürzel; Datei bleibt unverändert".into());
            }
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
