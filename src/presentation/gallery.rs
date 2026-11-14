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
    Option<String>,
    String,
    BTreeSet<String>,
);
#[derive(Default)]
pub(super) struct GalleryState {
    pub selection: Selection,
    pub scope: Option<Scope>,
    pub pending: Option<(PathBuf, Vec<LibraryFile>)>,
    pub job: photosuite_application::media_trash::TrashSession,
    pub report: Option<String>,
    pub refresh: Option<PathBuf>,
}
impl super::Suite {
    pub(super) fn prepare_gallery_selection(&mut self) {
        let scope = (
            self.root.clone(),
            self.group.clone(),
            self.tag_view.active.clone(),
            self.query.clone(),
            self.tag_view.filters.clone(),
        );
        if self.gallery.scope.as_ref() != Some(&scope) {
            self.gallery.selection.clear();
            self.gallery.scope = Some(scope);
        }
    }
    pub(super) fn gallery_actions(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.gallery.job.poll() {
            match result {
                Ok(outcome) => {
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
                    self.gallery.refresh = Some(outcome.root);
                    self.gallery.report = Some(report);
                }
                Err(error) => self.gallery.report = Some(error),
            }
        }
        if let Some(root) = self.gallery.refresh.clone() {
            if self.root.as_ref() != Some(&root) {
                self.gallery.refresh = None;
            } else if !self.organization.dirty()
                && !self.organization.saving()
                && !self.comments.has_unsaved()
                && !self.comments.is_saving()
            {
                let group = self.group.clone();
                self.load_library(root, true, false);
                self.group = group;
                self.gallery.refresh = None;
            }
        }
        if self.gallery.job.running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            egui::Window::new("In den Papierkorb verschieben")
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
            egui::Window::new("Ausgewählte Medien löschen?").collapsible(false).resizable(false)
                .max_width((ctx.content_rect().width()-32.).max(260.)).show(ctx,|ui|{
                ui.label(format!("{} Datei(en) in den macOS-Papierkorb verschieben?",files.len()));
                ui.add_space(8.);
                egui::ScrollArea::vertical().max_height(180.).show(ui,|ui|{for file in &files {ui.add(egui::Label::new(file.as_str()).wrap());}});
                ui.small(format!("Bibliothek: {}",root.display()));
                ui.small("Tags und Kommentarzuordnungen bleiben erhalten. Keine endgültige Löschung.");
                ui.add_space(12.);
                ui.horizontal(|ui|{
                    if ui.button("Abbrechen").clicked(){cancel=true;}
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
            egui::Window::new("Papierkorb – Ergebnis")
                .open(&mut open)
                .max_width(560.)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(260.)
                        .show(ui, |ui| {
                            ui.label(report);
                        });
                    if ui.button("OK").clicked() {
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
