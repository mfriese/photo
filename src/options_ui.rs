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
pub fn show(
    ctx: &egui::Context,
    open: &mut bool,
    p: &mut Preferences,
    saving: bool,
    bindings: &mut bool,
    error: &str,
    comments: CommentIndexStatus<'_>,
) -> Option<CommentIndexAction> {
    let mut action = None;
    egui::Window::new("Einstellungen").open(open).default_width(480.)
        .max_width((ctx.content_rect().width()-32.).max(260.)).max_height((ctx.content_rect().height()-48.).max(240.)).vscroll(true).show(ctx,|ui|{
            spacing(ui);
            if p.library_scope.is_some() { ui.label("Änderungen werden in photosuite.yaml in der Bibliothek gespeichert."); }
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
            section(ui,"Audiokommentare",|ui|{
                ui.label("Einzelne YAML-Dateien unter Jahr/audio/y sind die dauerhafte Quelle der Zuordnungen.");
                ui.small("Der Neuaufbau durchsucht die Bibliothek und ersetzt den Gesamtindex erst nach vollständiger Prüfung. Medien bleiben unverändert.");
                if ui.add_enabled(comments.available,egui::Button::new("Kommentarindex vollständig neu aufbauen")).clicked(){action=Some(CommentIndexAction::Rebuild);}
                if comments.rebuilding {ui.spinner();if ui.button("Neuaufbau abbrechen").clicked(){action=Some(CommentIndexAction::Cancel);}}
                if !comments.message.is_empty(){ui.label(comments.message);}
                if !comments.error.is_empty(){ui.strong(comments.error);}
            });
            if !error.is_empty(){ui.colored_label(egui::Color32::LIGHT_RED,error);}
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
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
                            false,
                            &mut bindings,
                            "",
                            CommentIndexStatus::default(),
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
