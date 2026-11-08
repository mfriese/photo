use crate::theme;
use eframe::egui;
impl super::Suite {
    pub(super) fn toolbar(&mut self, ctx: &egui::Context) {
        let title = self
            .root
            .as_ref()
            .map(|path| format!("PhotoSuite — {}", path.display()))
            .unwrap_or_else(|| "PhotoSuite".into());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        egui::TopBottomPanel::top("toolbar")
            .frame(theme::panel(theme::SURFACE, 8))
            .show(ctx, |ui| {
                ui.add_space(2.);
                // Compact actions wrap as complete controls instead of overflowing.
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::Image::new(egui::include_image!(
                            "../../assets/app-icon/photosuite.png"
                        ))
                        .fit_to_exact_size(egui::vec2(22., 22.)),
                    );
                    ui.label(egui::RichText::new("PhotoSuite").size(17.).strong());
                    ui.add_space(4.);
                    if ui
                        .button("Ordner öffnen …")
                        .on_hover_text("Fotosammlung öffnen")
                        .clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder()
                    {
                        self.load(path);
                    }
                    if ui
                        .add_enabled(self.root.is_some(), egui::Button::new("Aktualisieren"))
                        .on_hover_text("Sammlung aktualisieren")
                        .clicked()
                        && let Some(path) = self.root.clone()
                    {
                        self.load(path);
                    }
                    if ui.button("Import …").clicked() {
                        self.import_panel.open(self.root.clone());
                    }
                    ui.menu_button("Ansicht", |ui| {
                        ui.set_width(230.);
                        crate::options_ui::spacing(ui);
                        ui.strong("Darstellung");
                        ui.checkbox(&mut self.show_library, "Jahre und Monate anzeigen");
                        ui.checkbox(&mut self.square_previews, "Quadratische Vorschau");
                        ui.separator();
                        ui.label("Vorschaugröße");
                        ui.add(egui::Slider::new(&mut self.size, 0..=4).show_value(false));
                        ui.small(["Sehr klein", "Klein", "Mittel", "Groß", "Sehr groß"][self.size]);
                    });
                    if ui.button("Einstellungen …").clicked() {
                        self.options_open = true;
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text("Sammlung durchsuchen …")
                            .desired_width(180.),
                    )
                    .on_hover_text("Dateinamen und Verzeichnispfade filtern");
                    ui.menu_button(
                        if self.videos_only {
                            "Filter: Video"
                        } else {
                            "Filter"
                        },
                        |ui| {
                            ui.set_width(210.);
                            crate::options_ui::spacing(ui);
                            ui.strong("Medienfilter");
                            ui.selectable_value(
                                &mut self.videos_only,
                                false,
                                "Alle Fotos & Videos",
                            );
                            ui.selectable_value(&mut self.videos_only, true, "Nur Videos");
                        },
                    );
                    if self.write_exif {
                        ui.add(
                            egui::Button::new(
                                egui::RichText::new("EXIF speichern: an")
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(30, 57, 80))
                            .stroke(egui::Stroke::NONE)
                            .sense(egui::Sense::hover()),
                        )
                        .on_hover_text("Neue Drehungen ändern JPEG-Dateien ohne Sicherungsdatei");
                    }
                });
                ui.add_space(2.);
                if let Some(path) = &self.root {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("BIBLIOTHEK")
                                .size(12.)
                                .color(theme::MUTED),
                        );
                        let path = path.display().to_string();
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&path).size(12.).color(theme::MUTED),
                            )
                            .truncate(),
                        )
                        .on_hover_text(path);
                    });
                    ui.add_space(2.);
                }
            });
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
                        "Nur Lesen"
                    };
                    let text = if self.preferences_error.is_empty() {
                        format!("{mode}  ·  {}", self.status)
                    } else {
                        format!("{mode} · {} · {}", self.status, self.preferences_error)
                    };
                    let mut label = egui::RichText::new(&text).size(12.).color(theme::MUTED);
                    if !self.preferences_error.is_empty() {
                        label = label.color(egui::Color32::LIGHT_RED);
                    }
                    ui.add(egui::Label::new(label).truncate())
                        .on_hover_text(text);
                });
            });
    }
}
