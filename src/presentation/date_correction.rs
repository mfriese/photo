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
    hour: u32,
    minute: u32,
    second: u32,
    relocate: bool,
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
            original: original.clone(),
            year,
            month,
            day,
            hour: original
                .as_ref()
                .map(|date| date.time_label()[..2].parse().unwrap())
                .unwrap_or(0),
            minute: original
                .as_ref()
                .map(|date| date.time_label()[3..5].parse().unwrap())
                .unwrap_or(0),
            second: original
                .as_ref()
                .map(|date| date.time_label()[6..8].parse().unwrap())
                .unwrap_or(0),
            relocate: false,
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
            let relocated = self.date_corrections.take_relocated();
            if let (Some(root), Some(moved)) = (self.root.clone(), relocated.as_ref()) {
                let old = root.join(moved.from.as_str());
                let new = root.join(moved.to.as_str());
                if let Some(media) = self.items.iter_mut().find(|media| media.path() == old) {
                    let group = std::path::Path::new(moved.to.as_str())
                        .parent()
                        .unwrap_or(std::path::Path::new(""))
                        .to_string_lossy()
                        .into_owned();
                    if let Some(replacement) = Media::from_library_path(new.clone(), group) {
                        *media = replacement;
                    }
                }
                self.known_media.remove(&old);
                self.known_media.insert(new.clone());
                self.gallery.removed.insert(old.clone());
                self.gallery.selection.relocate(&old, &new);
                self.editor.relocate(&old, &new);
                self.place_filter.relocate(&moved.from, &moved.to);
                self.transcription_queue.media_paths_changed();
                if let Err(error) = self.organization.refresh_relocated(&moved.from, &moved.to) {
                    self.status = error;
                }
                self.storyboard.clear();
            }
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
                    if let Some(path) = relocated
                        .as_ref()
                        .map(|moved| root.join(moved.to.as_str()))
                        .or_else(|| {
                            self.date_editor
                                .as_ref()
                                .map(|editor| editor.media.path().to_owned())
                        })
                    {
                        self.follow_corrected_media(&path);
                    }
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
                    self.status = playback_error.map(|error|format!("Mediendatum gespeichert; Wiedergabe konnte nicht beendet werden: {error}")).unwrap_or_else(||if relocated.is_some() {"Datei verschoben und umbenannt · Metadaten erhalten".into()}else{"Mediendatum gespeichert · Original unverändert".into()});
                }
            }
        }
        if !self.date_corrections.busy() {
            for (root, media) in std::mem::take(&mut self.gallery.deferred) {
                self.accept_collection_media(&root, media);
            }
        }
        if self.date_corrections.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
    fn follow_corrected_media(&mut self, path: &std::path::Path) {
        let Some(media) = self.items.iter().find(|media| media.path() == path) else {
            return;
        };
        self.group = photosuite_application::date_correction::destination_group(
            self.group.as_deref(),
            media,
        );
        // Automatic following preserves filters that already matched this media. Manual navigation still resets them.
        let scope = (self.root.clone(), self.group.clone());
        self.tag_view.filter_scope = Some(scope.clone());
        self.place_scope = Some(scope);
        self.tag_view.options_cache = None;
        self.place_options = None;
        self.scroll_to_photo = Some(path.into());
        self.highlighted_photo = Some(path.into());
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
                    let clock = date.time_label();
                    editor.hour = clock[..2].parse().unwrap();
                    editor.minute = clock[3..5].parse().unwrap();
                    editor.second = clock[6..].parse().unwrap();
                    editor.original = Some(date.clone());
                }
                Err(error) => editor.error = error.clone(),
            }
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        crate::theme::dialog("Datum korrigieren")
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
                            if calendar_field(ui, label, value, range).changed() {
                                editor.error.clear();
                            }
                        }
                    });
                });
                ui.add_enabled_ui(!editor.saving && editor.original.is_some(), |ui| {
                    ui.horizontal(|ui| {
                        for (label, value, maximum) in [
                            ("Stunde", &mut editor.hour, 23),
                            ("Minute", &mut editor.minute, 59),
                            ("Sekunde", &mut editor.second, 60),
                        ] {
                            if calendar_field(ui, label, value, 0..=maximum).changed() {
                                editor.error.clear();
                            }
                        }
                    });
                    ui.add_space(8.);
                    if ui
                        .checkbox(
                            &mut editor.relocate,
                            "Datei auch verschieben und umbenennen",
                        )
                        .changed()
                    {
                        editor.error.clear();
                    }
                });
                ui.add_space(8.);
                if editor.original.is_none() {
                    ui.small("Aufnahmedatum wird gelesen …");
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
                let date = editor.original.as_ref().map(|_| {
                    photosuite_application::date_correction::replace_timestamp(
                        editor.year,
                        editor.month,
                        editor.day,
                        editor.hour,
                        editor.minute,
                        editor.second,
                    )
                });
                if editor.relocate {
                    if let (Some(root), Some(Ok(date))) = (&self.root, &date)
                        && let Ok(relative) = editor.media.path().strip_prefix(root)
                        && let Some(relative) = relative.to_str()
                        && let Ok(file) = LibraryFile::new(relative.into())
                        && let Ok(target) =
                            photosuite_application::date_correction::relocation_target(&file, date)
                    {
                        ui.add(egui::Label::new(format!("Ziel: {}", target.as_str())).wrap());
                    }
                    ui.small("Bei Kollision bleibt alles unverändert. Keine EXIF-Änderung.");
                } else {
                    ui.small("Nur Medien-YAML · Originaldatei und Metadaten bleiben unverändert.");
                }
                if editor.media.is_photo() {
                    ui.small("Audiokommentare folgen dem korrigierten Fotodatum.");
                } else {
                    ui.small("Tags und Transkripte bleiben erhalten.");
                }
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
                            !editor.saving
                                && date.as_ref().is_some_and(|r| r.is_ok())
                                && (!editor.relocate
                                    || (!self.organization.dirty()
                                        && !self.organization.saving()
                                        && !self.comments.busy()
                                        && !self.editor.is_saving()
                                        && !self.index_maintenance.busy()
                                        && !self.tag_view.open)),
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
                let date = photosuite_application::date_correction::replace_timestamp(
                    editor.year,
                    editor.month,
                    editor.day,
                    editor.hour,
                    editor.minute,
                    editor.second,
                )?;
                self.date_corrections.save_with_relocation(
                    LibraryFile::new(file.into())?,
                    date,
                    editor.relocate,
                )
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
fn calendar_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) -> egui::Response {
    ui.vertical(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(value)
                .range(range)
                .speed(theme::DATE_FIELD_DRAG_SPEED),
        )
        .on_hover_text("Langsam ziehen oder Zahl direkt eingeben")
    })
    .inner
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

#[cfg(test)]
mod follow_tests {
    use super::*;
    use photosuite_application::{
        media_geo::{LocatedPhoto, LocationIndex},
        organization::Action,
        place_filter::{Selection, Session},
    };
    use std::{
        path::Path,
        sync::{Arc, atomic::AtomicBool},
    };
    struct Places {
        file: LibraryFile,
    }
    impl LocationIndex for Places {
        fn locations(&self, _: &Path, _: &AtomicBool) -> Result<Vec<LocatedPhoto>, String> {
            Ok(vec![LocatedPhoto {
                photo: self.file.clone(),
                country: "DE".into(),
                country_name: "Deutschland".into(),
                region: None,
                place_id: 1,
                place_name: "Berlin".into(),
            }])
        }
    }
    #[test]
    fn corrected_media_keeps_tags_and_places_visible_and_selected_in_destination() {
        for initial in [Some("2011/11"), Some("2011"), None] {
            let mut suite = super::super::layout_tests::test_suite();
            let root = std::path::PathBuf::from("/memory-only-fixture");
            suite.videos_only = false;
            suite.root = Some(root.clone());
            suite.group = initial.map(str::to_owned);
            suite.organization.open(root.clone()).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while suite.organization.catalog().is_none() {
                suite.organization.poll();
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            let file = LibraryFile::new("2011/11/img_20111111_124844.jpg".into()).unwrap();
            let path = root.join(file.as_str());
            suite.items = vec![Media::from_library_path(path.clone(), "2011/11".into()).unwrap()];
            suite
                .organization
                .apply(Action::Create {
                    name: "urlaub".into(),
                    file: Some(file.clone()),
                })
                .unwrap();
            suite.prepare_tag_filter();
            suite.tag_view.filters.insert("urlaub".into());
            suite.place_filter = Session::new(Arc::new(Places { file }), Arc::default());
            loop {
                suite.place_filter.poll(Some(&root));
                if !suite.place_filter.pending() {
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            suite.place_scope = Some((Some(root.clone()), suite.group.clone()));
            suite
                .place_filter
                .selected
                .insert(Selection::Country("DE".into()));
            assert_eq!(suite.visible(), vec![0]);
            suite.items[0] = suite.items[0].clone().with_capture_correction(Some(
                CaptureTime::parse("1980:07:08 12:48:44", "test").unwrap(),
            ));
            suite.follow_corrected_media(&path);
            suite.prepare_tag_filter();
            let expected = match initial {
                Some("2011") => Some("1980"),
                Some(_) => Some("1980/07"),
                None => None,
            };
            assert_eq!(suite.group.as_deref(), expected);
            assert_eq!(suite.visible(), vec![0]);
            assert!(suite.tag_view.filters.contains("urlaub"));
            assert!(
                suite
                    .place_filter
                    .selected
                    .contains(&Selection::Country("DE".into()))
            );
            assert!(
                suite
                    .tag_view
                    .options_cache
                    .as_ref()
                    .unwrap()
                    .tags
                    .contains(&"urlaub".into())
            );
            let rows = suite.place_filter.catalog().options(
                &suite.items,
                &root,
                &suite.unfiltered_visible(),
            );
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].name, "Deutschland");
            assert_eq!(rows[0].count, 1);
            assert_eq!(suite.scroll_to_photo.as_deref(), Some(path.as_path()));
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            for frame in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800., 700.),
                        )),
                        ..Default::default()
                    },
                    |ctx| suite.library_sidebar(ctx),
                );
                if frame == 2 {
                    for label in ["urlaub", "Deutschland"] {
                        assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()==label)),"missing sidebar entry {label}");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod drag_tests {
    use super::*;
    #[test]
    fn dragging_year_twenty_pixels_changes_it_by_one_instead_of_twenty() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut year = 2024;
        let mut rect = egui::Rect::NOTHING;
        let mut frame = |events: Vec<egui::Event>| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400., 200.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        rect = calendar_field(ui, "Jahr", &mut year, 1..=9999).rect;
                    });
                },
            );
            rect
        };
        let start = frame(vec![]).center();
        frame(vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let end = start + egui::vec2(20., 0.);
        frame(vec![egui::Event::PointerMoved(end)]);
        frame(vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(
            (2025..=2026).contains(&year),
            "drag unexpectedly changed year to {year}"
        );
    }
}

#[cfg(test)]
mod relocation_view_tests {
    use super::*;
    use photosuite_application::{
        date_correction::Session,
        diary::{AudioControl, AudioPlayer, VerificationProgress},
        library_index::Rebuilder,
        media_geo::{LocatedPhoto, LocationIndex},
        place_filter::Selection,
    };
    use std::{
        path::Path,
        sync::{Arc, atomic::AtomicBool},
    };
    struct Player;
    impl AudioPlayer for Player {
        fn open(
            &self,
            _: &crate::domain::diary_timeline::DiaryTimeline,
            _: &Path,
            _: &[Media],
        ) -> Result<(), String> {
            panic!("must not play")
        }
        fn revise(
            &self,
            _: &crate::domain::diary_timeline::DiaryTimeline,
            _: &VerificationProgress,
        ) -> Result<(), String> {
            Ok(())
        }
        fn control(&self, _: AudioControl) -> Result<(), String> {
            Ok(())
        }
    }
    struct Places(LibraryFile);
    impl LocationIndex for Places {
        fn locations(&self, _: &Path, _: &AtomicBool) -> Result<Vec<LocatedPhoto>, String> {
            Ok(vec![LocatedPhoto {
                photo: self.0.clone(),
                country: "DE".into(),
                country_name: "Deutschland".into(),
                region: None,
                place_id: 1,
                place_name: "Berlin".into(),
            }])
        }
    }
    #[test]
    fn completed_transfer_rebinds_viewer_tags_places_rotation_selection_and_deferred_scan() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let database = Arc::new(crate::library_db::Database);
        let old = LibraryFile::new("broken/img_20111111_124844.jpg".into()).unwrap();
        let old_path = root.join(old.as_str());
        std::fs::create_dir_all(old_path.parent().unwrap()).unwrap();
        std::fs::write(&old_path, b"original picture").unwrap();
        let mut record = crate::media_records_fs::Record::new(&old);
        record.tags = Some(["urlaub".into()].into());
        crate::media_records_fs::write(root, &old, &record, None).unwrap();
        crate::library_index_fs::SqliteRebuilder {
            database: database.clone(),
            priority: Arc::default(),
        }
        .rebuild(root, &AtomicBool::new(false), &|_| {})
        .unwrap();
        let mut suite = super::super::layout_tests::test_suite();
        suite.root = Some(root.into());
        suite.group = Some("broken".into());
        suite.videos_only = false;
        suite.items = vec![Media::from_library_path(old_path.clone(), "broken".into()).unwrap()];
        suite.known_media.insert(old_path.clone());
        suite.selected = Some(0);
        suite.gallery.selection.paths.insert(old_path.clone());
        suite.services.audio_player = Arc::new(Player);
        suite.organization = photosuite_application::organization::OrganizationSession::new(
            Arc::new(crate::tag_store::SqliteTags {
                database: database.clone(),
                priority: Arc::default(),
            }),
        );
        suite.organization.open_cached(root.into()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while suite.organization.loading() {
            suite.organization.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        suite.prepare_tag_filter();
        suite.tag_view.filters.insert("urlaub".into());
        suite.place_filter = photosuite_application::place_filter::Session::new(
            Arc::new(Places(old.clone())),
            Arc::default(),
        );
        loop {
            suite.place_filter.poll(Some(root));
            if !suite.place_filter.pending() {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        suite
            .place_filter
            .selected
            .insert(Selection::Country("DE".into()));
        suite.date_corrections =
            Session::new(Some(Arc::new(crate::date_correction_fs::CaptureDates {
                database,
                priority: Arc::default(),
            })));
        suite.date_corrections.open(root);
        let ctx = egui::Context::default();
        while suite.date_corrections.busy() {
            suite.poll_date_corrections(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        suite
            .editor
            .execute(
                crate::commands::Command::RotateRight,
                &suite.items[0],
                false,
                suite.services.orientation_writer.clone(),
                || {},
            )
            .unwrap();
        suite.open_date_correction(&suite.items[0].clone());
        suite.date_editor.as_mut().unwrap().saving = true;
        let date = CaptureTime::parse("1980:07:08 12:48:45", "Korrigiertes Mediendatum").unwrap();
        let new = photosuite_application::date_correction::relocation_target(&old, &date).unwrap();
        let new_path = root.join(new.as_str());
        suite
            .date_corrections
            .save_with_relocation(old, date, true)
            .unwrap();
        suite.accept_collection_media(
            root,
            Media::from_library_path(new_path.clone(), "1980/07".into()).unwrap(),
        );
        assert_eq!(suite.items.len(), 1);
        assert_eq!(suite.gallery.deferred.len(), 1);
        while suite.date_corrections.busy() {
            suite.poll_date_corrections(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(
            suite.date_corrections.error.is_empty(),
            "{}",
            suite.date_corrections.error
        );
        assert!(suite.date_editor.is_none());
        assert!(suite.gallery.deferred.is_empty());
        assert_eq!(suite.items.len(), 1);
        assert_eq!(suite.items[0].path(), new_path);
        assert_eq!(suite.group.as_deref(), Some("1980/07"));
        assert_eq!(suite.selected, Some(0));
        assert_eq!(suite.editor.rotations().get(&new_path), Some(&1));
        assert!(suite.gallery.selection.paths.contains(&new_path));
        assert!(suite.gallery.removed.contains(&old_path));
        assert!(suite.tag_view.filters.contains("urlaub"));
        assert!(
            suite
                .place_filter
                .selected
                .contains(&Selection::Country("DE".into()))
        );
        suite.prepare_tag_filter();
        assert_eq!(suite.visible(), vec![0]);
        let options = suite
            .place_filter
            .catalog()
            .options(&suite.items, root, &[0]);
        assert_eq!(options[0].name, "Deutschland");
        while suite.organization.loading() {
            suite.organization.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(
            suite.organization.catalog().unwrap().tags_for(&new),
            vec!["urlaub"]
        );
    }
}
