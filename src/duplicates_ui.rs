//! One collision family at a time; actions require an explicit user decision.
use eframe::egui;
use photosuite_application::duplicates::{self, Comparison};
use std::{path::PathBuf, sync::Arc};
#[derive(Default)]
pub struct DuplicateReview {
    session: photosuite_application::duplicate_review::ReviewSession,
    keepers: Vec<usize>,
    choice_group: Option<String>,
    error: String,
    changed: bool,
}
impl DuplicateReview {
    pub fn running(&self) -> bool {
        self.session.running()
    }
    pub fn cancel(&mut self) {
        self.session.cancel();
    }
    pub fn poll(&mut self, adapter: Arc<dyn duplicates::DuplicateImages>) -> bool {
        self.session.poll(adapter);
        self.session.take_refresh()
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        library: Option<&PathBuf>,
        repository: Arc<dyn crate::ports::MediaRepository>,
        adapter: Arc<dyn duplicates::DuplicateImages>,
        opener: Arc<dyn crate::ports::MediaOpener>,
    ) -> bool {
        if self.poll(adapter.clone()) {
            self.changed = true;
        }
        if self.session.waiting() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        if let Some(report) = self.session.report()
            && self.choice_group.as_ref() != Some(&report.group.name)
        {
            self.keepers = report.classes().iter().map(|c| c[0]).collect();
            self.choice_group = Some(report.group.name.clone());
        }
        crate::options_ui::section(ui, "Kollisionsduplikate prüfen", |ui| {
            ui.label("Prüft Bildgruppen mit gleichem Zeitstempel und beliebigen Namensanhängen in der Zielbibliothek.");
            if !self.running() {
                if ui
                    .add_enabled(
                        library.is_some(),
                        egui::Button::new("Duplikatsuche starten"),
                    )
                    .clicked()
                {
                    self.choice_group = None;
                    if let Err(error) = self.session.start(library.cloned().unwrap(), repository) {
                        self.error = error;
                    }
                }
            } else if ui.button("Abbrechen").clicked() {
                self.cancel();
            }

            ui.add(egui::Label::new(self.session.status()).wrap());
            let mut skip = false;
            let mut remove = false;
            if let Some(report) = self.session.report() {
                ui.strong(format!(
                    "Gruppe {} / {}",
                    self.session.position().0,
                    self.session.position().1
                ));
                ui.add(egui::Label::new(&report.group.name).wrap());
                ui.small("Vergleich in voller Auflösung mit EXIF-Ausrichtung und Farbprofilprüfung. Metadaten können trotz gleicher Pixel abweichen. Die gewählte Datei mitsamt ihren Metadaten bleibt erhalten.");
                let classes = report.classes();
                egui::ScrollArea::vertical()
                    .id_salt("duplicate_variants")
                    .max_height(300.)
                    .show(ui, |ui| {
                        for (class_index, class) in classes.iter().enumerate() {
                            ui.strong(if class.len() > 1 {
                                format!("Pixelidentische Teilgruppe · {} Dateien", class.len())
                            } else {
                                "Eigenständiges oder nicht sicher vergleichbares Bild".into()
                            });
                            for &file in class {
                                let path = &report.group.files[file];
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::Image::from_uri(format!(
                                            "oriented://0/{}",
                                            path.display()
                                        ))
                                        .max_size(egui::vec2(84., 64.)),
                                    );
                                    ui.vertical(|ui| {
                                        if class.len() > 1 {
                                            ui.radio_value(
                                                &mut self.keepers[class_index],
                                                file,
                                                "Diese Datei behalten",
                                            );
                                        }
                                        ui.add(
                                            egui::Label::new(
                                                path.file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy(),
                                            )
                                            .truncate(),
                                        )
                                        .on_hover_text(path.display().to_string());
                                        if ui.small_button("Öffnen").clicked()
                                            && let Err(e) = opener.open(path)
                                        {
                                            self.error = e;
                                        }
                                    });
                                });
                            }
                            ui.separator();
                        }
                    });
                egui::CollapsingHeader::new(format!(
                    "Alle {} Paarvergleiche anzeigen",
                    report.pairs.len()
                ))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("duplicate_comparisons")
                        .max_height(180.)
                        .show_rows(ui, 22., report.pairs.len(), |ui, rows| {
                            for i in rows {
                                let pair = &report.pairs[i];
                                let text = match &pair.result {
                                    Comparison::Identical => "Exakt gleiche Bildpixel".into(),
                                    Comparison::Different(reason) => {
                                        format!("Unterschiedlich: {reason}")
                                    }
                                    Comparison::Uncertain(reason) => {
                                        format!("Nicht sicher: {reason}")
                                    }
                                };
                                let line = format!(
                                    "{} ↔ {}: {text}",
                                    report.group.files[pair.a].display(),
                                    report.group.files[pair.b].display()
                                );
                                ui.add(egui::Label::new(&line).truncate())
                                    .on_hover_text(line);
                            }
                        });
                });
                let count = report
                    .removal_plan(&self.keepers)
                    .map(|p| p.len())
                    .unwrap_or(0);
                ui.horizontal_wrapped(|ui| {
                    skip = ui.button("Alle behalten und weiter").clicked();
                    remove = ui
                        .add_enabled(
                            count > 0,
                            egui::Button::new(format!("{count} Duplikate in Papierkorb & weiter")),
                        )
                        .clicked();
                });
            }
            if skip && let Err(error) = self.session.keep_all(adapter.clone()) {
                self.error = error;
            }
            if remove
                && let Err(error) = self.session.remove_selected(adapter, self.keepers.clone())
            {
                self.error = error;
            }
            if !self.error.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &self.error);
            }
            if !self.session.log().is_empty() {
                egui::CollapsingHeader::new("Entscheidungsprotokoll").show(ui, |ui| {
                    if ui.button("Protokoll kopieren").clicked() {
                        ui.ctx().copy_text(self.session.log().join("\n"));
                    }
                    for line in self.session.log() {
                        ui.add(egui::Label::new(line).wrap());
                    }
                });
            }
        });
        std::mem::take(&mut self.changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoIo;
    impl crate::ports::MediaRepository for NoIo {
        fn scan(&self, _: &std::path::Path) -> (Vec<crate::domain::Media>, usize) {
            (
                ["img_20080916_035811.jpg", "img_20080916_035811_random.jpg"]
                    .into_iter()
                    .map(|p| crate::domain::Media::from_path(p.into(), String::new()).unwrap())
                    .collect(),
                0,
            )
        }
    }
    impl crate::ports::MediaOpener for NoIo {
        fn reveal(&self, _: &std::path::Path) -> Result<(), String> {
            panic!("test must not reveal files")
        }
        fn open(&self, _: &std::path::Path) -> Result<(), String> {
            panic!("layout must not open")
        }
    }
    impl duplicates::DuplicateImages for NoIo {
        fn compare(&self, _: &std::path::Path, _: &std::path::Path) -> Result<Comparison, String> {
            Ok(Comparison::Identical)
        }
        fn trash_identical(
            &self,
            _: &std::path::Path,
            _: &std::path::Path,
            _: &std::path::Path,
            _: &std::sync::atomic::AtomicBool,
        ) -> Result<(), String> {
            panic!("layout must never trash")
        }
    }
    #[test]
    fn review_layout_fits_narrow_and_wide_windows() {
        for width in [360., 600., 1280.] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let adapter = Arc::new(NoIo);
            let mut view = DuplicateReview::default();
            view.session
                .start("/library".into(), adapter.clone())
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while view.session.report().is_none() {
                view.poll(adapter.clone());
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 900.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        view.show(ui, None, adapter.clone(), adapter.clone(), adapter.clone());
                    });
                },
            );
            for shape in output.shapes {
                if let egui::Shape::Text(text) = shape.shape {
                    if text.galley.job.text.is_empty() {
                        continue;
                    }
                    let rect = text.visual_bounding_rect();
                    assert!(
                        rect.min.x >= -1. && rect.max.x <= width + 1.,
                        "Review overflow {width}: {:?} {rect:?}",
                        text.galley.job.text
                    );
                }
            }
        }
    }
}
