//! Compact rendering of the existing shared transport; no decoding or filesystem access.
use crate::domain::diary_timeline::DiaryTimeline;
use crate::theme;
use eframe::egui;
use photosuite_application::diary::{AudioControl, AudioTransportSnapshot, VerificationProgress};
#[derive(Default)]
pub(super) struct AudioHeader {
    pub draft: photosuite_application::diary::GalleryAudio,
    drag: Option<f64>,
}
#[derive(Default)]
struct Actions {
    start: bool,
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
    let mut actions = Actions::default();
    let width = ui.available_width();
    ui.horizontal(|ui| {
        ui.set_height(26.);
        ui.spacing_mut().item_spacing.x = 6.;
        let caption = if state.active && state.playing {
            "⏸"
        } else {
            "▶"
        };
        if ui
            .add_enabled(
                !preparing && !state.failed && (state.active || can_start),
                egui::Button::new(caption).min_size(egui::vec2(28., 22.)),
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
        let bar_width = (width - 28. - 6.).max(1.);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(bar_width, 22.), egui::Sense::click_and_drag());
        let track = rect.shrink2(egui::vec2(0., 4.));
        ui.painter().rect_filled(track, 2., theme::RAISED);
        let mut hover = None;
        if let Some(timeline) = timeline {
            let total: f64 = timeline.clips().iter().map(|c| c.weight()).sum();
            let mut offset = 0.;
            for (index, clip) in timeline.clips().iter().enumerate() {
                let left = track.left() + track.width() * (offset / total) as f32;
                offset += clip.weight();
                let right = track.left() + track.width() * (offset / total) as f32;
                let color = if progress.errors.contains_key(clip.path()) {
                    egui::Color32::from_rgb(166, 64, 51)
                } else if !clip.is_verified() {
                    egui::Color32::from_rgb(71, 77, 87)
                } else if index % 2 == 1 {
                    egui::Color32::from_rgb(92, 140, 201)
                } else {
                    egui::Color32::from_rgb(59, 102, 166)
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
                            format!("{} {}", crate::audio_labels::kind_label(t.kind()), t.year())
                        })
                        .unwrap_or_else(|| "Keine Aufnahmen".into())
                })
        };
        let help = format!(
            "{date}\n{}\nGeprüft: {} / {} · {} Fehler",
            hover.unwrap_or_else(|| {
                "Jahresleiste · Abschnittsbreiten proportional zur Dauer, grau noch vorläufig"
                    .into()
            }),
            progress.checked,
            progress.total,
            progress.errors.len()
        );
        response.on_hover_text(help);
    });
    actions
}
impl super::Suite {
    pub(super) fn compact_audio(&mut self, ui: &mut egui::Ui) {
        let Some(root) = &self.root else {
            return;
        };
        let group = self.group.as_deref().unwrap_or("");
        let state = self.services.audio_player.transport();
        if !state.active
            && photosuite_application::diary::gallery_year(group).is_none()
            && !self.diary.is_loading()
        {
            return;
        }
        self.audio_header.draft.ensure(&self.items, root, group);
        let preparing = self.diary.is_loading() && self.diary.timeline().is_none();
        let timeline = if state.active {
            self.diary.timeline().or(self.audio_header.draft.timeline())
        } else {
            self.audio_header.draft.timeline()
        };
        let actions = controls(
            ui,
            timeline,
            &state,
            self.diary.progress(),
            preparing,
            self.audio_header.draft.selected().is_some() && !self.diary.is_loading(),
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
        if state.active || self.diary.is_loading() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    pub(super) fn start_audio_for_view(&mut self) {
        let Some(root) = &self.root else {
            return;
        };
        let group = self.group.as_deref().unwrap_or("");
        self.audio_header.draft.ensure(&self.items, root, group);
        if let Some(path) = self.audio_header.draft.selected()
            && let Some(selected) = self.items.iter().find(|media| media.path() == path)
            && let Err(error) = self.diary.start_embedded(
                &self.items,
                selected,
                self.services.audio_source.clone(),
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
}
