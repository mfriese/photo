//! Date-editing widgets only; persistence and calendar validation live behind application ports.
use super::Suite;
use crate::{
    domain::{CaptureTime, Media, tags::LibraryFile},
    theme,
};
use eframe::egui;
pub(super) struct Editor {
    media: Media,
    original: Option<CaptureTime>,
    year: u32,
    month: u32,
    day: u32,
    saving: bool,
    error: String,
}
impl Suite {
    pub(super) fn open_date_correction(&mut self, media: &Media) {
        let original = self
            .media_dates
            .get(media)
            .and_then(|r| r.as_ref().ok())
            .cloned();
        let (year, month, day) = original.as_ref().map(parts).unwrap_or((2000, 1, 1));
        self.date_editor = Some(Editor {
            media: media.clone(),
            original,
            year,
            month,
            day,
            saving: false,
            error: String::new(),
        });
    }
    pub(super) fn poll_date_corrections(&mut self, ctx: &egui::Context) {
        if self.audio_library.complete()
            && self.date_projection_revision != self.audio_library.revision()
            && !self.date_corrections.busy()
        {
            self.date_projection_revision = self.audio_library.revision();
            self.date_corrections.refresh();
        }
        if self.date_corrections.poll() {
            let saved = self.date_editor.as_ref().is_some_and(|e| e.saving);
            if !self.date_corrections.error.is_empty() {
                self.status = self.date_corrections.error.clone();
                if let Some(editor) = &mut self.date_editor {
                    editor.saving = false;
                    editor.error = self.date_corrections.error.clone();
                }
            } else if let Some(root) = self.root.clone() {
                for media in &mut self.items {
                    *media = photosuite_application::date_correction::project(
                        &root,
                        &self.date_corrections.catalog,
                        media.clone(),
                    );
                }
                self.media_dates.clear();
                self.tag_view.options_cache = None;
                self.place_options = None;
                if saved {
                    self.date_editor = None;
                    self.diary.cancel();
                    let playback_error = self
                        .services
                        .audio_player
                        .control(photosuite_application::diary::AudioControl::Stop)
                        .err();

                    self.diary_catalog.cancel();
                    self.audio_preparation.reset();
                    self.audio_header = Default::default();
                    self.viewer_audio = Default::default();
                    self.comments.load_cached(root.clone());
                    self.audio_library.open(&root);
                    self.transcription_seen = None;
                    self.status = playback_error.map(|error|format!("Mediendatum gespeichert; Wiedergabe konnte nicht beendet werden: {error}")).unwrap_or_else(||"Mediendatum gespeichert · Original unverändert".into());
                }
            }
        }
        if self.date_corrections.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
    pub(super) fn date_correction_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.date_editor.take() else {
            return;
        };
        if editor.original.is_none()
            && let Some(result) = self.media_dates.get(&editor.media)
        {
            match result {
                Ok(date) => {
                    let (year, month, day) = parts(date);
                    editor.year = year;
                    editor.month = month;
                    editor.day = day;
                    editor.original = Some(date.clone());
                }
                Err(error) => editor.error = error.clone(),
            }
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Datum korrigieren")
            .id(egui::Id::new("photo_date_correction"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(340.)
            .show(ctx, |ui| {
                ui.add(
                    egui::Label::new(
                        editor
                            .media
                            .path()
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                    )
                    .truncate(),
                );
                ui.add_space(10.);
                ui.add_enabled_ui(!editor.saving && editor.original.is_some(), |ui| {
                    ui.horizontal(|ui| {
                        for (label, value, range) in [
                            ("Jahr", &mut editor.year, 1..=9999),
                            ("Monat", &mut editor.month, 1..=12),
                            ("Tag", &mut editor.day, 1..=31),
                        ] {
                            ui.vertical(|ui| {
                                ui.label(label);
                                ui.add(egui::DragValue::new(value).range(range).speed(1.));
                            });
                        }
                    });
                });
                ui.add_space(8.);
                if let Some(date) = &editor.original {
                    ui.small(format!("Uhrzeit bleibt {}", date.time_label()));
                } else {
                    ui.small("Aufnahmedatum wird gelesen …");
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
                ui.small("Nur Medien-YAML · Originaldatei und Metadaten bleiben unverändert.");
                if editor.media.is_photo() {
                    ui.small("Audiokommentare folgen dem korrigierten Fotodatum.");
                } else {
                    ui.small("Tags und Transkripte bleiben erhalten.");
                }
                let date = editor.original.as_ref().map(|date| {
                    photosuite_application::date_correction::replace_date(
                        date,
                        editor.year,
                        editor.month,
                        editor.day,
                    )
                });
                if let Some(Err(error)) = &date {
                    ui.colored_label(theme::DANGER, error);
                }
                if !editor.error.is_empty() {
                    ui.colored_label(theme::DANGER, &editor.error);
                }
                ui.add_space(12.);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!editor.saving, egui::Button::new("Abbrechen"))
                        .clicked()
                    {
                        cancel = true;
                    }
                    if ui
                        .add_enabled(
                            !editor.saving && date.as_ref().is_some_and(|r| r.is_ok()),
                            egui::Button::new("Übernehmen"),
                        )
                        .clicked()
                    {
                        apply = true;
                    }
                    if editor.saving {
                        ui.spinner();
                    }
                });
            });
        if apply {
            let result = (|| {
                let root = self.root.as_ref().ok_or("Keine Bibliothek geöffnet")?;
                let file = editor
                    .media
                    .path()
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("Ungültiger Medienpfad")?;
                let date = photosuite_application::date_correction::replace_date(
                    editor.original.as_ref().ok_or("Mediendatum fehlt")?,
                    editor.year,
                    editor.month,
                    editor.day,
                )?;
                self.date_corrections
                    .save(LibraryFile::new(file.into())?, date)
            })();
            match result {
                Ok(()) => editor.saving = true,
                Err(error) => editor.error = error,
            }
        }
        if (open && !cancel) || editor.saving {
            self.date_editor = Some(editor);
        }
    }
}
fn parts(date: &CaptureTime) -> (u32, u32, u32) {
    (
        date.year().parse().unwrap(),
        date.month().parse().unwrap(),
        date.stamp()[6..8].parse().unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_command_targets_the_clicked_photo_without_opening_the_viewer() {
        let ctx = egui::Context::default();
        let mut suite = super::super::layout_tests::test_suite();
        suite.items = (1..=2)
            .map(|day| {
                Media::from_library_path(
                    format!("/photos/2024/01/img_202401{day:02}_120000.jpg").into(),
                    "2024/01".into(),
                )
                .unwrap()
            })
            .collect();
        suite.execute_for(crate::commands::Command::CorrectDate, 1, &ctx);
        assert!(suite.selected.is_none());
        assert_eq!(
            suite.date_editor.as_ref().unwrap().media.path(),
            suite.items[1].path()
        );
    }
    #[test]
    fn video_context_command_opens_the_shared_date_editor_without_starting_playback() {
        let ctx = egui::Context::default();
        let mut suite = super::super::layout_tests::test_suite();
        suite.items = vec![
            Media::from_library_path(
                "/photos/2024/02/img_20240229_035811.mp4".into(),
                "2024/02".into(),
            )
            .unwrap(),
        ];
        suite.execute_for(crate::commands::Command::CorrectDate, 0, &ctx);
        assert!(suite.selected.is_none());
        let editor = suite.date_editor.as_ref().unwrap();
        assert!(editor.media.is_video());
        assert_eq!((editor.year, editor.month, editor.day), (2024, 2, 29));
    }
    #[test]
    fn date_editor_fits_small_windows_and_exposes_calendar_fields_and_original_protection() {
        for width in [360., 600., 1280.] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let mut suite = super::super::layout_tests::test_suite();
            let media = Media::from_library_path(
                "/photos/2024/scans/img_20240229_035811.jpg".into(),
                "2024/scans".into(),
            )
            .unwrap();
            suite.open_date_correction(&media);
            for frame in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 400.),
                        )),
                        ..Default::default()
                    },
                    |ctx| suite.date_correction_dialog(ctx),
                );
                if frame < 2 {
                    continue;
                }
                let text = output
                    .shapes
                    .iter()
                    .filter_map(|s| {
                        if let egui::Shape::Text(t) = &s.shape {
                            Some(t)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                for expected in [
                    "Datum korrigieren",
                    "Jahr",
                    "Monat",
                    "Tag",
                    "Übernehmen",
                    "Abbrechen",
                ] {
                    assert!(
                        text.iter().any(|t| t.galley.text() == expected),
                        "missing {expected}"
                    );
                }
                for t in text {
                    let rect = t.visual_bounding_rect();
                    assert!(
                        rect.right() <= width + 1. && rect.bottom() <= 400.,
                        "clipped at {width}: {} {rect:?}",
                        t.galley.text()
                    );
                }
            }
            let editor = suite.date_editor.as_ref().unwrap();
            assert_eq!((editor.year, editor.month, editor.day), (2024, 2, 29));
            assert_eq!(editor.original.as_ref().unwrap().time_label(), "03:58:11");
        }
    }
}
