//! Compact rendering of the existing shared transport; no decoding or filesystem access.
use crate::domain::diary_timeline::DiaryTimeline;
use crate::theme;
use eframe::egui;
use photosuite_application::diary::{AudioControl, AudioTransportSnapshot, VerificationProgress};
#[derive(Default)]
pub(super) struct AudioHeader {
    pub draft: photosuite_application::diary::GalleryAudio,
    drag: Option<f64>,
    photo: Option<std::path::PathBuf>,
    followed_scope: Option<String>,
    year_draft: photosuite_application::diary::GalleryAudio,
}
#[derive(Default)]
struct Actions {
    start: bool,
    window: bool,
    record: bool,
    control: Option<AudioControl>,
}
fn controls(
    ui: &mut egui::Ui,
    timeline: Option<&DiaryTimeline>,
    state: &AudioTransportSnapshot,
    progress: &VerificationProgress,
    preparing: bool,
    can_start: bool,
    drag: &mut Option<f64>,
) -> Actions {
    controls_sized(
        ui,
        timeline,
        state,
        progress,
        drag,
        ControlStyle {
            preparing,
            can_start,
            can_window: state.active || can_start,
            size: egui::vec2(28., 22.),
            record: None,
        },
    )
}
struct ControlStyle {
    preparing: bool,
    can_start: bool,
    can_window: bool,
    size: egui::Vec2,
    record: Option<bool>,
}
fn controls_sized(
    ui: &mut egui::Ui,
    timeline: Option<&DiaryTimeline>,
    state: &AudioTransportSnapshot,
    progress: &VerificationProgress,
    drag: &mut Option<f64>,
    style: ControlStyle,
) -> Actions {
    let ControlStyle {
        preparing,
        can_start,
        can_window,
        size: button_size,
        record,
    } = style;
    let mut actions = Actions::default();
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(width, button_size.y.max(26.)),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.;
            let caption = if state.active && state.playing {
                "⏸"
            } else {
                "▶"
            };
            if theme::transport_button(
                ui,
                !preparing && !state.failed && (state.active || can_start),
                button_size,
                caption,
            )
            .on_hover_text(if state.playing {
                "Wiedergabe pausieren"
            } else {
                "Wiedergabe starten"
            })
            .clicked()
            {
                if state.active && !preparing && timeline.is_some() {
                    actions.control = Some(AudioControl::Toggle);
                } else {
                    actions.start = true;
                }
            }
            if let Some(enabled) = record {
                actions.record = super::icons::button(
                    ui,
                    super::icons::Icon::Record,
                    enabled,
                    "Audiokommentar zu diesem Foto aufnehmen",
                )
                .clicked();
            }
            let button_count = if record.is_some() { 3. } else { 2. };
            let bar_width = (width - button_size.x * button_count - 6. * button_count).max(1.);
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(bar_width, button_size.y),
                egui::Sense::click_and_drag(),
            );
            let track = rect.shrink2(egui::vec2(0., 4.));
            ui.painter().rect_filled(
                track,
                2.,
                if state.active || can_start {
                    theme::RAISED
                } else {
                    theme::DISABLED_BG
                },
            );
            let mut hover = None;
            if let Some(timeline) = timeline.filter(|_| state.active || can_start) {
                let total: f64 = timeline.clips().iter().map(|c| c.weight()).sum();
                let mut offset = 0.;
                for (index, clip) in timeline.clips().iter().enumerate() {
                    let left = track.left() + track.width() * (offset / total) as f32;
                    offset += clip.weight();
                    let right = track.left() + track.width() * (offset / total) as f32;
                    let color = if progress.errors.contains_key(clip.path()) {
                        theme::AUDIO_ERROR
                    } else if !clip.is_verified() {
                        theme::AUDIO_PENDING
                    } else if index % 2 == 1 {
                        theme::AUDIO_ODD
                    } else {
                        theme::AUDIO_EVEN
                    };
                    ui.painter().rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(left, track.top()),
                            egui::pos2(right, track.bottom()),
                        ),
                        0.,
                        color,
                    );
                    if right - left > 3. {
                        ui.painter().line_segment(
                            [
                                egui::pos2(left, track.top()),
                                egui::pos2(left, track.bottom()),
                            ],
                            egui::Stroke::new(1_f32, theme::TEXT),
                        );
                    }
                    if response
                        .hover_pos()
                        .is_some_and(|p| p.x >= left && p.x <= right)
                    {
                        hover = Some(format!(
                            "{} · {}\n{}",
                            clip.captured()
                                .map(|d| format!("{} {}", d.date_label(), d.time_label()))
                                .unwrap_or_else(|| "Datum wird geprüft".into()),
                            if clip.is_verified() {
                                "geprüft"
                            } else if progress.errors.contains_key(clip.path()) {
                                "Prüfung fehlgeschlagen"
                            } else {
                                "vorläufig"
                            },
                            clip.path().display()
                        ));
                    }
                }
            }
            if state.active && !preparing && timeline.is_some() {
                if response.dragged()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    *drag = Some(((pos.x - track.left()) / track.width()).clamp(0., 1.) as f64);
                }
                if response.clicked() || response.drag_stopped() {
                    let fraction = response
                        .interact_pointer_pos()
                        .map(|p| ((p.x - track.left()) / track.width()).clamp(0., 1.) as f64)
                        .or(*drag);
                    if let Some(fraction) = fraction {
                        actions.control = Some(AudioControl::Seek(fraction));
                    }
                    *drag = None;
                }
                let fraction = drag.unwrap_or(state.fraction).clamp(0., 1.);
                let x = track.left() + track.width() * fraction as f32;
                ui.painter().line_segment(
                    [
                        egui::pos2(x, rect.top() + 2.),
                        egui::pos2(x, rect.bottom() - 2.),
                    ],
                    egui::Stroke::new(2_f32, theme::TEXT),
                );
                ui.painter()
                    .circle_filled(egui::pos2(x, rect.top() + 3.), 3., theme::TEXT);
            } else {
                *drag = None;
            }
            let date = if state.failed {
                "Audiofehler · Playerfenster öffnen".into()
            } else {
                state
                    .current_date
                    .as_ref()
                    .map(|d| format!("{} {}", d.date_label(), d.time_label()))
                    .unwrap_or_else(|| {
                        timeline
                            .map(|t| {
                                format!(
                                    "{} {}",
                                    crate::audio_labels::kind_label(t.kind()),
                                    t.year()
                                )
                            })
                            .unwrap_or_else(|| "Keine Aufnahmen".into())
                    })
            };
            let help = format!(
                "{date}\n{}\nGeprüft: {} / {} · {} Fehler",
                hover.unwrap_or_else(|| {
                    "Audioleiste · Abschnittsbreiten proportional zur Dauer, grau noch vorläufig"
                        .into()
                }),
                progress.checked,
                progress.total,
                progress.errors.len()
            );
            response.on_hover_text(help);
            let enabled = !preparing && can_window;
            let open = theme::transport_button(ui, enabled, button_size, "");
            let icon = egui::Rect::from_center_size(open.rect.center(), egui::vec2(13., 11.));
            ui.painter().rect_stroke(
                icon,
                1.,
                egui::Stroke::new(1_f32, theme::transport_symbol(enabled, false)),
                egui::StrokeKind::Inside,
            );
            ui.painter().line_segment(
                [
                    egui::pos2(icon.left(), icon.top() + 3.),
                    egui::pos2(icon.right(), icon.top() + 3.),
                ],
                egui::Stroke::new(1_f32, theme::transport_symbol(enabled, false)),
            );
            open.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    open.enabled(),
                    "Player für diese Auswahl öffnen",
                )
            });
            actions.window = open
                .on_hover_text("Player für diese Auswahl öffnen")
                .clicked();
        },
    );
    actions
}
impl super::Suite {
    pub(super) fn compact_audio(&mut self, ui: &mut egui::Ui) {
        let Some(root) = &self.root else {
            return;
        };
        let year_group =
            photosuite_application::diary::header_audio_group(&self.items, self.group.as_deref());
        let group = year_group.as_str();
        let changed = self.audio_header.draft.ensure(&self.items, root, group);
        if self.selected.is_none()
            && (changed || self.audio_header.followed_scope.as_deref() != Some(group))
        {
            self.audio_header.followed_scope = Some(group.to_owned());
            if let Err(error) = self.diary.follow_view(
                &self.items,
                root,
                self.audio_preparation.source(),
                group,
                self.services.audio_player.as_ref(),
            ) {
                self.status = error;
            }
        }

        if self.selected.is_none() {
            self.audio_preparation
                .ensure(group, self.audio_header.draft.timeline(), changed);
        }
        if let Err(error) = self
            .audio_header
            .draft
            .apply_prepared(&self.audio_preparation)
        {
            self.status = error;
        }
        let live = self.services.audio_player.transport();
        let active_timeline = self
            .diary
            .timeline_for_view(group)
            .filter(|timeline| self.audio_header.draft.accepts_active(timeline));
        let matches = live.active && live.category_filter.is_none() && active_timeline.is_some();
        let state = if matches {
            live
        } else {
            AudioTransportSnapshot::default()
        };
        let preparing = self.diary.preparing_for_view(group);
        let timeline = if matches {
            active_timeline
        } else {
            self.audio_header.draft.timeline()
        };
        let empty_progress = self
            .audio_preparation
            .progress(self.audio_header.draft.timeline());
        let progress = if matches {
            self.diary.progress()
        } else {
            &empty_progress
        };
        if !matches {
            self.audio_header.drag = None;
        }
        let actions = controls(
            ui,
            timeline,
            &state,
            progress,
            preparing,
            self.audio_header.draft.selected().is_some() && !preparing,
            &mut self.audio_header.drag,
        );
        if let Some(command) = actions.control {
            if let Err(error) = self.services.audio_player.control(command) {
                self.status = error;
            }
            if command == AudioControl::Stop {
                self.diary.cancel();
            }
        }
        if actions.start {
            self.start_audio_for_view();
        }
        if actions.window {
            if matches {
                if let Err(error) = self.services.audio_player.control(AudioControl::ShowWindow) {
                    self.status = error;
                }
            } else {
                self.open_audio_for_view();
            }
        }
        if state.active || self.diary.is_loading() || self.audio_preparation.is_loading() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    pub(super) fn compact_photo_audio(
        &mut self,
        ui: &mut egui::Ui,
        photo: &std::path::Path,
        can_record: bool,
    ) -> bool {
        let audio = self.comments.audio_for_photo(photo);
        if self.viewer_audio.photo.as_deref() != Some(photo) {
            self.viewer_audio.photo = Some(photo.to_owned());
            self.viewer_audio.draft = Default::default();
            self.viewer_audio.drag = None;
            // Preserve the year cache when browsing photos in the same year.
        }
        if let Some(root) = &self.root {
            let changed = self
                .viewer_audio
                .draft
                .ensure(&audio.items, root, &audio.group);
            self.audio_preparation.ensure(
                &audio.group,
                self.viewer_audio.draft.timeline(),
                changed,
            );
            if let Err(error) = self
                .viewer_audio
                .draft
                .apply_prepared(&self.audio_preparation)
            {
                self.status = error;
            }
        }
        let live = self.services.audio_player.transport();
        let active = self
            .diary
            .timeline_for_view(&audio.group)
            .filter(|t| self.viewer_audio.draft.accepts_active(t));
        let matches = live.active && live.category_filter.is_none() && active.is_some();
        let state = if matches {
            live
        } else {
            AudioTransportSnapshot::default()
        };
        let selected_period =
            photosuite_application::diary::header_audio_group(&self.items, self.group.as_deref());
        let year = if selected_period.is_empty() {
            audio.year.clone()
        } else {
            selected_period
        };
        if let Some(root) = &self.root {
            self.viewer_audio
                .year_draft
                .ensure(&self.items, root, &year);
        }
        let preparing = self.diary.preparing_for_view(&audio.group);
        let timeline = if matches {
            active
        } else {
            self.viewer_audio.draft.timeline()
        };
        let empty = self
            .audio_preparation
            .progress(self.viewer_audio.draft.timeline());
        if !matches {
            self.viewer_audio.drag = None;
        }
        let actions = controls_sized(
            ui,
            timeline,
            &state,
            if matches {
                self.diary.progress()
            } else {
                &empty
            },
            &mut self.viewer_audio.drag,
            ControlStyle {
                preparing,
                can_start: self.viewer_audio.draft.selected().is_some() && !preparing,
                can_window: self.viewer_audio.year_draft.selected().is_some(),
                size: egui::vec2(34., 34.),
                record: Some(can_record),
            },
        );
        if let Some(command) = actions.control
            && let Err(error) = self.services.audio_player.control(command)
        {
            self.status = error;
        }
        if actions.start
            && let Some(path) = self.viewer_audio.draft.selected()
            && let Some(selected) = audio.items.iter().find(|m| m.path() == path)
            && let Err(error) = self.diary.request_embedded(
                &audio.items,
                selected,
                self.audio_preparation.source(),
                &audio.group,
            )
        {
            self.status = error;
        }
        if actions.window {
            let year_state = self.services.audio_player.transport();
            let year_is_live = year_state.active
                && self
                    .diary
                    .timeline_for_view(&year)
                    .is_some_and(|timeline| self.viewer_audio.year_draft.accepts_active(timeline));
            if year_is_live {
                if let Err(error) = self.services.audio_player.control(AudioControl::ShowWindow) {
                    self.status = error;
                }
            } else {
                let selected = self
                    .viewer_audio
                    .draft
                    .selected()
                    .and_then(|path| audio.items.iter().find(|m| m.path() == path))
                    .or_else(|| {
                        self.viewer_audio
                            .year_draft
                            .selected()
                            .and_then(|path| self.items.iter().find(|m| m.path() == path))
                    });
                if let Some(selected) = selected
                    && let Err(error) = self.diary.request_window(
                        &self.items,
                        selected,
                        self.audio_preparation.source(),
                        &year,
                    )
                {
                    self.status = error;
                }
            }
        }
        if state.active || self.diary.is_loading() || self.audio_preparation.is_loading() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        actions.record
    }
    pub(super) fn start_audio_for_view(&mut self) {
        let Some(root) = &self.root else {
            return;
        };
        let year_group =
            photosuite_application::diary::header_audio_group(&self.items, self.group.as_deref());
        let group = year_group.as_str();
        self.audio_header.draft.ensure(&self.items, root, group);
        let state = self.services.audio_player.transport();
        let same_year = state.active
            && self
                .diary
                .timeline_for_view(group)
                .is_some_and(|timeline| self.audio_header.draft.accepts_active(timeline));
        if same_year && state.category_filter.is_some() {
            if let Err(error) = self.services.audio_player.control(AudioControl::PlayAll) {
                self.status = error;
            }
            return;
        }
        if let Some(path) = self.audio_header.draft.selected()
            && let Some(selected) = self.items.iter().find(|media| media.path() == path)
            && let Err(error) = self.diary.request_embedded(
                &self.items,
                selected,
                self.audio_preparation.source(),
                group,
            )
        {
            self.status = error;
        }
    }
    fn open_audio_for_view(&mut self) {
        let Some(root) = &self.root else {
            return;
        };
        let year_group =
            photosuite_application::diary::header_audio_group(&self.items, self.group.as_deref());
        let group = year_group.as_str();
        self.audio_header.draft.ensure(&self.items, root, group);
        if self.services.audio_player.transport().active
            && self
                .diary
                .timeline_for_view(group)
                .is_some_and(|timeline| self.audio_header.draft.accepts_active(timeline))
        {
            if let Err(error) = self.services.audio_player.control(AudioControl::ShowWindow) {
                self.status = error;
            }
            return;
        }
        if let Some(path) = self.audio_header.draft.selected()
            && let Some(selected) = self.items.iter().find(|media| media.path() == path)
            && let Err(error) = self.diary.request_window(
                &self.items,
                selected,
                self.audio_preparation.source(),
                group,
            )
        {
            self.status = error;
        }
    }
    pub(super) fn page_heading(&mut self, ui: &mut egui::Ui, title: &str) {
        let width = ui.available_width();
        ui.horizontal(|ui| {
            ui.set_height(26.);
            let label_width = (width * 0.32).clamp(44., 180.);
            ui.add_sized(
                [label_width, 26.],
                egui::Label::new(egui::RichText::new(title).size(18.).strong()).truncate(),
            )
            .on_hover_text(title);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width().max(1.), 26.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| self.compact_audio(ui),
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_stays_one_compact_row_in_narrow_and_wide_layouts() {
        use crate::domain::diary_timeline::DiaryClip;
        let timeline = DiaryTimeline::new(
            "2023".into(),
            vec![DiaryClip::from_filename("aud_20230101_120000.wav".into())],
        )
        .unwrap();
        for width in [190., 360., 800.] {
            for active in [false, true] {
                let ctx = egui::Context::default();
                theme::install(&ctx);
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    ctx.fonts_mut(|fonts| {
                        let font = egui::FontId::proportional(14.);
                        for symbol in ['▶', '⏸'] {
                            assert!(
                                fonts.has_glyph(&font, symbol),
                                "missing transport glyph {symbol}"
                            );
                        }
                    });
                });
                let state = AudioTransportSnapshot {
                    active,
                    playing: active,
                    fraction: 0.5,
                    ..Default::default()
                };
                for _ in 0..2 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 100.),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                let before = ui.cursor().top();
                                controls(
                                    ui,
                                    Some(&timeline),
                                    &state,
                                    &VerificationProgress::default(),
                                    false,
                                    true,
                                    &mut None,
                                );
                                assert!(
                                    ui.cursor().top() - before <= 38.,
                                    "header grew vertically at {width}"
                                );
                            });
                        },
                    );
                    for shape in output.shapes {
                        if let egui::Shape::Text(text) = shape.shape {
                            if text.galley.job.text.is_empty() {
                                continue;
                            }
                            theme::assert_text_glyphs(&ctx, &text);
                            assert!(
                                text.visual_bounding_rect().right() <= width + 1.,
                                "text overflow at {width}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn right_hand_window_symbol_opens_only_when_the_current_scope_has_audio() {
        use crate::domain::diary_timeline::DiaryClip;
        let timeline = DiaryTimeline::new(
            "2008".into(),
            vec![DiaryClip::from_filename("aud_20080901_120000.wav".into())],
        )
        .unwrap();
        for available in [true, false] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let input = |events| egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400., 100.),
                )),
                events,
                ..Default::default()
            };
            let output = ctx.run(input(vec![]), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    controls(
                        ui,
                        Some(&timeline),
                        &AudioTransportSnapshot::default(),
                        &VerificationProgress::default(),
                        false,
                        available,
                        &mut None,
                    );
                });
            });
            let expected = theme::transport_symbol(available, false);
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect.size() == egui::vec2(13., 11.) && rect.stroke.color == expected)));
            let icon = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.rect.size() == egui::vec2(13., 11.) => {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .unwrap();
            assert!(icon.center().x > 350.);
            let point = icon.center();
            let mut opened = false;
            let _ = ctx.run(
                input(vec![
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
                ]),
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        opened = controls(
                            ui,
                            Some(&timeline),
                            &AudioTransportSnapshot::default(),
                            &VerificationProgress::default(),
                            false,
                            available,
                            &mut None,
                        )
                        .window;
                    });
                },
            );
            assert_eq!(opened, available);
        }
    }
}
