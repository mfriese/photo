//! Menu projection and UI action dispatch. Native menu ownership stays in the injected adapter.
use super::Suite;
use crate::desktop_menu::{Action, Entry};
use eframe::egui;
use photosuite_application::preferences::Shortcut;
fn shortcut(key: &str, shift: bool) -> Option<Shortcut> {
    Some(Shortcut {
        key: key.into(),
        alt: false,
        ctrl: false,
        shift,
        command: true,
        mac_cmd: false,
    })
}
impl Suite {
    fn menu_entries(&self, ctx: &egui::Context) -> Vec<Entry> {
        let blocked = self.comments.is_open()
            || self.options_open
            || self.import_panel.visible
            || self.tag_view.open
            || self.bindings.visible
            || self.search_open;
        let ready = self.settings_ready() && !blocked;
        let editing = ready
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.focused)
            && self.selected.is_some()
            && !self.editor.is_saving();
        let mut entries = vec![
            Entry {
                action: Action::OpenLibrary,
                enabled: !blocked,
                checked: None,
                shortcut: shortcut("O", false),
            },
            Entry {
                action: Action::Refresh,
                enabled: self.root.is_some() && !blocked,
                checked: None,
                shortcut: shortcut("R", false),
            },
            Entry {
                action: Action::Import,
                enabled: !blocked,
                checked: None,
                shortcut: shortcut("I", true),
            },
            Entry {
                action: Action::Settings,
                enabled: !blocked,
                checked: None,
                shortcut: shortcut("Comma", false),
            },
            Entry {
                action: Action::Shortcuts,
                enabled: !blocked,
                checked: None,
                shortcut: None,
            },
            Entry {
                action: Action::Tags,
                enabled: ready && self.organization.catalog().is_some(),
                checked: None,
                shortcut: None,
            },
            Entry {
                action: Action::ToggleLibrary,
                enabled: ready,
                checked: Some(self.show_library),
                shortcut: None,
            },
            Entry {
                action: Action::ToggleSquare,
                enabled: ready,
                checked: Some(self.square_previews),
                shortcut: None,
            },
            Entry {
                action: Action::ToggleVideos,
                enabled: ready,
                checked: Some(self.videos_only),
                shortcut: None,
            },
            Entry {
                action: Action::Find,
                enabled: !blocked,
                checked: None,
                shortcut: shortcut("F", false),
            },
            Entry {
                action: Action::Help,
                enabled: true,
                checked: None,
                shortcut: None,
            },
            Entry {
                action: Action::Quit,
                enabled: true,
                checked: None,
                shortcut: shortcut("Q", false),
            },
        ];
        for (action, key, shift) in [
            (Action::Undo, "Z", false),
            (Action::Redo, "Z", true),
            (Action::Cut, "X", false),
            (Action::Copy, "C", false),
            (Action::Paste, "V", false),
            (Action::SelectAll, "A", false),
        ] {
            entries.push(Entry {
                action,
                enabled: ctx.wants_keyboard_input() && ctx.input(|i| i.focused),
                checked: None,
                shortcut: shortcut(key, shift),
            });
        }
        entries.push(Entry {
            action: Action::AudioWindow,
            enabled: self.services.audio_player.transport().active,
            checked: None,
            shortcut: None,
        });
        entries.push(Entry {
            action: Action::StartViewAudio,
            enabled: ready
                && !self.diary.is_loading()
                && self.audio_header.draft.selected().is_some(),
            checked: None,
            shortcut: None,
        });
        for size in 0..5 {
            entries.push(Entry {
                action: Action::PreviewSize(size),
                enabled: ready,
                checked: Some(self.size == size),
                shortcut: None,
            });
        }
        let bindings = self.bindings.export();
        for command in crate::commands::ALL {
            let enabled = editing
                && self
                    .selected
                    .and_then(|i| self.items.get(i))
                    .is_some_and(|m| {
                        if command == crate::commands::Command::PlayVideo {
                            m.is_video()
                        } else {
                            m.is_photo()
                        }
                    });
            entries.push(Entry {
                action: Action::Edit(command),
                enabled,
                checked: None,
                shortcut: bindings.get(command.id()).cloned().flatten(),
            });
        }
        entries
    }
    pub(super) fn poll_menu(&mut self, ctx: &egui::Context) {
        let entries = self.menu_entries(ctx);
        let Some(menu) = self.native_menu.as_mut() else {
            return;
        };
        if let Err(error) = menu.sync(&entries) {
            self.status = format!("Menü: {error}");
        }
        let actions = menu.drain();
        for action in actions {
            // Revalidate queued activations against current UI state, not just the native widget.
            if self
                .menu_entries(ctx)
                .iter()
                .any(|e| e.action == action && e.enabled)
            {
                self.menu_action(action, ctx);
            }
        }
    }
    fn menu_action(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Cut => ctx.send_viewport_cmd(egui::ViewportCommand::RequestCut),
            Action::Copy => ctx.send_viewport_cmd(egui::ViewportCommand::RequestCopy),
            Action::Paste => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
            Action::Undo | Action::Redo | Action::SelectAll => ctx.input_mut(|i| {
                i.events.push(egui::Event::Key {
                    key: if action == Action::SelectAll {
                        egui::Key::A
                    } else {
                        egui::Key::Z
                    },
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: if action == Action::Redo {
                        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
                    } else {
                        egui::Modifiers::COMMAND
                    },
                })
            }),
            Action::OpenLibrary => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.load(path);
                }
            }
            Action::Refresh => {
                if let Some(root) = self.root.clone() {
                    self.load(root.clone());
                    if self.root.as_ref() == Some(&root) {
                        self.comments.refresh_index();
                    }
                }
            }
            Action::Import => self.import_panel.open(self.root.clone()),
            Action::AudioWindow => {
                if let Err(error) = self
                    .services
                    .audio_player
                    .control(photosuite_application::diary::AudioControl::ShowWindow)
                {
                    self.status = error;
                }
            }
            Action::StartViewAudio => self.start_audio_for_view(),
            Action::Settings => self.options_open = true,
            Action::Shortcuts => self.bindings.visible = true,
            Action::Tags => {
                if let Some(media) = self.selected.and_then(|index| self.items.get(index)) {
                    match self.organization.file(media.path()) {
                        Ok(file) => self.tag_view.edit(file),
                        Err(error) => self.status = error,
                    }
                } else {
                    self.tag_view.file = None;
                    self.tag_view.open = true;
                }
            }
            Action::ToggleLibrary => self.show_library = !self.show_library,
            Action::ToggleSquare => self.square_previews = !self.square_previews,
            Action::ToggleVideos => self.videos_only = !self.videos_only,
            Action::PreviewSize(size) => self.size = size.min(4),
            Action::Edit(command) => self.execute(command, ctx),
            Action::Find => {
                self.search_open = true;
                self.focus_search = true;
            }
            Action::Help => self.help_open = true,
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }
    pub(super) fn search_dialog(&mut self, ctx: &egui::Context) {
        if !self.search_open {
            return;
        }
        let mut clear = false;
        egui::Window::new("Find in Library")
            .open(&mut self.search_open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.)
            .max_width((ctx.content_rect().width() - 32.).max(240.))
            .show(ctx, |ui| {
                ui.label("Dateinamen und Verzeichnispfade durchsuchen");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .id(egui::Id::new("library_search"))
                        .desired_width(ui.available_width())
                        .hint_text("Suchbegriff …"),
                );
                if self.focus_search {
                    response.request_focus();
                    self.focus_search = false;
                }
                clear = ui
                    .add_enabled(
                        !self.query.is_empty(),
                        egui::Button::new("Filter zurücksetzen"),
                    )
                    .clicked();
                ui.small(
                    "Die Fotowand wird direkt gefiltert. Der Filter bleibt beim Schließen aktiv.",
                );
            });
        if clear {
            self.query.clear();
            self.focus_search = true;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.search_open = false;
        }
        if !self.search_open {
            self.focus_search = false;
            ctx.memory_mut(|memory| memory.surrender_focus(egui::Id::new("library_search")));
        }
    }
    pub(super) fn help(&mut self, ctx: &egui::Context) {
        egui::Window::new("PhotoSuite Help").open(&mut self.help_open).default_width(430.).max_width((ctx.content_rect().width()-32.).max(260.)).vscroll(true).show(ctx,|ui|{
            ui.heading("PhotoSuite");
            ui.label("File → Open Library…: Fotosammlung öffnen. Jahre links aufklappen und ein Jahr oder einen Monat auswählen.");
            ui.label("View: Seitenleiste, quadratische Vorschauen, Videofilter und fünf Vorschaugrößen.");
            ui.label("Ein Foto anklicken, um die Großansicht zu öffnen. Pfeiltasten wechseln das Bild, Escape schließt die Ansicht.");
            ui.label("Edit: Drehbefehle, Videowiedergabe, Tags und Alben sowie anpassbare Tastenkürzel. Bildbefehle benötigen eine geöffnete Großansicht.");
            ui.label("Die Audioleiste startet und pausiert mit ▶ / ⏸. Ein Klick auf die Jahresachse springt zur gewünschten Aufnahme.");
            ui.label("PhotoSuite → Settings…: Einstellungen der Bibliothek. Originale bleiben unverändert, außer bei ausdrücklich aktivierten Schreib- oder bestätigten Bereinigungsaktionen.");
        });
    }
}
