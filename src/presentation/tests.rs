use super::*;
use std::sync::Arc;
struct UnusedAdapters;
impl photosuite_application::diary::AudioSource for UnusedAdapters {
    fn file_size(&self, _: &std::path::Path) -> Result<u64, String> {
        panic!("layout must not stat")
    }

    fn probe(
        &self,
        _: &std::path::Path,
    ) -> Result<photosuite_application::diary::AudioProbe, String> {
        unreachable!()
    }
    fn capture_time(&self, path: &std::path::Path) -> Result<crate::domain::CaptureTime, String> {
        crate::domain::audio_date::select_audio_date(
            &Default::default(),
            path.file_stem().and_then(|s| s.to_str()),
            None,
        )
    }
}
impl photosuite_application::diary::AudioPlayer for UnusedAdapters {
    fn revise(
        &self,
        _: &crate::domain::diary_timeline::DiaryTimeline,
        _: &photosuite_application::diary::VerificationProgress,
    ) -> Result<(), String> {
        unreachable!()
    }

    fn open(
        &self,
        _: &crate::domain::diary_timeline::DiaryTimeline,
        _: &std::path::Path,
        _: &[Media],
    ) -> Result<(), String> {
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
    fn frame_at(&self, _: &std::path::Path, _: f64) -> Result<crate::ports::VideoFrame, String> {
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
        diary_catalog: Default::default(),
        remember_after_scan: false,
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
        collection: Default::default(),
        status: String::new(),
        videos_only: true,
        square_previews: true,
        show_library: true,
    }
}
#[test]
fn audio_photo_event_opens_normal_viewer_with_navigation_and_preserves_diary_filters() {
    struct PhotoClick(std::sync::Mutex<Option<PathBuf>>);
    impl photosuite_application::diary::AudioPlayer for PhotoClick {
        fn revise(
            &self,
            _: &crate::domain::diary_timeline::DiaryTimeline,
            _: &photosuite_application::diary::VerificationProgress,
        ) -> Result<(), String> {
            panic!("click must not revise audio")
        }

        fn open(
            &self,
            _: &crate::domain::diary_timeline::DiaryTimeline,
            _: &std::path::Path,
            _: &[Media],
        ) -> Result<(), String> {
            panic!("click must not restart audio")
        }
        fn poll(&self) -> photosuite_application::diary::AudioPlayerUpdate {
            photosuite_application::diary::AudioPlayerUpdate {
                is_open: true,
                photo_to_open: self.0.lock().unwrap().take(),
            }
        }
    }
    let ctx = egui::Context::default();
    let mut suite = test_suite();
    suite.items = [
        ("/Photos/2023/diary/aud_20231227_225132.wav", "2023/diary"),
        ("/Photos/2023/12/first.jpg", "2023/12"),
        ("/Photos/2023/12/second.jpg", "2023/12"),
    ]
    .into_iter()
    .map(|(path, group)| Media::from_path(path.into(), group.into()).unwrap())
    .collect();
    suite.group = Some("2023/diary".into());
    suite.query = "aud_".into();
    suite.services.audio_player = Arc::new(PhotoClick(std::sync::Mutex::new(Some(
        suite.items[1].path().into(),
    ))));
    suite.poll_audio_player(&ctx);
    assert_eq!(suite.selected, Some(1));
    assert_eq!(suite.group.as_deref(), Some("2023/diary"));
    assert_eq!(suite.query, "aud_");
    assert!(suite.videos_only);
    assert_eq!(suite.visible(), vec![0]);
    assert_eq!(suite.viewer_items(&suite.visible()), vec![1, 2]);
    suite.selected = None;
    suite.poll_audio_player(&ctx);
    assert!(suite.selected.is_none(), "click must be consumed once");
    suite.services.audio_player = Arc::new(PhotoClick(std::sync::Mutex::new(Some(
        "/Other/library.jpg".into(),
    ))));
    suite.poll_audio_player(&ctx);
    assert!(suite.selected.is_none());
    assert!(suite.status.contains("nicht mehr"));
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
    assert!(suite.collection.is_loading());
    assert!(suite.collection.poll().is_none());
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
                        crate::theme::assert_text_glyphs(&ctx, &text);
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
                    crate::theme::assert_text_glyphs(&ctx, &text);
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
    assert!(expanded.iter().any(|(text, _)| text == "Diary"));
    assert!(expanded.iter().any(|(text, _)| text == "Ganzes Jahr"));
}
#[test]
fn importing_elsewhere_keeps_the_open_photo_library_and_saved_path() {
    let mut suite = test_suite();
    suite.root = Some("/Fotos/Sammlung".into());
    suite.preferences.last_library = suite.root.clone();
    suite.refresh_after_import("/Andere/Sammlung".into());
    assert_eq!(suite.root, Some("/Fotos/Sammlung".into()));
    assert_eq!(suite.preferences.last_library, suite.root);
    assert!(!suite.collection.is_loading());
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
        suite
            .diary_catalog
            .ensure(&suite.items, "2026", suite.services.audio_source.clone());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while suite.diary_catalog.is_loading() {
            suite.diary_catalog.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
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
        let texts: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    Some(t.galley.job.text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(texts.contains(&"16.09.2026"));
        assert!(texts.contains(&"03:58:11"));
        assert!(
            !texts
                .iter()
                .any(|text| text.contains("aud_20260916_035811"))
        );
        for shape in output.shapes {
            if let egui::Shape::Text(text) = shape.shape {
                crate::theme::assert_text_glyphs(&ctx, &text);
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
        suite.root =
            Some("/Fotos/Ein sehr langer Bibliothekspfad/Urlaub und Familie/Meine Sammlung".into());
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
                crate::theme::assert_text_glyphs(&ctx, &text);
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
