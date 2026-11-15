//! Compact presentation of the shared comment session. No second recorder or persistence path.
use super::{
    Suite, comments,
    icons::{self, Icon},
};
use eframe::egui;
use photosuite_application::comments::{
    CaptureCommand, CapturePhase, CaptureSnapshot, PhotoTarget,
};
impl Suite {
    pub(super) fn inline_recording_row(
        &mut self,
        ui: &mut egui::Ui,
        photo: &std::path::Path,
        can_record: bool,
    ) -> bool {
        let target = self.root.as_ref().and_then(|root| {
            self.organization.file(photo).ok().map(|file| PhotoTarget {
                root: root.clone(),
                file,
            })
        });
        let owns = target
            .as_ref()
            .is_some_and(|t| self.comments.target() == Some(t));
        let snapshot = if owns {
            self.comments.snapshot().clone()
        } else {
            CaptureSnapshot::default()
        };
        let phase = snapshot.phase;
        let busy = self.comments.is_saving() || self.comments.is_save_pending();
        let foreign_take = self.comments.has_unsaved() && !owns;
        let enabled = can_record
            && target.is_some()
            && self.comments.is_loaded()
            && !busy
            && !foreign_take
            && !matches!(phase, CapturePhase::Preparing | CapturePhase::Finishing);
        let seekable = owns && matches!(phase, CapturePhase::Paused | CapturePhase::Ready) && !busy;
        let idle = !self.comments.has_unsaved() && !busy;
        let width = ui.available_width();
        let mut toggle = false;
        let mut save = false;
        let mut open = false;
        let mut seek = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.;
            let tip = if phase == CapturePhase::Recording {
                "Aufnahme pausieren"
            } else if phase == CapturePhase::Paused {
                "Aufnahme ab der Markierung fortsetzen"
            } else {
                "Audiokommentar zu diesem Foto aufnehmen"
            };
            let record =
                comments::transport_button(ui, phase == CapturePhase::Recording, enabled, false);
            record
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip));
            toggle = record.on_hover_text(tip).clicked();
            let saving_action = owns && self.comments.has_unsaved();
            let gap_count = if saving_action { 5. } else { 4. };
            let content =
                (width - 68. - if saving_action { 34. } else { 0. } - gap_count * 6.).max(1.);
            let wave = (content * 0.48).max(1.);
            let microphone = (content * 0.30).clamp(24., 160.);
            let meter = (content - wave - microphone).max(1.);
            ui.allocate_ui_with_layout(
                egui::vec2(wave, 34.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_width(wave);
                    seek = comments::recording_waveform_sized(ui, &snapshot, seekable, 34.);
                },
            );
            ui.allocate_ui_with_layout(
                egui::vec2(meter, 34.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_width(meter);
                    comments::compact_level(ui, snapshot.level, snapshot.peak);
                },
            );
            if saving_action {
                save = icons::button(ui, Icon::Save, !busy, "Aufnahme beenden und speichern")
                    .clicked();
            }
            ui.allocate_ui_with_layout(
                egui::vec2(microphone, 34.),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.set_width(microphone);
                    comments::microphone_picker(ui, &mut self.comments, can_record && idle);
                },
            );
            open =
                icons::button(ui, Icon::Microphone, can_record, "Aufnahmedialog öffnen").clicked();
        });
        if toggle && let Some(target) = target {
            match self
                .comments
                .toggle_photo_recording(target, self.services.audio_player.as_ref())
            {
                Ok(()) => self.comment_view.inline = true,
                Err(error) => self.comments.report_error(error),
            }
        }
        if let Some(position) = seek {
            self.comments.command(CaptureCommand::Seek(position));
        }
        if save {
            self.request_inline_save();
        }
        if !self.comments.error().is_empty() && owns {
            // Keep the two-row layout stable; detailed errors remain accessible in the full dialog.
            ui.interact(
                ui.min_rect(),
                ui.id().with("recording_error"),
                egui::Sense::hover(),
            )
            .on_hover_text(self.comments.error());
        }
        open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photosuite_application::comments::{CommentRecorder, CommentSession, InputDevice};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Default)]
    struct Recorder {
        snapshot: Mutex<CaptureSnapshot>,
        starts: AtomicUsize,
        discards: AtomicUsize,
    }
    impl CommentRecorder for Recorder {
        fn devices(&self) -> Result<Vec<InputDevice>, String> {
            Ok(vec![InputDevice {
                id: "mic".into(),
                name: "Ein sehr langer Mikrofonname für den Layouttest".into(),
                is_default: true,
            }])
        }
        fn start(&self, _: &str) -> Result<(), String> {
            self.starts.fetch_add(1, Ordering::SeqCst);
            *self.snapshot.lock().unwrap() = CaptureSnapshot {
                phase: CapturePhase::Recording,
                id: "owned".into(),
                wave: vec![0.16; 960],
                level: 0.16,
                peak: -12.,
                seconds: 5.,
                ..Default::default()
            };
            Ok(())
        }
        fn command(&self, command: CaptureCommand) -> Result<(), String> {
            match command {
                CaptureCommand::Discard => {
                    self.discards.fetch_add(1, Ordering::SeqCst);
                    *self.snapshot.lock().unwrap() = Default::default();
                }
                CaptureCommand::Pause => self.snapshot.lock().unwrap().phase = CapturePhase::Paused,
                CaptureCommand::Resume => {
                    self.snapshot.lock().unwrap().phase = CapturePhase::Recording
                }
                CaptureCommand::Finish => {
                    let mut snapshot = self.snapshot.lock().unwrap();
                    snapshot.phase = CapturePhase::Ready;
                    snapshot.take = Some(photosuite_application::comments::CapturedTake {
                        id: format!(
                            "12345678-1234-1234-1234-{:012}",
                            self.starts.load(Ordering::SeqCst)
                        ),
                        path: "/tmp/simulated.m4a".into(),
                        recorded_at: "2026-11-15T12:00:00+01:00".into(),
                    });
                }
                _ => {}
            }
            Ok(())
        }
        fn snapshot(&self) -> CaptureSnapshot {
            self.snapshot.lock().unwrap().clone()
        }
    }
    impl photosuite_application::comments::CommentMediaStore for Recorder {
        fn validate(
            &self,
            _: &PhotoTarget,
            _: &photosuite_application::comments::CapturedTake,
        ) -> Result<(), String> {
            Ok(())
        }
        fn exists(&self, _: &std::path::Path) -> Result<bool, String> {
            Ok(false)
        }
        fn copy_verified(&self, _: &std::path::Path, _: &std::path::Path) -> Result<(), String> {
            Ok(())
        }
    }
    #[test]
    fn inline_capture_preserves_take_in_dialog_and_blocks_unconfirmed_navigation_without_microphone()
     {
        let mut suite = super::super::layout_tests::test_suite();
        let recorder = Arc::new(Recorder::default());
        suite.root = Some("/Fotos".into());
        suite.organization.open("/Fotos".into()).unwrap();
        suite.comments = CommentSession::new(
            suite.services.comments.clone(),
            recorder.clone(),
            suite.services.comment_media.clone(),
        );
        suite.comments.load("/Fotos".into());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !suite.comments.is_loaded() {
            suite.comments.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let photo = std::path::Path::new("/Fotos/2008/09/img_20080916_035811.jpg");
        suite
            .items
            .push(crate::domain::Media::from_path(photo.into(), "2008/09".into()).unwrap());
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        ctx.style_mut(|s| s.animation_time = 0.);
        let mut record = None;
        for _ in 0..3 {
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(500.);
                    suite.inline_recording_row(ui, photo, true);
                });
            });
            record = output.shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Circle(c) if c.fill == crate::theme::RECORD_RED => Some(c.center),
                _ => None,
            });
        }
        let point = record.expect("enabled inline recording");
        assert_eq!(
            recorder.starts.load(Ordering::SeqCst),
            0,
            "rendering must not open microphone"
        );
        for pressed in [true, false] {
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.set_width(500.);
                        suite.inline_recording_row(ui, photo, true);
                    });
                },
            );
        }
        suite.poll_comments(&ctx);
        assert!(suite.comment_view.inline);
        assert!(suite.comments.has_unsaved());
        assert_eq!(recorder.starts.load(Ordering::SeqCst), 1);
        // All controls including save/microphone stay inside the row while recording.
        for width in [260., 500., 800.] {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width + 16., 500.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.set_width(width);
                        suite.inline_recording_row(ui, photo, true);
                    });
                },
            );
            for s in output.shapes {
                if let egui::Shape::Rect(r) = s.shape
                    && r.rect.size() == egui::vec2(34., 34.)
                {
                    assert!(
                        r.rect.right() <= width + 9.,
                        "width={width} overflow={:?}",
                        r.rect
                    );
                }
            }
        }
        let discards = recorder.discards.load(Ordering::SeqCst);
        suite.record_photo = Some(0);
        let _ = ctx.run(egui::RawInput::default(), |ctx| suite.comment_dialog(ctx));
        assert!(!suite.comment_view.inline);
        assert_eq!(suite.comments.snapshot().phase, CapturePhase::Recording);
        assert_eq!(recorder.discards.load(Ordering::SeqCst), discards);
        suite.comment_view.inline = true;
        assert!(!suite.leave_inline_recording());
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| suite.comment_dialog(ctx));
        }
        let output = ctx.run(egui::RawInput::default(), |ctx| suite.comment_dialog(ctx));
        let texts = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"Speichern") && texts.contains(&"Verwerfen"));
        assert!(suite.comments.has_unsaved());
    }
    #[test]
    fn arrow_navigation_during_recording_saves_old_photo_skips_video_and_starts_next_photo() {
        let mut suite = super::super::layout_tests::test_suite();
        let recorder = Arc::new(Recorder::default());
        suite.root = Some("/Fotos".into());
        suite.organization.open("/Fotos".into()).unwrap();
        suite.comments = CommentSession::new(
            suite.services.comments.clone(),
            recorder.clone(),
            recorder.clone(),
        );
        suite.comments.load("/Fotos".into());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !suite.comments.is_loaded() {
            suite.comments.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        for name in [
            "img_20080916_035811.jpg",
            "img_20080916_035812.mp4",
            "img_20080916_035813.jpg",
        ] {
            suite.items.push(
                crate::domain::Media::from_path(
                    format!("/Fotos/2008/09/{name}").into(),
                    "2008/09".into(),
                )
                .unwrap(),
            );
        }
        let old = suite.organization.file(suite.items[0].path()).unwrap();
        let next = suite.organization.file(suite.items[2].path()).unwrap();
        suite
            .comments
            .toggle_photo_recording(
                PhotoTarget {
                    root: "/Fotos".into(),
                    file: old.clone(),
                },
                suite.services.audio_player.as_ref(),
            )
            .unwrap();
        suite.comments.poll();
        suite.comment_view.inline = true;
        suite.selected = Some(0);
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        ctx.style_mut(|s| s.animation_time = 0.);
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(950., 700.),
                )),
                events: vec![egui::Event::Key {
                    key: egui::Key::ArrowRight,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| suite.media_viewer(ctx, &[0, 1, 2], &mut Vec::new()),
        );
        assert_eq!(
            suite.selected,
            Some(0),
            "photo remains until the old comment is committed"
        );
        assert!(suite.comments.is_switching_photo());
        while suite.comments.is_switching_photo() {
            suite.poll_comments(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(suite.selected, Some(2));
        assert_eq!(recorder.starts.load(Ordering::SeqCst), 2);
        assert_eq!(suite.comments.target().unwrap().file, next);
        assert_eq!(suite.comments.catalog().for_photo(&old).len(), 1);
        assert!(suite.comments.has_unsaved());
        assert!(suite.comment_view.inline);
    }
}
