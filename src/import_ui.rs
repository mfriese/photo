use crate::import::{self, Event, ImportAdapter, Job};
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
#[derive(Default)]
pub struct ImportPanel {
    duplicate_review: crate::duplicates_ui::DuplicateReview,
    pub visible: bool,
    source: Option<PathBuf>,
    target: Option<PathBuf>,
    job: Option<Job>,
    log: Vec<Event>,
    current: usize,
    total: usize,
    summary: String,
    imported: bool,
}
impl ImportPanel {
    pub fn running(&self) -> bool {
        self.job.is_some() || self.duplicate_review.running()
    }
    pub fn open(&mut self, library: Option<PathBuf>) {
        self.visible = true;
        if self.job.is_none() && self.target.is_none() {
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
        if let Some(job) = &self.job {
            let mut finished = false;
            for _ in 0..500 {
                let event = match job.events.try_recv() {
                    Ok(event) => event,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        if !finished {
                            self.summary="Importauftrag unerwartet beendet. Bereits kopierte Dateien bleiben erhalten.".into();
                        }
                        finished = true;
                        break;
                    }
                };
                if event.total > 0 {
                    self.current = event.file;
                    self.total = event.total;
                }
                if event.is_terminal() {
                    self.summary = event.message.clone();
                    finished = true;
                }
                self.log.push(event);
            }
            if finished {
                self.job = None;
                if self.imported && self.log.last().is_some_and(|e| matches!(&e.outcome, Some(import::ImportOutcome::Completed(s)) if s.copied > 0)) {
                    refresh = self.target.clone();
                }
            }
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
                ui.add_enabled_ui(self.job.is_none() && !self.duplicate_review.running(), |ui| {
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
                        ui.small("Jahr/Monat/img_JJJJMMTT_HHMMSS.ext");
                    });
                    ui.horizontal_wrapped(|ui| {
                        let ready = self.source.is_some() && self.target.is_some();
                        for (title, preview) in [("1. Prüflauf", true), ("2. Import starten", false)] {
                            if ui.add_enabled(ready, egui::Button::new(title)).clicked() {
                                self.log.clear(); self.current = 0; self.total = 0; self.summary.clear(); self.imported = !preview;
                                self.job = Some(import::start(adapter.clone(), self.source.clone().unwrap(), self.target.clone().unwrap(), preview));
                            }
                        }
                    });
                });
                if self.job.is_none()
                    && self.duplicate_review.show(ui,self.target.as_ref(),repository,duplicates,opener){refresh=self.target.clone();}
                ui.small("Prüflauf ohne Kopieren. Identische Fotos und vorhandene Videonamen werden übersprungen; unterschiedliche Fotos erhalten einen Nummernzusatz.");
                crate::options_ui::section(ui, "Fortschritt", |ui| {
                    if let Some(job) = &self.job
                        && ui.button("Abbrechen").clicked() { job.cancel.store(true, Ordering::Relaxed); }
                    if self.total > 0 {
                        ui.add(egui::ProgressBar::new(self.current as f32 / self.total as f32).text(format!("Datei {} / {}", self.current, self.total)));
                    }
                    if !self.summary.is_empty() { ui.label(&self.summary); }
                    if ui.button("Protokoll kopieren").clicked() {
                        ctx.copy_text(self.log.iter().map(|e| format!("[{}/{}] {}", e.file, e.total, e.message)).collect::<Vec<_>>().join("\n"));
                    }
                    show_import_log(ui, &self.log);
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
                for event in events {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_results_preserve_newlines_and_wrap_long_paths() {
        for width in [280., 700.] {
            let ctx = egui::Context::default();
            let events = vec![
                Event {
                    file: 1,
                    total: 2,
                    message: "Quelle: /Fotos/Urlaub\nDatum: 20260801_120000".into(),
                    outcome: None,
                },
                Event {
                    file: 2,
                    total: 2,
                    message: format!(
                        "Ziel: /Bibliothek/{}/img_20260801_120000.jpg",
                        "Langer Verzeichnisname/".repeat(5)
                    ),
                    outcome: None,
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
