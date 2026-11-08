use super::image_uri;
use eframe::egui;
impl super::Suite {
    pub(super) fn media_viewer(
        &mut self,
        ctx: &egui::Context,
        visible: &[usize],
        pending: &mut Vec<crate::commands::Command>,
    ) {
        if let Some(index) = self.selected {
            if !self.options_open
                && !self.bindings.visible
                && ctx.input(|i| i.key_pressed(egui::Key::Escape))
            {
                self.selected = None;
                self.storyboard.clear();
                return;
            }
            let position = visible.iter().position(|&i| i == index);
            let mut next = false;
            let mut previous = false;
            let mut open = true;
            egui::Window::new("Großansicht")
                .open(&mut open)
                .default_size([950., 700.])
                .max_width((ctx.content_rect().width() - 32.).max(280.))
                .max_height((ctx.content_rect().height() - 32.).max(240.))
                .show(ctx, |ui| {
                    crate::options_ui::spacing(ui);
                    let m = &self.items[index];
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(
                                m.path().file_name().unwrap_or_default().to_string_lossy(),
                            )
                            .strong(),
                        )
                        .truncate(),
                    )
                    .on_hover_text(m.path().display().to_string());
                    let actions = viewer_controls(
                        ui,
                        m.is_video(),
                        position,
                        visible.len(),
                        self.editor.is_saving(),
                        self.write_exif,
                    );
                    previous = actions.previous;
                    next = actions.next;
                    if actions.external
                        && let Err(error) = self.services.opener.open(m.path())
                    {
                        self.status = error;
                    }
                    if let Some(command) = actions.command {
                        pending.push(command);
                    }
                    if actions.reload {
                        self.storyboard.clear();
                    }
                    ui.add_space(4.);
                    if m.is_video() {
                        self.storyboard
                            .show(ui, m.path(), self.services.video_preview.clone());
                    } else {
                        self.storyboard.clear();
                        centered_preview(
                            ui,
                            egui::Image::from_uri(image_uri(m.path(), self.editor.rotations())),
                        );
                    }
                });
            next |= ctx.input(|i| i.key_pressed(egui::Key::ArrowRight));
            previous |= ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft));
            if let Some(p) = position {
                if next && p + 1 < visible.len() {
                    self.selected = Some(visible[p + 1]);
                }
                if previous && p > 0 {
                    self.selected = Some(visible[p - 1]);
                }
            }
            if !open {
                self.selected = None;
            }
        }
    }
}

#[derive(Default)]
struct ViewerActions {
    previous: bool,
    next: bool,
    external: bool,
    reload: bool,
    command: Option<crate::commands::Command>,
}
fn viewer_controls(
    ui: &mut egui::Ui,
    video: bool,
    position: Option<usize>,
    total: usize,
    saving: bool,
    write_exif: bool,
) -> ViewerActions {
    use crate::commands::Command;
    let mut actions = ViewerActions::default();
    egui::Frame::group(ui.style())
        .fill(crate::theme::CANVAS)
        .stroke(egui::Stroke::new(1_f32, crate::theme::BORDER))
        .corner_radius(6)
        .inner_margin(6.)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let cell_width =
                ((ui.available_width() - 3. * ui.spacing().item_spacing.x) / 4.).clamp(1., 112.);
            let button = |text| egui::Button::new(text).wrap_mode(egui::TextWrapMode::Truncate);
            ui.horizontal(|ui| {
                actions.previous = ui
                    .add_enabled_ui(position.is_some_and(|p| p > 0), |ui| {
                        ui.add_sized([cell_width, 28.], button("Zurück"))
                    })
                    .inner
                    .on_hover_text("Vorheriges Medium · Pfeiltaste links")
                    .clicked();
                actions.next = ui
                    .add_enabled_ui(position.is_some_and(|p| p + 1 < total), |ui| {
                        ui.add_sized([cell_width, 28.], button("Weiter"))
                    })
                    .inner
                    .on_hover_text("Nächstes Medium · Pfeiltaste rechts")
                    .clicked();
                actions.external = ui
                    .add_sized([cell_width, 28.], button("Extern öffnen"))
                    .on_hover_text("Im Standardprogramm öffnen")
                    .clicked();
                let position = position
                    .map(|p| format!("{} / {total}", p + 1))
                    .unwrap_or_else(|| "Außerhalb des Filters".into());
                ui.add_sized([cell_width, 28.], egui::Label::new(&position).truncate())
                    .on_hover_text(position);
            });
            ui.horizontal(|ui| {
                if video {
                    if ui
                        .add_sized([cell_width, 28.], button("Abspielen"))
                        .on_hover_text("Im PhotoSuite-Player abspielen · Leertaste (Standard)")
                        .clicked()
                    {
                        actions.command = Some(Command::PlayVideo);
                    }
                    actions.reload = ui
                        .add_sized([cell_width, 28.], button("Vorschau laden"))
                        .on_hover_text("Alle sechs Videovorschaubilder erneut laden")
                        .clicked();
                    ui.allocate_exact_size(egui::vec2(cell_width, 28.), egui::Sense::hover());
                } else {
                    for (command, label, help) in [
                        (
                            Command::RotateLeft,
                            "Links drehen",
                            "90° gegen den Uhrzeigersinn drehen",
                        ),
                        (
                            Command::RotateRight,
                            "Rechts drehen",
                            "90° im Uhrzeigersinn drehen",
                        ),
                        (
                            Command::ResetRotation,
                            "Zurücksetzen",
                            "Sitzungsdrehung verwerfen und gespeicherte EXIF-Ausrichtung anzeigen",
                        ),
                    ] {
                        if ui
                            .add_enabled_ui(!saving, |ui| {
                                ui.add_sized([cell_width, 28.], button(label))
                            })
                            .inner
                            .on_hover_text(help)
                            .clicked()
                        {
                            actions.command = Some(command);
                        }
                    }
                }
                let (status, help) = if video {
                    (
                        "Video",
                        "Video mit sechs Vorschaubildern; Wiedergabe im PhotoSuite-Player",
                    )
                } else if saving {
                    ("Speichert …", "EXIF-Ausrichtung wird gespeichert …")
                } else if write_exif {
                    (
                        "EXIF: an",
                        "Neue Drehungen ändern die JPEG-Datei ohne Sicherungsdatei",
                    )
                } else {
                    (
                        "Sitzungsdrehung",
                        "Drehung nur für diese Sitzung · Original unverändert",
                    )
                };
                ui.add_sized([cell_width, 28.], egui::Label::new(status).truncate())
                    .on_hover_text(help);
            });
        });
    actions
}

fn centered_preview(ui: &mut egui::Ui, image: egui::Image<'_>) -> egui::Response {
    let size = ui.available_size();
    egui::Frame::new()
        .fill(crate::theme::CANVAS)
        .corner_radius(6)
        .show(ui, |ui| {
            ui.set_min_width(size.x);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.add(image.max_size(size).maintain_aspect_ratio(true))
            })
            .inner
        })
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portrait_and_landscape_previews_are_horizontally_centered() {
        for dimensions in [[40, 100], [100, 40]] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let texture = ctx.load_texture(
                "preview",
                egui::ColorImage::filled(dimensions, egui::Color32::WHITE),
                Default::default(),
            );
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600., 500.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let center = ui.available_rect_before_wrap().center().x;
                        let response = centered_preview(ui, egui::Image::new(&texture));
                        assert!((response.rect.center().x - center).abs() < 1.);
                    });
                },
            );
        }
    }
    #[test]
    fn controls_keep_two_rows_across_media_states_and_widths() {
        let mut expected_height: Option<f32> = None;
        for width in [280., 360., 600., 950.] {
            for video in [false, true] {
                for (saving, write_exif, position) in [
                    (false, false, Some(0)),
                    (false, true, Some(999999)),
                    (true, true, None),
                ] {
                    let ctx = egui::Context::default();
                    crate::theme::install(&ctx);
                    let mut height = 0.;
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 800.),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                crate::options_ui::spacing(ui);
                                let top = ui.cursor().top();
                                let actions = viewer_controls(
                                    ui, video, position, 1000000, saving, write_exif,
                                );
                                assert!(
                                    !actions.previous && !actions.next && actions.command.is_none()
                                );
                                height = ui.cursor().top() - top;
                            });
                        },
                    );
                    assert!(height < 110., "Controls exceed two rows: {height}");
                    for shape in output.shapes {
                        if let egui::Shape::Text(text) = shape.shape {
                            crate::theme::assert_text_glyphs(&ctx, &text);
                            assert!(text.galley.rows.len() <= 1, "Control text wrapped");
                            assert!(
                                text.visual_bounding_rect().right() <= width + 1.,
                                "Control overflow"
                            );
                        }
                    }
                    if let Some(expected) = expected_height {
                        assert!(
                            (height - expected).abs() < 0.1,
                            "Height changed: {expected} -> {height} at width {width}, video={video}, saving={saving}"
                        );
                    } else {
                        expected_height = Some(height);
                    }
                }
            }
        }
    }
}
