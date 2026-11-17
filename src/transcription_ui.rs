//! Global model preparation UI. Reads session snapshots and emits explicit confirmation/reveal intents.
use eframe::egui;
use photosuite_application::transcription::{ModelSession, Settings, Stage, catalog};
pub struct View {
    model: String,
    language: String,
    initialized: bool,
    pending: Option<Settings>,
    reveal: Option<std::path::PathBuf>,
}
impl Default for View {
    fn default() -> Self {
        let settings = Settings::default();
        Self {
            model: settings.model().id.into(),
            language: settings.language().into(),
            initialized: false,
            pending: None,
            reveal: None,
        }
    }
}
impl View {
    pub fn take_reveal(&mut self) -> Option<std::path::PathBuf> {
        self.reveal.take()
    }
}
fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", bytes as f64 / (1024. * 1024. * 1024.))
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024. * 1024.))
    }
}
fn language_name(code: &str) -> &str {
    catalog::LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| *n)
        .unwrap_or(code)
}
pub fn show(ui: &mut egui::Ui, session: &mut ModelSession, view: &mut View) {
    session.ensure_loaded();
    if !view.initialized
        && !session.busy()
        && let Some(prepared) = session.prepared()
    {
        view.model = prepared.settings.model().id.into();
        view.language = prepared.settings.language().into();
        view.initialized = true;
    }
    let ready = !session.busy() && session.prepared().is_some();
    ui.label("Modell und Sprache gelten für alle Bibliotheken auf diesem Mac.");
    ui.small(
        "Mit einem geprüften Modell werden offene Audio- und Videoaufträge der Bibliothek automatisch lokal verarbeitet.",
    );
    crate::options_ui::section(ui, "Whisper-Modell", |ui| {
        ui.add_enabled_ui(ready,|ui| {
            let selected=catalog::model(&view.model).unwrap_or(Settings::default().model());
            egui::ComboBox::from_id_salt("whisper_model").width(ui.available_width()).height(260.).truncate()
                .selected_text(format!("{} · {}",selected.id,size(selected.bytes))).show_ui(ui,|ui| {
                    for model in catalog::MODELS {
                        let present=session.prepared().is_some_and(|p|p.available.contains(model.id));
                        let label=format!("{} · {}{}{}",model.id,size(model.bytes),if model.english_only{" · Englisch"}else{""},if present{" · lokal"}else{""});
                        ui.selectable_value(&mut view.model,model.id.into(),label);
                    }
                });
            let selected=catalog::model(&view.model).unwrap_or(Settings::default().model());
            ui.label(selected.description());
            ui.small(if selected.id.contains("-q5_") {"Q5: platzsparend quantisiert. Modellgröße ist nicht der gesamte Arbeitsspeicherbedarf."}else if selected.id.contains("-q8_"){"Q8: quantisiert. Modellgröße ist nicht der gesamte Arbeitsspeicherbedarf."}else{"Unquantisiertes Modell. Arbeitsspeicher wird zusätzlich zum Modell auf dem Datenträger benötigt."});
            if !selected.supports_language(&view.language) {view.language=if selected.english_only {"en"}else{"auto"}.into();}
            ui.label("Aufnahmesprache");
            egui::ComboBox::from_id_salt("whisper_language").width(ui.available_width()).height(240.).truncate()
                .selected_text(language_name(&view.language)).show_ui(ui,|ui| {
                    for (code,name) in catalog::LANGUAGES {
                        if selected.supports_language(code) {ui.selectable_value(&mut view.language,(*code).into(),*name);}
                    }
                });
            if selected.english_only {ui.small("Dieses .en-Modell unterstützt nur Englisch.");}
        });
        if ui
            .add_enabled(ready, egui::Button::new("Auswahl bestätigen …"))
            .clicked()
        {
            match Settings::new(&view.model, &view.language) {
                Ok(value) => view.pending = Some(value),
                Err(error) => session.report_error(error),
            }
        }
    });
    if session.busy() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(if session.cancelling() {
                "Abbruch wird abgeschlossen …"
            } else {
                match session.progress().stage {
                    Stage::Loading => "Lokale Einstellungen werden gelesen …",
                    Stage::Checking => "Modell wird mit SHA-256 geprüft …",
                    Stage::Downloading => "Modell wird heruntergeladen …",
                    Stage::Saving => "Auswahl wird gespeichert …",
                }
            });
        });
        if session.progress().total > 0 {
            ui.add(
                egui::ProgressBar::new(
                    (session.progress().bytes as f64 / session.progress().total as f64)
                        .clamp(0., 1.) as f32,
                )
                .text(format!(
                    "{} / {}",
                    size(session.progress().bytes),
                    size(session.progress().total)
                )),
            );
        }
        if ui
            .add_enabled(!session.cancelling(), egui::Button::new("Abbrechen"))
            .clicked()
        {
            session.cancel();
        }
    }
    if let Some(prepared) = session.prepared() {
        if !prepared.warning.is_empty() {
            ui.colored_label(crate::theme::IMPORT_WARNING, &prepared.warning);
        }
        if let Some(path) = &prepared.ready_path {
            ui.label(
                egui::RichText::new("Modell heruntergeladen und geprüft")
                    .color(crate::theme::IMPORT_SUCCESS),
            );
            ui.small(format!(
                "Aktive Auswahl: {} · {}",
                prepared.settings.model().id,
                language_name(prepared.settings.language())
            ));
            if ui.button("Im Finder anzeigen").clicked() {
                view.reveal = Some(path.clone());
            }
        } else if !session.busy() {
            ui.label("Noch kein geprüftes Modell für die aktive Auswahl vorhanden.");
        }
    }
    if !session.error().is_empty() {
        ui.colored_label(crate::theme::DANGER, session.error());
    }
    if ui
        .add_enabled(
            !session.busy(),
            egui::Button::new("Lokalen Modellstatus erneut prüfen"),
        )
        .clicked()
    {
        view.initialized = false;
        session.reload();
    }
    let locations = session.locations();
    crate::options_ui::section(ui, "Speicherorte auf diesem Mac", |ui| {
        ui.label("Modelldateien");
        ui.add(egui::Label::new(locations.models.display().to_string()).truncate())
            .on_hover_text(locations.models.display().to_string());
        ui.label("Transkriptions-Einstellungen");
        ui.add(egui::Label::new(locations.preferences.display().to_string()).truncate())
            .on_hover_text(locations.preferences.display().to_string());
        ui.small("Außerhalb der Bibliotheken; gemeinsam für alle Sammlungen. Audiodateien werden nicht hochgeladen.");
    });
    if let Some(settings) = view.pending.clone() {
        let present = session
            .prepared()
            .is_some_and(|p| p.available.contains(settings.model().id));
        egui::Modal::new(egui::Id::new("confirm_whisper_model")).show(ui.ctx(),|ui|{
            ui.set_width((ui.ctx().content_rect().width()-64.).clamp(220.,420.));
            ui.heading("Whisper-Modell vorbereiten");ui.add_space(8.);
            ui.label(format!("Modell: {}",settings.model().id));ui.label(format!("Sprache: {}",language_name(settings.language())));
            ui.label(format!("Modelldatei: {}",size(settings.model().bytes)));
            ui.label(if present{"Die vorhandene Datei wird geprüft. Bei fehlerhafter Größe oder Prüfsumme wird sie durch einen verifizierten Download ersetzt."}else{"Das Modell wird nach Bestätigung vom offiziellen whisper.cpp-Modellbestand heruntergeladen und geprüft."});
            ui.small("Erst nach erfolgreicher Prüfung wird diese Auswahl appweit gespeichert. Offene, nicht pausierte Aufträge starten anschließend automatisch.");ui.add_space(8.);
            ui.horizontal_wrapped(|ui|{
                if ui.button("Abbrechen").clicked(){view.pending=None;}
                if ui.button(if present{"Prüfen und übernehmen"}else{"Herunterladen und übernehmen"}).clicked(){
                    if let Err(error)=session.apply(settings){session.report_error(error);}view.pending=None;
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photosuite_application::transcription::{Locations, ModelStore, Prepared, Progress};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    #[derive(Default)]
    struct Store {
        prepares: AtomicUsize,
    }
    impl ModelStore for Store {
        fn locations(&self) -> Locations {
            Locations {
                models: "/Users/Test/Library/Application Support/PhotoSuite/Whisper/models".into(),
                preferences:
                    "/Users/Test/Library/Preferences/de.photosuite.PhotoSuite.transcription.plist"
                        .into(),
            }
        }
        fn inspect(
            &self,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<Prepared, String> {
            Ok(Prepared {
                settings: Settings::new("tiny", "de").unwrap(),
                ..Default::default()
            })
        }
        fn prepare(
            &self,
            settings: &Settings,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<Prepared, String> {
            self.prepares.fetch_add(1, Ordering::SeqCst);
            Ok(Prepared {
                settings: settings.clone(),
                ready_path: Some("/Global/Models/model.bin".into()),
                ..Default::default()
            })
        }
    }
    #[test]
    fn global_settings_fit_small_windows_expose_paths_and_require_download_confirmation() {
        for width in [360., 600., 1280.] {
            let store = Arc::new(Store::default());
            let mut session = ModelSession::new(store.clone());
            session.ensure_loaded();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while session.busy() {
                session.poll();
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            let mut view = crate::options_ui::SettingsView {
                page: crate::options_ui::SettingsPage::Transcription,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            ctx.style_mut(|s| s.animation_time = 0.);
            let mut open = true;
            let mut preferences = photosuite_application::preferences::Preferences::default();
            let original = preferences.clone();
            let mut bindings = false;
            for frame in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1100.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        crate::options_ui::show(
                            ctx,
                            &mut open,
                            &mut preferences,
                            &mut view,
                            crate::options_ui::SettingsContext {
                                index: None,
                                root: None,
                                can_rebuild_index: false,
                                saving: false,
                                bindings: &mut bindings,
                                error: "",
                                comments: Default::default(),
                                library_ready: false,
                                geodata: None,
                                transcription: Some(&mut session),
                            },
                        );
                    },
                );
                if frame == 0 {
                    continue;
                }
                let texts = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) => Some(t),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert!(texts.iter().any(|t| t.galley.job.text == "Transkription"));
                assert!(texts.iter().any(|t| t.galley.job.text == "Modelldateien"));
                assert!(
                    texts
                        .iter()
                        .any(|t| t.galley.job.text == "Transkriptions-Einstellungen")
                );
                for text in texts {
                    let rect = text.visual_bounding_rect();
                    assert!(
                        rect.left() >= -1. && rect.right() <= width + 1.,
                        "width={width} overflow={rect:?} text={}",
                        text.galley.job.text
                    );
                }
            }
            assert_eq!(
                preferences, original,
                "global tab cannot edit library preferences"
            );
            assert_eq!(store.prepares.load(Ordering::SeqCst), 0);
            view.transcription.pending = Some(Settings::new("tiny", "de").unwrap());
            let mut confirm = None;
            for _ in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1100.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .show(ctx, |ui| show(ui, &mut session, &mut view.transcription));
                    },
                );
                confirm = output.shapes.iter().find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.job.text == "Herunterladen und übernehmen" => {
                        Some(t.visual_bounding_rect().center())
                    }
                    _ => None,
                });
            }
            assert_eq!(
                store.prepares.load(Ordering::SeqCst),
                0,
                "opening confirmation must not download"
            );
            let point = confirm.expect("explicit download confirmation");
            for pressed in [true, false] {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1100.),
                        )),
                        events: vec![
                            egui::Event::PointerMoved(point),
                            egui::Event::PointerButton {
                                pos: point,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .show(ctx, |ui| show(ui, &mut session, &mut view.transcription));
                    },
                );
            }
            while session.busy() {
                session.poll();
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert_eq!(store.prepares.load(Ordering::SeqCst), 1);
            assert!(session.prepared().unwrap().ready_path.is_some());
        }
    }
}
