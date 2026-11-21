//! Startup feedback only; readiness, retries and stale-response handling live in application.
use eframe::egui;
use photosuite_application::preferences::{StartupPreferences, StartupState};
impl super::Suite {
    pub(super) fn poll_startup(&mut self, ctx: &egui::Context) {
        let Some(startup) = self.startup.as_mut() else {
            return;
        };
        let Some(outcome) = startup.poll() else {
            if let Some(local) = startup.location_preferences() {
                if self.root.is_none() {
                    self.preferences.last_library = local.last_library.clone();
                    self.preferences.recent_libraries = local.recent_libraries.clone();
                } else {
                    self.preferences.merge_library_history(local);
                }
            }
            if startup.waiting() || startup.busy() {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            if self.root.is_some() && !startup.waiting() && !startup.busy() {
                self.startup = None;
            }
            return;
        };
        if outcome.state != StartupState::Ready {
            if self.root.is_none() {
                if let Ok((preferences, _)) = outcome.result {
                    self.preferences.last_library = preferences.last_library;
                    self.preferences.recent_libraries = preferences.recent_libraries;
                }
                self.preferences.library_scope = None;
                self.status = if outcome.state == StartupState::Unavailable {
                    "Bibliothek aktuell nicht erreichbar."
                } else {
                    "Bibliothek nicht geöffnet."
                }
                .into();
            }
            return;
        }
        self.startup = None;
        if self.root.is_none() {
            let (mut preferences, mut error) = match outcome.result {
                Ok((p, warning)) => (p, warning.unwrap_or_default()),
                Err(error) => (
                    Default::default(),
                    format!("Einstellungen konnten nicht geladen werden: {error}"),
                ),
            };
            let mut bindings = crate::command_bindings::CommandBindings::default();
            if let Err(binding_error) = bindings.restore(&preferences.shortcuts) {
                preferences.library_scope = None;
                error = binding_error;
            }
            self.bindings = bindings;
            self.apply_preferences(preferences);
            self.preferences_error = error;
        } else if let Ok((preferences, _)) = outcome.result {
            self.preferences.merge_library_history(&preferences);
        }
        let snapshot = self.snapshot();
        if let Some(persistence) = &mut self.persistence {
            persistence.update(snapshot);
        } else {
            self.persistence = Some(photosuite_application::preferences::Persistence::new(
                self.services.preferences.clone(),
                snapshot,
            ));
        }
        if self.root.is_some() {
            self.persist_location();
        } else if let Some(path) = outcome.library {
            self.load_path(path, false);
        } else {
            self.status = "Bereit".into();
        }
    }
    pub(super) fn startup_dialog(&mut self, ctx: &egui::Context) {
        let Some(startup) = self.startup.as_ref().filter(|s| s.waiting()) else {
            return;
        };
        let state = startup.state();
        let slow = startup.slow_access();
        let attempts = startup.attempts();
        let elapsed = startup.elapsed().as_secs();
        let limit = StartupPreferences::LIMIT.as_secs();
        let activity = if attempts == 0 {
            format!("Ein Zugriff läuft noch · {elapsed} von {limit} Sekunden")
        } else {
            format!("Versuch {attempts} · {elapsed} von {limit} Sekunden")
        };
        let retry = startup.retry_in().as_secs_f64().ceil() as u64;
        let target = startup.target().map(|path| path.display().to_string());
        let error = startup.error().to_owned();
        let message = if state == StartupState::RetryWaiting {
            format!("Bibliothek noch nicht erreichbar. Neuer Versuch in {retry} Sekunden.")
        } else if attempts == 0 || slow {
            "Der Datenträger antwortet noch nicht. Er wird möglicherweise aufgeweckt.".into()
        } else {
            "Verbindung zur Bibliothek wird geprüft …".into()
        };
        let mut cancel = false;
        let mut other = false;
        let mut open = true;
        egui::Window::new("Bibliothek öffnen").id(egui::Id::new("startup_readiness")).open(&mut open).collapsible(false).resizable(false)
            .default_width(460.).max_width((ctx.content_rect().width()-32.).max(260.)).show(ctx,|ui|{
                crate::options_ui::spacing(ui);
                ui.horizontal(|ui|{ui.add(egui::Spinner::new().size(18.));ui.add(egui::Label::new(&message).wrap());});
                if let Some(target)=target{ui.add(egui::Label::new(&target).truncate()).on_hover_text(target);}
                ui.add_space(6.);
                ui.label(egui::RichText::new(activity).color(crate::theme::MUTED));
                if !error.is_empty(){egui::CollapsingHeader::new("Details").show(ui,|ui|{ui.add(egui::Label::new(error).wrap());});}
                ui.add_space(10.);
                ui.horizontal_wrapped(|ui|{cancel=ui.button("Abbrechen").clicked();other=ui.button("Andere Bibliothek öffnen …").clicked();});
                ui.small("Abbrechen startet ohne geöffnete Bibliothek. Der gespeicherte Pfad bleibt erhalten.");
            });
        if (cancel || !open)
            && let Some(startup) = self.startup.as_mut()
        {
            startup.cancel();
        }
        if other && let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.load(path);
        }
    }
    pub(super) fn retry_startup(&mut self) {
        if let Some(startup) = self.startup.as_mut() {
            startup.retry();
            self.status = "Verbindung zur Bibliothek wird geprüft …".into();
        }
    }
}
