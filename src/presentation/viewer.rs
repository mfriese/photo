use super::icons::{Icon, button};
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
            if !self.comments.is_open()
                && !self.options_open
                && !self.tag_view.open
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
            let m = self.items[index].clone();
            let (date, clock, date_detail) = super::captions::text(&mut self.media_dates, &m);
            egui::Window::new(format!("Großansicht · {date} {clock}"))
                .id(egui::Id::new("media_viewer"))
                .open(&mut open)
                .default_size([950., 700.])
                .max_width((ctx.content_rect().width() - 32.).max(280.))
                .max_height((ctx.content_rect().height() - 32.).max(240.))
                .show(ctx, |ui| {
                    crate::options_ui::spacing(ui);
                    let actions = viewer_controls(
                        ui,
                        ViewerState {
                            video: m.is_video(),
                            position,
                            total: visible.len(),
                            saving: self.editor.is_saving(),
                            write_exif: self.write_exif,
                        },
                        |ui| self.compact_photo_audio(ui, m.path()),
                    );
                    if actions.record {
                        self.record_photo = Some(index);
                    }
                    if actions.tags {
                        match self.organization.file(m.path()) {
                            Ok(file) => self.tag_view.edit(file),
                            Err(error) => self.status = error,
                        };
                    }
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
                        )
                        .on_hover_text(&date_detail);
                    }
                });
            if !self.comments.is_open() && !self.tag_view.open {
                next |= ctx.input(|i| i.key_pressed(egui::Key::ArrowRight));
                previous |= ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft));
            }
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
    record: bool,
    tags: bool,
    command: Option<crate::commands::Command>,
}
struct ViewerState {
    video: bool,
    position: Option<usize>,
    total: usize,
    saving: bool,
    write_exif: bool,
}
fn viewer_controls(
    ui: &mut egui::Ui,
    state: ViewerState,
    audio: impl FnOnce(&mut egui::Ui),
) -> ViewerActions {
    let ViewerState {
        video,
        position,
        total,
        saving,
        write_exif,
    } = state;
    use crate::commands::Command;
    let mut actions = ViewerActions::default();
    egui::Frame::group(ui.style())
        .fill(crate::theme::CANVAS)
        .stroke(egui::Stroke::new(1_f32, crate::theme::BORDER))
        .corner_radius(6)
        .inner_margin(6.)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.x = 6.;
            ui.horizontal(|ui| {
                actions.previous = button(
                    ui,
                    Icon::Previous,
                    position.is_some_and(|p| p > 0),
                    "Vorheriges Medium · Pfeiltaste links",
                )
                .clicked();
                actions.next = button(
                    ui,
                    Icon::Next,
                    position.is_some_and(|p| p + 1 < total),
                    "Nächstes Medium · Pfeiltaste rechts",
                )
                .clicked();
                actions.external =
                    button(ui, Icon::External, true, "Im Standardprogramm öffnen").clicked();
                actions.tags = button(ui, Icon::Tag, true, "Tags bearbeiten").clicked();
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width().max(1.), 34.),
                    egui::Layout::left_to_right(egui::Align::Center),
                    audio,
                );
            });
            ui.horizontal(|ui| {
                if video {
                    if button(ui, Icon::Play, true, "Video abspielen · Leertaste").clicked() {
                        actions.command = Some(Command::PlayVideo);
                    }
                    actions.reload = button(
                        ui,
                        Icon::Reload,
                        true,
                        "Sechs Videovorschaubilder erneut laden",
                    )
                    .clicked();
                    ui.allocate_exact_size(egui::vec2(34., 34.), egui::Sense::hover());
                } else {
                    for (command, icon, help) in [
                        (
                            Command::RotateLeft,
                            Icon::RotateLeft,
                            "90° gegen den Uhrzeigersinn drehen",
                        ),
                        (
                            Command::RotateRight,
                            Icon::RotateRight,
                            "90° im Uhrzeigersinn drehen",
                        ),
                        (
                            Command::ResetRotation,
                            Icon::Reset,
                            "Sitzungsdrehung zurücksetzen",
                        ),
                    ] {
                        let mode = if saving {
                            "Speichert …"
                        } else if write_exif {
                            "EXIF-Schreiben aktiviert: neue Drehungen ändern die JPEG-Datei"
                        } else {
                            "Nur Sitzungsdrehung · Original unverändert"
                        };
                        if button(ui, icon, !saving, &format!("{help}\n{mode}")).clicked() {
                            actions.command = Some(command);
                        }
                    }
                }
                actions.record = button(
                    ui,
                    Icon::Record,
                    !video,
                    "Audiokommentar zu diesem Foto aufnehmen",
                )
                .clicked();
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
                let image = image.max_size(size).maintain_aspect_ratio(true);
                let pending = matches!(
                    image.load_for_size(ui.ctx(), size),
                    Ok(egui::load::TexturePoll::Pending { .. })
                );
                let response = ui.add(image.show_loading_spinner(false));
                if pending {
                    crate::theme::image_spinner(ui, response.rect);
                }
                response
            })
            .inner
        })
        .inner
}

impl super::Suite {
    /// A photo opened from Diary needs photo navigation while Diary remains selected behind it.
    pub(super) fn viewer_items(&self, visible: &[usize]) -> Vec<usize> {
        if let Some(index) = self.selected
            && !visible.contains(&index)
            && self.items[index].is_photo()
        {
            crate::application::visible(&self.items, "", Some(self.items[index].group()), false)
        } else {
            visible
                .iter()
                .copied()
                .filter(|&index| !self.items[index].is_audio())
                .collect()
        }
    }
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
                                    ui,
                                    ViewerState {
                                        video,
                                        position,
                                        total: 1000000,
                                        saving,
                                        write_exif,
                                    },
                                    |_| {},
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
                            if text.galley.job.text.is_empty() {
                                continue;
                            }
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
