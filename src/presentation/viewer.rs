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
        if self.selected.is_none() && self.viewer_fullscreen {
            self.viewer_fullscreen = false;
            self.viewer_video = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        if let Some(index) = self.selected {
            let Some(m) = self.items.get(index).cloned() else {
                self.selected = None;
                self.storyboard.clear();
                return;
            };
            if !ctx.wants_keyboard_input()
                && !self.comments.is_open()
                && !self.options_open
                && !self.tag_view.open
                && !self.bindings.visible
                && ctx.input(|i| i.modifiers.is_none() && i.key_pressed(egui::Key::F))
            {
                self.viewer_fullscreen = !self.viewer_fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.viewer_fullscreen));
            }
            if self.viewer_fullscreen && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.viewer_fullscreen = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                return;
            }
            if !self.comments.is_open()
                && !self.location_open
                && !self.options_open
                && !self.tag_view.open
                && !self.bindings.visible
                && ctx.input(|i| i.key_pressed(egui::Key::Escape))
            {
                if self.leave_inline_recording() {
                    self.selected = None;
                    self.storyboard.clear();
                }
                return;
            }
            let position = visible.iter().position(|&i| i == index);
            let mut next = false;
            let mut previous = false;
            let mut open = true;
            let (date, clock, date_detail) = super::captions::text(&mut self.media_dates, &m);
            if self.viewer_fullscreen {
                egui::Area::new(egui::Id::new("fullscreen_viewer"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(ctx.content_rect().min)
                    .show(ctx, |ui| {
                        ui.set_min_size(ctx.content_rect().size());
                        ui.painter()
                            .rect_filled(ctx.content_rect(), 0., egui::Color32::BLACK);
                        if !m.is_video() {
                            centered_preview(
                                ui,
                                egui::Image::from_uri(image_uri(m.path(), self.editor.rotations())),
                            );
                        } else {
                            ui.label("Videowiedergabe im nativen Player · F beendet Vollbild");
                        }
                    });
                if m.is_video() && self.viewer_video.as_deref() != Some(m.path()) {
                    let transport = self.services.audio_player.transport();
                    let result = if transport.playing {
                        self.services.audio_player.control(
                            photosuite_application::diary::AudioControl::Configure {
                                playing: false,
                                category: transport.category_filter,
                            },
                        )
                    } else {
                        Ok(())
                    };
                    if let Err(error) =
                        result.and_then(|_| self.services.video_player.play(m.path()))
                    {
                        self.status = error;
                    }
                    self.viewer_video = Some(m.path().to_owned());
                } else if !m.is_video() {
                    self.viewer_video = None;
                }
            } else {
                self.viewer_video = None;
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
                            |ui| self.compact_photo_audio(ui, m.path(), m.is_photo()),
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
                        if actions.reveal
                            && let Err(error) = self.services.opener.reveal(m.path())
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
                                egui::Image::from_uri(image_uri(m.path(), self.editor.rotations()))
                                    .sense(egui::Sense::click()),
                            )
                            .on_hover_text(&date_detail)
                            .context_menu(|ui| {
                                if ui.button("Ort anzeigen …").clicked() {
                                    self.open_photo_location(&m);
                                    ui.close();
                                }
                            });
                        }
                    });
            }
            if !ctx.wants_keyboard_input()
                && (!self.comments.is_open() || self.comment_view.inline)
                && !self.tag_view.open
            {
                next |= ctx.input(|i| i.key_pressed(egui::Key::ArrowRight));
                previous |= ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft));
            }
            if let Some(p) = position {
                let recording = self.comment_view.inline
                    && self.comments.snapshot().phase
                        == photosuite_application::comments::CapturePhase::Recording;
                let eligible =
                    |i: &usize| !recording || self.items.get(*i).is_some_and(|m| m.is_photo());
                let destination = if next {
                    visible
                        .get(p + 1..)
                        .unwrap_or(&[])
                        .iter()
                        .copied()
                        .find(eligible)
                } else if previous {
                    visible
                        .get(..p)
                        .unwrap_or(&[])
                        .iter()
                        .rev()
                        .copied()
                        .find(eligible)
                } else {
                    None
                };
                if let Some(destination) = destination {
                    self.navigate_viewer(destination);
                }
            }
            if !open && self.leave_inline_recording() {
                self.selected = None;
            }
        }
    }
    pub(super) fn navigate_viewer(&mut self, destination: usize) {
        let recording = self.comment_view.inline
            && self.comments.snapshot().phase
                == photosuite_application::comments::CapturePhase::Recording;
        if self.comments.is_switching_photo() {
            return;
        }
        if recording {
            let result = (|| {
                let media = self
                    .items
                    .get(destination)
                    .ok_or("Foto nicht mehr vorhanden")?;
                let target = photosuite_application::comments::PhotoTarget {
                    root: self.root.clone().ok_or("Keine Bibliothek geöffnet")?,
                    file: self.organization.file(media.path())?,
                };
                self.comments.continue_recording_at(target)
            })();
            if let Err(error) = result {
                self.status = error;
            }
        } else if self.leave_inline_recording() {
            self.selected = Some(destination);
        }
    }
}

#[derive(Default)]
struct ViewerActions {
    previous: bool,
    next: bool,
    reveal: bool,
    reload: bool,
    record: bool,
    tags: bool,
    command: Option<crate::commands::Command>,
}
#[derive(Clone, Copy)]
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
    audio: impl FnOnce(&mut egui::Ui) -> bool,
) -> ViewerActions {
    let mut actions = ViewerActions::default();
    ui.spacing_mut().item_spacing = egui::vec2(6., 4.);
    if ui.available_width() < 560. {
        ui.horizontal(|ui| leading_controls(ui, state, &mut actions));
        let width = (ui.available_width() - 14.).max(1.);
        actions.record = control_group(ui, width, egui::Layout::top_down(egui::Align::Min), audio);
    } else {
        ui.horizontal(|ui| {
            leading_controls(ui, state, &mut actions);
            let width = (ui.available_width() - 14.).max(1.);
            actions.record =
                control_group(ui, width, egui::Layout::top_down(egui::Align::Min), audio);
        });
    }
    actions
}
fn leading_controls(ui: &mut egui::Ui, state: ViewerState, actions: &mut ViewerActions) {
    let ViewerState {
        video,
        position,
        total,
        saving,
        write_exif,
    } = state;
    use crate::commands::Command;
    control_group(ui, 114., egui::Layout::top_down(egui::Align::Min), |ui| {
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
        });
    });
    control_group(ui, 34., egui::Layout::top_down(egui::Align::Min), |ui| {
        actions.tags = button(ui, Icon::Tag, true, "Tags bearbeiten").clicked();
        actions.reveal = button(ui, Icon::Reveal, true, "Im Finder anzeigen").clicked();
    });
}
/// Three equally high presentation groups with shared theme surfaces and borders.
fn control_group<R>(
    ui: &mut egui::Ui,
    width: f32,
    layout: egui::Layout,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(crate::theme::SURFACE)
        .stroke(egui::Stroke::new(1_f32, crate::theme::BORDER))
        .corner_radius(6)
        .inner_margin(6.)
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(egui::vec2(width, 72.), layout, |ui| {
                ui.set_min_size(egui::vec2(width, 72.));
                body(ui)
            })
            .inner
        })
        .inner
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
            && let Some(media) = self.items.get(index).filter(|media| media.is_photo())
        {
            crate::application::visible(&self.items, "", Some(media.group()), false)
        } else {
            visible
                .iter()
                .copied()
                .filter(|&index| self.items.get(index).is_some_and(|media| !media.is_audio()))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct RevealSpy(std::sync::Mutex<Vec<std::path::PathBuf>>);
    impl crate::ports::MediaOpener for RevealSpy {
        fn open(&self, _: &std::path::Path) -> Result<(), String> {
            panic!("Finder action must not open media")
        }
        fn reveal(&self, path: &std::path::Path) -> Result<(), String> {
            self.0.lock().unwrap().push(path.into());
            Ok(())
        }
    }
    #[test]
    fn viewer_title_and_real_comment_controls_fit_without_status_labels() {
        for width in [360., 600., 1280.] {
            let mut suite = super::super::layout_tests::test_suite();
            suite.root = Some("/Fotos".into());
            suite.items = vec![
                crate::domain::Media::from_path(
                    "/Fotos/2008/09/img_20080916_035811.jpg".into(),
                    "2008/09".into(),
                )
                .unwrap(),
            ];
            suite.selected = Some(0);
            let revealed = std::sync::Arc::new(RevealSpy::default());
            suite.services.opener = revealed.clone();
            let mut reveal_point = None;
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            ctx.style_mut(|s| s.animation_time = 0.);
            for frame in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 700.),
                        )),
                        ..Default::default()
                    },
                    |ctx| suite.media_viewer(ctx, &[0], &mut Vec::new()),
                );
                if frame == 0 {
                    continue;
                }
                let record = output
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Circle(circle) if (circle.radius - 6.).abs() < 0.01 => {
                            Some(circle.center)
                        }
                        _ => None,
                    })
                    .expect("record symbol");
                let play = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Rect(rect)
                            if rect.rect.size() == egui::vec2(34., 34.)
                                && (rect.rect.center().x - record.x).abs() < 0.5
                                && rect.rect.center().y < record.y =>
                        {
                            Some(rect.rect.center())
                        }
                        _ => None,
                    })
                    .max_by(|a, b| a.y.total_cmp(&b.y))
                    .expect("play directly above recording");
                assert!((record.y - play.y - 38.).abs() < 0.5);
                assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(rect) if (rect.rect.height()-34.).abs()<0.1 && rect.fill==crate::theme::DISABLED_BG && (rect.rect.center().y-play.y).abs()<0.5)),"playback track centered in top row");
                assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(rect) if (rect.rect.height()-34.).abs()<0.1 && rect.fill==crate::theme::CANVAS && (rect.rect.center().y-record.y).abs()<0.5)),"recording waveform matches playback track height");
                let mut groups: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Rect(rect)
                            if rect.fill == crate::theme::SURFACE
                                && rect.stroke.color == crate::theme::BORDER
                                && rect.rect.height() > 70.
                                && rect.rect.height() < 100. =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .collect();
                groups.sort_by(|a, b| {
                    a.top()
                        .total_cmp(&b.top())
                        .then(a.left().total_cmp(&b.left()))
                });
                assert_eq!(groups.len(), 3, "three visible control groups");
                assert!(groups[2].contains(play) && groups[2].contains(record));
                let dialog = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Rect(rect)
                            if rect.rect.size() == egui::vec2(34., 34.)
                                && (rect.rect.center().y - record.y).abs() < 0.5
                                && groups[2].contains(rect.rect.center()) =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .max_by(|a, b| a.right().total_cmp(&b.right()))
                    .unwrap();
                assert!(
                    (dialog.right() - (groups[2].right() - 7.)).abs() < 2.,
                    "width={width} dialog={dialog:?} group={:?}",
                    groups[2]
                );

                let file_buttons: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Rect(rect)
                            if rect.rect.size() == egui::vec2(34., 34.)
                                && groups[1].contains(rect.rect.center()) =>
                        {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(file_buttons.len(), 2);
                reveal_point = file_buttons
                    .iter()
                    .max_by(|a, b| a.center().y.total_cmp(&b.center().y))
                    .map(|rect| rect.center());
                assert!((file_buttons[0].center().x - file_buttons[1].center().x).abs() < 0.5);
                assert!((file_buttons[0].center().y - file_buttons[1].center().y).abs() > 34.);

                let mut buttons = 0;
                for shape in output.shapes {
                    match shape.shape {
                        egui::Shape::Rect(rect) if rect.rect.size() == egui::vec2(34., 34.) => {
                            buttons += 1;
                            assert!(
                                rect.rect.right() <= width + 1.,
                                "button overflow at {width}: {:?}",
                                rect.rect
                            );
                        }
                        egui::Shape::Text(text) => {
                            let label = &text.galley.job.text;
                            assert!(!label.contains("EXIF: an") && !label.contains("1 / 1"));
                        }
                        _ => {}
                    }
                }
                assert!(
                    buttons >= 10,
                    "all eight actions and two audio buttons must be visible: {buttons}"
                );
            }
            let point = reveal_point.unwrap();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 700.),
                    )),
                    events: vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ctx| suite.media_viewer(ctx, &[0], &mut Vec::new()),
            );
            assert_eq!(
                *revealed.0.lock().unwrap(),
                vec![suite.items[0].path().to_owned()]
            );
        }
    }
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
    fn controls_keep_two_rows_per_group_and_stable_height_across_media_states() {
        for width in [280., 360., 600., 950.] {
            let mut expected_height: Option<f32> = None;
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
                                    |_| false,
                                );
                                assert!(
                                    !actions.previous && !actions.next && actions.command.is_none()
                                );
                                height = ui.cursor().top() - top;
                            });
                        },
                    );
                    assert!(
                        height < if width < 576. { 190. } else { 110. },
                        "Control groups exceed responsive row height: {height}"
                    );
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
