//! Shared formatting only; no I/O, persistence or workflow decisions.
use crate::theme;
use eframe::egui;
pub(super) fn file(ui: &mut egui::Ui, label: &str, path: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).small().color(theme::MUTED));
        ui.add(
            egui::Label::new(egui::RichText::new(path.rsplit('/').next().unwrap_or(path)).strong())
                .truncate(),
        )
        .on_hover_text(path);
    });
}
pub(super) fn value(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(theme::MUTED));
        ui.add(egui::Label::new(value).truncate())
            .on_hover_text(value);
    });
}
pub(super) fn stage(ui: &mut egui::Ui, title: &str, done: usize, total: usize, detail: &str) {
    ui.horizontal(|ui| {
        ui.strong(title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{done} / {total}"));
        });
    });
    ui.add(
        egui::ProgressBar::new(if total > 0 {
            done as f32 / total as f32
        } else {
            0.
        })
        .desired_height(4.)
        .fill(theme::ACCENT),
    );
    ui.label(egui::RichText::new(detail).small().color(theme::MUTED));
}
// Pagination bounds even arbitrarily long error lists/text without a scroll area.
// State is transient presentation state; no library or application data is changed.
pub(super) fn page_selector(ui: &mut egui::Ui, salt: impl std::hash::Hash, count: usize) -> usize {
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

pub(super) fn paged_text(ui: &mut egui::Ui, id: egui::Id, text: &str, error: bool) {
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
