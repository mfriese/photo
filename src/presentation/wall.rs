use super::image_uri;
use eframe::egui;
impl super::Suite {
    pub(super) fn photo_wall(&mut self, ctx: &egui::Context, visible: &[usize]) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.root.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(120.);
                    ui.heading("Deine Erinnerungen. An einem Ort.");
                    ui.label("Öffne einen Fotoordner, um deine Sammlung zu entdecken.");
                    ui.label("Originale werden weder verändert noch verschoben.");
                });
                return;
            }
            ui.heading(self.group.as_deref().unwrap_or("Alle Erinnerungen"));
            ui.label(format!("{} Medien", visible.len()));
            let target = [90., 130., 170., 240., 320.][self.size];
            let gap = ui.spacing().item_spacing.x;
            let width = (ui.available_width() - ui.spacing().scroll.allocated_width()).max(1.);
            let columns = ((width + gap) / (target + gap)).floor().max(1.) as usize;
            let tile = ((width - gap * (columns - 1) as f32) / columns as f32).max(1.);
            let row_height = tile + 36.;
            egui::ScrollArea::vertical().show_rows(
                ui,
                row_height,
                visible.len().div_ceil(columns),
                |ui, rows| {
                    for row in rows {
                        ui.horizontal(|ui| {
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
                                            if let Ok(egui::load::TexturePoll::Ready { texture }) =
                                                image.load_for_size(ui.ctx(), size)
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
                                            image =
                                                image.max_size(size).maintain_aspect_ratio(true);
                                        }
                                        let (rect, response) =
                                            ui.allocate_exact_size(size, egui::Sense::click());
                                        let mut paint_rect = rect;
                                        if !self.square_previews
                                            && let Ok(egui::load::TexturePoll::Ready { texture }) =
                                                image.load_for_size(ui.ctx(), size)
                                        {
                                            let scale =
                                                (tile / texture.size.x).min(tile / texture.size.y);
                                            paint_rect = egui::Rect::from_center_size(
                                                rect.center(),
                                                texture.size * scale,
                                            );
                                        }
                                        image.paint_at(ui, paint_rect);
                                        if m.is_video() {
                                            ui.painter().text(
                                                rect.left_bottom() + egui::vec2(8., -8.),
                                                egui::Align2::LEFT_BOTTOM,
                                                "▶ Video",
                                                egui::FontId::proportional(16.),
                                                egui::Color32::WHITE,
                                            );
                                        }
                                        response
                                    };
                                    if response
                                        .on_hover_text(m.path().display().to_string())
                                        .clicked()
                                    {
                                        self.selected = Some(index);
                                    }
                                    ui.add(
                                        egui::Label::new(
                                            m.path()
                                                .file_name()
                                                .unwrap_or_default()
                                                .to_string_lossy(),
                                        )
                                        .truncate(),
                                    );
                                });
                            }
                        });
                    }
                },
            );
        });
    }
}
