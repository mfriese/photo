//! Library-wide transcription status beside the overview player; no IO in widget callbacks.
use super::Suite;
use eframe::egui;
use photosuite_application::transcription::queue::{
    Candidate, Indicator, Mode, ModelChoice, Phase,
};
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
                let mut candidates = self
                    .items
                    .iter()
                    .filter(|m| m.is_audio() || m.is_video())
                    .filter_map(|media| {
                        let source = self.organization.file(media.path()).ok()?;
                        let record = self
                            .comments
                            .catalog()
                            .by_audio(&source)
                            .map(|c| c.photo().clone())
                            .unwrap_or_else(|| source.clone());
                        Some(Candidate { source, record })
                    })
                    .collect::<Vec<_>>();
                candidates.sort_by(|a, b| a.source.cmp(&b.source));
                candidates.dedup_by(|a, b| a.source == b.source);
                if !candidates.is_empty() {
                    self.transcription.ensure_loaded();
                }
                self.transcription_queue
                    .sync(root, candidates, self.transcription_complete);
                self.transcription_seen = Some(observed);
            }
        }
        let model = self.transcription.prepared().and_then(|p| {
            p.ready_path.as_ref().map(|path| ModelChoice {
                path: path.clone(),
                settings: p.settings.clone(),
            })
        });
        self.transcription_queue.set_model(model);
        self.transcription_queue.set_hold(
            self.comments.has_unsaved()
                || self.comments.is_saving()
                || self.services.work_priority.audio_active(),
        );
        self.transcription_queue.poll();
        if self.transcription_queue.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
    }
    pub(super) fn transcription_button(&mut self, ui: &mut egui::Ui) {
        let model_busy = self.transcription.busy() && self.transcription_queue.waiting_for_model();
        let references_busy = self.comments.is_loading();
        let references_wait = self.root.is_some() && !self.comments.is_loaded();
        let state = if model_busy || references_busy {
            Indicator::Busy
        } else if references_wait {
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
        egui::Popup::from_toggle_button_response(&response).width((ui.ctx().content_rect().width()-40.).clamp(250.,420.)).show(|ui|{
            ui.set_max_width((ui.ctx().content_rect().width()-48.).clamp(240.,410.));
            let max_height=(ui.ctx().content_rect().height()-96.).max(160.);
            egui::ScrollArea::vertical().max_height(max_height).show(ui,|ui| {
            crate::options_ui::spacing(ui);ui.strong("Transkription · gesamte Bibliothek");
            let (done,total,failed)=self.transcription_queue.counts();ui.label(format!("Fertig: {done} / {total} · Fehler: {failed}"));
            let status=if references_busy{"Kommentarzuordnungen werden geladen"}else if references_wait{"Wartet auf gültige Kommentarzuordnungen"}else if model_busy{"Modell wird vorbereitet"}else if self.transcription_queue.held(){"Wartet auf die laufende Audioaufnahme"}else if self.transcription_queue.waiting_for_model(){"Wartet auf ein geprüftes Whisper-Modell"}else if self.transcription_queue.mode()==Mode::Stopped{"Gestoppt"}else if self.transcription_queue.mode()==Mode::Paused{"Pausiert"}else if self.transcription_queue.busy(){match self.transcription_queue.progress().phase {Phase::Planning=>"Mediendateien und gespeicherte Aufträge werden geprüft",Phase::LoadingModel=>"Audio und Whisper-Modell werden vorbereitet",Phase::Transcribing=>"Tonspur wird transkribiert",Phase::Saving=>"Transkript wird gespeichert"}}else{"Keine laufende Verarbeitung"};
            ui.label(status);
            if let Some(current)=self.transcription_queue.current(){ui.add(egui::Label::new(current.as_str()).truncate()).on_hover_text(current.as_str());}
            if self.transcription_queue.busy()&&self.transcription_queue.progress().phase==Phase::Transcribing {ui.add(egui::ProgressBar::new(self.transcription_queue.progress().fraction.clamp(0.,1.)).show_percentage());}
            ui.horizontal_wrapped(|ui|{
                let running=self.transcription_queue.mode()==Mode::Running;
                if ui.add_enabled(total>done,egui::Button::new(if running{"Pause"}else if self.transcription_queue.mode()==Mode::Stopped{"Starten"}else{"Fortsetzen"})).clicked(){self.transcription_queue.set_mode(if running{Mode::Paused}else{Mode::Running});}
                if ui.add_enabled(total>done||self.transcription_queue.busy(),egui::Button::new("Abbrechen")).clicked(){self.transcription_queue.set_mode(Mode::Stopped);}
                if ui.add_enabled(failed>0,egui::Button::new("Fehler erneut versuchen")).clicked(){self.transcription_queue.retry_failed();}
            });
            if ui.button("Dateien erneut prüfen").clicked(){self.transcription_queue.rescan();}
            if references_wait && !self.comments.error().is_empty(){ui.colored_label(crate::theme::DANGER,self.comments.error());}
            if references_wait && ui.button("Kommentar-Einstellungen …").clicked(){self.settings_view.page=crate::options_ui::SettingsPage::General;self.options_open=true;ui.close();}
            if let Some(model)=self.transcription.prepared(){ui.small(format!("Modellauswahl: {} · {}",model.settings.model().id,model.settings.language()));}
            if self.transcription_queue.waiting_for_model()&&ui.button("Modell einrichten …").clicked(){self.settings_view.page=crate::options_ui::SettingsPage::Transcription;self.options_open=true;ui.close();}
            if !self.transcription_queue.error().is_empty(){ui.colored_label(crate::theme::DANGER,self.transcription_queue.error());}
            let failures=self.transcription_queue.failures();if !failures.is_empty(){ui.collapsing("Fehlerdetails",|ui|{egui::ScrollArea::vertical().max_height(140.).show(ui,|ui|{for (path,error) in failures.iter().take(100){ui.label(path.as_str());ui.colored_label(crate::theme::DANGER,*error);}});});}
            if let Some(receipt)=self.transcription_queue.last(){ui.separator();ui.label("Zuletzt fertig:");ui.add(egui::Label::new(receipt.source.as_str()).truncate()).on_hover_text(receipt.source.as_str());if !receipt.excerpt.is_empty(){ui.collapsing("Textvorschau",|ui|{egui::ScrollArea::vertical().max_height(140.).show(ui,|ui|{ui.label(&receipt.excerpt);});});}
                if ui.button("Ergebnis im Finder anzeigen").clicked()&&let Some(root)=&self.root {match receipt.record.metadata_path().and_then(|p|self.services.opener.reveal(&root.join(p))){Ok(())=>{},Err(e)=>self.status=e}}
            }
            ui.small("Lokal auf diesem Mac. Während der Verarbeitung bleibt der Mac wach; der Bildschirm darf ausgehen. Fertige Texte bleiben bei Abbruch erhalten.");
            });
        });
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
    fn overview_places_transcription_to_the_right_of_the_player_without_overflow() {
        for width in [360., 600., 1280.] {
            let mut suite = super::super::layout_tests::test_suite();
            suite.root = Some("/Fotos".into());
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
