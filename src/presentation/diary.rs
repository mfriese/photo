use crate::theme;
use eframe::egui;
impl super::Suite {
    pub(super) fn diary_wall(&mut self, ctx: &egui::Context, visible: &[usize]) {
        let mut chosen = None;
        egui::CentralPanel::default().frame(theme::panel(theme::CANVAS,12)).show(ctx, |ui| {
            let year = crate::domain::diary::diary_year(self.group.as_deref().unwrap_or("")).unwrap_or("");
            ui.heading(format!("Diary · {year}"));
            ui.label("Eine Hörleiste für das ganze Jahr · Abschnitte proportional zur Aufnahmedauer");
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!visible.is_empty() && !self.diary.is_loading(), egui::Button::new("▶ Jahresverlauf öffnen")).clicked() { chosen = visible.first().copied(); }
                if self.diary.is_loading() && ui.button("Laden abbrechen").clicked() { self.diary.cancel(); }
                ui.label(format!("{} Audiodateien",visible.len()));
            });
            ui.label("Der Player enthält alle Aufnahmen dieses Jahres, unabhängig vom Suchfilter.");
            if !self.diary.status().is_empty() { ui.add(egui::Label::new(self.diary.status()).wrap()); }
            if !self.diary.warnings().is_empty() {
                egui::CollapsingHeader::new("Datumsquellen der Aufnahmen").show(ui,|ui| {
                    egui::ScrollArea::vertical().max_height(130.).show(ui,|ui| {
                        for source in self.diary.warnings() { ui.add(egui::Label::new(source).wrap()); }
                    });
                });
            }
            ui.separator();
            if visible.is_empty() { ui.label("Noch keine Aufnahmen. Über Import Audio in Jahr/diary kopieren oder den Suchfilter leeren."); }
            egui::ScrollArea::vertical().show_rows(ui,34.,visible.len(),|ui,rows| {
                for row in rows {
                    let index = visible[row];
                    let media = &self.items[index];
                    let name = media.path().file_name().unwrap_or_default().to_string_lossy();
                    let label = format!("♪  {name}");
                    if ui.add_sized([ui.available_width(),34.],egui::Button::new(label).wrap_mode(egui::TextWrapMode::Truncate))
                        .on_hover_text(format!("{}\nIm Jahresverlauf an dieser Aufnahme starten",media.path().display())).clicked() { chosen=Some(index); }
                }
            });
        });
        if let Some(index) = chosen
            && let Err(error) = self.diary.start(
                &self.items,
                &self.items[index],
                self.services.audio_source.clone(),
            )
        {
            self.status = error;
        }
    }
}
