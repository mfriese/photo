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

/// Context of an existing fixed binding. Platform-owned bindings are documented,
/// not persisted as media overrides or dispatched by the main-window UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Application,
    Text,
    Viewer,
    Dialog,
    VideoPlayer,
    AudioPlayer,
    ImportPath,
}
#[derive(Clone, Debug)]
pub struct FixedBinding {
    pub id: &'static str,
    pub title: &'static str,
    pub scope: Scope,
    pub shortcut: Shortcut,
}
#[derive(Clone, Copy, Default)]
pub struct BindingContext {
    pub focused: bool,
    pub modal: bool,
    pub text_input: bool,
    pub viewer: bool,
    pub dialog: bool,
    pub video_player: bool,
    pub audio_player: bool,
    pub import_path: bool,
}
impl BindingContext {
    pub fn allows(self, scope: Scope) -> bool {
        if !self.focused {
            return false;
        }
        match scope {
            Scope::Application => !self.modal,
            Scope::Text => self.text_input,
            Scope::Viewer => self.viewer && !self.modal && !self.text_input,
            Scope::Dialog => self.dialog,
            Scope::VideoPlayer => self.video_player && !self.text_input,
            Scope::AudioPlayer => self.audio_player && !self.text_input,
            Scope::ImportPath => self.import_path && self.text_input,
        }
    }
}
/// Inventory complements the existing configurable media commands. Identical keys
/// in distinct windows are distinct actions; availability is evaluated per scope.
pub fn fixed_bindings() -> Vec<FixedBinding> {
    use Scope::*;
    [
        (
            "open_library",
            "Bibliothek öffnen",
            Application,
            "O",
            true,
            false,
        ),
        (
            "refresh_library",
            "Bibliothek neu laden",
            Application,
            "R",
            true,
            false,
        ),
        ("import_media", "Importieren", Application, "I", true, true),
        (
            "settings",
            "Einstellungen",
            Application,
            "Comma",
            true,
            false,
        ),
        ("find", "Suchen", Application, "F", true, false),
        ("quit", "Beenden", Application, "Q", true, false),
        ("undo", "Rückgängig", Text, "Z", true, false),
        ("redo", "Wiederholen", Text, "Z", true, true),
        ("cut", "Ausschneiden", Text, "X", true, false),
        ("copy", "Kopieren", Text, "C", true, false),
        ("paste", "Einfügen", Text, "V", true, false),
        (
            "select_all",
            "Alles auswählen (Text oder Raster)",
            Text,
            "A",
            true,
            false,
        ),
        (
            "viewer_fullscreen",
            "Vollbild umschalten",
            Viewer,
            "F",
            false,
            false,
        ),
        (
            "viewer_previous",
            "Vorheriges Medium",
            Viewer,
            "ArrowLeft",
            false,
            false,
        ),
        (
            "viewer_next",
            "Nächstes Medium",
            Viewer,
            "ArrowRight",
            false,
            false,
        ),
        (
            "viewer_close",
            "Vollbild oder Betrachter schließen",
            Viewer,
            "Escape",
            false,
            false,
        ),
        (
            "dialog_accept",
            "Dialog bestätigen",
            Dialog,
            "Enter",
            false,
            false,
        ),
        (
            "dialog_close",
            "Such-/Ortsdialog oder Tastenerfassung schließen",
            Dialog,
            "Escape",
            false,
            false,
        ),
        (
            "import_path_accept",
            "Importpfad übernehmen",
            ImportPath,
            "Enter",
            false,
            false,
        ),
        (
            "video_toggle",
            "Video abspielen/pausieren",
            VideoPlayer,
            "Space",
            false,
            false,
        ),
        (
            "video_fullscreen",
            "Video-Vollbild umschalten",
            VideoPlayer,
            "F",
            false,
            false,
        ),
        (
            "video_close",
            "Video-Vollbild oder Fenster schließen",
            VideoPlayer,
            "Escape",
            false,
            false,
        ),
        (
            "audio_toggle",
            "Audio abspielen/pausieren",
            AudioPlayer,
            "Space",
            false,
            false,
        ),
        (
            "audio_back",
            "Audio fünf Sekunden zurück",
            AudioPlayer,
            "ArrowLeft",
            false,
            false,
        ),
        (
            "audio_forward",
            "Audio fünf Sekunden vor",
            AudioPlayer,
            "ArrowRight",
            false,
            false,
        ),
        (
            "audio_close",
            "Audiofenster schließen",
            AudioPlayer,
            "Escape",
            false,
            false,
        ),
        (
            "system_hide",
            "Anwendung ausblenden",
            Application,
            "H",
            true,
            false,
        ),
        (
            "system_minimize",
            "Fenster minimieren",
            Application,
            "M",
            true,
            false,
        ),
        (
            "system_close",
            "Fenster schließen",
            Application,
            "W",
            true,
            false,
        ),
    ]
    .into_iter()
    .map(|(id, title, scope, key, command, shift)| FixedBinding {
        id,
        title,
        scope,
        shortcut: Shortcut {
            key: key.into(),
            alt: false,
            ctrl: false,
            shift,
            command,
            mac_cmd: false,
        },
    })
    .collect()
}

#[cfg(test)]
mod context_tests {
    use super::*;
    #[test]
    fn viewer_keys_require_focus_and_yield_to_text_and_modals() {
        let mut context = BindingContext {
            focused: true,
            viewer: true,
            ..Default::default()
        };
        assert!(context.allows(Scope::Viewer));
        context.text_input = true;
        assert!(!context.allows(Scope::Viewer));
        assert!(context.allows(Scope::Text));
        context.text_input = false;
        context.modal = true;
        assert!(!context.allows(Scope::Viewer));
        context.focused = false;
        assert!(!context.allows(Scope::Application));
    }
}
