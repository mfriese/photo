//! Framework-free defaults for the existing media action catalog.
use super::Shortcut;
use crate::domain::commands::{ALL, Command};
use std::collections::BTreeMap;

/// Missing overrides inherit defaults; `None` explicitly disables a binding.
/// New actions must declare their default in the exhaustive match below.
pub fn defaults() -> BTreeMap<String, Option<Shortcut>> {
    ALL.into_iter()
        .map(|command| {
            let (key, alt) = match command {
                Command::RotateLeft => ("L", true),
                Command::RotateRight => ("R", true),
                Command::ResetRotation => ("Backspace", true),
                Command::PlayVideo => ("Space", false),
                Command::CorrectDate => ("D", true),
            };
            (
                command.id().into(),
                Some(Shortcut {
                    key: key.into(),
                    alt,
                    ctrl: false,
                    shift: false,
                    command: false,
                    mac_cmd: false,
                }),
            )
        })
        .collect()
}

pub fn resolve(
    overrides: &BTreeMap<String, Option<Shortcut>>,
) -> Result<BTreeMap<String, Option<Shortcut>>, String> {
    let mut bindings = defaults();
    for (id, shortcut) in overrides {
        let target = bindings
            .get_mut(id)
            .ok_or_else(|| format!("Unbekannter Befehl in Tastenkürzeln: {id}"))?;
        *target = shortcut.clone();
    }
    Ok(bindings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overrides_extend_defaults_and_can_disable_actions() {
        let original = defaults();
        let mut changes = BTreeMap::new();
        changes.insert("rotate_left".into(), None);
        let mut video = original["play_video"].clone().unwrap();
        video.key = "P".into();
        changes.insert("play_video".into(), Some(video.clone()));
        let resolved = resolve(&changes).unwrap();
        assert_eq!(resolved["rotate_left"], None);
        assert_eq!(resolved["play_video"], Some(video));
        assert_eq!(resolved["rotate_right"], original["rotate_right"]);
        changes.insert("typo".into(), None);
        assert!(resolve(&changes).is_err());
    }
}
