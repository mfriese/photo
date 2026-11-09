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
    pub(super) fn status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .frame(theme::panel(theme::SURFACE, 8))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.;
                    let mode = if self.import_panel.running() {
                        "Import / Duplikatprüfung aktiv"
                    } else if self.write_exif {
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
                    let text = if self.preferences_error.is_empty() {
                        format!("{mode}{filters}  ·  {}", self.status)
                    } else {
                        format!(
                            "{mode}{filters} · {} · {}",
                            self.status, self.preferences_error
                        )
                    };
                    let text = if self.organization.error().is_empty() {
                        text
                    } else {
                        format!("{text} · {}", self.organization.error())
                    };
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
