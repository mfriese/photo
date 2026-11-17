//! Consistent presentation of settings. No file I/O in UI callbacks.
use eframe::egui;
use photosuite_application::preferences::Preferences;
pub fn spacing(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(8., 4.);
    ui.spacing_mut().button_padding = egui::vec2(8., 3.);
}
pub fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(crate::theme::CANVAS)
        .stroke(egui::Stroke::new(1_f32, crate::theme::BORDER))
        .corner_radius(6)
        .inner_margin(8.)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.strong(title);
            ui.add_space(2.);
            body(ui);
        });
}
#[derive(Default)]
pub struct CommentIndexStatus<'a> {
    pub available: bool,
    pub rebuilding: bool,
    pub message: &'a str,
    pub error: &'a str,
}
pub enum CommentIndexAction {
    Rebuild,
    Cancel,
}
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    #[default]
    General,
    Database,
    Transcription,
    Geodata,
}
#[derive(Default)]
pub struct SettingsView {
    pub page: SettingsPage,
    pub transcription: crate::transcription_ui::View,
    pub geodata: crate::geodata_ui::View,
}
pub struct SettingsContext<'a> {
    pub index: Option<&'a mut photosuite_application::library_index::Session>,
    pub root: Option<&'a std::path::Path>,
    pub can_rebuild_index: bool,
    pub saving: bool,
    pub bindings: &'a mut bool,
    pub error: &'a str,
    pub comments: CommentIndexStatus<'a>,
    pub library_ready: bool,
    pub geodata: Option<&'a mut photosuite_application::geodata::Session>,
    pub transcription: Option<&'a mut photosuite_application::transcription::ModelSession>,
}
pub fn show(
    ctx: &egui::Context,
    open: &mut bool,
    p: &mut Preferences,
    view: &mut SettingsView,
    state: SettingsContext<'_>,
) -> Option<CommentIndexAction> {
    let SettingsContext {
        saving,
        index,
        root,
        can_rebuild_index,
        bindings,
        error,
        comments,
        library_ready,
        transcription,
        geodata,
    } = state;
    let mut action = None;
    egui::Window::new("Einstellungen").open(open).default_width(480.).default_height(720.)
        .max_width((ctx.content_rect().width()-32.).max(260.)).max_height((ctx.content_rect().height()-48.).max(240.)).vscroll(true).show(ctx,|ui|{
            spacing(ui);
            ui.horizontal_wrapped(|ui|{
                ui.selectable_value(&mut view.page,SettingsPage::General,"Allgemein");
                ui.selectable_value(&mut view.page,SettingsPage::Database,"Datenbank");
                ui.selectable_value(&mut view.page,SettingsPage::Transcription,"Transkription");
                ui.selectable_value(&mut view.page,SettingsPage::Geodata,"Geodaten");
            });ui.separator();
            if view.page==SettingsPage::Geodata {
                if let Some(session)=geodata {crate::geodata_ui::show(ui,session,&mut view.geodata);}
                else {ui.label("Geodatenvorbereitung nicht verfügbar.");}
                return;
            }
            if view.page==SettingsPage::Transcription {
                if let Some(session)=transcription{crate::transcription_ui::show(ui,session,&mut view.transcription);}
                else{ui.label("Transkriptionsvorbereitung nicht verfügbar.");}
                return;
            }
            if view.page==SettingsPage::Database {
                if root.is_none() {ui.label("Zum Verwalten der Datenbank eine Bibliothek öffnen.");}
            if let Some(index)=index {section(ui,"Bibliotheksdatenbank",|ui|{
                ui.label("yaml/library.sqlite enthält ausschließlich neu aufbaubare Indizes und Caches.");
                if let Some(root)=root {ui.add(egui::Label::new(root.join("yaml/library.sqlite").display().to_string()).truncate()).on_hover_text(root.join("yaml/library.sqlite").display().to_string());}
                if ui.add_enabled(root.is_some() && can_rebuild_index && !index.busy(),egui::Button::new("Datenbank aus Einzel-YAMLs neu aufbauen")).clicked() && let Some(root)=root {index.start(root.into());}
                if index.busy(){ui.spinner();ui.label(format!("{} Einzel-YAMLs geprüft",index.checked()));if ui.button("Neuaufbau abbrechen").clicked(){index.cancel();}}
                if !index.error().is_empty(){ui.colored_label(crate::theme::DANGER,index.error());}
            });}
            section(ui,"Audiokommentare",|ui|{
                ui.label("Einzelne YAML-Dateien unter yaml/Jahr/Monat sind die dauerhafte Quelle der Zuordnungen.");
                ui.small("Der Neuaufbau durchsucht die Bibliothek und ersetzt den Gesamtindex erst nach vollständiger Prüfung. Medien bleiben unverändert.");
                if ui.add_enabled(comments.available,egui::Button::new("Kommentarindex vollständig neu aufbauen")).clicked(){action=Some(CommentIndexAction::Rebuild);}
                if comments.rebuilding {ui.spinner();if ui.button("Neuaufbau abbrechen").clicked(){action=Some(CommentIndexAction::Cancel);}}
                if !comments.message.is_empty(){ui.label(comments.message);}
                if !comments.error.is_empty(){ui.strong(comments.error);}
            });
                return;
            }
            if !library_ready {ui.spinner();ui.label("Bibliotheks-Einstellungen werden geladen …");return;}

            if p.library_scope.is_some() { ui.label("Änderungen werden in yaml/settings.yaml in der Bibliothek gespeichert."); }
            else { ui.label("Ohne geladene Bibliothek gelten Einstellungen nur für diese Sitzung."); }
            ui.small("Der zuletzt geöffnete Bibliothekspfad bleibt lokal auf diesem Mac.");
            section(ui,"Darstellung",|ui|{
                ui.checkbox(&mut p.show_library,"Jahre und Monate anzeigen");
                ui.checkbox(&mut p.square_previews,"Quadratische Vorschau");
                ui.label("Vorschaugröße");ui.add(egui::Slider::new(&mut p.preview_size,0..=4).show_value(false));
                ui.small(["Sehr klein","Klein","Mittel","Groß","Sehr groß"][p.preview_size]);
            });
            section(ui,"Beim Start",|ui|{
                ui.checkbox(&mut p.reopen_library,"Letzte Bibliothek wieder öffnen");
                if let Some(path)=&p.last_library {ui.label("Zuletzt geöffnet:");ui.add(egui::Label::new(path.display().to_string()).truncate()).on_hover_text(path.display().to_string());}else{ui.small("Noch keine Bibliothek geöffnet.");}
            });
            section(ui,"Bildänderungen",|ui|{
                ui.add_enabled(!saving,egui::Checkbox::new(&mut p.write_exif,"Drehungen in JPEG speichern"));
                ui.label("Aktiviert: EXIF im Original ändern, ohne Backup. Bildpixel bleiben unverändert.");
                ui.small("Deaktiviert: Manuelle Drehungen gelten nur für die Sitzung.");
            });
            section(ui,"Bedienung",|ui|{if ui.button("Befehle & Tastenkürzel …").clicked(){*bindings=true;}});
            if !error.is_empty(){ui.colored_label(egui::Color32::LIGHT_RED,error);}
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn database_maintenance_has_its_own_tab_and_does_not_require_loaded_preferences() {
        use std::{
            path::Path,
            sync::{Arc, atomic::AtomicBool},
        };
        struct Rebuilder;
        impl photosuite_application::library_index::Rebuilder for Rebuilder {
            fn rebuild(
                &self,
                _: &Path,
                _: &AtomicBool,
                _: &(dyn Fn(usize) + Sync),
            ) -> Result<(), String> {
                panic!("opening settings must not rebuild data")
            }
        }
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        ctx.style_mut(|s| s.animation_time = 0.);
        let mut index = photosuite_application::library_index::Session::new(Arc::new(Rebuilder));
        let mut view = SettingsView::default();
        let mut p = Preferences::default();
        let mut open = true;
        let mut bindings = false;
        let mut render = |events, view: &mut SettingsView| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600., 650.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    show(
                        ctx,
                        &mut open,
                        &mut p,
                        view,
                        SettingsContext {
                            index: Some(&mut index),
                            root: Some(Path::new("/Fotos")),
                            can_rebuild_index: true,
                            saving: false,
                            bindings: &mut bindings,
                            error: "",
                            comments: CommentIndexStatus::default(),
                            library_ready: view.page != SettingsPage::Database,
                            geodata: None,
                            transcription: None,
                        },
                    );
                },
            )
        };
        let _ = render(vec![], &mut view);
        let output = render(vec![], &mut view);
        fn text(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(t.visual_bounding_rect().center())
                }
                _ => None,
            })
        }
        assert!(text(&output, "Darstellung").is_some());
        assert!(text(&output, "Bibliotheksdatenbank").is_none());
        let pos = text(&output, "Datenbank").unwrap();
        for pressed in [true, false] {
            let _ = render(
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                &mut view,
            );
        }
        let output = render(vec![], &mut view);
        assert_eq!(view.page, SettingsPage::Database);
        assert!(text(&output, "Bibliotheksdatenbank").is_some());
        assert!(text(&output, "Audiokommentare").is_some());
        assert!(text(&output, "Darstellung").is_none());
    }
    #[test]
    fn settings_dialog_fits_supported_widths() {
        for width in [360., 600., 1280.] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let mut open = true;
            let mut bindings = false;
            let mut p = Preferences::default();
            for _ in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 600.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        show(
                            ctx,
                            &mut open,
                            &mut p,
                            &mut SettingsView::default(),
                            SettingsContext {
                                index: None,
                                root: None,
                                can_rebuild_index: false,
                                saving: false,
                                bindings: &mut bindings,
                                error: "",
                                comments: CommentIndexStatus::default(),
                                library_ready: true,
                                transcription: None,
                                geodata: None,
                            },
                        );
                    },
                );
                for shape in output.shapes {
                    if let egui::Shape::Text(text) = shape.shape {
                        let rect = text.visual_bounding_rect();
                        assert!(
                            rect.min.x >= -1. && rect.max.x <= width + 1.,
                            "Settings text overflow at {width}: {rect:?}"
                        );
                    }
                }
            }
        }
    }
}
