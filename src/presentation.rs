mod diary;
mod library;
mod toolbar;
mod viewer;
mod wall;
use crate::domain::Media;
use eframe::egui;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::mpsc;
pub struct Suite {
    diary: photosuite_application::diary::DiarySession,
    diary_catalog: photosuite_application::diary::DiaryCatalog,
    remember_after_scan: bool,
    storyboard: crate::storyboard_ui::StoryboardView,
    preferences: photosuite_application::preferences::Preferences,
    persistence: Option<photosuite_application::preferences::Persistence>,
    preferences_error: String,
    options_open: bool,
    import_panel: crate::import_ui::ImportPanel,
    bindings: crate::command_bindings::CommandBindings,
    write_exif: bool,
    editor: photosuite_application::editing::EditingSession,
    services: crate::application::Services,
    root: Option<PathBuf>,
    items: Vec<Media>,
    query: String,
    group: Option<String>,
    size: usize,
    selected: Option<usize>,
    highlighted_photo: Option<PathBuf>,
    scroll_to_photo: Option<PathBuf>,
    collection: photosuite_application::collection::CollectionSession,
    status: String,
    videos_only: bool,
    square_previews: bool,
    show_library: bool,
}
impl Suite {
    fn load(&mut self, path: PathBuf) {
        self.load_path(path, true);
    }
    fn load_path(&mut self, path: PathBuf, remember: bool) {
        self.storyboard.clear();
        self.diary_catalog.cancel();
        self.remember_after_scan = !remember;
        self.diary.cancel();
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&path))
                .unwrap_or(path)
        };
        self.collection
            .request(self.services.repository.clone(), path.clone());
        if remember {
            self.preferences.last_library = Some(path.clone());
            self.persist_preferences();
        }
        self.root = Some(path);
        self.items.clear();
        self.highlighted_photo = None;
        self.scroll_to_photo = None;
        self.selected = None;
        self.group = None;
        self.status = "Sammlung wird gelesen …".into();
    }
    fn execute(&mut self, command: crate::commands::Command, ctx: &egui::Context) {
        let Some(index) = self.selected else {
            return;
        };
        if command == crate::commands::Command::PlayVideo {
            if let Err(error) = photosuite_application::playback::play(
                &self.items[index],
                self.services.video_player.as_ref(),
            ) {
                self.status = error;
            }
            return;
        }
        let context = ctx.clone();
        match self.editor.execute(
            command,
            &self.items[index],
            self.write_exif,
            self.services.orientation_writer.clone(),
            move || context.request_repaint(),
        ) {
            Ok(()) if self.editor.is_saving() => {
                self.status = "EXIF-Ausrichtung wird gespeichert …".into()
            }
            Ok(()) => {}
            Err(error) => self.status = error,
        }
    }
    fn snapshot(&self) -> photosuite_application::preferences::Preferences {
        let mut p = self.preferences.clone();
        p.preview_size = self.size;
        p.square_previews = self.square_previews;
        p.show_library = self.show_library;
        p.videos_only = self.videos_only;
        p.write_exif = self.write_exif;
        p.shortcuts = self.bindings.export();
        p
    }
    fn apply_preferences(&mut self, p: photosuite_application::preferences::Preferences) {
        self.size = p.preview_size;
        self.square_previews = p.square_previews;
        self.show_library = p.show_library;
        self.videos_only = p.videos_only;
        self.write_exif = p.write_exif;
        self.preferences = p;
    }
    fn settings(&mut self, ctx: &egui::Context) {
        let mut p = self.snapshot();
        crate::options_ui::show(
            ctx,
            &mut self.options_open,
            &mut p,
            self.editor.is_saving(),
            &mut self.bindings.visible,
            &self.preferences_error,
        );
        self.apply_preferences(p);
        self.persist_preferences();
    }
    fn persist_preferences(&mut self) {
        let snapshot = self.snapshot();
        if let Some(persistence) = &mut self.persistence {
            persistence.update(snapshot);
            if let Some(error) = persistence.poll_error() {
                self.preferences_error =
                    format!("Einstellungen konnten nicht gespeichert werden: {error}");
            }
        }
    }
    fn visible(&self) -> Vec<usize> {
        crate::application::visible(
            &self.items,
            &self.query,
            self.group.as_deref(),
            self.videos_only,
        )
    }
}
impl eframe::App for Suite {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.diary_catalog.poll();
        if self.diary_catalog.is_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.diary.poll(self.services.audio_player.as_ref());
        self.poll_audio_player(ctx);
        if self.diary.is_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(saved) = self.editor.poll() {
            match saved.result {
                Ok(()) => {
                    for turns in 0..4 {
                        for edge in [256, 512, 1024, 4096] {
                            ctx.forget_image(&format!(
                                "oriented://{turns}:{edge}/{}",
                                saved.path.display()
                            ));
                        }
                    }
                    self.status =
                        "EXIF gespeichert · JPEG-Pixel unverändert · ohne Sicherungsdatei".into();
                }
                Err(error) => {
                    self.status =
                        format!("Speichern fehlgeschlagen; Drehung bleibt in der Sitzung: {error}")
                }
            }
        }
        if self.editor.is_saving() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let mut pending = self.bindings.shortcuts(
            ctx,
            self.selected.is_some()
                && !self.editor.is_saving()
                && !self.options_open
                && !self.import_panel.visible,
        );
        if let Some(result) = self.collection.poll() {
            match result {
                Ok(collection) => {
                    self.status = format!(
                        "{} Medien · {} nicht lesbare Einträge",
                        collection.items.len(),
                        collection.unreadable
                    );
                    if self.remember_after_scan
                        && (collection.unreadable == 0 || !collection.items.is_empty())
                    {
                        self.preferences.last_library = Some(collection.root);
                        self.persist_preferences();
                    }
                    self.items = collection.items;
                }
                Err(error) => self.status = error,
            }
            self.remember_after_scan = false;
        }
        if self.collection.is_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Comma,
            ))
        }) {
            self.options_open = true;
        }
        self.toolbar(ctx);
        self.status_bar(ctx);
        self.library_sidebar(ctx);
        let visible = self.visible();
        self.photo_wall(ctx, &visible);
        let viewer_visible = self.viewer_items(&visible);
        self.media_viewer(ctx, &viewer_visible, &mut pending);
        if self.selected.is_none() {
            self.storyboard.clear();
        }
        for command in pending {
            self.execute(command, ctx);
        }
        self.bindings.show(ctx);
        self.settings(ctx);
        if let Some(path) = self.import_panel.show(
            ctx,
            self.services.importer.clone(),
            self.services.repository.clone(),
            self.services.duplicates.clone(),
            self.services.opener.clone(),
        ) {
            self.refresh_after_import(path);
        }
    }
}
impl Suite {
    fn refresh_after_import(&mut self, path: PathBuf) {
        if self.root.as_ref().is_none_or(|root| root == &path) {
            self.load(path);
        } else {
            self.status = format!("Importziel aktualisiert: {}", path.display());
        }
    }
}

pub fn create(cc: &eframe::CreationContext<'_>, services: crate::application::Services) -> Suite {
    egui_extras::install_image_loaders(&cc.egui_ctx);
    cc.egui_ctx.add_image_loader(std::sync::Arc::new(
        crate::image_loader::OrientedLoader::new(services.decoder.clone()),
    ));
    crate::theme::install(&cc.egui_ctx);
    let (preferences, mut preferences_error) = match services.preferences.load() {
        Ok(p) => (p, String::new()),
        Err(error) => (
            Default::default(),
            format!("Einstellungen konnten nicht geladen werden: {error}"),
        ),
    };
    let mut bindings = crate::command_bindings::CommandBindings::default();
    if let Err(error) = bindings.restore(&preferences.shortcuts) {
        preferences_error = error;
    }
    let mut persistence = Some(photosuite_application::preferences::Persistence::new(
        services.preferences.clone(),
        preferences.clone(),
    ));
    if let Some(persistence) = &mut persistence {
        persistence.flush_current();
    }
    let mut app = Suite {
        diary: Default::default(),
        diary_catalog: Default::default(),
        remember_after_scan: false,
        storyboard: Default::default(),
        preferences: preferences.clone(),
        persistence,
        preferences_error,
        options_open: false,
        import_panel: Default::default(),
        bindings,
        write_exif: preferences.write_exif,
        editor: Default::default(),
        size: preferences.preview_size,
        square_previews: preferences.square_previews,
        show_library: preferences.show_library,
        status: "Bereit".into(),
        services,
        root: None,
        items: Vec::new(),
        query: String::new(),
        group: None,
        selected: None,
        highlighted_photo: None,
        scroll_to_photo: None,
        collection: Default::default(),
        videos_only: preferences.videos_only,
    };
    if let Some(path) = preferences.startup_library(std::env::args_os().skip(1)) {
        app.load_path(path, false);
    }
    app
}

fn image_uri(path: &std::path::Path, rotations: &std::collections::HashMap<PathBuf, u8>) -> String {
    format!(
        "oriented://{}:4096/{}",
        rotations.get(path).copied().unwrap_or(0),
        path.display()
    )
}

fn preview_uri(
    path: &std::path::Path,
    rotations: &std::collections::HashMap<PathBuf, u8>,
    tile: f32,
) -> String {
    let edge = if tile <= 128. {
        256
    } else if tile <= 256. {
        512
    } else {
        1024
    };
    format!(
        "oriented://{}:{edge}/{}",
        rotations.get(path).copied().unwrap_or(0),
        path.display()
    )
}

#[cfg(test)]
#[path = "presentation/tests.rs"]
mod layout_tests;
