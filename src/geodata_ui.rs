//! Geodata preparation controls only. The session owns worker and persistence lifecycle.
use eframe::egui;
use photosuite_application::geodata::{Phase, SOURCE, Session};
#[derive(Default)]
pub struct View {
    enabled: bool,
    initialized: bool,
    reveal: Option<std::path::PathBuf>,
}
impl View {
    pub fn take_reveal(&mut self) -> Option<std::path::PathBuf> {
        self.reveal.take()
    }
}
pub fn show(ui: &mut egui::Ui, session: &mut Session, view: &mut View) {
    session.ensure_loaded();
    if !view.initialized
        && let Some(prepared) = session.prepared()
    {
        view.enabled = prepared.enabled;
        view.initialized = true;
    }
    ui.label("Geodaten gelten für alle Bibliotheken auf diesem Mac.");
    crate::options_ui::section(ui, "Offline-Geodaten", |ui| {
        ui.add_enabled(
            !session.busy(),
            egui::Checkbox::new(
                &mut view.enabled,
                "Offline-Geodaten aktivieren und bereithalten",
            ),
        );
        ui.label("Weltweite Ortsdaten · Download etwa 13–15 MB");
        ui.small("ZIP und SQLite-Ortsdatenbank werden geprüft und vorbereitet. Fotos werden noch nicht ausgewertet. GPS-Koordinaten werden nicht übertragen.");
        ui.small("Gültige Daten werden wiederverwendet; fehlende oder beschädigte Daten werden erneut vorbereitet. Deaktivieren behält beide Dateien.");
        if ui
            .add_enabled(
                !session.busy(),
                egui::Button::new(if view.enabled {
                    "Prüfen und vorbereiten"
                } else {
                    "Einstellung speichern"
                }),
            )
            .clicked()
        {
            session.apply(view.enabled);
        }
        ui.hyperlink_to("Quelle: GeoNames · CC BY 4.0", "https://www.geonames.org/");
        ui.hyperlink_to("Downloadquelle", SOURCE);
        ui.hyperlink_to(
            "Lizenz und Namensnennung",
            "https://creativecommons.org/licenses/by/4.0/",
        );
    });
    if session.busy() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(if session.cancelling() {
                "Abbruch wird abgeschlossen …"
            } else {
                match session.progress().map(|p| p.phase) {
                    Some(Phase::Downloading) => "Geodaten werden heruntergeladen …",
                    Some(Phase::CheckingArchive) => "ZIP-Datei wird geprüft …",
                    Some(Phase::Importing) => "Orte werden in SQLite importiert …",
                    Some(Phase::CheckingDatabase) => {
                        "Datenbank und räumlicher Index werden geprüft …"
                    }
                    Some(Phase::Saving) => "Einstellung wird gespeichert …",
                    _ => "Lokale Geodaten werden geprüft …",
                }
            });
        });
        if let Some(p) = session.progress() {
            if matches!(p.phase, Phase::Importing) {
                ui.label(format!("{} Orte importiert", p.current));
            } else if p.total > 0 {
                ui.add(
                    egui::ProgressBar::new((p.current as f64 / p.total as f64).clamp(0., 1.) as f32)
                        .text(format!(
                            "{:.1} / {:.1} MB",
                            p.current as f64 / 1e6,
                            p.total as f64 / 1e6
                        )),
                );
            } else if p.current > 0 {
                ui.label(format!("{:.1} MB geladen", p.current as f64 / 1e6));
            }
        }
        if ui
            .add_enabled(!session.cancelling(), egui::Button::new("Abbrechen"))
            .clicked()
        {
            session.cancel();
        }
    }
    if let Some(p) = session.prepared() {
        if !p.warning.is_empty() {
            ui.colored_label(crate::theme::IMPORT_WARNING, &p.warning);
        }
        if p.archive_ready {
            ui.colored_label(
                crate::theme::IMPORT_SUCCESS,
                format!(
                    "ZIP-Datei vorhanden und geprüft · {:.1} MB",
                    p.bytes as f64 / 1e6
                ),
            );
            if ui.button("Im Finder anzeigen").clicked() {
                view.reveal = Some(session.locations().archive);
            }
        } else if !session.busy() {
            ui.label("Noch keine gültige ZIP-Datei vorhanden.");
        }
    }
    if let Some(info) = session.prepared().and_then(|p| p.database.as_ref()) {
        ui.colored_label(
            crate::theme::IMPORT_SUCCESS,
            format!("SQLite geprüft · {} Orte", info.places),
        );
        ui.small(format!("Importiert: {}", info.imported_at));
        if ui.button("Datenbank im Finder anzeigen").clicked() {
            view.reveal = Some(session.locations().database);
        }
    } else if !session.busy() {
        ui.label("Noch kein gültiger SQLite-Import vorhanden.");
    }
    if ui
        .add_enabled(
            !session.busy() && view.enabled,
            egui::Button::new("SQLite aus ZIP neu aufbauen"),
        )
        .clicked()
    {
        session.rebuild_index();
    }
    if !session.error().is_empty() {
        ui.colored_label(crate::theme::DANGER, session.error());
    }
    if ui
        .add_enabled(
            !session.busy(),
            egui::Button::new("Lokalen Status erneut prüfen"),
        )
        .clicked()
    {
        view.initialized = false;
        session.reload();
    }
    let locations = session.locations();
    crate::options_ui::section(ui, "Speicherorte auf diesem Mac", |ui| {
        for (title, path) in [
            ("Geodaten-ZIP", locations.archive),
            ("Ortsdatenbank", locations.database),
            ("Globale Einstellungen", locations.preferences),
        ] {
            ui.label(title);
            ui.add(egui::Label::new(path.display().to_string()).truncate())
                .on_hover_text(path.display().to_string());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use photosuite_application::{
        geodata::Progress,
        geodata::{Locations, Prepared, Store},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    #[derive(Default)]
    struct Fake {
        writes: AtomicUsize,
    }
    impl Store for Fake {
        fn locations(&self) -> Locations {
            Locations {
                archive: "/Global/GeoNames/cities500.zip".into(),
                database: "/Global/GeoNames/places.sqlite".into(),
                preferences: "/Global/Preferences/geodata.plist".into(),
            }
        }
        fn inspect(
            &self,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<Prepared, String> {
            Ok(Default::default())
        }
        fn prepare(
            &self,
            enabled: bool,
            _: &AtomicBool,
            _: &(dyn Fn(Progress) + Sync),
        ) -> Result<Prepared, String> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(Prepared {
                enabled,
                ready: enabled,
                archive_ready: enabled,
                database: Some(photosuite_application::geodata::DatabaseInfo {
                    places: 2,
                    source_sha256: "a".repeat(64),
                    imported_at: "2026-11-17T12:00:00Z".into(),
                }),
                bytes: 13_000_000,
                ..Default::default()
            })
        }
    }
    fn wait(session: &mut Session) {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while session.busy() {
            session.poll();
            assert!(std::time::Instant::now() < until);
            std::thread::yield_now();
        }
    }
    #[test]
    fn global_geodata_tab_works_without_library_shows_paths_and_downloads_only_after_confirmation()
    {
        for width in [360., 600., 1280.] {
            let store = Arc::new(Fake::default());
            let mut session = Session::new(store.clone());
            session.ensure_loaded();
            wait(&mut session);
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            ctx.style_mut(|s| s.animation_time = 0.);
            let mut view = crate::options_ui::SettingsView {
                page: crate::options_ui::SettingsPage::Geodata,
                ..Default::default()
            };
            let mut prefs = photosuite_application::preferences::Preferences::default();
            let original = prefs.clone();
            let mut open = true;
            let mut bindings = false;
            let mut draw = |session: &mut Session, events| {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1100.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        crate::options_ui::show(
                            ctx,
                            &mut open,
                            &mut prefs,
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
                                transcription: None,
                                geodata: Some(session),
                            },
                        );
                    },
                );
                output
                    .shapes
                    .into_iter()
                    .filter_map(|shape| match shape.shape {
                        egui::Shape::Text(t) => {
                            Some((t.galley.job.text.clone(), t.visual_bounding_rect()))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            let click = |pos| {
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            };
            let mut labels = Vec::new();
            for _ in 0..3 {
                labels = draw(&mut session, vec![]);
            }
            for text in [
                "Geodaten",
                "Geodaten-ZIP",
                "Ortsdatenbank",
                "Globale Einstellungen",
            ] {
                assert!(labels.iter().any(|(t, _)| t == text));
            }
            assert!(labels.iter().any(|(t, _)| t.contains("cities500.zip")));
            assert_eq!(store.writes.load(Ordering::SeqCst), 0);
            let checkbox = labels
                .iter()
                .find(|(t, _)| t.contains("aktivieren und bereithalten"))
                .unwrap()
                .1
                .center();
            draw(&mut session, click(checkbox));
            assert_eq!(store.writes.load(Ordering::SeqCst), 0);
            labels = draw(&mut session, vec![]);
            let confirm = labels
                .iter()
                .find(|(t, _)| t == "Prüfen und vorbereiten")
                .unwrap()
                .1
                .center();
            draw(&mut session, click(confirm));
            wait(&mut session);
            assert_eq!(store.writes.load(Ordering::SeqCst), 1);
            assert!(session.prepared().unwrap().enabled);
            labels = draw(&mut session, vec![]);
            assert!(
                labels
                    .iter()
                    .any(|(t, _)| t.contains("vorhanden und geprüft"))
            );
            assert!(labels.iter().any(|(t, _)| t.contains("SQLite geprüft")));
            let rebuild = labels
                .iter()
                .find(|(t, _)| t == "SQLite aus ZIP neu aufbauen")
                .unwrap()
                .1
                .center();
            draw(&mut session, click(rebuild));
            wait(&mut session);
            assert_eq!(store.writes.load(Ordering::SeqCst), 2);
            assert_eq!(
                prefs, original,
                "global changes must not enter library settings"
            );
        }
    }
}
