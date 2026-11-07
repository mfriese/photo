use super::image_uri;
use crate::theme;
use eframe::egui;
impl super::Suite {
    pub(super) fn photo_wall(&mut self, ctx: &egui::Context, visible: &[usize]) {
        if crate::domain::diary::diary_year(self.group.as_deref().unwrap_or("")).is_some() {
            self.diary_wall(ctx, visible);
            return;
        }
        egui::CentralPanel::default()
            .frame(theme::panel(theme::CANVAS, 12))
            .show(ctx, |ui| {
                if self.root.is_none() {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() * 0.12).clamp(12., 60.));
                        ui.add(
                            egui::Image::new(egui::include_image!(
                                "../../assets/app-icon/photosuite.png"
                            ))
                            .fit_to_exact_size(egui::vec2(64., 64.)),
                        );
                        ui.add_space(8.);
                        ui.label(
                            egui::RichText::new("Platz für Erinnerungen.")
                                .size(22.)
                                .strong(),
                        );
                        ui.add_space(6.);
                        ui.label(
                            egui::RichText::new("Deine Fotos. Deine Ordner. Deine Sammlung.")
                                .color(theme::MUTED),
                        );
                        ui.add_space(10.);
                        if ui
                            .add_sized(
                                [ui.available_width().min(200.), 30.],
                                egui::Button::new("Fotosammlung öffnen …")
                                    .fill(theme::SELECTED)
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                            )
                            .clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder()
                        {
                            self.load(path);
                        }
                        ui.add_space(6.);
                        ui.label(
                            egui::RichText::new("Lokal auf deinem Mac · Ohne Cloud oder Konto")
                                .small()
                                .color(theme::MUTED),
                        );
                    });
                    return;
                }
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(self.group.as_deref().unwrap_or("Alle Erinnerungen"))
                            .size(20.)
                            .strong(),
                    )
                    .truncate(),
                );
                ui.label(
                    egui::RichText::new(format!("{} Fotos & Videos", visible.len()))
                        .size(12.)
                        .color(theme::MUTED),
                );
                ui.add_space(6.);
                if visible.is_empty() && self.receiver.is_none() {
                    ui.add_space(16.);
                    ui.label(
                        egui::RichText::new("Hier ist noch Platz für Erinnerungen.").size(17.),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Öffne einen anderen Ordner oder passe Suche und Filter an.",
                        )
                        .color(theme::MUTED),
                    );
                }
                ui.spacing_mut().item_spacing = egui::vec2(8., 6.);
                let target = [90., 130., 170., 240., 320.][self.size];
                let gap = ui.spacing().item_spacing.x;
                let width = (ui.available_width() - ui.spacing().scroll.allocated_width()).max(1.);
                let columns = ((width + gap) / (target + gap)).floor().max(1.) as usize;
                let tile = ((width - gap * (columns - 1) as f32) / columns as f32).max(1.);
                let row_height = tile + 24.;
                egui::ScrollArea::vertical().show_rows(
                    ui,
                    row_height,
                    visible.len().div_ceil(columns),
                    |ui, rows| {
                        for row in rows {
                            ui.horizontal(|ui| {
                                ui.set_min_height(row_height);
                                for &index in visible.iter().skip(row * columns).take(columns) {
                                    let m = &self.items[index];
                                    ui.vertical(|ui| {
                                        ui.set_width(tile);
                                        let response = {
                                            let mut image = egui::Image::from_uri(image_uri(
                                                m.path(),
                                                self.editor.rotations(),
                                            ));
                                            let size = egui::vec2(tile, tile);
                                            if self.square_previews {
                                                if let Ok(egui::load::TexturePoll::Ready {
                                                    texture,
                                                }) = image.load_for_size(ui.ctx(), size)
                                                {
                                                    let dimensions = texture.size;
                                                    let mut uv = egui::Rect::from_min_max(
                                                        egui::pos2(0., 0.),
                                                        egui::pos2(1., 1.),
                                                    );
                                                    if dimensions.x > dimensions.y {
                                                        let inset =
                                                            (1. - dimensions.y / dimensions.x) / 2.;
                                                        uv.min.x = inset;
                                                        uv.max.x = 1. - inset;
                                                    } else if dimensions.y > dimensions.x {
                                                        let inset =
                                                            (1. - dimensions.x / dimensions.y) / 2.;
                                                        uv.min.y = inset;
                                                        uv.max.y = 1. - inset;
                                                    }
                                                    image = image.uv(uv);
                                                }
                                                image = image
                                                    .fit_to_exact_size(size)
                                                    .maintain_aspect_ratio(false);
                                            } else {
                                                image = image
                                                    .max_size(size)
                                                    .maintain_aspect_ratio(true);
                                            }
                                            let (rect, response) =
                                                ui.allocate_exact_size(size, egui::Sense::click());
                                            ui.painter().rect_filled(rect, 8., theme::SURFACE);
                                            let mut paint_rect = rect;
                                            if !self.square_previews
                                                && let Ok(egui::load::TexturePoll::Ready {
                                                    texture,
                                                }) = image.load_for_size(ui.ctx(), size)
                                            {
                                                let scale = (tile / texture.size.x)
                                                    .min(tile / texture.size.y);
                                                paint_rect = egui::Rect::from_center_size(
                                                    rect.center(),
                                                    texture.size * scale,
                                                );
                                            }
                                            image.corner_radius(8).paint_at(ui, paint_rect);
                                            if response.hovered() {
                                                ui.painter().rect_stroke(
                                                    rect.shrink(1.),
                                                    8.,
                                                    egui::Stroke::new(2_f32, theme::ACCENT),
                                                    egui::StrokeKind::Inside,
                                                );
                                            }
                                            if m.is_video() {
                                                let badge = egui::Rect::from_min_size(
                                                    rect.left_bottom() + egui::vec2(8., -30.),
                                                    egui::vec2(70., 23.),
                                                );
                                                ui.painter().rect_filled(
                                                    badge,
                                                    5.,
                                                    egui::Color32::from_black_alpha(200),
                                                );
                                                ui.painter().text(
                                                    rect.left_bottom() + egui::vec2(8., -8.),
                                                    egui::Align2::LEFT_BOTTOM,
                                                    "▶ Video",
                                                    egui::FontId::proportional(16.),
                                                    egui::Color32::WHITE,
                                                );
                                            }
                                            response.on_hover_cursor(egui::CursorIcon::PointingHand)
                                        };
                                        if response
                                            .on_hover_text(m.path().display().to_string())
                                            .clicked()
                                        {
                                            self.selected = Some(index);
                                        }
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(
                                                    m.path()
                                                        .file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy(),
                                                )
                                                .size(12.)
                                                .color(theme::MUTED),
                                            )
                                            .truncate(),
                                        )
                                        .on_hover_text(m.path().display().to_string());
                                    });
                                }
                            });
                        }
                    },
                );
            });
    }
}
