mod diary;
mod library;
mod toolbar;
mod viewer;
mod wall;
use crate::domain::Media;
use eframe::egui;
use std::{path::PathBuf, sync::mpsc};
pub struct Suite {
    diary: photosuite_application::diary::DiarySession,
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
    receiver: Option<mpsc::Receiver<(Vec<Media>, usize)>>,
    status: String,
    videos_only: bool,
    square_previews: bool,
    show_library: bool,
}
impl Suite {
    fn load(&mut self, path: PathBuf) {
        self.storyboard.clear();
        self.diary.cancel();
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&path))
                .unwrap_or(path)
        };
        self.receiver = Some(crate::application::load_collection(
            self.services.repository.clone(),
            path.clone(),
        ));
        self.preferences.last_library = Some(path.clone());
        self.persist_preferences();
        self.root = Some(path);
        self.items.clear();
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
        self.diary.poll(self.services.audio_player.as_ref());
        if self.diary.is_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(saved) = self.editor.poll() {
            match saved.result {
                Ok(()) => {
                    for turns in 0..4 {
                        ctx.forget_image(&format!("oriented://{turns}/{}", saved.path.display()));
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
        if let Some(rx) = &self.receiver {
            match rx.try_recv() {
                Ok((items, errors)) => {
                    self.status =
                        format!("{} Medien · {} nicht lesbare Einträge", items.len(), errors);
                    self.items = items;
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100))
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.status="Sammlung konnte nicht geladen werden: Hintergrundauftrag unerwartet beendet".into();
                }
            }
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
        self.media_viewer(ctx, &visible, &mut pending);
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
            self.load(path);
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
    let persistence = Some(photosuite_application::preferences::Persistence::new(
        services.preferences.clone(),
        preferences.clone(),
    ));
    let mut app = Suite {
        diary: Default::default(),
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
        receiver: None,
        videos_only: preferences.videos_only,
    };
    if let Some(path) = preferences.startup_library(std::env::args_os().skip(1)) {
        app.load(path);
    }
    app
}

fn image_uri(path: &std::path::Path, rotations: &std::collections::HashMap<PathBuf, u8>) -> String {
    format!(
        "oriented://{}/{}",
        rotations.get(path).copied().unwrap_or(0),
        path.display()
    )
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use std::sync::Arc;
    struct UnusedAdapters;
    impl photosuite_application::diary::AudioSource for UnusedAdapters {
        fn probe(
            &self,
            _: &std::path::Path,
        ) -> Result<photosuite_application::diary::AudioProbe, String> {
            unreachable!()
        }
        fn capture_time(&self, _: &std::path::Path) -> Result<crate::domain::CaptureTime, String> {
            unreachable!()
        }
    }
    impl photosuite_application::diary::AudioPlayer for UnusedAdapters {
        fn open(&self, _: &crate::domain::diary::YearTimeline, _: f64) -> Result<(), String> {
            unreachable!()
        }
    }
    impl crate::ports::MediaRepository for UnusedAdapters {
        fn scan(&self, _: &std::path::Path) -> (Vec<Media>, usize) {
            panic!("layout must not scan")
        }
    }
    impl crate::ports::ImageDecoder for UnusedAdapters {
        fn decode(&self, _: &std::path::Path, _: u8) -> Result<crate::ports::DecodedImage, String> {
            panic!("layout must not decode")
        }
    }
    impl crate::ports::MediaOpener for UnusedAdapters {
        fn open(&self, _: &std::path::Path) -> Result<(), String> {
            panic!("layout must not open")
        }
    }
    impl crate::ports::OrientationWriter for UnusedAdapters {
        fn write_rotation(&self, _: &std::path::Path, _: u8) -> Result<(), String> {
            panic!("layout must not write")
        }
    }
    impl crate::import::ImportAdapter for UnusedAdapters {
        fn validate(&self, _: &std::path::Path, _: &std::path::Path) -> Result<(), String> {
            unreachable!()
        }
        fn discover(&self, _: &std::path::Path) -> Result<Vec<Media>, String> {
            unreachable!()
        }
        fn capture_time(&self, _: &Media) -> Result<crate::domain::CaptureTime, String> {
            unreachable!()
        }
        fn exists(&self, _: &std::path::Path) -> Result<bool, String> {
            unreachable!()
        }
        fn identical(&self, _: &std::path::Path, _: &std::path::Path) -> Result<bool, String> {
            unreachable!()
        }
        fn copy_verified(
            &self,
            _: &std::path::Path,
            _: &std::path::Path,
            _: &std::sync::atomic::AtomicBool,
            _: &dyn Fn(crate::import::CopyProgress),
        ) -> Result<(), String> {
            unreachable!()
        }
    }
    impl photosuite_application::preferences::PreferencesStore for UnusedAdapters {
        fn load(&self) -> Result<photosuite_application::preferences::Preferences, String> {
            Ok(Default::default())
        }
        fn save(&self, _: &photosuite_application::preferences::Preferences) -> Result<(), String> {
            panic!("layout must not save")
        }
    }
    impl crate::ports::VideoPreviewSource for UnusedAdapters {
        fn duration(&self, _: &std::path::Path) -> Result<f64, String> {
            unreachable!()
        }
        fn frame_at(
            &self,
            _: &std::path::Path,
            _: f64,
        ) -> Result<crate::ports::VideoFrame, String> {
            unreachable!()
        }
    }
    impl crate::ports::VideoPlayer for UnusedAdapters {
        fn play(&self, _: &std::path::Path) -> Result<(), String> {
            unreachable!()
        }
    }
    impl photosuite_application::duplicates::DuplicateImages for UnusedAdapters {
        fn compare(
            &self,
            _: &std::path::Path,
            _: &std::path::Path,
        ) -> Result<photosuite_application::duplicates::Comparison, String> {
            unreachable!()
        }
        fn trash_identical(
            &self,
            _: &std::path::Path,
            _: &std::path::Path,
            _: &std::path::Path,
            _: &std::sync::atomic::AtomicBool,
        ) -> Result<(), String> {
            unreachable!()
        }
    }
    fn test_suite() -> Suite {
        let adapters = Arc::new(UnusedAdapters);
        Suite {
            diary: Default::default(),
            storyboard: Default::default(),
            preferences: Default::default(),
            persistence: None,
            preferences_error: String::new(),
            options_open: false,
            services: crate::application::Services {
                audio_source: adapters.clone(),
                audio_player: adapters.clone(),
                duplicates: adapters.clone(),
                video_player: adapters.clone(),
                video_preview: adapters.clone(),
                preferences: adapters.clone(),
                importer: adapters.clone(),
                repository: adapters.clone(),
                decoder: adapters.clone(),
                opener: adapters.clone(),
                orientation_writer: adapters,
            },
            import_panel: Default::default(),
            bindings: Default::default(),
            write_exif: true,
            editor: Default::default(),
            root: None,
            items: vec![],
            query: String::new(),
            group: None,
            size: 2,
            selected: None,
            receiver: None,
            status: String::new(),
            videos_only: true,
            square_previews: true,
            show_library: true,
        }
    }
    #[test]
    fn opening_library_persists_before_scan_finishes() {
        use photosuite_application::preferences::{Persistence, PreferencesStore};
        struct WaitingRepository(std::sync::Mutex<mpsc::Receiver<()>>);
        impl crate::ports::MediaRepository for WaitingRepository {
            fn scan(&self, _: &std::path::Path) -> (Vec<Media>, usize) {
                self.0.lock().unwrap().recv().unwrap();
                (vec![], 1)
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(crate::preferences_fs::PlistPreferences::at(
            dir.path().join("prefs.plist"),
        ));
        let (release, wait) = mpsc::channel();
        let mut suite = test_suite();
        suite.services.repository = Arc::new(WaitingRepository(std::sync::Mutex::new(wait)));
        suite.persistence = Some(Persistence::new(store.clone(), suite.snapshot()));
        let library = dir.path().join("Meine Fotos");
        suite.load(library.clone());
        drop(suite.persistence.take());
        let restored = store.load().unwrap();
        assert_eq!(restored.last_library, Some(library.clone()));
        assert_eq!(restored.startup_library([]), Some(library));
        assert!(matches!(
            suite.receiver.as_ref().unwrap().try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        release.send(()).unwrap();
    }
    #[test]
    fn gallery_theme_fits_small_and_large_windows() {
        for width in [360., 600., 1280.] {
            for populated in [false, true] {
                let ctx = egui::Context::default();
                crate::theme::install(&ctx);
                let mut suite = test_suite();
                if populated {
                    suite.root = Some("/Fotos/Demosammlung".into());
                    suite.items = (0..18)
                        .map(|i| {
                            Media::from_path(
                                format!("/Fotos/Demosammlung/2026/09/img_202609{i:02}_153000.jpg")
                                    .into(),
                                "2026/09".into(),
                            )
                            .unwrap()
                        })
                        .collect();
                }
                for _ in 0..3 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 850.),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            suite.toolbar(ctx);
                            suite.status_bar(ctx);
                            suite.library_sidebar(ctx);
                            suite.photo_wall(ctx, &(0..suite.items.len()).collect::<Vec<_>>());
                        },
                    );
                    for shape in output.shapes {
                        if let egui::Shape::Text(text) = shape.shape {
                            if text.galley.job.text.is_empty() {
                                continue;
                            }
                            let rect = text.visual_bounding_rect();
                            assert!(
                                rect.left() >= -1. && rect.right() <= width + 1.,
                                "Gallery overflow at {width}, populated={populated}: {:?} {rect:?}",
                                text.galley.job.text
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn library_years_start_collapsed_and_expand_on_click() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        ctx.style_mut(|style| style.animation_time = 0.);
        let mut suite = test_suite();
        suite.root = Some("/Fotos/Demo".into());
        suite.items =
            vec![Media::from_path("/Fotos/Demo/2026/09/a.jpg".into(), "2026/09".into()).unwrap()];
        let mut draw = |events| {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| suite.library_sidebar(ctx),
            );
            output
                .shapes
                .into_iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = shape.shape {
                        Some((text.galley.job.text.clone(), text.visual_bounding_rect()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        let initial = draw(vec![]);
        assert!(
            !initial
                .iter()
                .any(|(text, _)| text == "September" || text == "Ganzes Jahr")
        );
        let header = initial
            .iter()
            .find(|(text, _)| text == "2026")
            .unwrap()
            .1
            .center();
        draw(vec![
            egui::Event::PointerMoved(header),
            egui::Event::PointerButton {
                pos: header,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: header,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let expanded = draw(vec![]);
        assert!(expanded.iter().any(|(text, _)| text == "September"));
        assert!(expanded.iter().any(|(text, _)| text == "♪ Diary"));
        assert!(expanded.iter().any(|(text, _)| text == "Ganzes Jahr"));
    }
    #[test]
    fn diary_file_view_remains_readable_at_supported_widths() {
        for width in [360., 600., 1280.] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let mut suite = test_suite();
            suite.root = Some("/Fotos".into());
            suite.group = Some("2026/diary".into());
            suite.items = vec![
                Media::from_path(
                    "/Fotos/2026/diary/aud_20260916_035811.wav".into(),
                    "2026/diary".into(),
                )
                .unwrap(),
            ];
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    suite.toolbar(ctx);
                    suite.status_bar(ctx);
                    suite.library_sidebar(ctx);
                    suite.photo_wall(ctx, &[0]);
                },
            );
            for shape in output.shapes {
                if let egui::Shape::Text(text) = shape.shape {
                    if text.galley.job.text.is_empty() {
                        continue;
                    }
                    let rect = text.visual_bounding_rect();
                    assert!(
                        rect.left() >= -1. && rect.right() <= width + 1.,
                        "Diary overflow at {width}: {:?} {rect:?}",
                        text.galley.job.text
                    );
                }
            }
        }
    }
    #[test]
    fn toolbar_text_stays_inside_narrow_and_wide_windows() {
        for width in [360., 600., 1280.] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let mut suite = test_suite();
            suite.root = Some(
                "/Fotos/Ein sehr langer Bibliothekspfad/Urlaub und Familie/Meine Sammlung".into(),
            );
            suite.status = "Sammlung wird gelesen …".repeat(20);
            suite.preferences_error = "Speichern fehlgeschlagen".into();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    suite.toolbar(ctx);
                    suite.status_bar(ctx);
                },
            );
            let mut text_count = 0;
            for shape in output.shapes {
                if let egui::Shape::Text(text) = shape.shape {
                    // Empty TextEdit galleys contain no visible glyph bounds.
                    if text.galley.job.text.is_empty() {
                        continue;
                    }
                    text_count += 1;
                    let rect = text.visual_bounding_rect();
                    assert!(
                        rect.min.x >= -1. && rect.max.x <= width + 1.,
                        "text overflow at width {width}: {rect:?}, {:?}",
                        text.galley.job.text
                    );
                }
            }
            assert!(text_count > 3);
        }
    }
}
