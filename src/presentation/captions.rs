//! Gallery date typography; date resolution lives in the application session.
use crate::{domain::Media, theme};
use eframe::egui;
use photosuite_application::media_dates::MediaDates;
pub(super) fn caption(ui: &mut egui::Ui, dates: &mut MediaDates, media: &Media, width: f32) {
    let (day, clock, detail) = text(dates, media);
    for (label, color) in [(day, theme::TEXT), (clock, theme::MUTED)] {
        ui.add_sized(
            [width, 16.],
            egui::Label::new(egui::RichText::new(label).size(12.).color(color)).truncate(),
        )
        .on_hover_text(&detail);
    }
    if dates.is_loading() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
}

pub(super) fn text(dates: &mut MediaDates, media: &Media) -> (String, String, String) {
    match dates.get(media) {
        Some(Ok(date)) => (
            date.date_label(),
            date.time_label(),
            format!("{}\n{}", media.path().display(), date.origin()),
        ),
        Some(Err(error)) => (
            "Datum unbekannt".into(),
            String::new(),
            format!("{}\n{error}", media.path().display()),
        ),
        None => (
            "Datum wird geladen".into(),
            String::new(),
            media.path().display().to_string(),
        ),
    }
}
