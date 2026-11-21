//! Library-wide transcription status beside the overview player; no IO in widget callbacks.
use super::{
    Suite,
    processing_ui::{self, page_selector, paged_text},
};
use eframe::egui;
use photosuite_application::transcription::queue::{Indicator, Mode, ModelChoice, Phase};
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Seen {
    root: std::path::PathBuf,
    items: usize,
    comments: usize,
    complete: bool,
}
impl Suite {
    pub(super) fn poll_transcription_queue(&mut self, ctx: &egui::Context) {
        if let Some(root) = &self.root
            && self.comments.is_loaded()
        {
            let observed = Seen {
                root: root.clone(),
                items: self.items.len(),
                comments: self.comments.catalog().entries().len(),
                complete: self.transcription_complete,
            };
            if self.transcription_seen.as_ref() != Some(&observed) {
                if self.transcription_complete
                    && self.transcription_seen.as_ref().is_some_and(|old| {
                        old.complete
                            && (old.items != observed.items || old.comments != observed.comments)
                    })
                {
                    self.audio_library.retry();
                }
                let inventory = match photosuite_application::processing::inventory(
                    root,
                    &self.items,
                    self.comments.catalog(),
                ) {
                    Ok(inventory) => inventory,
                    Err(error) => {
                        self.status = format!("Medienverarbeitung: {error}");
                        photosuite_application::processing::advance_with_audio(
                            &mut self.audio_library,
                            &mut self.geo_sweep,
                            &mut self.transcription_queue,
                            photosuite_application::processing::Readiness {
                                library_open: true,
                                ..Default::default()
                            },
                            self.transcription.busy(),
                        );
                        if self.geo_sweep.busy() || self.transcription_queue.busy() {
                            ctx.request_repaint_after(std::time::Duration::from_millis(80));
                        }
                        return;
                    }
                };

                self.transcription_queue.sync(
                    root,
                    inventory.transcriptions,
                    self.transcription_complete,
                );
                if self.transcription_complete {
                    self.geo_sweep.sync(root, inventory.photos);
                }
                self.transcription_seen = Some(observed);
            }
        }
        if self.audio_library.complete()
            && self.geo_sweep.complete()
            && self.transcription_complete
            && self.transcription_queue.mode() == Mode::Running
            && !self.transcription_queue.busy()
            && !self.geo_sweep.busy()
            && !self.audio_library.busy()
        {
            self.transcription.ensure_loaded();
        }
        let model = self
            .transcription
            .prepared()
            .filter(|_| !self.transcription.busy() && self.transcription.error().is_empty())
            .and_then(|p| {
                p.ready_path.as_ref().map(|path| ModelChoice {
                    path: path.clone(),
                    settings: p.settings.clone(),
                })
            });
        self.transcription_queue.set_model(model);
        photosuite_application::processing::advance_with_audio(
            &mut self.audio_library,
            &mut self.geo_sweep,
            &mut self.transcription_queue,
            photosuite_application::processing::Readiness {
                library_open: self.root.is_some(),
                inventory_complete: self.transcription_complete,
                index_maintenance: self.index_maintenance.busy(),
                comments_ready: self.comments.is_loaded(),
                comments_pending: self.comments.has_unsaved() || self.comments.is_saving(),
                audio_active: self.services.work_priority.audio_active(),
            },
            self.transcription.busy(),
        );
        if self.audio_library.revision() != self.audio_header.cache_revision {
            self.audio_header.cache_revision = self.audio_library.revision();
            self.audio_header.draft = Default::default();
            if self.audio_library.complete()
                && let Some(root) = &self.root
            {
                self.comments.load_cached(root.clone());
                if let Err(error) = self.organization.open_cached(root.clone()) {
                    self.status = error;
                }
                self.organization
                    .observe_media(root, &self.items, self.transcription_complete);
                self.transcription_seen = None;
            }
            self.audio_preparation
                .display_hints(&self.audio_library.snapshot().durations);
        }
        if self.transcription_queue.busy()
            || self.geo_sweep.busy()
            || self.audio_library.busy()
            || (self.root.is_some()
                && self.transcription_queue.mode() == Mode::Running
                && self.transcription_queue.held()
                && self.audio_library.complete()
                && self.geo_sweep.complete())
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
    }
    pub(super) fn transcription_button(&mut self, ui: &mut egui::Ui) {
        let model_busy = self.transcription.busy() && self.transcription_queue.waiting_for_model();
        let references_busy = self.comments.is_loading();
        let references_wait = self.root.is_some() && !self.comments.is_loaded();
        let state = if model_busy
            || references_busy
            || self.geo_sweep.busy()
            || self.audio_library.busy()
        {
            Indicator::Busy
        } else if references_wait
            || !self.transcription.error().is_empty()
            || (self.root.is_some()
                && (!self.geo_sweep.complete() || !self.audio_library.complete()))
        {
            Indicator::Paused
        } else {
            self.transcription_queue.indicator()
        };
        let response = crate::theme::transport_button(
            ui,
            true,
            egui::vec2(crate::theme::TRANSPORT_EDGE, crate::theme::TRANSPORT_EDGE),
            "",
        );
        paint(ui, &response, state);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Transkription öffnen")
        });
        let response = response.on_hover_text(match state {
            Indicator::Busy => "Transkription wird verarbeitet",
            Indicator::Paused => "Transkription pausiert oder wartet",
            Indicator::Idle => "Transkription",
        });
        let width = (ui.ctx().content_rect().width() - 32.).clamp(240., 560.);
        // Pinned information panel: only its parent button changes visibility.
        // Popup's menu memory would otherwise close on action clicks or Escape.
        let open_id = response.id.with("transcription_pinned_open");
        let clicked = response.clicked();
        let open = ui.data_mut(|data| {
            let open = data.get_temp::<bool>(open_id).unwrap_or(false) ^ clicked;
            data.insert_temp(open_id, open);
            open
        });
        egui::Popup::from_response(&response)
            .open(open)
            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
            .width(width)
            .show(|ui| {
                ui.set_width(width - 16.);
                self.transcription_popup(ui, model_busy, references_busy, references_wait);
            });
    }

    fn transcription_popup(
        &mut self,
        ui: &mut egui::Ui,
        model_busy: bool,
        references_busy: bool,
        references_wait: bool,
    ) {
        crate::options_ui::spacing(ui);
        crate::theme::section_heading(ui, "Verarbeitung");
        ui.label(
            egui::RichText::new(if !self.audio_library.complete() {
                self.audio_library
                    .stage()
                    .map_or("Audioübersicht laden", |s| s.label())
            } else {
                "Audio → GPS/Orte → Transkription"
            })
            .small()
            .color(crate::theme::MUTED),
        );
        let (done, total, failed) = self.transcription_queue.counts();
        let status = if self.root.is_none() {
            "Zum Starten eine Bibliothek öffnen"
        } else if self.transcription_queue.mode() == Mode::Paused {
            "Verarbeitung pausiert"
        } else if self.transcription_queue.mode() == Mode::Stopped {
            "Verarbeitung gestoppt"
        } else if !self.transcription_queue.error().is_empty() {
            "Transkriptionswarteschlange benötigt einen Neustart"
        } else if !self.transcription.error().is_empty() {
            "Whisper-Modell konnte nicht geprüft werden"
        } else if !self.audio_library.error().is_empty() {
            "Audioprüfung benötigt Aufmerksamkeit"
        } else if !self.audio_library.loaded() {
            "Gespeicherte Audioübersicht wird geladen"
        } else if !self.audio_library.complete() {
            self.audio_library
                .stage()
                .map_or("Audioübersicht wird vorbereitet", |s| s.label())
        } else if self.geo_sweep.busy() {
            "GPS und Ortszuordnungen werden geprüft"
        } else if !self.transcription_complete && !self.collection.is_loading() {
            "Medienbestand unvollständig – bitte erneut einlesen"
        } else if !self.transcription_complete {
            "Bibliotheksinventar wird geladen"
        } else if references_busy {
            "Kommentarzuordnungen werden geladen"
        } else if references_wait {
            "Wartet auf gültige Kommentarzuordnungen"
        } else if !self.geo_sweep.error().is_empty() {
            "GPS-Prüfung benötigt Aufmerksamkeit"
        } else if self.index_maintenance.busy() {
            "Wartet auf die Datenbankpflege"
        } else if self.services.work_priority.audio_active()
            || self.comments.has_unsaved()
            || self.comments.is_saving()
        {
            "Wartet auf die Audioaufnahme"
        } else if model_busy {
            "Whisper-Modell wird vorbereitet"
        } else if self.transcription_queue.waiting_for_model() {
            "Whisper-Modell noch nicht bereit"
        } else if self.transcription_queue.busy() {
            match self.transcription_queue.progress().phase {
                Phase::Planning => "Mediendateien werden geprüft",
                Phase::LoadingModel => "Audio und Modell werden vorbereitet",
                Phase::Transcribing => "Tonspur wird transkribiert",
                Phase::Saving => "Transkript wird gespeichert",
            }
        } else if failed > 0 {
            "Fehlgeschlagene Aufträge warten auf Wiederholung"
        } else if self.transcription_queue.pending() > 0 {
            "Offene Transkriptionsaufträge werden gestartet"
        } else if total == 0 {
            "Keine Audio- oder Videodateien zur Transkription vorhanden"
        } else {
            "Alle Transkriptionsaufträge sind abgeschlossen"
        };
        ui.add_space(4.);
        let busy = self.audio_library.busy()
            || self.geo_sweep.busy()
            || self.transcription_queue.busy()
            || model_busy
            || references_busy;
        let paused = self.transcription_queue.mode() != Mode::Running
            || (self.transcription_queue.held()
                && !self.geo_sweep.busy()
                && !self.audio_library.busy())
            || references_wait;
        let attention = !self.audio_library.error().is_empty()
            || !self.transcription_queue.error().is_empty()
            || !self.transcription.error().is_empty()
            || !self.geo_sweep.error().is_empty()
            || (failed > 0 && self.transcription_queue.pending() == 0);
        ui.horizontal_wrapped(|ui| {
            crate::theme::status_label(
                ui,
                if self.transcription_queue.mode() == Mode::Paused {
                    "Pausiert"
                } else if self.transcription_queue.mode() == Mode::Stopped {
                    "Gestoppt"
                } else if attention {
                    "Fehler"
                } else if busy && !paused {
                    "Aktiv"
                } else if paused {
                    "Wartet"
                } else {
                    "Bereit"
                },
                if attention {
                    crate::theme::DANGER
                } else if busy && !paused {
                    crate::theme::ACCENT
                } else {
                    crate::theme::MUTED
                },
            );
            ui.label(status);
        });
        ui.add_space(4.);
        let failures = self.transcription_queue.failures();
        let audio_error = self.audio_library.error().to_owned();
        let audio_warnings = self.audio_library.snapshot().warnings.join("; ");
        let mut errors = self
            .geo_sweep
            .progress()
            .errors
            .iter()
            .map(|(p, e)| (p.as_str(), e.as_str()))
            .collect::<Vec<_>>();
        if !audio_error.is_empty() {
            errors.push(("Audioverarbeitung", audio_error.as_str()));
        }
        if !audio_warnings.is_empty() {
            errors.push(("Audiodateien", audio_warnings.as_str()));
        }
        if let Some(warning) = &self.geo_sweep.progress().data_warning {
            errors.push(("Offline-Geodaten", warning.as_str()));
        }
        if !self.geo_sweep.error().is_empty() {
            errors.push(("GPS-Prüfung", self.geo_sweep.error()));
        }
        if references_wait && !self.comments.error().is_empty() {
            errors.push(("Kommentarzuordnungen", self.comments.error()));
        }
        if !self.transcription.error().is_empty() {
            errors.push(("Whisper-Modell", self.transcription.error()));
        }
        if !self.transcription.busy()
            && let Some(model) = self.transcription.prepared()
            && !model.warning.is_empty()
        {
            errors.push(("Whisper-Modell", model.warning.as_str()));
        }
        if !self.transcription_queue.error().is_empty() {
            errors.push(("Verarbeitung", self.transcription_queue.error()));
        }
        errors.extend(failures.iter().map(|(path, error)| (path.as_str(), *error)));
        let has_text = !self.transcription_queue.log().is_empty();
        let section_id = ui.id().with("transcription_section");
        let mut section = ui.data_mut(|d| d.get_temp::<u8>(section_id).unwrap_or(0));
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut section, 0, "Übersicht");
            ui.add_enabled_ui(!errors.is_empty(), |ui| {
                ui.selectable_value(&mut section, 1, format!("Fehler ({})", errors.len()));
            });
            ui.add_enabled_ui(has_text, |ui| {
                ui.selectable_value(&mut section, 2, "Live-Text");
            });
            ui.selectable_value(&mut section, 3, "Details");
        });
        ui.add_space(4.);
        ui.data_mut(|d| d.insert_temp(section_id, section));
        match section {
            1 => {
                crate::theme::compact_group(ui, |ui| {
                    if errors.is_empty() {
                        ui.label("Keine Fehler vorhanden.");
                        return;
                    }
                    let page = page_selector(ui, "transcription_error", errors.len());
                    let (path, error) = errors[page];
                    processing_ui::file(ui, "Betroffene Datei / Bereich", path);
                    ui.add_space(4.);
                    paged_text(
                        ui,
                        egui::Id::new(("transcription_error_text", path, error)),
                        error,
                        true,
                    );
                    if references_wait && ui.button("Kommentar-Einstellungen …").clicked() {
                        self.settings_view.page = crate::options_ui::SettingsPage::Database;
                        self.options_open = true;
                    }
                });
            }
            2 => {
                crate::theme::compact_group(ui, |ui| {
                    ui.strong("Zuletzt erkannte Texte");
                    if has_text {
                        live_log(ui, self.transcription_queue.log());
                    } else {
                        ui.label(
                            "Während der Transkription erscheinen hier die letzten 16 Textfunde.",
                        );
                    }
                });
            }
            3 => {
                crate::theme::compact_group(ui, |ui| {
                    ui.strong("Modell & Sprache").on_hover_text("Reihenfolge: Audiobestand → Dauern → Zuordnungen → Jahresblöcke → GPS/Orte → Transkription");
                    if let Some(model) = self.transcription.prepared() {
                        processing_ui::value(ui, "Modell", model.settings.model().id);
                        let code = model.settings.language();
                        let language = photosuite_application::transcription::catalog::LANGUAGES
                            .iter()
                            .find(|(key, _)| *key == code)
                            .map(|(_, name)| *name)
                            .unwrap_or(code);
                        processing_ui::value(ui, "Sprache", language);
                    } else {
                        ui.small("Noch kein geprüftes Whisper-Modell.");
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Transkription …").clicked() {
                            self.settings_view.page =
                                crate::options_ui::SettingsPage::Transcription;
                            self.options_open = true;
                        }
                        if ui.button("Geodaten …").clicked() {
                            self.settings_view.page = crate::options_ui::SettingsPage::Geodata;
                            self.options_open = true;
                        }
                    });
                    if let Some(receipt) = self.transcription_queue.last() {
                        ui.separator();
                        processing_ui::file(ui, "Zuletzt abgeschlossen", receipt.source.as_str());
                        if ui.button("Ergebnis im Finder anzeigen").clicked()
                            && let Some(root) = &self.root
                        {
                            match receipt
                                .record
                                .metadata_path()
                                .and_then(|p| self.services.opener.reveal(&root.join(p)))
                            {
                                Ok(()) => {}
                                Err(error) => self.status = error,
                            }
                        }
                    }
                    ui.separator();
                    ui.small("Lokal auf diesem Mac · Ergebnisse bleiben erhalten.").on_hover_text("Während der Transkription bleibt der Mac wach; der Bildschirm darf ausgehen. Fertige Ergebnisse bleiben bei Abbruch erhalten.");
                });
            }
            _ => {
                crate::theme::compact_group(ui, |ui| {
                    if !self.audio_library.complete() {
                        let progress = self.audio_library.progress();
                        let label = self
                            .audio_library
                            .stage()
                            .map_or("Audioübersicht laden", |s| s.label());
                        processing_ui::stage(
                            ui,
                            label,
                            progress.checked,
                            progress.total,
                            "Danach: Jahresblöcke → GPS/Orte → Transkription",
                        );
                        if self.audio_library.busy()
                            && let Some(path) = &progress.current
                        {
                            processing_ui::file(ui, "Aktuell", &path.to_string_lossy());
                        }
                    } else {
                        let geo = self.geo_sweep.progress();
                        processing_ui::stage(
                            ui,
                            "5 · GPS & Orte",
                            geo.checked,
                            geo.total,
                            &format!(
                                "{} mit GPS · {} ohne · {} Fehler",
                                geo.available, geo.missing, geo.failed
                            ),
                        );
                        if geo.location_failed > 0 {
                            ui.colored_label(
                                crate::theme::DANGER,
                                format!("Ortssuche: {} Fehler", geo.location_failed),
                            );
                        }
                        if self.geo_sweep.busy()
                            && let Some(photo) = &geo.current
                        {
                            processing_ui::file(ui, "Aktuell", photo.as_str());
                        }
                    }
                    ui.separator();
                    processing_ui::stage(
                        ui,
                        "6 · Transkription",
                        done,
                        total,
                        &format!(
                            "{} offen · {failed} Fehler",
                            total.saturating_sub(done + failed)
                        ),
                    );
                    if self.transcription_queue.busy()
                        && let Some(current) = self.transcription_queue.current()
                    {
                        processing_ui::file(ui, "Aktuell", current.as_str());
                        if self.transcription_queue.progress().phase == Phase::Transcribing {
                            ui.add(
                                egui::ProgressBar::new(
                                    self.transcription_queue.progress().fraction.clamp(0., 1.),
                                )
                                .desired_height(6.),
                            );
                        }
                    }
                });
            }
        }
        ui.add_space(6.);
        ui.horizontal_wrapped(|ui| {
            use photosuite_application::processing::{self, Control};
            let primary = processing::primary_control(
                &self.geo_sweep,
                &self.transcription_queue,
                &self.transcription,
            );
            let label = match primary {
                Control::Pause => "Pause",
                Control::Retry => "Fehler wiederholen",
                _ if self.transcription_queue.mode() == Mode::Stopped => "Starten",
                _ if !self.transcription_queue.error().is_empty()
                    || !self.transcription.error().is_empty() =>
                {
                    "Erneut starten"
                }
                _ => "Fortsetzen",
            };
            if ui
                .add_enabled(
                    self.root.is_some()
                        && (total > done
                            || !self.geo_sweep.complete()
                            || !self.transcription_queue.error().is_empty()
                            || !self.transcription.error().is_empty()),
                    egui::Button::new(label),
                )
                .clicked()
            {
                if matches!(primary, Control::Resume | Control::Retry)
                    && !self.audio_library.error().is_empty()
                {
                    self.audio_library.retry();
                }
                processing::control(
                    primary,
                    &mut self.geo_sweep,
                    &mut self.transcription_queue,
                    &mut self.transcription,
                );
            }
            if ui
                .add_enabled(
                    self.root.is_some()
                        && (total > done
                            || self.transcription_queue.busy()
                            || !self.geo_sweep.complete()),
                    egui::Button::new("Abbrechen"),
                )
                .clicked()
            {
                processing::control(
                    Control::Stop,
                    &mut self.geo_sweep,
                    &mut self.transcription_queue,
                    &mut self.transcription,
                );
            }
            if ui
                .add_enabled(
                    !self.audio_library.error().is_empty()
                        || !self.audio_library.snapshot().warnings.is_empty()
                        || !self.transcription_queue.error().is_empty()
                        || !self.transcription.error().is_empty()
                        || failed > 0
                        || self.geo_sweep.progress().failed > 0
                        || self.geo_sweep.progress().location_failed > 0
                        || self.geo_sweep.progress().data_warning.is_some(),
                    egui::Button::new("Fehler erneut versuchen"),
                )
                .clicked()
            {
                if !self.audio_library.error().is_empty()
                    || !self.audio_library.snapshot().warnings.is_empty()
                {
                    self.audio_library.retry();
                }
                processing::control(
                    Control::Retry,
                    &mut self.geo_sweep,
                    &mut self.transcription_queue,
                    &mut self.transcription,
                );
            }
        });
        if ui
            .add_enabled(
                self.root.is_some(),
                egui::Button::new("Dateien erneut prüfen"),
            )
            .clicked()
        {
            if !self.transcription_complete
                && !self.collection.is_loading()
                && let Some(root) = self.root.clone()
            {
                self.load_library(root, true, true);
                self.transcription.reload();
            } else {
                self.audio_library.retry();
                photosuite_application::processing::control(
                    photosuite_application::processing::Control::Rescan,
                    &mut self.geo_sweep,
                    &mut self.transcription_queue,
                    &mut self.transcription,
                );
            }
        }
        if section == 0
            && self.transcription_queue.waiting_for_model()
            && ui.button("Modell einrichten …").clicked()
        {
            self.settings_view.page = crate::options_ui::SettingsPage::Transcription;
            self.options_open = true;
        }
    }
}

fn live_log(
    ui: &mut egui::Ui,
    log: &std::collections::VecDeque<photosuite_application::transcription::queue::LogEntry>,
) {
    let mut rows = Vec::new();
    let mut source = None;
    for entry in log {
        if source != Some(&entry.source) || entry.text.is_none() {
            rows.push((
                true,
                entry.source.as_str().to_owned(),
                entry.source.as_str().to_owned(),
            ));
            source = Some(&entry.source);
        }
        if let Some(text) = &entry.text {
            rows.push((false, text.replace(['\r', '\n'], " "), text.clone()));
        }
    }
    let line_height = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
    let per_page = ((ui.ctx().content_rect().height() - 290.) / line_height)
        .floor()
        .clamp(1., 8.) as usize;
    let count = rows.len().div_ceil(per_page).max(1);
    // A new finding follows the newest page; within each page FIFO stays top to bottom.
    let tail = log.back().map(|e| (e.source.as_str(), e.text.as_deref()));
    let page = page_selector(ui, ("live_findings", tail), count);
    let end = rows.len().saturating_sub(page * per_page);
    let start = end.saturating_sub(per_page);
    for (heading, text, full) in &rows[start..end] {
        let text = if *heading {
            egui::RichText::new(text.rsplit('/').next().unwrap_or(text)).strong()
        } else {
            egui::RichText::new(text)
        };
        ui.add(egui::Label::new(text).truncate())
            .on_hover_text(full);
    }
}

fn paint(ui: &egui::Ui, response: &egui::Response, state: Indicator) {
    let center = response.rect.center();
    let stroke = egui::Stroke::new(1.5_f32, crate::theme::TEXT);
    let p = ui.painter();
    match state {
        Indicator::Busy => egui::Spinner::new()
            .size(17.)
            .color(crate::theme::TEXT)
            .paint_at(
                ui,
                egui::Rect::from_center_size(center, egui::vec2(17., 17.)),
            ),
        Indicator::Paused => {
            for x in [-3.5, 3.5] {
                p.rect_filled(
                    egui::Rect::from_center_size(center + egui::vec2(x, 0.), egui::vec2(3., 13.)),
                    0.,
                    crate::theme::TEXT,
                );
            }
        }
        Indicator::Idle => {
            p.rect_stroke(
                egui::Rect::from_center_size(center, egui::vec2(14., 18.)),
                1.,
                stroke,
                egui::StrokeKind::Inside,
            );
            for y in [-4., 0., 4.] {
                p.line_segment(
                    [center + egui::vec2(-4., y), center + egui::vec2(4., y)],
                    stroke,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_errors_are_visible_and_restart_rechecks_the_model_without_restarting_finished_gps() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        ctx.style_mut(|s| s.animation_time = 0.);
        let mut suite = super::super::layout_tests::test_suite();
        suite.root = Some("/Fotos".into());
        suite.transcription_complete = true;
        suite.geo_sweep.sync(std::path::Path::new("/Fotos"), vec![]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !suite.geo_sweep.complete() {
            suite.geo_sweep.poll(true);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        suite
            .transcription
            .report_error("Modellprüfung fehlgeschlagen".into());
        let frame = |suite: &mut Suite, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(360., 400.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| suite.transcription_popup(ui, false, false, false));
                },
            )
        };
        fn position(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
            output.shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(t.visual_bounding_rect().center())
                }
                _ => None,
            })
        }
        let output = frame(&mut suite, vec![]);
        assert!(position(&output, "Whisper-Modell konnte nicht geprüft werden").is_some());
        assert!(position(&output, "Fehler (1)").is_some());
        let pos = position(&output, "Erneut starten").unwrap();
        for pressed in [true, false] {
            let _ = frame(
                &mut suite,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert!(suite.transcription.error().is_empty());
        assert!(suite.geo_sweep.complete());
        while suite.transcription.busy() || suite.transcription.prepared().is_none() {
            suite.poll_transcription_queue(&ctx);
            suite.transcription.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(suite.transcription.prepared().is_some());
        assert_eq!(suite.transcription_queue.mode(), Mode::Running);
    }
    #[test]
    fn pinned_popup_only_closes_when_its_parent_button_is_clicked_again() {
        fn frame(
            ctx: &egui::Context,
            suite: &mut Suite,
            events: Vec<egui::Event>,
        ) -> egui::FullOutput {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        suite.transcription_button(ui);
                        ui.add_space(440.);
                        let _ = ui.button("Außerhalb");
                    });
                },
            )
        }
        fn text_position(output: &egui::FullOutput, text: &str) -> Option<egui::Pos2> {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(t) if t.galley.text() == text => {
                    Some(t.pos + t.galley.size() / 2.)
                }
                _ => None,
            })
        }
        fn click(ctx: &egui::Context, suite: &mut Suite, pos: egui::Pos2) -> egui::FullOutput {
            for pressed in [true, false] {
                let _ = frame(
                    ctx,
                    suite,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            frame(ctx, suite, vec![])
        }
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        ctx.style_mut(|s| s.animation_time = 0.);
        let mut suite = super::super::layout_tests::test_suite();
        let output = frame(&ctx, &mut suite, vec![]);
        let parent = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.rect.size() == egui::vec2(14., 18.) => {
                    Some(r.rect.center())
                }
                _ => None,
            })
            .unwrap();
        let title = "Verarbeitung";
        assert!(text_position(&output, title).is_none());
        let output = click(&ctx, &mut suite, parent);
        assert!(text_position(&output, title).is_some());
        let action = text_position(&output, "Details").unwrap();
        let output = click(&ctx, &mut suite, action);
        assert!(
            text_position(&output, title).is_some(),
            "internal action closed popup"
        );
        let outside = text_position(&output, "Außerhalb").unwrap();
        let output = click(&ctx, &mut suite, outside);
        assert!(
            text_position(&output, title).is_some(),
            "outside click closed popup"
        );
        let _ = frame(
            &ctx,
            &mut suite,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        let output = frame(&ctx, &mut suite, vec![]);
        assert!(
            text_position(&output, title).is_some(),
            "Escape closed popup"
        );
        let output = click(&ctx, &mut suite, parent);
        assert!(
            text_position(&output, title).is_none(),
            "parent failed to close popup"
        );
    }

    #[test]
    fn popup_sections_and_controls_fit_with_long_gps_errors_at_all_supported_sizes() {
        use crate::domain::tags::LibraryFile;
        use photosuite_application::media_geo::{
            Fingerprint, Position, Repository, ResolvedPlace, Snapshot,
        };
        use std::{
            path::Path,
            sync::{Arc, atomic::AtomicBool},
        };
        struct Invalid;
        impl Repository for Invalid {
            fn inspect(
                &self,
                _: &Path,
                _: &LibraryFile,
                _: &AtomicBool,
            ) -> Result<Snapshot, String> {
                Ok(Snapshot {
                    fingerprint: Fingerprint {
                        bytes: 1,
                        modified: "now".into(),
                    },
                    position: Position::Invalid(
                        "Datei konnte nicht gelesen werden 日本語\n".repeat(200),
                    ),
                    resolved: None,
                })
            }
            fn save_place(
                &self,
                _: &Path,
                _: &LibraryFile,
                _: &Fingerprint,
                _: &ResolvedPlace,
                _: &AtomicBool,
            ) -> Result<(), String> {
                unreachable!()
            }
        }
        struct BrokenGeodata;
        impl photosuite_application::geodata::Store for BrokenGeodata {
            fn locations(&self) -> photosuite_application::geodata::Locations {
                photosuite_application::geodata::Locations {
                    archive: "archive.zip".into(),
                    database: "places.sqlite".into(),
                    preferences: "preferences.plist".into(),
                }
            }
            fn inspect(
                &self,
                _: &AtomicBool,
                _: &(dyn Fn(photosuite_application::geodata::Progress) + Sync),
            ) -> Result<photosuite_application::geodata::Prepared, String> {
                Err("Die lokalen Ortsdaten konnten nicht geprüft werden 日本語".repeat(100))
            }
            fn prepare(
                &self,
                _: bool,
                _: &AtomicBool,
                _: &(dyn Fn(photosuite_application::geodata::Progress) + Sync),
            ) -> Result<photosuite_application::geodata::Prepared, String> {
                unreachable!()
            }
        }
        for (width, height) in [(360., 400.), (600., 400.), (1280., 720.)] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            ctx.style_mut(|s| s.animation_time = 0.);
            let mut suite = super::super::layout_tests::test_suite();
            suite.root = Some("/Fotos".into());
            suite.transcription_complete = true;
            suite.geo_sweep =
                photosuite_application::geo_sweep::Session::new(Arc::new(Invalid), Arc::default())
                    .with_places(Arc::new(BrokenGeodata), suite.services.places.clone());
            suite.geo_sweep.sync(
                Path::new("/Fotos"),
                vec![LibraryFile::new(format!("2023/01/{}.jpg", "lang".repeat(100))).unwrap()],
            );
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !suite.geo_sweep.complete() {
                suite.geo_sweep.poll(true);
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            for mode in [Mode::Running, Mode::Paused, Mode::Stopped] {
                suite.transcription_queue.set_mode(mode);
                for section in 0..4u8 {
                    for frame in 0..3 {
                        let mut bounds = None;
                        let output = ctx.run(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    egui::vec2(width, height),
                                )),
                                ..Default::default()
                            },
                            |ctx| {
                                let shown = egui::Popup::new(
                                    egui::Id::new("section_popup"),
                                    ctx.clone(),
                                    egui::pos2(width - 20., 40.),
                                    egui::LayerId::background(),
                                )
                                .width((width - 32.).min(560.))
                                .show(|ui| {
                                    ui.set_width((width - 48.).min(544.));
                                    ui.data_mut(|d| {
                                        d.insert_temp(
                                            ui.id().with("transcription_section"),
                                            section,
                                        )
                                    });
                                    suite.transcription_popup(ui, false, false, false);
                                    ui.min_rect()
                                })
                                .unwrap();
                                bounds = Some((shown.response.rect, shown.inner));
                            },
                        );
                        let (outer, inner) = bounds.unwrap();
                        assert!(outer.contains_rect(inner));
                        assert!(
                            outer.width() <= width && outer.height() < height - 40.,
                            "section {section}, {mode:?}: {outer:?}"
                        );
                        if frame >= 2 {
                            assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(t) if t.galley.job.text=="Abbrechen")),"controls must remain reachable: section {section}, {mode:?}");
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn progress_popup_grows_to_its_content_and_fits_small_windows() {
        for (width, height) in [(360., 400.), (600., 400.), (1280., 720.)] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            ctx.style_mut(|s| s.animation_time = 0.);
            let mut suite = super::super::layout_tests::test_suite();
            for _ in 0..4 {
                let mut bounds = None;
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, height),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        let popup_width = (width - 32.).clamp(240., 560.);
                        let shown = egui::Popup::new(
                            egui::Id::new("layout_popup"),
                            ctx.clone(),
                            egui::pos2(width - 20., 40.),
                            egui::LayerId::background(),
                        )
                        .width(popup_width)
                        .show(|ui| {
                            ui.set_width(popup_width - 16.);
                            suite.transcription_popup(ui, false, false, true);
                            ui.min_rect()
                        })
                        .unwrap();
                        bounds = Some((shown.response.rect, shown.inner));
                    },
                );
                let (outer, content) = bounds.unwrap();
                assert!(
                    outer.contains_rect(content),
                    "content clipped: {outer:?} {content:?}"
                );
                assert!(outer.height() < height - 40.);
                assert!(outer.width() <= width);
            }
        }
    }

    #[test]
    fn long_multiline_unicode_details_remain_compact_and_can_be_paged() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let text = "ÄÖÜ 日本語 😀\n".repeat(1000);
        let id = egui::Id::new("long_text");
        let mut next = None;
        for frame in 0..4 {
            let mut events = vec![];
            if frame == 2 || frame == 3 {
                let pos = next.unwrap();
                events.push(egui::Event::PointerMoved(pos));
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: frame == 2,
                    modifiers: Default::default(),
                });
            }
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(360., 400.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::Area::new(egui::Id::new("details"))
                        .fixed_pos(egui::pos2(10., 10.))
                        .show(ctx, |ui| {
                            ui.set_width(300.);
                            paged_text(ui, id, &text, true);
                            assert!(ui.min_rect().height() < 200.);
                        });
                },
            );
            for shape in &output.shapes {
                if let egui::Shape::Text(t) = &shape.shape
                    && t.galley.text() == "Weiter"
                {
                    next = Some(t.pos + t.galley.size() / 2.);
                }
            }
        }
        assert!(next.is_some());
        // The visible page changed through the real button, not a mutated test snapshot.
        assert!(
            ctx.data(|d| d.get_temp::<usize>(egui::Id::new("details").with(id)))
                .is_some_and(|page| page == 1)
        );
    }

    #[test]
    fn document_spinner_and_pause_share_geometry_and_busy_requests_animation() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        for state in [Indicator::Idle, Indicator::Busy, Indicator::Paused] {
            let mut bounds = None;
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let response =
                        crate::theme::transport_button(ui, true, egui::vec2(34., 34.), "");
                    bounds = Some(response.rect);
                    paint(ui, &response, state);
                });
            });
            assert_eq!(bounds.unwrap().size(), egui::vec2(34., 34.));
            let lines = output
                .shapes
                .iter()
                .filter(|s| matches!(s.shape, egui::Shape::LineSegment { .. }))
                .count();
            let bars=output.shapes.iter().filter(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect.size()==egui::vec2(3.,13.))).count();
            match state {
                Indicator::Idle => {
                    assert_eq!(lines, 3);
                    assert_eq!(bars, 0);
                }
                Indicator::Paused => assert_eq!(bars, 2),
                Indicator::Busy => assert!(
                    output
                        .shapes
                        .iter()
                        .any(|s| matches!(&s.shape,egui::Shape::Path(p) if p.points.len()>5))
                ),
            }
        }
    }
    #[test]
    fn incomplete_inventory_and_gps_preflight_hold_transcription() {
        let mut suite = super::super::layout_tests::test_suite();
        suite.root = Some("/Fotos".into());
        let ctx = egui::Context::default();
        suite.poll_transcription_queue(&ctx);
        assert!(suite.transcription_queue.held());
        suite.transcription_complete = true;
        suite.poll_transcription_queue(&ctx);
        assert!(
            suite.transcription_queue.held(),
            "GPS must finish before inference"
        );
    }
    #[test]
    fn overview_places_transcription_to_the_right_of_the_player_without_overflow() {
        for width in [360., 600., 1280.] {
            let mut suite = super::super::layout_tests::test_suite();
            suite.root = Some("/Fotos".into());
            suite.transcription_complete = true;
            suite.geo_sweep.sync(std::path::Path::new("/Fotos"), vec![]);
            let ready_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !suite.geo_sweep.complete() {
                suite.geo_sweep.poll(true);
                assert!(std::time::Instant::now() < ready_deadline);
                std::thread::yield_now();
            }
            suite.comments.load("/Fotos".into());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !suite.comments.is_loaded() {
                suite.comments.poll();
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            ctx.style_mut(|s| s.animation_time = 0.);
            for frame in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 600.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .show(ctx, |ui| suite.page_heading(ui, "November 2026"));
                    },
                );
                if frame == 0 {
                    continue;
                }
                let boxes = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Rect(r) if r.rect.size() == egui::vec2(34., 34.) => {
                            Some(r.rect)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert!(boxes.len() >= 3);
                for rect in &boxes {
                    assert!(rect.right() <= width + 1.);
                    assert!((rect.center().y - boxes[0].center().y).abs() < 0.5);
                }
                let icon = output
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Rect(r) if r.rect.size() == egui::vec2(14., 18.) => {
                            Some(r.rect)
                        }
                        _ => None,
                    })
                    .unwrap();
                assert!(icon.center().x > boxes[1].center().x);
            }
        }
    }
}
