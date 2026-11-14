mod audio_header;
mod badges;
mod captions;
mod chrome;
mod comments;
mod diary;
mod icons;
mod library;
mod menu;
mod tag_cloud;
mod tags;
mod viewer;
mod wall;
use crate::domain::Media;
use eframe::egui;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::mpsc;
pub struct Suite {
    comments: photosuite_application::comments::CommentSession,
    comment_view: comments::CommentView,
    record_photo: Option<usize>,
    known_media: std::collections::HashSet<PathBuf>,
    media_dates: photosuite_application::media_dates::MediaDates,
    media_counts: photosuite_application::collection::MediaCounts,
    native_menu: Option<Box<dyn crate::desktop_menu::DesktopMenu>>,
    focus_search: bool,
    search_open: bool,
    help_open: bool,
    audio_header: audio_header::AudioHeader,
    audio_preparation: photosuite_application::diary_preparation::AudioPreparation,
    viewer_audio: audio_header::AudioHeader,
    organization: photosuite_application::organization::OrganizationSession,
    tag_view: tags::TagView,
    diary: photosuite_application::diary::DiarySession,
    diary_catalog: photosuite_application::diary::DiaryCatalog,
    remember_after_scan: bool,
    storyboard: crate::storyboard_ui::StoryboardView,
    preferences: photosuite_application::preferences::Preferences,
    persistence: Option<photosuite_application::preferences::Persistence>,
    library_settings: photosuite_application::preferences::LibrarySettings,
    startup: Option<photosuite_application::preferences::StartupPreferences>,
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
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&path))
                .unwrap_or(path)
        };
        if self.comments.has_unsaved() || self.comments.is_saving() {
            self.status = "Bitte die Audioaufnahme zuerst speichern oder verwerfen".into();
            self.comments.show();
            return;
        }
        if self.root.as_ref() != Some(&path) || self.organization.catalog().is_none() {
            if let Err(error) = self.organization.open(path.clone()) {
                self.status = error;
                self.tag_view.open = true;
                return;
            }
            self.tag_view.clear_scope();
        }
        self.audio_preparation.reset();
        self.organization.begin_media_scan(&path);
        self.comments.load(path.clone());
        self.persist_preferences();
        self.preferences.library_scope = None;
        self.storyboard.clear();
        self.diary_catalog.cancel();
        self.remember_after_scan = !remember;
        if self.root.as_ref() != Some(&path) {
            let _ = self
                .services
                .audio_player
                .control(photosuite_application::diary::AudioControl::Stop);
            self.diary.cancel();
        }
        self.library_settings
            .request(self.services.preferences.clone(), path.clone());
        self.collection
            .request(self.services.repository.clone(), path.clone());
        if remember {
            self.preferences.last_library = Some(path.clone());
            self.persist_preferences();
        }
        self.root = Some(path);
        self.items.clear();
        self.known_media.clear();
        self.media_dates.clear();
        self.media_counts = Default::default();
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
    fn settings_ready(&self) -> bool {
        self.startup.is_none() && !self.library_settings.is_loading()
    }
    fn poll_startup(&mut self, ctx: &egui::Context) {
        let Some(startup) = self.startup.as_mut() else {
            return;
        };
        let Some(outcome) = startup.poll() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        };
        self.startup = None;
        if self.root.is_none() {
            let (mut preferences, mut error) = match outcome.result {
                Ok((p, warning)) => (p, warning.unwrap_or_default()),
                Err(error) => (
                    Default::default(),
                    format!("Einstellungen konnten nicht geladen werden: {error}"),
                ),
            };
            let mut bindings = crate::command_bindings::CommandBindings::default();
            if let Err(binding_error) = bindings.restore(&preferences.shortcuts) {
                preferences.library_scope = None;
                error = binding_error;
            }
            self.bindings = bindings;
            self.apply_preferences(preferences);
            self.preferences_error = error;
        }
        let snapshot = self.snapshot();
        if let Some(persistence) = &mut self.persistence {
            persistence.update(snapshot);
        } else {
            self.persistence = Some(photosuite_application::preferences::Persistence::new(
                self.services.preferences.clone(),
                snapshot,
            ));
        }
        if self.root.is_some() {
            self.persist_location();
        } else {
            self.persistence.as_mut().unwrap().flush_current();
            if let Some(path) = self.preferences.startup_library(outcome.arguments) {
                self.load_path(path, false);
            } else {
                self.status = "Bereit".into();
            }
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
        if !self.settings_ready() {
            egui::Window::new("Einstellungen")
                .open(&mut self.options_open)
                .show(ctx, |ui| {
                    ui.label("Einstellungen werden geladen …");
                });
            return;
        }
        let mut p = self.snapshot();
        let index_action = crate::options_ui::show(
            ctx,
            &mut self.options_open,
            &mut p,
            self.editor.is_saving(),
            &mut self.bindings.visible,
            &self.preferences_error,
            crate::options_ui::CommentIndexStatus {
                available: self.comments.can_rebuild(),
                rebuilding: self.comments.is_rebuilding(),
                message: if self.comments.is_rebuilding() {
                    &self.comments.rebuild_progress().phase
                } else {
                    self.comments.message()
                },
                error: self.comments.error(),
            },
        );
        match index_action {
            Some(crate::options_ui::CommentIndexAction::Rebuild) => {
                if let Err(error) = self.comments.rebuild() {
                    self.status = error;
                }
            }
            Some(crate::options_ui::CommentIndexAction::Cancel) => self.comments.cancel_rebuild(),
            None => {}
        }
        self.apply_preferences(p);
        self.persist_preferences();
    }
    fn poll_library_settings(&mut self, ctx: &egui::Context) {
        if let Some((root, result)) = self.library_settings.poll()
            && self.root.as_ref() == Some(&root)
        {
            match result {
                Ok(loaded) => {
                    let mut preferences = loaded.unwrap_or_default();
                    preferences.last_library = self.preferences.last_library.clone();
                    preferences.library_scope = Some(root);
                    let mut bindings = crate::command_bindings::CommandBindings::default();
                    match bindings.restore(&preferences.shortcuts) {
                        Ok(()) => {
                            self.bindings = bindings;
                            self.apply_preferences(preferences);
                            self.preferences_error.clear();
                            self.persist_preferences();
                        }
                        Err(error) => self.preferences_error = error,
                    }
                }
                Err(error) => {
                    self.preferences_error =
                        format!("Bibliotheks-Einstellungen konnten nicht geladen werden: {error}")
                }
            }
        }
        if self.library_settings.is_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
    fn persist_location(&mut self) {
        let mut location = self.snapshot();
        location.library_scope = None;
        if let Some(persistence) = &mut self.persistence {
            persistence.update(location);
        }
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
        let base = self.unfiltered_visible();
        if self.tag_view.filters.is_empty() {
            return base;
        }
        match (self.organization.root(), self.organization.catalog()) {
            (Some(root), Some(catalog)) => photosuite_application::organization::filter_any(
                &self.items,
                root,
                catalog,
                &base,
                &self.tag_view.filters,
            ),
            _ => base,
        }
    }
    fn prepare_tag_cloud(&mut self) {
        let scope = (
            self.root.clone(),
            self.group.clone(),
            self.tag_view.active.clone(),
        );
        if self.tag_view.cloud_scope.as_ref() != Some(&scope) {
            self.tag_view.filters.clear();
            self.tag_view.cloud_cache = None;
            self.tag_view.cloud_scope = Some(scope);
        }
    }
    fn unfiltered_visible(&self) -> Vec<usize> {
        if let Some(tag) = &self.tag_view.active {
            return match (self.organization.root(), self.organization.catalog()) {
                (Some(root), Some(catalog)) => photosuite_application::organization::visible(
                    &self.items,
                    &self.query,
                    self.videos_only,
                    root,
                    catalog,
                    tag,
                ),
                _ => Vec::new(),
            };
        }
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
        self.poll_startup(ctx);
        self.poll_comments(ctx);
        self.media_dates.poll();
        if self.media_dates.is_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.organization.poll();
        if self.organization.loading() || self.organization.saving() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        self.audio_preparation.poll();
        if !self.audio_preparation.error().is_empty() {
            self.status = self.audio_preparation.error().into();
        }
        if self.audio_preparation.is_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
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
        let mut pending = if self.native_menu.is_some() {
            Vec::new()
        } else {
            self.bindings.shortcuts(
                ctx,
                self.selected.is_some()
                    && !self.editor.is_saving()
                    && !self.options_open
                    && !self.import_panel.visible
                    && !self.comments.is_open()
                    && !self.tag_view.open
                    && self.settings_ready(),
            )
        };
        self.poll_library_settings(ctx);
        if let Some(result) = self.collection.poll() {
            match result {
                Ok(collection) => {
                    self.organization.observe_media(
                        &collection.root,
                        &collection.items,
                        !self.collection.is_loading() && collection.unreadable == 0,
                    );
                    for media in collection.items {
                        let media = self.comments.project(media);
                        if self.known_media.insert(media.path().to_owned()) {
                            self.items.push(media);
                        }
                    }
                    self.status = if collection.unreadable == 0 {
                        "Bereit".into()
                    } else {
                        format!("{} nicht lesbare Einträge", collection.unreadable)
                    };
                    if self.remember_after_scan
                        && (collection.unreadable == 0 || !self.items.is_empty())
                    {
                        self.preferences.last_library = Some(collection.root);
                        self.persist_location();
                    }
                }
                Err(error) => self.status = error,
            }
            self.remember_after_scan = false;
        }
        if self.collection.is_loading() {
            if let (Some(tag), Some(catalog)) = (&self.tag_view.active, self.organization.catalog())
                && catalog.tags().contains_key(tag)
            {
                let years = catalog.partitions(tag);
                if let Some(year) = years.into_iter().rev().find(|year| {
                    self.collection.years().iter().any(|y| y == year)
                        && !self.collection.partition_loaded(year)
                }) {
                    self.collection.prioritize(year);
                }
            }
            self.status = "Weitere Jahre werden gelesen …".into();
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if self.native_menu.is_none()
            && ctx.input_mut(|i| {
                i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Comma,
                ))
            })
        {
            self.options_open = true;
        }
        self.poll_menu(ctx);
        self.window_title(ctx);
        self.search_dialog(ctx);
        self.status_bar(ctx);
        self.library_sidebar(ctx);
        self.prepare_tag_cloud();
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
        let previous_tag = self.tag_view.active.clone();
        self.tag_view.show(ctx, &mut self.organization);
        if self.tag_view.active != previous_tag {
            self.group = None;
            self.selected = None;
            self.scroll_to_photo = None;
            self.highlighted_photo = None;
            self.storyboard.clear();
        }
        self.comment_dialog(ctx);
        self.bindings.show(ctx);
        self.settings(ctx);
        self.help(ctx);
        self.poll_menu(ctx);
        let tag_choices = self
            .organization
            .catalog()
            .map(|c| c.tags().keys().cloned().collect())
            .unwrap_or_default();
        if let Some(path) =
            self.import_panel
                .show(ctx, &self.services, self.root.as_deref(), tag_choices)
        {
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

pub fn create(
    cc: &eframe::CreationContext<'_>,
    services: crate::application::Services,
    menu: Box<dyn crate::desktop_menu::DesktopMenu>,
) -> Suite {
    egui_extras::install_image_loaders(&cc.egui_ctx);
    cc.egui_ctx.add_image_loader(std::sync::Arc::new(
        crate::image_loader::OrientedLoader::new(services.decoder.clone()),
    ));
    crate::theme::install(&cc.egui_ctx);
    let preferences = photosuite_application::preferences::Preferences::default();
    let startup = photosuite_application::preferences::StartupPreferences::new(
        services.preferences.clone(),
        std::env::args_os().skip(1).collect(),
    );
    Suite {
        comments: photosuite_application::comments::CommentSession::new(
            services.comments.clone(),
            services.recorder.clone(),
            services.comment_media.clone(),
        ),
        comment_view: Default::default(),
        record_photo: None,
        known_media: Default::default(),
        media_dates: photosuite_application::media_dates::MediaDates::new(
            services.media_dates.clone(),
            services.work_priority.clone(),
        ),
        media_counts: Default::default(),
        native_menu: Some(menu),
        focus_search: false,
        search_open: false,
        help_open: false,
        audio_header: Default::default(),
        audio_preparation: photosuite_application::diary_preparation::AudioPreparation::new(
            services.audio_source.clone(),
            services.work_priority.clone(),
        ),
        viewer_audio: Default::default(),
        organization: photosuite_application::organization::OrganizationSession::new(
            services.organization.clone(),
        ),
        tag_view: Default::default(),
        diary: photosuite_application::diary::DiarySession::with_priority(
            services.work_priority.clone(),
        ),
        diary_catalog: photosuite_application::diary::DiaryCatalog::with_priority(
            services.work_priority.clone(),
        ),
        remember_after_scan: false,
        storyboard: Default::default(),
        preferences: preferences.clone(),
        persistence: Some(photosuite_application::preferences::Persistence::new(
            services.preferences.clone(),
            preferences.clone(),
        )),
        library_settings: Default::default(),
        startup: Some(startup),
        preferences_error: String::new(),
        options_open: false,
        import_panel: Default::default(),
        bindings: Default::default(),
        write_exif: preferences.write_exif,
        editor: Default::default(),
        size: preferences.preview_size,
        square_previews: preferences.square_previews,
        show_library: preferences.show_library,
        status: "Start-Einstellungen werden geladen …".into(),
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
    }
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
