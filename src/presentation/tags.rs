//! Tag controls only; validation and persistence remain in the inner layers.
use crate::domain::tags::LibraryFile;
use eframe::egui;
use photosuite_application::organization::{Action, Change, OrganizationSession};
#[derive(Default)]
pub(super) struct TagView {
    pub open: bool,
    pub active: Option<String>,
    pub file: Option<LibraryFile>,
    selected: Option<String>,
    new_name: String,
    rename: String,
    error: String,
}
impl TagView {
    pub fn clear_scope(&mut self) {
        let open = self.open;
        *self = Self::default();
        self.open = open;
    }
    pub fn edit(&mut self, file: LibraryFile) {
        self.open = true;
        self.file = Some(file);
        self.selected = None;
        self.error.clear();
    }
    pub fn show(&mut self, ctx: &egui::Context, session: &mut OrganizationSession) {
        let mut actions = Vec::new();
        let mut retry = false;
        let mut discard = false;
        egui::Window::new("Tags und Alben").open(&mut self.open).default_width(520.)
            .max_width((ctx.content_rect().width()-32.).max(260.)).max_height((ctx.content_rect().height()-48.).max(240.)).vscroll(true).show(ctx,|ui|{
                crate::options_ui::spacing(ui);
                ui.label("Tags ordnen Dateien zu. Ein als Album markierter Tag erscheint links.");
                ui.small("Originaldateien bleiben unverändert. Automatisch in yaml/tags.yaml speichern.");
                if let Some(file)=&self.file{
                    ui.add(egui::Label::new(file.as_str()).truncate()).on_hover_text(file.as_str());
                    if ui.small_button("Alle Tags verwalten").clicked(){self.file=None;}
                }
                if session.loading(){ui.horizontal(|ui|{ui.spinner();ui.label("Tags werden geladen …");});}
                if session.saving(){ui.label("Änderungen werden gespeichert …");}
                let rows:Vec<_>=session.catalog().map(|c|c.tags().iter().map(|(name,files)|
                    (name.clone(),files.len(),c.albums().contains(name),self.file.as_ref().is_some_and(|file|files.contains(file))))
                    .collect()).unwrap_or_default();
                ui.add_enabled_ui(session.catalog().is_some(),|ui|{
                    ui.horizontal_wrapped(|ui|{
                        ui.add(egui::TextEdit::singleline(&mut self.new_name).hint_text("Neuer Tag").desired_width(180.));
                        if ui.button("Tag anlegen").clicked(){actions.push(Action::Create{name:self.new_name.clone(),file:self.file.clone()});}
                    });
                    egui::ScrollArea::vertical().id_salt("tag_rows").max_height(280.).auto_shrink([false,true]).show(ui,|ui|{
                        if rows.is_empty(){ui.label("Noch keine Tags. Einen Tag anlegen und Dateien zuordnen.");}
                        for (name,count,mut album,mut assigned) in rows{
                            ui.horizontal_wrapped(|ui|{
                                if self.file.is_some(){
                                    if ui.checkbox(&mut assigned,&name).changed(){actions.push(Action::Assign{tag:name.clone(),file:self.file.clone().unwrap(),present:assigned});}
                                }else if ui.selectable_label(self.selected.as_ref()==Some(&name),&name).clicked(){self.selected=Some(name.clone());self.rename=name.clone();}
                                ui.label(format!("({count})"));
                                if ui.checkbox(&mut album,"Album").changed(){actions.push(Action::Album{tag:name.clone(),enabled:album});}
                                if ui.small_button("Anzeigen").clicked(){self.active=Some(name.clone());}
                            });
                        }
                    });
                    if let Some(selected)=&self.selected{
                        ui.separator();ui.horizontal_wrapped(|ui|{
                            ui.add(egui::TextEdit::singleline(&mut self.rename).desired_width(180.));
                            if ui.button("Umbenennen").clicked(){actions.push(Action::Rename{old:selected.clone(),new:self.rename.clone()});}
                            if ui.button("Tag entfernen").on_hover_text("Entfernt nur den Tag und seine Albumzuordnung; alle Dateien bleiben erhalten").clicked(){actions.push(Action::Remove(selected.clone()));}
                        });
                    }
                });
                if !self.error.is_empty(){ui.colored_label(egui::Color32::LIGHT_RED,&self.error);}
                if !session.error().is_empty(){ui.colored_label(egui::Color32::LIGHT_RED,session.error());}
                ui.horizontal_wrapped(|ui|{
                    if session.dirty() && ui.add_enabled(!session.saving(),egui::Button::new("Erneut speichern")).clicked(){retry=true;}
                    let label=if session.dirty(){"Nicht gespeicherte Änderungen verwerfen"}else{"Datei neu laden"};
                    if ui.add_enabled(session.root().is_some() && !session.saving(),egui::Button::new(label)).clicked(){discard=true;}
                });
            });
        for action in actions {
            match session.apply(action) {
                Ok(change) => {
                    self.error.clear();
                    match change {
                        Change::Created(_) => self.new_name.clear(),
                        Change::Renamed { old, new } => {
                            if self.active.as_ref() == Some(&old) {
                                self.active = Some(new.clone());
                            }
                            self.selected = Some(new.clone());
                            self.rename = new;
                        }
                        _ => {}
                    }
                }
                Err(error) => self.error = error,
            }
        }
        if retry && let Err(error) = session.retry() {
            self.error = error;
        }
        if discard && let Err(error) = session.discard() {
            self.error = error;
        }
        if let Some(catalog) = session.catalog() {
            if self
                .active
                .as_ref()
                .is_some_and(|tag| !catalog.tags().contains_key(tag))
            {
                self.active = None;
            }
            if self
                .selected
                .as_ref()
                .is_some_and(|tag| !catalog.tags().contains_key(tag))
            {
                self.selected = None;
            }
        }
    }
}
