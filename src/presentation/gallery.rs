//! Gallery selection and confirmation UI; file operations remain behind the application port.
use crate::{domain::tags::LibraryFile, theme};
use eframe::egui;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
#[derive(Default)]
pub(super) struct Selection {
    pub paths: BTreeSet<PathBuf>,
    anchor: Option<PathBuf>,
}
impl Selection {
    pub fn clear(&mut self) {
        self.paths.clear();
        self.anchor = None;
    }
    pub fn click(&mut self, path: &Path, ordered: &[PathBuf], toggle: bool, range: bool) {
        if range
            && let Some((a, b)) = self
                .anchor
                .as_ref()
                .and_then(|a| ordered.iter().position(|p| p == a))
                .zip(ordered.iter().position(|p| p == path))
        {
            if !toggle {
                self.paths.clear();
            }
            self.paths
                .extend(ordered[a.min(b)..=a.max(b)].iter().cloned());
            return;
        }
        if toggle {
            if !self.paths.remove(path) {
                self.paths.insert(path.into());
            }
        } else {
            self.paths.clear();
            self.paths.insert(path.into());
        }
        self.anchor = Some(path.into());
    }
    pub fn context(&mut self, path: &Path) {
        if !self.paths.contains(path) {
            self.paths.clear();
            self.paths.insert(path.into());
            self.anchor = Some(path.into());
        }
    }
}
type Scope = (
    Option<PathBuf>,
    Option<String>,
    String,
    BTreeSet<String>,
    BTreeSet<photosuite_application::place_filter::Selection>,
);
#[derive(Default)]
pub(super) struct GalleryState {
    pub selection: Selection,
    pub scope: Option<Scope>,
    pub pending: Option<(PathBuf, Vec<LibraryFile>)>,
    pub job: photosuite_application::media_trash::TrashSession,
    pub report: Option<String>,
    pub removed: BTreeSet<PathBuf>,
}
impl super::Suite {
    pub(super) fn prepare_gallery_selection(&mut self) {
        let scope = (
            self.root.clone(),
            self.group.clone(),
            self.query.clone(),
            self.tag_view.filters.clone(),
            self.place_filter.selected.clone(),
        );
        if self.gallery.scope.as_ref() != Some(&scope) {
            self.gallery.selection.clear();
            self.gallery.scope = Some(scope);
        }
    }
    pub(super) fn accept_collection_media(&mut self, root: &Path, media: crate::domain::Media) {
        if self.root.as_deref() != Some(root) || self.gallery.removed.contains(media.path()) {
            return;
        }
        let media = self.comments.project(media);
        let media = photosuite_application::date_correction::project(
            root,
            &self.date_corrections.catalog,
            media,
        );
        if self.known_media.insert(media.path().to_owned()) {
            self.items.push(media);
        }
    }
    pub(super) fn apply_trash_outcome(
        &mut self,
        outcome: &photosuite_application::media_trash::Outcome,
    ) {
        let Some(root) = self.root.as_deref() else {
            return;
        };
        if root != outcome.root {
            return;
        }
        let record_path = self
            .record_photo
            .and_then(|index| self.items.get(index))
            .map(|media| media.path().to_owned());
        let removal = photosuite_application::media_trash::apply_outcome(
            &mut self.items,
            self.selected,
            root,
            outcome,
        );
        self.selected = removal.selected;
        self.record_photo =
            record_path.and_then(|path| self.items.iter().position(|media| media.path() == path));
        if self.selected.is_none() {
            self.storyboard.clear();
        }
        self.gallery
            .selection
            .paths
            .retain(|path| !removal.removed.contains(path));
        if self
            .gallery
            .selection
            .anchor
            .as_ref()
            .is_some_and(|path| removal.removed.contains(path))
        {
            self.gallery.selection.anchor = None;
        }
        for path in &removal.removed {
            self.known_media.remove(path);
        }
        if self
            .highlighted_photo
            .as_ref()
            .is_some_and(|path| removal.removed.contains(path))
        {
            self.highlighted_photo = None;
        }
        if self
            .scroll_to_photo
            .as_ref()
            .is_some_and(|path| removal.removed.contains(path))
        {
            self.scroll_to_photo = None;
        }
        self.gallery.removed.extend(removal.removed);
        self.tag_view.options_cache = None;
        self.place_options = None;
        // Reconcile remaining transcription candidates without restarting audio/library maintenance.
        self.transcription_seen = None;
    }
    pub(super) fn gallery_actions(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.gallery.job.poll() {
            match result {
                Ok(outcome) => {
                    self.apply_trash_outcome(&outcome);
                    let successes = outcome.files.iter().filter(|(_, r)| r.is_ok()).count();
                    let mut report = format!("{successes} Datei(en) in den Papierkorb verschoben.");
                    if outcome.files.len() < outcome.requested {
                        report.push_str(&format!(
                            "\n{} Datei(en) nicht bearbeitet (abgebrochen).",
                            outcome.requested - outcome.files.len()
                        ));
                    }
                    for (file, result) in outcome.files {
                        if let Err(error) = result {
                            report.push_str(&format!("\n{}: {error}", file.as_str()));
                        }
                    }
                    self.gallery.report = Some(report);
                }
                Err(error) => self.gallery.report = Some(error),
            }
        }
        if self.gallery.job.running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            crate::theme::dialog("In den Papierkorb verschieben")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Ausgewählte Medien werden verschoben …");
                    });
                    if ui.button("Abbrechen").clicked() {
                        self.gallery.job.cancel();
                    }
                    ui.small("Bereits verschobene Dateien bleiben im macOS-Papierkorb.");
                });
        }
        if let Some((root, files)) = self.gallery.pending.clone() {
            let mut confirm = false;
            let mut cancel = false;
            crate::theme::dialog("Ausgewählte Medien löschen?").collapsible(false).resizable(false)
                .max_width((ctx.content_rect().width()-32.).max(260.)).show(ctx,|ui|{
                ui.label(format!("{} Datei(en) in den macOS-Papierkorb verschieben?",files.len()));
                ui.add_space(8.);
                egui::ScrollArea::vertical().max_height(180.).show(ui,|ui|{for file in &files {ui.add(egui::Label::new(file.as_str()).wrap());}});
                ui.small(format!("Bibliothek: {}",root.display()));
                ui.small("Tags und Kommentarzuordnungen bleiben erhalten. Keine endgültige Löschung.");
                ui.add_space(12.);
                ui.horizontal(|ui|{
                    if theme::default_dialog_button(ui,"Abbrechen"){cancel=true;}
                    if ui.add(egui::Button::new(egui::RichText::new("In den Papierkorb").color(theme::DANGER))).clicked(){confirm=true;}
                });
            });
            if cancel {
                self.gallery.pending = None;
            }
            if confirm {
                match self
                    .gallery
                    .job
                    .start(self.services.media_trash.clone(), root, files)
                {
                    Ok(()) => self.gallery.pending = None,
                    Err(error) => {
                        self.gallery.pending = None;
                        self.gallery.report = Some(error);
                    }
                }
            }
        }
        if let Some(report) = self.gallery.report.clone() {
            let mut open = true;
            crate::theme::dialog("Papierkorb – Ergebnis")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .max_width(560.)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(260.)
                        .show(ui, |ui| {
                            ui.label(report);
                        });
                    if theme::default_dialog_button(ui, "OK") {
                        self.gallery.report = None;
                    }
                });
            if !open {
                self.gallery.report = None;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_toggle_and_context_preserve_the_intended_batch() {
        let paths: Vec<PathBuf> = ["a.jpg", "b.jpg", "c.jpg", "d.jpg"]
            .into_iter()
            .map(Into::into)
            .collect();
        let mut selection = Selection::default();
        selection.click(&paths[0], &paths, false, false);
        selection.click(&paths[2], &paths, false, true);
        assert_eq!(selection.paths.len(), 3);
        selection.click(&paths[1], &paths, true, false);
        assert!(!selection.paths.contains(&paths[1]));
        selection.context(&paths[2]);
        assert_eq!(selection.paths.len(), 2);
        selection.context(&paths[3]);
        assert_eq!(selection.paths, [paths[3].clone()].into());
        selection.clear();
        assert!(selection.paths.is_empty());
    }
}

#[cfg(test)]
mod incremental_tests {
    use super::*;
    use crate::domain::Media;
    use photosuite_application::{
        media_geo::{LocatedPhoto, LocationIndex},
        media_trash::Outcome,
        organization::Action,
        place_filter::Session,
    };
    use std::sync::{Arc, atomic::AtomicBool};
    struct Places;
    impl LocationIndex for Places {
        fn locations(&self, _: &Path, _: &AtomicBool) -> Result<Vec<LocatedPhoto>, String> {
            Ok(["a.jpg", "b.jpg"]
                .map(|name| LocatedPhoto {
                    photo: LibraryFile::new(format!("2026/11/{name}")).unwrap(),
                    country: "DE".into(),
                    country_name: "Deutschland".into(),
                    region: None,
                    place_id: 1,
                    place_name: "Berlin".into(),
                })
                .to_vec())
        }
    }
    #[test]
    fn trash_preserves_sidebar_catalogs_filters_and_remaining_media_without_any_scan() {
        let root = PathBuf::from("/memory-only-gallery");
        let mut suite = super::super::layout_tests::test_suite();
        suite.root = Some(root.clone());
        suite.group = Some("2026/11".into());
        suite.videos_only = false;
        suite.organization.open(root.clone()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while suite.organization.catalog().is_none() {
            suite.organization.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        suite.items = ["a.jpg", "b.jpg", "c.jpg"]
            .map(|name| {
                Media::from_library_path(root.join("2026/11").join(name), "2026/11".into()).unwrap()
            })
            .to_vec();
        suite.known_media = suite
            .items
            .iter()
            .map(|media| media.path().to_owned())
            .collect();
        suite
            .organization
            .apply(Action::CreateMany {
                name: "urlaub".into(),
                files: ["a.jpg", "b.jpg"]
                    .map(|name| LibraryFile::new(format!("2026/11/{name}")).unwrap())
                    .to_vec(),
            })
            .unwrap();
        suite.prepare_tag_filter();
        suite.tag_view.filters.insert("urlaub".into());
        suite.place_filter = Session::new(Arc::new(Places), Arc::default());
        loop {
            suite.place_filter.poll(Some(&root));
            if !suite.place_filter.pending() {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let location = photosuite_application::place_filter::Selection::Country("DE".into());
        suite.place_filter.selected.insert(location.clone());
        suite.place_scope = Some((Some(root.clone()), suite.group.clone()));
        suite.selected = Some(1);
        let deleted = suite.items[0].clone();
        let outcome = Outcome {
            root: root.clone(),
            requested: 2,
            files: vec![
                (LibraryFile::new("2026/11/a.jpg".into()).unwrap(), Ok(())),
                (
                    LibraryFile::new("2026/11/c.jpg".into()).unwrap(),
                    Err("simulated failure".into()),
                ),
            ],
        };
        suite.apply_trash_outcome(&outcome);
        suite.accept_collection_media(&root, deleted);
        assert_eq!(suite.items.len(), 2);
        assert_eq!(suite.selected, Some(0));
        assert!(suite.items[0].path().ends_with("b.jpg"));
        assert!(!suite.collection.is_loading());
        assert_eq!(suite.group.as_deref(), Some("2026/11"));
        assert!(suite.tag_view.filters.contains("urlaub"));
        assert!(suite.place_filter.selected.contains(&location));
        suite.prepare_tag_filter();
        assert!(
            suite
                .tag_view
                .options_cache
                .as_ref()
                .unwrap()
                .tags
                .contains(&"urlaub".into())
        );
        assert_eq!(suite.visible(), vec![0]);
        let places =
            suite
                .place_filter
                .catalog()
                .options(&suite.items, &root, &suite.unfiltered_visible());
        assert_eq!(places[0].name, "Deutschland");
        assert_eq!(places[0].count, 1);
        assert_eq!(
            suite.organization.catalog().unwrap().tags()["urlaub"].len(),
            2,
            "metadata membership must not be rewritten"
        );
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        for frame in 0..3 {
            let output = ctx.run(input(vec![]), |ctx| suite.library_sidebar(ctx));
            if frame == 2 {
                for label in ["urlaub", "Deutschland"] {
                    assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()==label)),"missing sidebar entry {label}");
                }
            }
        }
    }
    fn input(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 700.),
            )),
            events,
            ..Default::default()
        }
    }
    fn enter() -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }
    #[test]
    fn result_dialog_closes_with_enter_and_delete_confirmation_defaults_to_cancel() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut suite = super::super::layout_tests::test_suite();
        suite.gallery.report = Some("Fertig".into());
        for _ in 0..3 {
            let _ = ctx.run(input(vec![]), |ctx| suite.gallery_actions(ctx));
        }
        let _ = ctx.run(input(enter()), |ctx| suite.gallery_actions(ctx));
        assert!(suite.gallery.report.is_none());
        assert!(!suite.gallery.job.running());
        suite.gallery.pending = Some((
            PathBuf::from("/memory-only"),
            vec![LibraryFile::new("2026/11/a.jpg".into()).unwrap()],
        ));
        for _ in 0..3 {
            let _ = ctx.run(input(vec![]), |ctx| suite.gallery_actions(ctx));
        }
        let _ = ctx.run(input(enter()), |ctx| suite.gallery_actions(ctx));
        assert!(suite.gallery.pending.is_none());
        assert!(
            !suite.gallery.job.running(),
            "Enter must not start a destructive action"
        );
    }
}
