use crate::import::{self, Event};
use eframe::egui;
use std::path::PathBuf;
#[derive(Default)]
pub struct ImportPanel {
    cleanup: crate::import_cleanup_ui::SourceCleanupDialog,
    pub visible: bool,
    source: Option<PathBuf>,
    target: Option<PathBuf>,
    session: photosuite_application::import_session::ImportSession,
    detailed_log: bool,
    tags: std::collections::BTreeSet<String>,
    new_tag: String,
    photo_layout: import::PhotoImportLayout,
    start_error: Option<String>,
}
impl ImportPanel {
    pub fn running(&self) -> bool {
        self.session.running() || self.cleanup.busy()
    }
    pub fn open(&mut self, library: Option<PathBuf>) {
        self.visible = true;
        if !self.session.running() {
            self.target = library;
        }
    }
    /// Both columns reserve exactly half of the available content width.
    fn source_and_target(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let top = ui.cursor().min;
        let width = ui.available_width();
        let gap = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = 12.;
        ui.columns(2, |columns| {
            crate::theme::content_group(&mut columns[0], |ui| {
                ui.set_min_height(132.);
                crate::theme::section_heading(ui, "Quelle");
                ui.add_space(4.);
                if ui
                    .add_sized(
                        [ui.available_width().min(160.), 24.],
                        egui::Button::new("Quellordner wählen …")
                            .wrap_mode(egui::TextWrapMode::Truncate),
                    )
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder()
                {
                    self.source = Some(path);
                }
                let text = self
                    .source
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Keine Quelle gewählt".into());
                ui.add(egui::Label::new(&text).truncate())
                    .on_hover_text(text);
                ui.small("Quelldateien bleiben unverändert.");
            });
            crate::theme::content_group(&mut columns[1], |ui| {
                ui.set_min_height(132.);
                crate::theme::section_heading(ui, "Ziel");
                ui.add_space(4.);
                let text = self
                    .target
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Keine Bibliothek geöffnet".into());
                ui.add(egui::Label::new(&text).truncate())
                    .on_hover_text(text);
                ui.radio_value(
                    &mut self.photo_layout,
                    import::PhotoImportLayout::YearMonth,
                    "Jahr/Monat",
                );
                ui.radio_value(
                    &mut self.photo_layout,
                    import::PhotoImportLayout::Scans,
                    "Jahr/scans",
                );
                ui.small("Videos: Jahr/Monat. Audio: Jahr/audio/diary.");
            });
        });
        ui.spacing_mut().item_spacing.x = gap;
        egui::Rect::from_min_max(
            top,
            egui::pos2(
                top.x + width,
                ui.cursor().top() - ui.spacing().item_spacing.y,
            ),
        )
    }
    fn tags(&mut self, ui: &mut egui::Ui, choices: &[String]) {
        crate::theme::content_group(ui, |ui| {
            section(ui, "Tags", |ui| {
                let all: std::collections::BTreeSet<_> =
                    choices.iter().chain(self.tags.iter()).cloned().collect();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(3., 3.);
                    for tag in all {
                        if crate::theme::tag_chip(ui, &tag, self.tags.contains(&tag)).clicked()
                            && !self.tags.remove(&tag)
                        {
                            self.tags.insert(tag);
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut self.new_tag)
                            .hint_text("Neuer Tag")
                            .desired_width(160.),
                    );
                    if ui.button("Hinzufügen").clicked()
                        || (input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        match crate::domain::tags::normalize_tag_name(&self.new_tag) {
                            Ok(tag) => {
                                self.tags.insert(tag);
                                self.new_tag.clear();
                                self.start_error = None;
                            }
                            Err(error) => self.start_error = Some(error),
                        }
                    }
                });
            });
        });
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        services: &crate::application::Services,
        library: Option<&std::path::Path>,
        tag_choices: Vec<String>,
    ) -> Option<PathBuf> {
        self.cleanup.show(ctx, services.import_cleanup.clone());
        self.session.poll();
        let refresh = self.session.take_refresh();
        if self.session.running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        } else {
            self.target = library.map(std::path::Path::to_path_buf);
        }
        let mut visible = self.visible;
        egui::Window::new("Importieren · Schritt für Schritt").open(&mut visible).default_size([700.,600.])
            .max_width((ctx.content_rect().width()-32.).max(260.)).max_height((ctx.content_rect().height()-48.).max(240.)).vscroll(true)
            .show(ctx,|ui|{
                crate::options_ui::spacing(ui);
                if self.target.is_none(){ui.label("Zum Importieren zuerst eine Bibliothek öffnen.");}
                ui.add_enabled_ui(!self.session.running() && !self.cleanup.busy(),|ui|{
                    self.source_and_target(ui);
                    ui.add_space(8.);
                    self.tags(ui,&tag_choices);
                    ui.horizontal_wrapped(|ui|{
                        let ready=self.source.is_some() && self.target.is_some();
                        for (title,preview) in [("1. Prüflauf",true),("2. Import starten",false)]{
                            if ui.add_enabled(ready,egui::Button::new(title)).clicked(){
                                self.start_error=self.session.start_configured(services.importer.clone(),self.source.clone().unwrap(),self.target.clone().unwrap(),import::ImportOptions {
                                    preview,photo_layout:self.photo_layout,
                                    tagging:(!self.tags.is_empty()).then(||import::TaggingOptions{repository:services.organization.clone(),tags:self.tags.iter().cloned().collect()}),
                                }).err();
                            }
                        }
                    });
                });
                if ui.add_enabled(self.session.can_cleanup() && !self.cleanup.busy(),egui::Button::new("Importquellen in den Papierkorb …")).clicked(){
                    match self.session.cleanup_request(){Ok(request)=>self.cleanup.open(request,services.import_cleanup.clone()),Err(error)=>self.start_error=Some(error)}
                }
                ui.small("Prüflauf ohne Kopieren. Identische Fotos und vorhandene Videonamen werden übersprungen; unterschiedliche Fotos erhalten einen Nummernzusatz.");
                section(ui,"Fortschritt",|ui|{
                    if self.session.running() && ui.button("Abbrechen").clicked(){self.session.cancel();}
                    let (current,total)=self.session.progress();
                    if total>0{ui.add(egui::ProgressBar::new(current as f32/total as f32).text(format!("Datei {current} / {total}")));}
                    if !self.session.status().is_empty(){let level=import::log_levels(self.session.log()).last().copied().unwrap_or(import::LogLevel::Neutral);ui.colored_label(crate::theme::import_log_color(level),self.session.status());}
                    if let Some(error)=&self.start_error{ui.colored_label(crate::theme::IMPORT_ERROR,error);}
                    ui.horizontal_wrapped(|ui|{
                        ui.colored_label(crate::theme::IMPORT_SUCCESS,"Erfolg");ui.colored_label(crate::theme::IMPORT_WARNING,"Fallback / Hinweis");ui.colored_label(crate::theme::IMPORT_ERROR,"Fehler");
                    });
                    ui.horizontal_wrapped(|ui|{
                        ui.checkbox(&mut self.detailed_log,"Alle Schritte anzeigen");
                        if ui.button("Protokoll kopieren").clicked(){ctx.copy_text(export_log(self.session.log(),self.detailed_log,self.target.as_deref()));}
                    });
                    if self.detailed_log{show_import_log(ui,self.session.log());}else{show_compact_log(ui,self.session.log(),self.target.as_deref(),self.session.running());}
                });
            });
        self.visible = visible;
        refresh
    }
}
fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    crate::theme::section_heading(ui, title);
    ui.add_space(4.);
    body(ui);
}

fn show_import_log(ui: &mut egui::Ui, events: &[Event]) {
    egui::ScrollArea::vertical()
        .id_salt("import_log")
        .stick_to_bottom(true)
        .max_height(220.)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                for (event, level) in events
                    .iter()
                    .zip(import::log_levels(events))
                    .filter(|(event, _)| !event.message.is_empty())
                {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!(
                                "[{}/{}] {}",
                                event.file, event.total, event.message
                            ))
                            .color(crate::theme::import_log_color(level)),
                        )
                        .wrap(),
                    );
                }
            });
        });
}

fn decision_text(decision: &import::FileDecision) -> &str {
    match decision {
        import::FileDecision::Planned => "Geplant",
        import::FileDecision::Copied => "Kopiert",
        import::FileDecision::SkippedIdentical => "Identisch · übersprungen",
        import::FileDecision::SkippedVideoName => "Videoname vorhanden · übersprungen",
        import::FileDecision::Failed(_) => "Fehler",
        import::FileDecision::Cancelled => "Abgebrochen",
    }
}
fn compact_line(event: &Event, root: Option<&std::path::Path>) -> Option<String> {
    let report = event.file_report.as_ref()?;
    let source = report
        .source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let destination = report.target.as_deref().map(|path| {
        root.and_then(|root| path.strip_prefix(root).ok())
            .unwrap_or(path)
            .display()
            .to_string()
    });
    let mut line = format!(
        "[{}/{}] {} · {source}",
        event.file,
        event.total,
        decision_text(&report.decision)
    );
    if let Some(target) = destination {
        line.push_str(&format!(" → {target}"));
    }
    if let Some(time) = &report.captured
        && time.is_fallback()
    {
        line.push_str(&format!(" · {}", time.origin()));
    }
    if report.collision_renamed {
        line.push_str(" · Kollisionszusatz");
    }
    if !report.tags.is_empty() {
        line.push_str(&format!(" · Tags: {}", report.tags.join(", ")));
    }
    if let Some(error) = &report.tag_error {
        line.push_str(&format!(" · Tagfehler: {error}"));
    }
    if let import::FileDecision::Failed(error) = &report.decision {
        line.push_str(&format!(" · {error}"));
    }
    Some(line.replace(['\n', '\r', '\t'], " "))
}
fn report_details(report: &import::FileReport) -> String {
    let mut details = format!("Quelle: {}", report.source.display());
    if let Some(time) = &report.captured {
        details.push_str(&format!(
            "\nDatum: {}\nDatumsquelle: {}",
            time.stamp(),
            time.origin()
        ));
    }
    if let Some(target) = &report.target {
        details.push_str(&format!("\nZiel: {}", target.display()));
    }
    details.push_str(&format!(
        "\nEntscheidung: {}",
        decision_text(&report.decision)
    ));
    if let import::FileDecision::Failed(error) = &report.decision {
        details.push_str(&format!("\n{error}"));
    }
    details
}
fn export_log(events: &[Event], detailed: bool, root: Option<&std::path::Path>) -> String {
    events
        .iter()
        .filter_map(|event| {
            if detailed {
                (!event.message.is_empty())
                    .then(|| format!("[{}/{}] {}", event.file, event.total, event.message))
            } else {
                compact_line(event, root)
                    .or_else(|| event.is_terminal().then(|| event.message.clone()))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn show_compact_log(
    ui: &mut egui::Ui,
    events: &[Event],
    root: Option<&std::path::Path>,
    running: bool,
) {
    let reports: Vec<_> = events
        .iter()
        .filter(|event| event.file_report.is_some())
        .collect();
    if running
        && let Some(current) = events.last().filter(|event| {
            !event.is_terminal() && event.file_report.is_none() && !event.message.is_empty()
        })
    {
        ui.add(
            egui::Label::new(if current.file == 0 {
                current.message.clone()
            } else {
                format!("Datei {} / {} wird geprüft …", current.file, current.total)
            })
            .truncate(),
        )
        .on_hover_text(&current.message);
    }
    egui::ScrollArea::vertical()
        .id_salt("import_compact_log")
        .stick_to_bottom(true)
        .max_height(220.)
        .show_rows(ui, 22., reports.len(), |ui, rows| {
            for index in rows {
                let event = reports[index];
                let report = event.file_report.as_ref().unwrap();
                let color = crate::theme::import_log_color(report.level());
                ui.add_sized(
                    [ui.available_width(), 22.],
                    egui::Label::new(
                        egui::RichText::new(compact_line(event, root).unwrap()).color(color),
                    )
                    .truncate(),
                )
                .on_hover_text(report_details(report));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_log_has_one_entry_per_file_and_retains_diagnostics_on_request() {
        let report = import::FileReport {
            source: "/Quelle/2023-12-27_22-51-32.wav".into(),
            target: Some("/Bibliothek/2023/audio/aud_20231227_225132.wav".into()),
            captured: Some(
                crate::domain::CaptureTime::parse(
                    "2023:12:27 22:51:32",
                    "FALLBACK Audio-Dateiname",
                )
                .unwrap(),
            ),
            decision: import::FileDecision::Planned,
            tags: Vec::new(),
            tag_error: None,
            collision_renamed: false,
        };
        let events = vec![
            Event {
                file: 1,
                total: 1,
                message: "Metadaten lesen …".into(),
                outcome: None,
                file_report: None,
            },
            Event {
                file: 1,
                total: 1,
                message: "Dateiinhalt vergleichen …".into(),
                outcome: None,
                file_report: None,
            },
            Event {
                file: 1,
                total: 1,
                message: String::new(),
                outcome: None,
                file_report: Some(report.clone()),
            },
        ];
        let compact = export_log(&events, false, Some(std::path::Path::new("/Bibliothek")));
        assert_eq!(compact.lines().count(), 1);
        assert!(compact.contains("Geplant"));
        assert!(compact.contains("2023/audio/aud_20231227_225132.wav"));
        assert!(!compact.contains("/Bibliothek/"));
        assert!(!compact.contains("Metadaten lesen"));
        let detailed = export_log(&events, true, None);
        assert!(detailed.contains("Metadaten lesen"));
        assert!(detailed.contains("Dateiinhalt vergleichen"));
        assert!(report_details(&report).contains("FALLBACK Audio-Dateiname"));
        for width in [280., 700.] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 400.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        show_compact_log(
                            ui,
                            &events,
                            Some(std::path::Path::new("/Bibliothek")),
                            false,
                        )
                    });
                },
            );
            let text: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::Text(t) = &s.shape {
                        Some(t)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(text.len(), 1);
            assert_eq!(text[0].galley.rows.len(), 1);
            assert!(text[0].visual_bounding_rect().right() <= width + 1.);
        }
    }
    #[test]
    fn import_results_preserve_newlines_and_wrap_long_paths() {
        for width in [280., 700.] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let events = vec![
                Event {
                    file: 1,
                    total: 2,
                    message: "Quelle: /Fotos/Urlaub\nDatum: 20260801_120000".into(),
                    outcome: None,
                    file_report: None,
                },
                Event {
                    file: 2,
                    total: 2,
                    message: format!(
                        "Ziel: /Bibliothek/{}/img_20260801_120000.jpg",
                        "Langer Verzeichnisname/".repeat(5)
                    ),
                    outcome: None,
                    file_report: None,
                },
            ];
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 500.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| show_import_log(ui, &events));
                },
            );
            let text: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text),
                    _ => None,
                })
                .collect();
            assert_eq!(text.len(), 2);
            assert!(text[0].galley.rows.len() >= 2);
            assert!(text[1].galley.rows.len() >= 2);
            assert!(text[1].pos.y >= text[0].visual_bounding_rect().bottom());
            for text in text {
                assert!(text.visual_bounding_rect().right() <= width + 1.);
            }
        }
    }
}

#[cfg(test)]
mod log_color_tests {
    use super::*;
    #[test]
    fn compact_and_detailed_logs_use_the_same_readable_semantic_colors() {
        for (source, decision, expected) in [
            (
                crate::domain::DateSource::Metadata,
                import::FileDecision::Copied,
                crate::theme::IMPORT_SUCCESS,
            ),
            (
                crate::domain::DateSource::FileSystem,
                import::FileDecision::Copied,
                crate::theme::IMPORT_WARNING,
            ),
            (
                crate::domain::DateSource::Metadata,
                import::FileDecision::Failed("error".into()),
                crate::theme::IMPORT_ERROR,
            ),
        ] {
            let events = vec![Event {
                file: 1,
                total: 1,
                message: "Ergebnis".into(),
                outcome: None,
                file_report: Some(import::FileReport {
                    source: "photo.jpg".into(),
                    target: None,
                    captured: Some(
                        crate::domain::CaptureTime::parse(
                            "2026:11:14 12:00:00",
                            "FALLBACK Dateisystemdatum",
                        )
                        .unwrap()
                        .with_source(source),
                    ),
                    decision,
                    tags: vec![],
                    tag_error: None,
                    collision_renamed: false,
                }),
            }];
            if source == crate::domain::DateSource::FileSystem {
                assert!(compact_line(&events[0], None).unwrap().contains("FALLBACK"));
            }
            for detailed in [false, true] {
                let ctx = egui::Context::default();
                crate::theme::install(&ctx);
                let output = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        if detailed {
                            show_import_log(ui, &events);
                        } else {
                            show_compact_log(ui, &events, None, false);
                        }
                    });
                });
                assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.job.sections.iter().any(|section|section.format.color==expected))));
            }
        }
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn source_target_cards_are_balanced_and_share_heading_typography() {
        for width in [280., 360., 800.] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let mut panel = ImportPanel {
                source: Some("/Sehr/langer/Pfad/zur/Quelle".into()),
                target: Some("/Sehr/langer/Pfad/zur/Bibliothek".into()),
                ..Default::default()
            };
            let mut region = egui::Rect::NOTHING;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        crate::options_ui::spacing(ui);
                        region = panel.source_and_target(ui);
                    });
                },
            );
            let cards: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(card)
                        if card.fill == crate::theme::SURFACE
                            && card.stroke.color == crate::theme::BORDER =>
                    {
                        Some(card.rect)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(cards.len(), 2);
            assert!((cards[0].width() - cards[1].width()).abs() < 0.1);
            assert!(cards[0].right() < region.center().x && cards[1].left() > region.center().x);
            assert!(!output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::LineSegment{points,..} if (points[0].x-region.center().x).abs()<0.1 && points[0].x==points[1].x)));
            for heading in ["Quelle", "Ziel"] {
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == heading => Some(text),
                        _ => None,
                    })
                    .unwrap();
                assert!(text.galley.job.sections.iter().all(
                    |section| section.format.font_id.size == crate::theme::SECTION_HEADING_SIZE
                ));
                if heading == "Quelle" {
                    assert!(text.visual_bounding_rect().right() < region.center().x);
                } else {
                    assert!(text.visual_bounding_rect().left() > region.center().x);
                }
            }
        }
    }
    #[test]
    fn compact_import_tag_chips_are_not_duplicated_and_keep_selection_visible() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut panel = ImportPanel::default();
        panel.tags.insert("scan".into());
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default()
                .show(ctx, |ui| panel.tags(ui, &["scan".into(), "dia".into()]));
        });
        let tags: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if ["scan", "dia"].contains(&text.galley.job.text.as_str()) =>
                {
                    Some(text)
                }
                _ => None,
            })
            .collect();
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().all(|text| {
            text.galley
                .job
                .sections
                .iter()
                .all(|section| section.format.font_id.size == crate::theme::BADGE_FONT_SIZE)
        }));
        assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Rect(rect) if rect.fill==crate::theme::SELECTED && rect.rect.height()<=22.)));
    }
}
