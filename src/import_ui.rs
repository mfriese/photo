use crate::import::{self, Event, ImportAdapter};
use eframe::egui;
use std::{path::PathBuf, sync::Arc};
#[derive(Default)]
pub struct ImportPanel {
    duplicate_review: crate::duplicates_ui::DuplicateReview,
    pub visible: bool,
    source: Option<PathBuf>,
    target: Option<PathBuf>,
    session: photosuite_application::import_session::ImportSession,
    detailed_log: bool,
    start_error: Option<String>,
}
impl ImportPanel {
    pub fn running(&self) -> bool {
        self.session.running() || self.duplicate_review.running()
    }
    pub fn open(&mut self, library: Option<PathBuf>) {
        self.visible = true;
        if !self.session.running() && self.target.is_none() {
            self.target = library;
        }
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        adapter: Arc<dyn ImportAdapter>,
        repository: Arc<dyn crate::ports::MediaRepository>,
        duplicates: Arc<dyn photosuite_application::duplicates::DuplicateImages>,
        opener: Arc<dyn crate::ports::MediaOpener>,
    ) -> Option<PathBuf> {
        let mut refresh = None;
        if !self.visible && self.duplicate_review.running() {
            self.duplicate_review.cancel();
            refresh = self.target.clone();
        }
        if self.duplicate_review.poll(duplicates.clone()) {
            refresh = self.target.clone();
        }
        self.session.poll();
        refresh = self.session.take_refresh().or(refresh);
        if self.session.running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        egui::Window::new("Importieren · Schritt für Schritt")
            .open(&mut self.visible)
            .default_size([700., 600.])
            .max_width((ctx.content_rect().width() - 32.).max(260.))
            .max_height((ctx.content_rect().height() - 48.).max(240.))
            .vscroll(true)
            .show(ctx, |ui| {
                crate::options_ui::spacing(ui);
                ui.label("Dateien in die Bibliothek kopieren und nach Aufnahmedatum benennen.");
                ui.add_enabled_ui(!self.session.running() && !self.duplicate_review.running(), |ui| {
                    crate::options_ui::section(ui, "Quelle", |ui| {
                        if ui.button("Quellverzeichnis wählen …").clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder() { self.source = Some(path); }
                        let text = self.source.as_ref().map(|p| p.display().to_string()).unwrap_or("Keine Quelle gewählt".into());
                        ui.add(egui::Label::new(&text).truncate()).on_hover_text(text);
                        ui.small("Quelldateien bleiben unverändert.");
                    });
                    crate::options_ui::section(ui, "Zielbibliothek", |ui| {
                        if ui.button("Zielbibliothek wählen …").clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder() { self.target = Some(path); }
                        let text = self.target.as_ref().map(|p| p.display().to_string()).unwrap_or("Keine Bibliothek gewählt".into());
                        ui.add(egui::Label::new(&text).truncate()).on_hover_text(text);
                        ui.small("Fotos/Video: Jahr/Monat/img_… · Audio: Jahr/diary/aud_…");
                    });
                    ui.horizontal_wrapped(|ui| {
                        let ready = self.source.is_some() && self.target.is_some();
                        for (title, preview) in [("1. Prüflauf", true), ("2. Import starten", false)] {
                            if ui.add_enabled(ready, egui::Button::new(title)).clicked() {
                                self.start_error=self.session.start(adapter.clone(),self.source.clone().unwrap(),self.target.clone().unwrap(),preview).err();
                            }
                        }
                    });
                });
                if !self.session.running()
                    && self.duplicate_review.show(ui,self.target.as_ref(),repository,duplicates,opener){refresh=self.target.clone();}
                ui.small("Prüflauf ohne Kopieren. Identische Fotos und vorhandene Videonamen werden übersprungen; unterschiedliche Fotos erhalten einen Nummernzusatz.");
                crate::options_ui::section(ui, "Fortschritt", |ui| {
                    if self.session.running() && ui.button("Abbrechen").clicked(){self.session.cancel();}
                    let (current,total)=self.session.progress();
                    if total>0{ui.add(egui::ProgressBar::new(current as f32/total as f32).text(format!("Datei {current} / {total}")));}
                    if !self.session.status().is_empty(){ui.label(self.session.status());}
                    if let Some(error)=&self.start_error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut self.detailed_log, "Alle Schritte anzeigen");
                        if ui.button("Protokoll kopieren").clicked() {
                            ctx.copy_text(export_log(self.session.log(), self.detailed_log, self.target.as_deref()));
                        }
                    });
                    if self.detailed_log { show_import_log(ui, self.session.log()); }
                    else { show_compact_log(ui, self.session.log(), self.target.as_deref(), self.session.running()); }
                });
            });
        refresh
    }
}

fn show_import_log(ui: &mut egui::Ui, events: &[Event]) {
    egui::ScrollArea::vertical()
        .id_salt("import_log")
        .stick_to_bottom(true)
        .max_height(220.)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                for event in events.iter().filter(|event| !event.message.is_empty()) {
                    ui.add(
                        egui::Label::new(format!(
                            "[{}/{}] {}",
                            event.file, event.total, event.message
                        ))
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
                let color = if matches!(report.decision, import::FileDecision::Failed(_)) {
                    egui::Color32::LIGHT_RED
                } else {
                    crate::theme::TEXT
                };
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
            target: Some("/Bibliothek/2023/diary/aud_20231227_225132.wav".into()),
            captured: Some(
                crate::domain::CaptureTime::parse(
                    "2023:12:27 22:51:32",
                    "FALLBACK Audio-Dateiname",
                )
                .unwrap(),
            ),
            decision: import::FileDecision::Planned,
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
        assert!(compact.contains("2023/diary/aud_20231227_225132.wav"));
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
