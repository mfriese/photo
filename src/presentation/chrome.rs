use crate::theme;
use eframe::egui;
impl super::Suite {
    pub(super) fn window_title(&mut self, ctx: &egui::Context) {
        let title = self
            .root
            .as_ref()
            .map(|path| format!("PhotoSuite — {}", path.display()))
            .unwrap_or_else(|| "PhotoSuite".into());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }
    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        self.media_counts.observe(&self.items);
        let counts = self.media_counts;
        egui::TopBottomPanel::bottom("status")
            .frame(theme::panel(theme::SURFACE, 8))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.;
                    let mode = if self.write_exif {
                        "Schreibmodus: JPEG-EXIF"
                    } else {
                        "Originale: Nur Lesen"
                    };
                    let filters = match (self.query.is_empty(), self.videos_only) {
                        (true, false) => String::new(),
                        (true, true) => " · Nur Videos".into(),
                        (false, videos) => format!(
                            " · Suche: {}{}",
                            self.query,
                            if videos { " · Nur Videos" } else { "" }
                        ),
                    };
                    let mut details = format!("{mode}{filters}");
                    if self.import_panel.running() {
                        details.push_str(" · Import / Duplikatprüfung aktiv");
                    }
                    if !self.status.is_empty() {
                        details.push_str(&format!(" · {}", self.status));
                    }
                    for error in [&self.preferences_error, self.organization.error()] {
                        if !error.is_empty() {
                            details.push_str(&format!(" · {error}"));
                        }
                    }
                    let totals = format!(
                        "Medien {} · Photos {} · Audios {} · Videos {}",
                        counts.total, counts.photos, counts.audios, counts.videos
                    );
                    let tooltip = format!("{totals}\n{details}");
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(count_label(counts, ui.available_width() < 600.))
                                .size(12.)
                                .color(theme::TEXT),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&tooltip);
                    let text = details;
                    let mut label = egui::RichText::new(&text).size(12.).color(theme::MUTED);
                    if !self.preferences_error.is_empty() || !self.organization.error().is_empty() {
                        label = label.color(egui::Color32::LIGHT_RED);
                    }
                    ui.add(egui::Label::new(label).truncate())
                        .on_hover_text(text);
                });
            });
    }
}

fn count_label(counts: photosuite_application::collection::MediaCounts, compact: bool) -> String {
    let number = |n: usize| {
        if compact && n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.)
        } else if compact && n >= 10_000 {
            format!("{:.1}k", n as f64 / 1000.)
        } else {
            n.to_string()
        }
    };
    format!(
        "Medien {} · Photos {} · Audios {} · Videos {}",
        number(counts.total),
        number(counts.photos),
        number(counts.audios),
        number(counts.videos)
    )
}
