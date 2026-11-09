use crate::command_labels::CommandLabel;
use crate::commands::{ALL, Command};
use eframe::egui;
pub struct CommandBindings {
    bindings: Vec<(Command, Option<egui::KeyboardShortcut>)>,
    capturing: Option<Command>,
    pub visible: bool,
    error: String,
}
// Native application and text-editing shortcuts must never be assigned to media commands.
fn reserved(shortcut: egui::KeyboardShortcut) -> bool {
    let m = shortcut.modifiers;
    let primary = m.command || m.mac_cmd || m.ctrl;
    primary
        && !m.alt
        && match shortcut.logical_key {
            egui::Key::O
            | egui::Key::R
            | egui::Key::Comma
            | egui::Key::F
            | egui::Key::Q
            | egui::Key::X
            | egui::Key::C
            | egui::Key::V
            | egui::Key::A
            | egui::Key::Z
            | egui::Key::H
            | egui::Key::M
            | egui::Key::W => true,
            egui::Key::I => m.shift,
            _ => false,
        }
}
impl Default for CommandBindings {
    fn default() -> Self {
        Self {
            bindings: ALL
                .into_iter()
                .map(|command| {
                    let (modifiers, key) = match command {
                        Command::RotateLeft => (egui::Modifiers::ALT, egui::Key::L),
                        Command::RotateRight => (egui::Modifiers::ALT, egui::Key::R),
                        Command::ResetRotation => (egui::Modifiers::ALT, egui::Key::Backspace),
                        Command::PlayVideo => (egui::Modifiers::NONE, egui::Key::Space),
                    };
                    (command, Some(egui::KeyboardShortcut::new(modifiers, key)))
                })
                .collect(),
            capturing: None,
            visible: false,
            error: String::new(),
        }
    }
}
impl CommandBindings {
    pub fn export(
        &self,
    ) -> std::collections::BTreeMap<String, Option<photosuite_application::preferences::Shortcut>>
    {
        self.bindings
            .iter()
            .map(|(command, s)| {
                (
                    command.id().into(),
                    s.map(|s| photosuite_application::preferences::Shortcut {
                        key: s.logical_key.name().into(),
                        alt: s.modifiers.alt,
                        ctrl: s.modifiers.ctrl,
                        shift: s.modifiers.shift,
                        command: s.modifiers.command,
                        mac_cmd: s.modifiers.mac_cmd,
                    }),
                )
            })
            .collect()
    }
    pub fn restore(
        &mut self,
        saved: &std::collections::BTreeMap<
            String,
            Option<photosuite_application::preferences::Shortcut>,
        >,
    ) -> Result<(), String> {
        let mut candidate = Self::default().bindings;
        for (command, shortcut) in &mut candidate {
            if let Some(saved) = saved.get(command.id()) {
                *shortcut = match saved {
                    None => None,
                    Some(s) => Some(egui::KeyboardShortcut::new(
                        egui::Modifiers {
                            alt: s.alt,
                            ctrl: s.ctrl,
                            shift: s.shift,
                            command: s.command,
                            mac_cmd: s.mac_cmd,
                        },
                        egui::Key::from_name(&s.key)
                            .ok_or_else(|| format!("Unbekannte Taste: {}", s.key))?,
                    )),
                };
            }
        }
        for (i, (_, s)) in candidate.iter().enumerate() {
            if s.is_some() && candidate[..i].iter().any(|(_, other)| other == s) {
                return Err("Gespeicherte Tastenkürzel sind mehrfach vergeben".into());
            }
        }
        if candidate.iter().any(|(_, s)| s.is_some_and(reserved)) {
            return Err(
                "Gespeichertes Kürzel ist für ein Anwendungs- oder Textmenü reserviert".into(),
            );
        }
        self.bindings = candidate;
        Ok(())
    }
    pub fn shortcuts(&mut self, ctx: &egui::Context, enabled: bool) -> Vec<Command> {
        if self.visible || !enabled || ctx.wants_keyboard_input() {
            return Vec::new();
        }
        self.bindings
            .iter()
            .filter_map(|(c, s)| {
                if *c == Command::PlayVideo && s.is_some_and(|shortcut| ctx.input(|i| i.events.iter().any(|e| matches!(e,egui::Event::Key{key,repeat:true,..} if *key==shortcut.logical_key)))) {return None;}
                s.filter(|s| ctx.input_mut(|i| i.consume_shortcut(s)))
                    .map(|_| *c)
            })
            .collect()
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if let Some(command) = self.capturing {
            let event = ctx.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key,
                        modifiers,
                        pressed: true,
                        repeat: false,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, modifiers)) = event {
                if key == egui::Key::Escape {
                    self.capturing = None;
                } else if modifiers.alt
                    || modifiers.ctrl
                    || modifiers.command
                    || (command == Command::PlayVideo && key == egui::Key::Space)
                {
                    let shortcut = egui::KeyboardShortcut::new(modifiers, key);
                    if reserved(shortcut) {
                        self.error =
                            "Dieses Kürzel ist für ein Anwendungs- oder Textmenü reserviert."
                                .into();
                    } else if self
                        .bindings
                        .iter()
                        .any(|(c, s)| *c != command && *s == Some(shortcut))
                    {
                        self.error = "Dieses Kürzel ist bereits vergeben.".into();
                    } else {
                        self.bindings
                            .iter_mut()
                            .find(|(c, _)| *c == command)
                            .unwrap()
                            .1 = Some(shortcut);
                        self.capturing = None;
                        self.error.clear();
                    }
                } else {
                    self.error =
                        "Bitte Alt, Strg oder Command zusammen mit einer Taste drücken.".into();
                }
            }
        }
        egui::Window::new("Befehle & Tastenkürzel")
            .default_width(500.).max_width((ctx.content_rect().width()-32.).max(260.))
            .max_height((ctx.content_rect().height()-48.).max(200.)).vscroll(true)
            .open(&mut self.visible).show(ctx,|ui| {
                crate::options_ui::spacing(ui);
                ui.label("Tastenkürzel werden automatisch gespeichert.");
                ui.small("Kürzel mit Alt, Strg oder Command; für Wiedergabe auch Leertaste. Escape bricht ab.");
                ui.add_space(8.);
                for (command,shortcut) in &mut self.bindings {
                    egui::Frame::group(ui.style()).inner_margin(8.).show(ui,|ui| {
                        ui.strong(command.title());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(shortcut.map(|s|ctx.format_shortcut(&s)).unwrap_or("Nicht zugewiesen".into()));
                            if ui.button("Zuweisen …").clicked(){self.capturing=Some(*command);}
                            if ui.add_enabled(shortcut.is_some(),egui::Button::new("Entfernen")).clicked(){*shortcut=None;}
                        });
                    });
                }
                if let Some(command)=self.capturing {ui.label(format!("{}: Jetzt Tastenkombination drücken …",command.title()));}
                if !self.error.is_empty(){ui.colored_label(egui::Color32::LIGHT_RED,&self.error);}
            });
        if !self.visible {
            self.capturing = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_roundtrip_including_removed_command() {
        let mut bindings = CommandBindings::default();
        bindings.bindings[0].1 = None;
        let saved = bindings.export();
        let mut restored = CommandBindings::default();
        restored.restore(&saved).unwrap();
        assert_eq!(restored.export(), saved);
    }
    #[test]
    fn duplicate_bindings_are_rejected_without_changes() {
        let mut bindings = CommandBindings::default();
        let original = bindings.export();
        let mut saved = original.clone();
        saved.insert("rotate_left".into(), saved["rotate_right"].clone());
        assert!(bindings.restore(&saved).is_err());
        assert_eq!(bindings.export(), original);
    }
    #[test]
    fn old_preferences_keep_space_for_playback() {
        let mut bindings = CommandBindings::default();
        let mut legacy = bindings.export();
        legacy.remove("play_video");
        bindings.restore(&legacy).unwrap();
        let shortcut = bindings.export()["play_video"].clone().unwrap();
        assert_eq!(shortcut.key, "Space");
        assert!(!shortcut.alt && !shortcut.command && !shortcut.ctrl);
    }
}

#[cfg(test)]
mod menu_shortcut_tests {
    use super::*;
    #[test]
    fn reserved_menu_shortcuts_are_rejected_without_losing_existing_bindings() {
        let mut bindings = CommandBindings::default();
        let before = bindings.export();
        let mut saved = before.clone();
        saved.insert(
            "rotate_left".into(),
            Some(photosuite_application::preferences::Shortcut {
                key: "O".into(),
                alt: false,
                ctrl: false,
                shift: false,
                command: true,
                mac_cmd: false,
            }),
        );
        assert!(bindings.restore(&saved).unwrap_err().contains("reserviert"));
        assert_eq!(bindings.export(), before);
    }
}
