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
        ui.strong("Transkription · gesamte Bibliothek");
        let (done, total, failed) = self.transcription_queue.counts();
        ui.label(format!("Fertig: {done} / {total} · Fehler: {failed}"));
        let status = if references_busy {
            "Kommentarzuordnungen werden geladen"
        } else if references_wait {
            "Wartet auf gültige Kommentarzuordnungen"
        } else if model_busy {
            "Modell wird vorbereitet"
        } else if self.transcription_queue.held() {
            "Wartet auf die laufende Audioaufnahme"
        } else if self.transcription_queue.waiting_for_model() {
            "Wartet auf ein geprüftes Whisper-Modell"
        } else if self.transcription_queue.mode() == Mode::Stopped {
            "Gestoppt"
        } else if self.transcription_queue.mode() == Mode::Paused {
            "Pausiert"
        } else if self.transcription_queue.busy() {
            match self.transcription_queue.progress().phase {
                Phase::Planning => "Mediendateien und gespeicherte Aufträge werden geprüft",
                Phase::LoadingModel => "Audio und Whisper-Modell werden vorbereitet",
                Phase::Transcribing => "Tonspur wird transkribiert",
                Phase::Saving => "Transkript wird gespeichert",
            }
        } else {
            "Keine laufende Verarbeitung"
        };
        ui.label(status);

        let failures = self.transcription_queue.failures();
        let mut errors = Vec::new();
        if references_wait && !self.comments.error().is_empty() {
            errors.push(("Kommentarzuordnungen", self.comments.error()));
        }
        if !self.transcription_queue.error().is_empty() {
            errors.push(("Verarbeitung", self.transcription_queue.error()));
        }
        errors.extend(failures.iter().map(|(path, error)| (path.as_str(), *error)));
        let has_text = self
            .transcription_queue
            .last()
            .is_some_and(|r| !r.excerpt.is_empty());
        let section_id = ui.id().with("transcription_section");
        let mut section = ui.data_mut(|d| d.get_temp::<u8>(section_id).unwrap_or(0));
        if (section == 1 && errors.is_empty()) || (section == 2 && !has_text) {
            section = 0;
        }
        if !errors.is_empty() || has_text {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut section, 0, "Übersicht");
                if !errors.is_empty() {
                    ui.selectable_value(&mut section, 1, format!("Fehler ({})", errors.len()));
                }
                if has_text {
                    ui.selectable_value(&mut section, 2, "Textvorschau");
                }
            });
            ui.separator();
        }
        ui.data_mut(|d| d.insert_temp(section_id, section));
        if section == 1 {
            let page = page_selector(ui, "transcription_error", errors.len());
            let (path, error) = errors[page];
            ui.add(egui::Label::new(path).truncate())
                .on_hover_text(path);
            paged_text(
                ui,
                egui::Id::new(("transcription_error_text", path, error)),
                error,
                true,
            );
            if references_wait && ui.button("Kommentar-Einstellungen …").clicked() {
                self.settings_view.page = crate::options_ui::SettingsPage::General;
                self.options_open = true;
            }
            return;
        }
        if section == 2 {
            if let Some(receipt) = self.transcription_queue.last() {
                ui.add(egui::Label::new(receipt.source.as_str()).truncate())
                    .on_hover_text(receipt.source.as_str());
                paged_text(
                    ui,
                    egui::Id::new((
                        "transcription_text",
                        receipt.source.as_str(),
                        &receipt.excerpt,
                    )),
                    &receipt.excerpt,
                    false,
                );
            }
            return;
        }
        // UI selection/formatting only; all queue transitions remain in QueueSession.
        if let Some(current) = self.transcription_queue.current() {
            ui.add(egui::Label::new(current.as_str()).truncate())
                .on_hover_text(current.as_str());
        }
        if self.transcription_queue.busy()
            && self.transcription_queue.progress().phase == Phase::Transcribing
        {
            ui.add(
                egui::ProgressBar::new(self.transcription_queue.progress().fraction.clamp(0., 1.))
                    .show_percentage(),
            );
        }
        ui.horizontal_wrapped(|ui| {
            let running = self.transcription_queue.mode() == Mode::Running;
            let label = if running {
                "Pause"
            } else if self.transcription_queue.mode() == Mode::Stopped {
                "Starten"
            } else {
                "Fortsetzen"
            };
            if ui
                .add_enabled(total > done, egui::Button::new(label))
                .clicked()
            {
                self.transcription_queue.set_mode(if running {
                    Mode::Paused
                } else {
                    Mode::Running
                });
            }
            if ui
                .add_enabled(
                    total > done || self.transcription_queue.busy(),
                    egui::Button::new("Abbrechen"),
                )
                .clicked()
            {
                self.transcription_queue.set_mode(Mode::Stopped);
            }
            if ui
                .add_enabled(failed > 0, egui::Button::new("Fehler erneut versuchen"))
                .clicked()
            {
                self.transcription_queue.retry_failed();
            }
        });
        if ui.button("Dateien erneut prüfen").clicked() {
            self.transcription_queue.rescan();
        }
        if let Some(model) = self.transcription.prepared() {
            ui.small(format!(
                "Modellauswahl: {} · {}",
                model.settings.model().id,
                model.settings.language()
            ));
        }
        if self.transcription_queue.waiting_for_model()
            && ui.button("Modell einrichten …").clicked()
        {
            self.settings_view.page = crate::options_ui::SettingsPage::Transcription;
            self.options_open = true;
        }
        if let Some(receipt) = self.transcription_queue.last() {
            ui.separator();
            ui.label("Zuletzt fertig:");
            ui.add(egui::Label::new(receipt.source.as_str()).truncate())
                .on_hover_text(receipt.source.as_str());
            if ui.button("Ergebnis im Finder anzeigen").clicked()
                && let Some(root) = &self.root
            {
                match receipt
                    .record
                    .metadata_path()
                    .and_then(|p| self.services.opener.reveal(&root.join(p)))
                {
                    Ok(()) => {}
                    Err(e) => self.status = e,
                }
            }
        }
        ui.separator();
        ui.small("Lokal auf diesem Mac. Während der Verarbeitung bleibt der Mac wach; der Bildschirm darf ausgehen. Fertige Texte bleiben bei Abbruch erhalten.");
    }
}

// Pagination bounds even arbitrarily long error lists/text without a scroll area.
// State is transient presentation state; no library or application data is changed.
fn page_selector(ui: &mut egui::Ui, salt: impl std::hash::Hash, count: usize) -> usize {
    let id = ui.id().with(salt);
    let mut page = ui
        .data_mut(|d| d.get_temp::<usize>(id).unwrap_or(0))
        .min(count.saturating_sub(1));
    if count > 1 {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(page > 0, egui::Button::new("Zurück"))
                .clicked()
            {
                page -= 1;
            }
            ui.label(format!("{} / {count}", page + 1));
            if ui
                .add_enabled(page + 1 < count, egui::Button::new("Weiter"))
                .clicked()
            {
                page += 1;
            }
        });
    }
    ui.data_mut(|d| d.insert_temp(id, page));
    page
}

fn paged_text(ui: &mut egui::Ui, id: egui::Id, text: &str, error: bool) {
    // Count line breaks as a full line, avoiding tall pages from many short lines.
    let mut pages = vec![String::new()];
    let mut used = 0;
    for character in text.chars() {
        let cost = if character == '\n' { 40 } else { 1 };
        if used + cost > 200 && used > 0 {
            pages.push(String::new());
            used = 0;
        }
        pages.last_mut().unwrap().push(character);
        used += cost;
    }
    let page = page_selector(ui, id, pages.len());
    let label = egui::RichText::new(&pages[page]);
    ui.add(
        egui::Label::new(if error {
            label.color(crate::theme::DANGER)
        } else {
            label
        })
        .wrap(),
    );
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
        let title = "Transkription · gesamte Bibliothek";
        assert!(text_position(&output, title).is_none());
        let output = click(&ctx, &mut suite, parent);
        assert!(text_position(&output, title).is_some());
        let action = text_position(&output, "Dateien erneut prüfen").unwrap();
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
