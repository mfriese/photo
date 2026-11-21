mod audio_header;
mod badges;
mod captions;
mod chrome;
mod comments;
mod diary;
mod gallery;
mod icons;
mod inline_recording;
mod library;
mod menu;
mod photo_location;
mod place_tree;
mod processing_ui;
mod startup;
mod tag_filter;
mod tags;
mod transcription_control;
mod viewer;
mod wall;
use crate::domain::Media;
use eframe::egui;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::mpsc;
pub struct Suite {
    processing_repeats: photosuite_application::processing::Repeats,
    audio_library: photosuite_application::audio_library::Session,
    transcription_queue: photosuite_application::transcription::queue::QueueSession,
    transcription_seen: Option<transcription_control::Seen>,
    transcription_complete: bool,
    transcription: photosuite_application::transcription::ModelSession,
    geodata: photosuite_application::geodata::Session,
    photo_location: photosuite_application::photo_location::Session,
    location_open: bool,
    geo_markers: photosuite_application::media_geo::Catalog,
    geo_sweep: photosuite_application::geo_sweep::Session,
    place_filter: photosuite_application::place_filter::Session,
    place_scope: Option<(Option<PathBuf>, Option<String>)>,
    place_refresh: Option<(usize, bool, usize)>,
    place_dataset: Option<String>,
    place_options: Option<library::PlaceOptionsCache>,
    index_maintenance: photosuite_application::library_index::Session,
    settings_view: crate::options_ui::SettingsView,
    gallery: gallery::GalleryState,
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
    duplicate_dialog: crate::duplicates_ui::DuplicateDialog,
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
        self.load_library(path, remember, false);
    }
    fn load_library(&mut self, path: PathBuf, remember: bool, refresh_tags: bool) {
        if self.root.as_ref() != Some(&path) {
            self.geo_markers.clear();
            self.photo_location.cancel();
            self.location_open = false;
            self.index_maintenance.cancel();
        }
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
        if refresh_tags
            || self.root.as_ref() != Some(&path)
            || self.organization.catalog().is_none()
        {
            if let Err(error) = self.organization.open_cached(path.clone()) {
                self.status = error;
                self.tag_view.open = true;
                return;
            }
            self.tag_view.clear_scope();
        }
        if let Some(startup) = self.startup.as_mut() {
            startup.cancel_for_navigation();
        }
        self.processing_repeats.clear();
        self.transcription_queue.reset();
        self.geo_sweep.reset();
        self.place_filter.reset();
        self.place_scope = None;
        self.place_refresh = None;
        self.place_options = None;
        self.transcription_seen = None;
        self.transcription_complete = false;
        self.audio_preparation.reset();
        self.audio_library.open(&path);
        self.organization.begin_media_scan(&path);
        self.comments.load_cached(path.clone());
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
        self.root = Some(path.clone());
        if remember {
            self.preferences.remember_library(path.clone());
            self.persist_preferences();
        }
        self.items.clear();
        self.gallery.selection.clear();
        self.known_media.clear();
        self.media_dates.clear();
        self.geo_markers.clear();
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
        let Some(media) = self.items.get(index) else {
            self.selected = None;
            self.storyboard.clear();
            return;
        };
        if command == crate::commands::Command::PlayVideo {
            if let Err(error) =
                photosuite_application::playback::play(media, self.services.video_player.as_ref())
            {
                self.status = error;
            }
            return;
        }
        let context = ctx.clone();
        match self.editor.execute(
            command,
            media,
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
        self.startup
            .as_ref()
            .is_none_or(|startup| !startup.waiting())
            && !self.library_settings.is_loading()
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
        let library_ready = self.settings_ready();
        let mut p = self.snapshot();
        let index_action = crate::options_ui::show(
            ctx,
            &mut self.options_open,
            &mut p,
            &mut self.settings_view,
            crate::options_ui::SettingsContext {
                library_ready,
                index: Some(&mut self.index_maintenance),
                root: self.root.as_deref(),
                can_rebuild_index: !self.comments.busy()
                    && !self.comments.has_unsaved()
                    && !self.organization.dirty()
                    && !self.organization.saving()
                    && !self.transcription_queue.busy(),
                transcription: Some(&mut self.transcription),
                geodata: Some(&mut self.geodata),
                saving: self.editor.is_saving(),
                bindings: &mut self.bindings.visible,
                error: &self.preferences_error,
                comments: crate::options_ui::CommentIndexStatus {
                    available: self.comments.can_rebuild(),
                    rebuilding: self.comments.is_rebuilding(),
                    message: if self.comments.is_rebuilding() {
                        &self.comments.rebuild_progress().phase
                    } else {
                        self.comments.message()
                    },
                    error: self.comments.error(),
                },
            },
        );
        if let Some(path) = self.settings_view.transcription.take_reveal()
            && let Err(error) = self.services.opener.reveal(&path)
        {
            self.status = error;
        }
        if let Some(path) = self.settings_view.geodata.take_reveal()
            && let Err(error) = self.services.opener.reveal(&path)
        {
            self.status = error;
        }
        match index_action {
            Some(crate::options_ui::CommentIndexAction::Rebuild) => {
                if let Err(error) = self.comments.rebuild() {
                    self.status = error;
                }
            }
            Some(crate::options_ui::CommentIndexAction::Cancel) => self.comments.cancel_rebuild(),
            None => {}
        }
        if self.settings_ready() {
            self.apply_preferences(p);
            self.persist_preferences();
        }
    }
    fn poll_library_settings(&mut self, ctx: &egui::Context) {
        if let Some((root, result)) = self.library_settings.poll()
            && self.root.as_ref() == Some(&root)
        {
            match result {
                Ok(loaded) => {
                    let mut preferences = loaded.unwrap_or_default();
                    preferences.last_library = self.preferences.last_library.clone();
                    preferences.recent_libraries = self.preferences.recent_libraries.clone();
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
        if self.root.is_none() {
            return;
        }
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
        let tagged = match (self.organization.root(), self.organization.catalog()) {
            (Some(root), Some(catalog)) => photosuite_application::organization::filter_all(
                &self.items,
                root,
                catalog,
                &base,
                &self.tag_view.filters,
            ),
            _ => base,
        };
        match &self.root {
            Some(root) => self.place_filter.catalog().filter(
                &self.items,
                root,
                &tagged,
                &self.place_filter.selected,
            ),
            None => tagged,
        }
    }

    fn unfiltered_visible(&self) -> Vec<usize> {
        crate::application::visible(
            &self.items,
            &self.query,
            self.group.as_deref(),
            self.videos_only,
        )
    }
}
impl eframe::App for Suite {
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.processing_repeats.clear();
        self.diary.cancel();
        let _ = self
            .services
            .audio_player
            .control(photosuite_application::diary::AudioControl::Stop);
        if let Err(error) = self.transcription_queue.shutdown() {
            eprintln!("PhotoSuite beim Beenden: {error}");
        }
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.index_maintenance.poll();
        if self.index_maintenance.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if let Some(root) = self.index_maintenance.take_completed()
            && self.root.as_ref() == Some(&root)
        {
            self.load_library(root, true, true);
        }
        let place_refresh = (
            self.geo_sweep.progress().checked,
            self.photo_location.busy(),
            self.items.len(),
        );
        if self.place_refresh != Some(place_refresh) {
            self.place_filter.refresh();
            self.place_refresh = Some(place_refresh);
        }
        self.place_filter.poll(self.root.as_deref());
        if self.place_filter.pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.geo_markers.poll();
        if self.geo_markers.loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
        self.photo_location.poll();
        if self.photo_location.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        self.geodata.poll();
        let dataset = self
            .geodata
            .prepared()
            .filter(|p| p.enabled && p.ready)
            .and_then(|p| p.database.as_ref())
            .map(|db| db.source_sha256.clone());
        if dataset != self.place_dataset {
            self.place_dataset = dataset;
            self.geo_sweep.retry();
            self.place_filter.refresh();
        }
        if self.geodata.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        self.transcription.poll();
        if self.transcription.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
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
                    && !self.duplicate_dialog.visible
                    && !self.duplicate_dialog.running()
                    && !self.import_panel.visible
                    && !self.comments.is_open()
                    && !self.tag_view.open
                    && self.gallery.pending.is_none()
                    && !self.gallery.job.running()
                    && self.gallery.report.is_none()
                    && self.settings_ready(),
            )
        };
        self.poll_library_settings(ctx);
        if let Some(result) = self.collection.poll() {
            match result {
                Ok(collection) => {
                    self.transcription_complete =
                        !self.collection.is_loading() && collection.unreadable == 0;
                    if self.audio_library.complete() {
                        self.organization.observe_media(
                            &collection.root,
                            &collection.items,
                            !self.collection.is_loading() && collection.unreadable == 0,
                        );
                    }
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
                        self.preferences.remember_library(collection.root);
                        self.persist_location();
                    }
                }
                Err(error) => self.status = error,
            }
            self.remember_after_scan = false;
        }
        self.poll_transcription_queue(ctx);
        if self.collection.is_loading() {
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
        self.gallery_actions(ctx);
        self.window_title(ctx);
        self.search_dialog(ctx);
        self.status_bar(ctx);
        self.library_sidebar(ctx);
        self.prepare_tag_filter();
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
        self.tag_view.show(ctx, &mut self.organization);
        self.photo_location_dialog(ctx);
        self.comment_dialog(ctx);
        self.bindings.show(ctx);
        self.settings(ctx);
        self.help(ctx);
        self.startup_dialog(ctx);
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
            self.refresh_changed_library(path);
        }
        if let Some(path) = self.duplicate_dialog.show(ctx, &self.services) {
            self.refresh_changed_library(path);
        }
    }
}
impl Suite {
    fn refresh_changed_library(&mut self, path: PathBuf) {
        if self.root.as_ref().is_none_or(|root| root == &path) {
            self.load_library(path, true, true);
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
        processing_repeats: Default::default(),
        transcription_queue: photosuite_application::transcription::queue::QueueSession::new(
            services.transcription_queue.clone(),
            services.transcription_engine.clone(),
            services.transcription_power.clone(),
        )
        .with_automatic_start(),
        transcription_seen: None,
        transcription_complete: false,
        geodata: photosuite_application::geodata::Session::new(services.geodata.clone()),
        photo_location: photosuite_application::photo_location::Session::new(
            services.media_geo.clone(),
            services.geodata.clone(),
            services.places.clone(),
        ),
        location_open: false,
        geo_sweep: photosuite_application::geo_sweep::Session::new(
            services.media_geo.clone(),
            services.work_priority.clone(),
        )
        .with_places(services.geodata.clone(), services.places.clone()),
        place_filter: photosuite_application::place_filter::Session::new(
            services.location_index.clone(),
            services.work_priority.clone(),
        ),
        place_scope: None,
        place_refresh: None,
        place_dataset: None,
        place_options: None,
        geo_markers: photosuite_application::media_geo::Catalog::new(
            services.media_geo.clone(),
            services.work_priority.clone(),
        ),
        index_maintenance: photosuite_application::library_index::Session::new(
            services.index_rebuilder.clone(),
        ),
        transcription: photosuite_application::transcription::ModelSession::new(
            services.transcription.clone(),
        ),
        settings_view: Default::default(),
        gallery: Default::default(),
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
        audio_library: photosuite_application::audio_library::Session::new(
            services.audio_library.clone(),
        ),
        audio_header: Default::default(),
        audio_preparation: photosuite_application::diary_preparation::AudioPreparation::with_cache(
            services.audio_source.clone(),
            services.work_priority.clone(),
            services.duration_cache.clone(),
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
        duplicate_dialog: Default::default(),
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
