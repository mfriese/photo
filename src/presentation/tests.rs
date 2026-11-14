use super::*;
use std::sync::Arc;
struct UnusedAdapters;
impl photosuite_application::media_dates::MediaDateSource for UnusedAdapters {
    fn captured_at(&self, _: &Media) -> Result<crate::domain::CaptureTime, String> {
        Err("Keine Testmetadaten".into())
    }
}
impl photosuite_application::diary::AudioSource for UnusedAdapters {
    fn probe(
        &self,
        _: &std::path::Path,
    ) -> Result<photosuite_application::diary::AudioProbe, String> {
        Err("Keine echten Audiodaten im Layouttest".into())
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
    fn reveal(&self, _: &std::path::Path) -> Result<(), String> {
        panic!("test must not reveal files")
    }
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
impl photosuite_application::import_cleanup::CleanupPort for UnusedAdapters {
    fn check(
        &self,
        _: &std::path::Path,
        _: &std::path::Path,
        _: &std::path::Path,
        _: &std::path::Path,
        _: &std::sync::atomic::AtomicBool,
    ) -> Result<photosuite_application::import_cleanup::Proof, String> {
        panic!("layout must not verify files")
    }
    fn trash_checked(
        &self,
        _: &photosuite_application::import_cleanup::Proof,
        _: &std::sync::atomic::AtomicBool,
    ) -> Result<(), String> {
        panic!("layout must not trash files")
    }
}
impl photosuite_application::organization::CatalogRepository for UnusedAdapters {
    fn load(
        &self,
        _: &std::path::Path,
    ) -> Result<photosuite_application::organization::StoredCatalog, String> {
        Ok(photosuite_application::organization::StoredCatalog {
            catalog: Default::default(),
            revision: None,
        })
    }
    fn save(
        &self,
        _: &std::path::Path,
        _: &crate::domain::tags::TagCatalog,
        _: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }
}
pub(super) fn test_suite() -> Suite {
    let adapters = Arc::new(UnusedAdapters);
    Suite {
        comments: photosuite_application::comments::CommentSession::new(
            adapters.clone(),
            adapters.clone(),
            adapters.clone(),
        ),
        comment_view: Default::default(),
        record_photo: None,
        known_media: Default::default(),
        media_dates: photosuite_application::media_dates::MediaDates::new(
            adapters.clone(),
            Default::default(),
        ),
        media_counts: Default::default(),
        native_menu: None,
        focus_search: false,
        search_open: false,
        help_open: false,
        audio_header: Default::default(),
        audio_preparation: photosuite_application::diary_preparation::AudioPreparation::new(
            adapters.clone(),
            Default::default(),
        ),
        viewer_audio: Default::default(),
        organization: photosuite_application::organization::OrganizationSession::new(
            adapters.clone(),
        ),
        tag_view: Default::default(),
        diary: Default::default(),
        diary_catalog: Default::default(),
        remember_after_scan: false,
        storyboard: Default::default(),
        preferences: Default::default(),
        persistence: None,
        library_settings: Default::default(),
        startup: None,
        preferences_error: String::new(),
        options_open: false,
        services: crate::application::Services {
            comments: adapters.clone(),
            comment_media: adapters.clone(),
            recorder: adapters.clone(),
            media_dates: adapters.clone(),
            organization: adapters.clone(),
            work_priority: Default::default(),
            import_cleanup: adapters.clone(),
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
        highlighted_photo: None,
        scroll_to_photo: None,
        collection: Default::default(),
        status: String::new(),
        videos_only: true,
        square_previews: true,
        show_library: true,
    }
}
#[test]
fn audio_photo_event_reveals_year_without_viewer_or_focus_change() {
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
                photo_to_reveal: self.0.lock().unwrap().take(),
                photo_to_record: None,
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
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        suite.poll_audio_player(ctx)
    });
    assert!(
        !output
            .viewport_output
            .values()
            .flat_map(|v| &v.commands)
            .any(|command| matches!(command, egui::ViewportCommand::Focus))
    );
    assert!(suite.selected.is_none());
    assert_eq!(suite.group.as_deref(), Some("2023"));
    assert!(suite.query.is_empty());
    assert!(!suite.videos_only);
    assert_eq!(suite.visible(), vec![1, 2]);
    assert_eq!(
        suite.highlighted_photo.as_deref(),
        Some(suite.items[1].path())
    );
    assert_eq!(
        suite.scroll_to_photo.as_deref(),
        Some(suite.items[1].path())
    );
    suite.scroll_to_photo = None;
    suite.poll_audio_player(&ctx);
    assert!(
        suite.scroll_to_photo.is_none(),
        "click must be consumed once"
    );
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
                        suite.window_title(ctx);
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
            .any(|(text, _)| text == "September" || text == "Photos")
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
    assert!(
        !expanded
            .iter()
            .any(|(text, _)| ["Audio", "Tagebuch", "Kommentare"].contains(&text.as_str()))
    );
    assert!(!expanded.iter().any(|(text, _)| text == "Photos"));
    assert!(!expanded.iter().any(|(text, _)| text == "Ganzes Jahr"));
    let click = |pos| {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    let month = expanded
        .iter()
        .find(|(text, _)| text == "September")
        .unwrap()
        .1
        .center();
    draw(click(month));
    assert_eq!(suite.group.as_deref(), Some("2026/09"));
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
                suite.window_title(ctx);
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
fn title_and_footer_stay_compact_without_a_header_panel() {
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
                let top = ctx.available_rect().top();
                suite.window_title(ctx);
                assert_eq!(
                    ctx.available_rect().top(),
                    top,
                    "window title must not reserve a header"
                );
                suite.status_bar(ctx);
                assert_eq!(
                    ctx.available_rect().top(),
                    top,
                    "footer must not reserve a header"
                );
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
        assert!(text_count >= 1);
        assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|command|matches!(command,egui::ViewportCommand::Title(title) if title.contains("Meine Sammlung"))));
    }
}

#[test]
fn audio_photo_jump_renders_far_target_with_highlight_in_virtualized_wall() {
    for width in [360., 900.] {
        for zoom in [0, 2, 4] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let mut suite = test_suite();
            suite.root = Some("/Photos".into());
            suite.group = Some("2023".into());
            suite.videos_only = false;
            suite.size = zoom;
            suite.items = (0..1000)
                .map(|i| {
                    Media::from_path(
                        format!("/Photos/2023/12/img_{i:04}.jpg").into(),
                        "2023/12".into(),
                    )
                    .unwrap()
                })
                .collect();
            suite.highlighted_photo = Some(suite.items[900].path().into());
            suite.scroll_to_photo = suite.highlighted_photo.clone();
            let visible = suite.visible();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.),
                    )),
                    ..Default::default()
                },
                |ctx| suite.photo_wall(ctx, &visible),
            );
            let highlighted: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.stroke.width == 3. && rect.stroke.color == crate::theme::ACCENT =>
                    {
                        Some((rect.rect, shape.clip_rect))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                highlighted.len(),
                1,
                "target not rendered at width {width}, zoom {zoom}"
            );
            assert!(
                highlighted[0].1.contains_rect(highlighted[0].0),
                "highlight outside viewport: {highlighted:?}"
            );
            assert!(suite.scroll_to_photo.is_none());
            assert!(suite.selected.is_none());
        }
    }
}

#[test]
fn indexed_years_are_clickable_before_any_media_arrives_even_while_animating() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct SlowYears;
    impl crate::ports::MediaRepository for SlowYears {
        fn scan(&self, _: &std::path::Path) -> (Vec<Media>, usize) {
            unreachable!()
        }
        fn partitions(&self, _: &std::path::Path) -> Result<Option<Vec<String>>, String> {
            Ok(Some(vec!["2023".into(), "2025".into()]))
        }
        fn scan_partition(
            &self,
            _: &std::path::Path,
            _: &str,
            cancel: &AtomicBool,
        ) -> (Vec<Media>, usize) {
            while !cancel.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            (Vec::new(), 0)
        }
    }
    let mut suite = test_suite();
    suite.services.repository = Arc::new(SlowYears);
    suite.load("/Fotos/Slow".into());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while suite.collection.years().is_empty() {
        assert!(suite.collection.poll().is_none());
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(suite.items.is_empty());
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let input = |events| egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800., 600.),
        )),
        events,
        ..Default::default()
    };
    let output = ctx.run(input(vec![]), |ctx| suite.library_sidebar(ctx));
    let target = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(text) if text.galley.job.text == "2023" => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .expect("year must be visible without media");
    assert!(suite.group.is_none());
    let _ = ctx.run(
        input(vec![
            egui::Event::PointerMoved(target),
            egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]),
        |ctx| suite.library_sidebar(ctx),
    );
    assert_eq!(suite.group.as_deref(), Some("2023"));
    assert!(suite.collection.is_loading());
    assert!(suite.items.is_empty());
}

#[test]
fn sidebar_months_are_compact_and_clickable_across_full_column_width() {
    for width in [360., 600., 1280.] {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        ctx.style_mut(|style| {
            style.animation_time = 0.;
            style.spacing.scroll = egui::style::ScrollStyle::solid();
        });
        let mut suite = test_suite();
        suite.root = Some("/Fotos/Demo".into());
        suite.items = (1..=12)
            .map(|month| {
                Media::from_path(
                    format!("/Fotos/Demo/2026/{month:02}/a.jpg").into(),
                    format!("2026/{month:02}"),
                )
                .unwrap()
            })
            .collect();
        for year in 1990..2026 {
            suite.items.push(
                Media::from_path(
                    format!("/Fotos/Demo/{year}/01/a.jpg").into(),
                    format!("{year}/01"),
                )
                .unwrap(),
            );
        }
        let mut draw = |events| {
            let mut right = 0.;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 480.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    suite.library_sidebar(ctx);
                    right = ctx.available_rect().left();
                },
            );
            (output, right)
        };
        let text_rect = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        Some(text.visual_bounding_rect())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {label} at {width}"))
        };
        let click = |pos| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let (initial, _) = draw(vec![]);
        let header = text_rect(&initial, "2026").center();
        draw(click(header));
        let (expanded, right) = draw(vec![]);
        let january = text_rect(&expanded, "Januar");
        let february = text_rect(&expanded, "Februar");
        assert!(
            february.center().y - january.center().y <= 22.,
            "month rows too tall at {width}"
        );
        assert!(expanded.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Rect(rect) if rect.rect.width()<=16. && rect.rect.height()>30. && rect.rect.right()>right-24. && rect.rect.right()<=right)),"scrollbar must sit at the right column edge at {width}");
        let december = text_rect(&expanded, "Dezember");
        draw(click(egui::pos2(right - 28., december.center().y)));
        assert_eq!(
            suite.group.as_deref(),
            Some("2026/12"),
            "full-width month hit target at {width}"
        );
    }
}

#[test]
fn library_switch_restores_yaml_and_keeps_previous_edits_in_previous_library() {
    use photosuite_application::preferences::{Persistence, Preferences, PreferencesStore};
    struct Empty;
    impl crate::ports::MediaRepository for Empty {
        fn scan(&self, _: &std::path::Path) -> (Vec<Media>, usize) {
            (Vec::new(), 0)
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let a = directory.path().join("a");
    let b = directory.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let legacy_path = directory.path().join("prefs.plist");
    let store = Arc::new(crate::library_preferences_fs::LibraryPreferences::new(
        crate::library_preferences_fs::LibraryPreferences::locator_path(&legacy_path),
        Arc::new(crate::preferences_fs::PlistPreferences::at(legacy_path)),
    ));
    store
        .save(&Preferences {
            preview_size: 0,
            show_library: false,
            library_scope: Some(a.clone()),
            ..Default::default()
        })
        .unwrap();
    store
        .save(&Preferences {
            preview_size: 4,
            square_previews: false,
            library_scope: Some(b.clone()),
            ..Default::default()
        })
        .unwrap();
    let mut suite = test_suite();
    suite.services.repository = Arc::new(Empty);
    suite.services.preferences = store.clone();
    suite.persistence = Some(Persistence::new(store.clone(), suite.snapshot()));
    let ctx = egui::Context::default();
    let wait = |suite: &mut Suite| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while suite.library_settings.is_loading() {
            suite.poll_library_settings(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    };
    suite.load(a.clone());
    wait(&mut suite);
    assert_eq!(suite.size, 0);
    assert!(!suite.show_library);
    suite.size = 1;
    suite.persist_preferences();
    suite.load(b.clone());
    wait(&mut suite);
    assert_eq!(suite.size, 4);
    assert!(!suite.square_previews);
    assert!(suite.show_library);
    drop(suite.persistence.take());
    assert_eq!(store.load_library(&a).unwrap().unwrap().preview_size, 1);
    assert_eq!(store.load_library(&b).unwrap().unwrap().preview_size, 4);
    assert_eq!(store.load().unwrap().last_library, Some(b));
}

#[test]
fn tag_cloud_includes_non_album_tags_and_preserves_year_navigation() {
    use photosuite_application::organization::Action;
    let mut suite = test_suite();
    suite.root = Some("/Fotos".into());
    suite.organization.open("/Fotos".into()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while suite.organization.loading() {
        suite.organization.poll();
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    suite.items = vec![
        Media::from_path("/Fotos/2023/01/a.jpg".into(), "2023/01".into()).unwrap(),
        Media::from_path("/Fotos/2024/01/a.jpg".into(), "2024/01".into()).unwrap(),
        Media::from_path("/Fotos/2023/diary/a.wav".into(), "2023/diary".into()).unwrap(),
    ];
    let file = suite.organization.file(suite.items[0].path()).unwrap();
    suite
        .organization
        .apply(Action::Create {
            name: "Urlaub".into(),
            file: Some(file),
        })
        .unwrap();
    for name in ["A", "B", "C"] {
        suite
            .organization
            .apply(Action::Create {
                name: name.into(),
                file: None,
            })
            .unwrap();
    }
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    ctx.style_mut(|style| style.animation_time = 0.);
    let input = |events| egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800., 600.),
        )),
        events,
        ..Default::default()
    };
    let output = ctx.run(input(vec![]), |ctx| suite.library_sidebar(ctx));
    let texts: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => {
                Some((text.galley.job.text.as_str(), text.visual_bounding_rect()))
            }
            _ => None,
        })
        .collect();
    for removed in ["Mediathek", "Alle Erinnerungen", "Tags & Alben …", "Alben"] {
        assert!(!texts.iter().any(|(text, _)| *text == removed));
    }
    let bounds = |label| texts.iter().find(|(text, _)| *text == label).unwrap().1;
    assert!(bounds("Alles").bottom() < bounds("2023").top());
    assert!(bounds("Tags").top() > bounds("2023").bottom());
    assert!(
        (bounds("a").bottom() - bounds("b").bottom()).abs() < 2.,
        "short tags should share a row"
    );
    let target = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == "urlaub" => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .expect("album in sidebar");
    let _ = ctx.run(
        input(vec![
            egui::Event::PointerMoved(target),
            egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]),
        |ctx| suite.library_sidebar(ctx),
    );
    assert_eq!(suite.tag_view.active.as_deref(), Some("urlaub"));
    suite.videos_only = false;
    assert_eq!(suite.visible(), vec![0]);
    suite.tag_view.active = None;
    suite.group = Some("2024".into());
    assert_eq!(suite.visible(), vec![1]);
    let point = bounds("Alles").center();
    let _ = ctx.run(
        input(vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]),
        |ctx| suite.library_sidebar(ctx),
    );
    assert!(suite.group.is_none() && suite.tag_view.active.is_none());
    assert_eq!(suite.visible(), vec![0, 1]);
}
#[test]
fn tag_dialog_fits_narrow_windows_and_mixed_albums_do_not_send_audio_to_image_viewer() {
    use photosuite_application::organization::Action;
    for width in [360., 600., 1280.] {
        let mut suite = test_suite();
        suite.organization.open("/Fotos".into()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while suite.organization.loading() {
            suite.organization.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        suite
            .organization
            .apply(Action::Create {
                name: "Familienurlaub Sommer".into(),
                file: None,
            })
            .unwrap();
        suite.tag_view.open = true;
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        for _ in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.),
                    )),
                    ..Default::default()
                },
                |ctx| suite.tag_view.show(ctx, &mut suite.organization),
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
                        "tag dialog overflows {width}: {} {rect:?}",
                        text.galley.job.text
                    );
                }
            }
        }
        suite.items = vec![
            Media::from_path("/Fotos/2023/01/a.jpg".into(), "2023/01".into()).unwrap(),
            Media::from_path("/Fotos/2023/diary/a.wav".into(), "2023/diary".into()).unwrap(),
        ];
        suite.selected = Some(0);
        assert_eq!(suite.viewer_items(&[0, 1]), vec![0]);
    }
}

#[test]
fn tag_checkbox_assigns_and_removes_the_current_file() {
    use photosuite_application::organization::Action;
    let mut suite = test_suite();
    suite.organization.open("/Fotos".into()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while suite.organization.loading() {
        suite.organization.poll();
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    suite
        .organization
        .apply(Action::Create {
            name: "Urlaub".into(),
            file: None,
        })
        .unwrap();
    let file = suite
        .organization
        .file(std::path::Path::new("/Fotos/2023/01/a.jpg"))
        .unwrap();
    suite.tag_view.edit(file);
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    ctx.style_mut(|style| style.animation_time = 0.);
    let input = |events| egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800., 600.),
        )),
        events,
        ..Default::default()
    };
    let mut output = ctx.run(input(vec![]), |ctx| {
        suite.tag_view.show(ctx, &mut suite.organization)
    });
    // First frame can be used by egui for automatic window sizing.
    if !output.shapes.iter().any(
        |shape| matches!(&shape.shape,egui::Shape::Text(text) if text.galley.job.text=="urlaub"),
    ) {
        output = ctx.run(input(vec![]), |ctx| {
            suite.tag_view.show(ctx, &mut suite.organization)
        });
    }
    let target = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == "urlaub" => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .unwrap();
    for assigned in [true, false] {
        let _ = ctx.run(
            input(vec![
                egui::Event::PointerMoved(target),
                egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]),
            |ctx| suite.tag_view.show(ctx, &mut suite.organization),
        );
        assert_eq!(
            suite
                .organization
                .catalog()
                .unwrap()
                .contains("Urlaub", "2023/01/a.jpg"),
            assigned
        );
    }
}

#[test]
fn delayed_startup_never_overwrites_a_manually_opened_library_or_its_settings() {
    use photosuite_application::preferences::{
        Persistence, Preferences, PreferencesStore, StartupPreferences,
    };
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };
    struct Slow {
        entered: AtomicBool,
        release: AtomicBool,
        saved: Mutex<Vec<Preferences>>,
    }
    impl PreferencesStore for Slow {
        fn load(&self) -> Result<Preferences, String> {
            unreachable!()
        }
        fn load_for_startup(
            &self,
            _: Option<&std::path::Path>,
        ) -> Result<(Preferences, Option<String>), String> {
            self.entered.store(true, Ordering::SeqCst);
            while !self.release.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            Ok((
                Preferences {
                    preview_size: 4,
                    last_library: Some("/Old".into()),
                    ..Default::default()
                },
                None,
            ))
        }
        fn load_library(&self, _: &std::path::Path) -> Result<Option<Preferences>, String> {
            Ok(Some(Preferences {
                preview_size: 1,
                ..Default::default()
            }))
        }
        fn save(&self, p: &Preferences) -> Result<(), String> {
            self.saved.lock().unwrap().push(p.clone());
            Ok(())
        }
    }
    struct Empty;
    impl crate::ports::MediaRepository for Empty {
        fn scan(&self, _: &std::path::Path) -> (Vec<Media>, usize) {
            (Vec::new(), 0)
        }
    }
    let store = Arc::new(Slow {
        entered: AtomicBool::new(false),
        release: AtomicBool::new(false),
        saved: Mutex::new(Vec::new()),
    });
    let mut suite = test_suite();
    suite.services.preferences = store.clone();
    suite.services.repository = Arc::new(Empty);
    suite.persistence = Some(Persistence::new(store.clone(), Preferences::default()));
    suite.startup = Some(StartupPreferences::new(store.clone(), Vec::new()));
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    suite.poll_startup(&ctx);
    assert!(!suite.settings_ready());
    suite.load("/Chosen".into());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while suite.library_settings.is_loading() {
        suite.poll_library_settings(&ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(suite.size, 1);
    store.release.store(true, Ordering::SeqCst);
    while suite.startup.is_some() {
        suite.poll_startup(&ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(suite.root.as_deref(), Some(std::path::Path::new("/Chosen")));
    assert_eq!(suite.size, 1);
    drop(suite.persistence.take());
    let saved = store.saved.lock().unwrap();
    assert_eq!(
        saved
            .iter()
            .rfind(|p| p.library_scope.is_none())
            .unwrap()
            .last_library
            .as_deref(),
        Some(std::path::Path::new("/Chosen"))
    );
}

#[derive(Default)]
struct MenuHarness {
    queued: std::sync::Mutex<Vec<crate::desktop_menu::Action>>,
    entries: std::sync::Mutex<Vec<crate::desktop_menu::Entry>>,
}
struct TestMenu(Arc<MenuHarness>);
impl crate::desktop_menu::DesktopMenu for TestMenu {
    fn sync(&mut self, entries: &[crate::desktop_menu::Entry]) -> Result<(), String> {
        *self.0.entries.lock().unwrap() = entries.to_vec();
        Ok(())
    }
    fn drain(&mut self) -> Vec<crate::desktop_menu::Action> {
        std::mem::take(&mut *self.0.queued.lock().unwrap())
    }
}
#[test]
fn native_menu_actions_share_editing_dispatch_and_revalidate_modal_state() {
    use crate::commands::Command;
    use crate::desktop_menu::Action;
    let ctx = egui::Context::default();
    let mut suite = test_suite();
    suite.write_exif = false;
    suite.items = vec![
        Media::from_path(
            "/library/2023/01/img_20230101_120000.jpg".into(),
            "2023/01".into(),
        )
        .unwrap(),
    ];
    suite.selected = Some(0);
    let state = Arc::new(MenuHarness::default());
    suite.native_menu = Some(Box::new(TestMenu(state.clone())));
    *state.queued.lock().unwrap() = vec![Action::Edit(Command::RotateRight)];
    let _ = ctx.run(egui::RawInput::default(), |ctx| suite.poll_menu(ctx));
    assert_eq!(
        suite.editor.rotations().get(suite.items[0].path()),
        Some(&1)
    );
    // A queued Settings activation blocks subsequent edit events from the same batch.
    *state.queued.lock().unwrap() = vec![Action::Settings, Action::Edit(Command::RotateRight)];
    let _ = ctx.run(egui::RawInput::default(), |ctx| suite.poll_menu(ctx));
    assert!(suite.options_open);
    assert_eq!(
        suite.editor.rotations().get(suite.items[0].path()),
        Some(&1)
    );
}
#[test]
fn native_menu_updates_view_checks_and_focuses_search() {
    use crate::desktop_menu::Action;
    let ctx = egui::Context::default();
    let mut suite = test_suite();
    let state = Arc::new(MenuHarness::default());
    suite.native_menu = Some(Box::new(TestMenu(state.clone())));
    let original = suite.square_previews;
    *state.queued.lock().unwrap() =
        vec![Action::ToggleSquare, Action::PreviewSize(4), Action::Find];
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        suite.poll_menu(ctx);
        suite.window_title(ctx);
        suite.search_dialog(ctx);
        suite.poll_menu(ctx);
    });
    assert_eq!(suite.square_previews, !original);
    assert_eq!(suite.size, 4);
    let entries = state.entries.lock().unwrap();
    assert_eq!(
        entries
            .iter()
            .find(|e| e.action == Action::ToggleSquare)
            .unwrap()
            .checked,
        Some(!original)
    );
    assert!(
        !entries
            .iter()
            .find(|e| matches!(e.action, Action::Edit(_)))
            .unwrap()
            .enabled
    );
    assert!(ctx.memory(|m| m.has_focus(egui::Id::new("library_search"))));
}

#[test]
fn native_text_menu_selects_search_text_and_requests_clipboard_without_rotating_photos() {
    use crate::desktop_menu::Action;
    let ctx = egui::Context::default();
    let mut suite = test_suite();
    suite.query = "Sommerurlaub".into();
    suite.focus_search = true;
    suite.search_open = true;
    let state = Arc::new(MenuHarness::default());
    suite.native_menu = Some(Box::new(TestMenu(state.clone())));
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            suite.window_title(ctx);
            suite.search_dialog(ctx);
            suite.poll_menu(ctx);
        });
    }
    *state.queued.lock().unwrap() = vec![Action::SelectAll, Action::Copy];
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        suite.poll_menu(ctx);
        suite.search_dialog(ctx);
    });
    let text = egui::TextEdit::load_state(&ctx, egui::Id::new("library_search")).unwrap();
    let [start, end] = text.cursor.char_range().unwrap().sorted_cursors();
    assert_eq!((start.index, end.index), (0, suite.query.chars().count()));
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::RequestCopy)
    );
    assert!(suite.editor.rotations().is_empty());
}

#[test]
fn closing_search_preserves_filter_and_releases_text_focus() {
    let ctx = egui::Context::default();
    let mut suite = test_suite();
    suite.query = "Urlaub".into();
    suite.search_open = true;
    suite.focus_search = true;
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| suite.search_dialog(ctx));
    }
    assert!(ctx.memory(|m| m.has_focus(egui::Id::new("library_search"))));
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| suite.search_dialog(ctx),
    );
    assert!(!suite.search_open);
    assert_eq!(suite.query, "Urlaub");
    assert!(!ctx.memory(|m| m.has_focus(egui::Id::new("library_search"))));
}

#[test]
fn photo_captions_show_calendar_date_and_time_instead_of_filenames() {
    for width in [360., 800.] {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut suite = test_suite();
        suite.root = Some("/Fotos".into());
        suite.group = Some("2023/12".into());
        suite.items = vec![
            Media::from_path(
                "/Fotos/2023/12/img_20231227_225132.jpg".into(),
                "2023/12".into(),
            )
            .unwrap(),
        ];
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 500.),
                )),
                ..Default::default()
            },
            |ctx| suite.photo_wall(ctx, &[0]),
        );
        let texts: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"27.12.2023"));
        assert!(texts.contains(&"22:51:32"));
        assert!(
            !texts
                .iter()
                .any(|text| text.contains("img_20231227_225132.jpg"))
        );
        for absent in ["Mehr", "Audio", "Diary 2023"] {
            assert!(!texts.contains(&absent));
        }
    }
}
#[test]
fn photo_page_player_is_beside_the_month_and_footer_keeps_counts_first() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut suite = test_suite();
    suite.status = "Bereit".into();
    suite.videos_only = false;
    suite.root = Some("/Fotos".into());
    suite.group = Some("2023/12".into());
    suite.items = vec![
        Media::from_path(
            "/Fotos/2023/12/img_20231227_225132.jpg".into(),
            "2023/12".into(),
        )
        .unwrap(),
        Media::from_path(
            "/Fotos/2023/audio/aud_20231227_225132.wav".into(),
            "2023/audio".into(),
        )
        .unwrap(),
        Media::from_path(
            "/Fotos/2023/12/img_20231227_230000.mp4".into(),
            "2023/12".into(),
        )
        .unwrap(),
    ];
    for width in [360., 800.] {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 500.),
                )),
                ..Default::default()
            },
            |ctx| {
                suite.status_bar(ctx);
                suite.photo_wall(ctx, &[0, 2]);
            },
        );
        let texts: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => {
                    Some((text.galley.job.text.as_str(), text.visual_bounding_rect()))
                }
                _ => None,
            })
            .collect();
        let rect = |name| {
            texts
                .iter()
                .find(|(text, _)| *text == name)
                .unwrap_or_else(|| panic!("missing {name} at {width}"))
                .1
        };
        let title = rect("Dezember 2023");
        let play = rect("▶");
        assert!((title.center().y - play.center().y).abs() < 5.);
        assert!(play.left() > title.left());
        let counts = rect("Medien 3 · Photos 1 · Audios 1 · Videos 1");
        let status = rect("Schreibmodus: JPEG-EXIF · Bereit");
        assert!(counts.left() < status.left());
        assert!((counts.center().y - status.center().y).abs() < 2.);
        assert!(!texts.iter().any(|(text, _)| *text == "Mehr"));
    }
}

impl photosuite_application::comments::CommentRecorder for UnusedAdapters {
    fn devices(&self) -> Result<Vec<photosuite_application::comments::InputDevice>, String> {
        Ok(Vec::new())
    }
    fn start(&self, _: &str) -> Result<(), String> {
        Err("No microphone in tests".into())
    }
    fn command(&self, _: photosuite_application::comments::CaptureCommand) -> Result<(), String> {
        Ok(())
    }
    fn snapshot(&self) -> photosuite_application::comments::CaptureSnapshot {
        Default::default()
    }
}
impl photosuite_application::comments::CommentRepository for UnusedAdapters {
    fn write_photo_record(
        &self,
        _: &std::path::Path,
        comment: &crate::domain::comments::Comment,
    ) -> Result<crate::domain::comments::CommentCatalog, String> {
        let mut record = crate::domain::comments::CommentCatalog::default();
        record.insert(comment.clone())?;
        Ok(record)
    }
    fn read_cache(
        &self,
        _: &std::path::Path,
    ) -> Result<photosuite_application::comments::CommentCache, String> {
        Ok(photosuite_application::comments::CommentCache::Current(
            photosuite_application::comments::StoredComments {
                catalog: Default::default(),
                revision: None,
            },
        ))
    }
    fn save(
        &self,
        _: &std::path::Path,
        _: &crate::domain::comments::CommentCatalog,
        _: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }
}

#[test]
fn photo_recording_dialog_fits_narrow_and_wide_windows_without_microphone_access() {
    for (width, height) in [(360., 400.), (800., 600.)] {
        let mut suite = test_suite();
        suite.root = Some("/library".into());
        suite.items = vec![
            Media::from_path(
                "/library/2008/09/img_20080916_035811.jpg".into(),
                "2008/09".into(),
            )
            .unwrap(),
        ];
        suite.comments.load("/library".into());
        for _ in 0..1000 {
            suite.comments.poll();
            if !suite.comments.busy() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        suite
            .comments
            .open_photo(photosuite_application::comments::PhotoTarget {
                root: "/library".into(),
                file: crate::domain::tags::LibraryFile::new(
                    "2008/09/img_20080916_035811.jpg".into(),
                )
                .unwrap(),
            })
            .unwrap();
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut found = false;
        for _ in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, height),
                    )),
                    ..Default::default()
                },
                |ctx| suite.comment_dialog(ctx),
            );
            let mut status_rect = None;
            let mut device_rect = None;
            for shape in output.shapes {
                if let egui::Shape::Circle(circle) = &shape.shape
                    && circle.radius == 6.
                    && circle.fill == crate::theme::RECORD_RED
                {
                    assert!(
                        circle.center.y + 6. <= height && circle.center.y - 6. >= 0.,
                        "record button must stay visible at {width}x{height}"
                    );
                }
                if let egui::Shape::Text(text) = shape.shape {
                    if text.galley.job.text.is_empty() {
                        continue;
                    }
                    if text.galley.job.text.starts_with("Bereit zur Aufnahme") {
                        status_rect = Some(text.visual_bounding_rect());
                    }
                    if text.galley.job.text == "Systemstandard" {
                        device_rect = Some(text.visual_bounding_rect());
                    }
                    assert_ne!(text.galley.job.text, "Schließen");
                    assert!(!text.galley.job.text.starts_with("Gelb bis Rot:"));
                    crate::theme::assert_text_glyphs(&ctx, &text);
                    if text.galley.job.text.starts_with("Bereit zur Aufnahme") {
                        found = true;
                    }
                    let rect = text.visual_bounding_rect();
                    assert!(
                        rect.left() >= -1. && rect.right() <= width + 1.,
                        "recording text outside window: {} {rect:?}",
                        text.galley.job.text
                    );
                }
            }
            if let (Some(status), Some(device)) = (status_rect, device_rect) {
                assert!(status.right() < device.left());
                assert!((status.center().y - device.center().y).abs() < 3.);
            }
        }
        assert!(found);
        assert!(!suite.comments.has_unsaved());
        assert!(!suite.comments.snapshot().playing);
    }
}

impl photosuite_application::comments::CommentMediaStore for UnusedAdapters {
    fn validate(
        &self,
        _: &photosuite_application::comments::PhotoTarget,
        _: &photosuite_application::comments::CapturedTake,
    ) -> Result<(), String> {
        Err("unused".into())
    }

    fn exists(&self, _: &std::path::Path) -> Result<bool, String> {
        Err("unused".into())
    }
    fn copy_verified(&self, _: &std::path::Path, _: &std::path::Path) -> Result<(), String> {
        Err("unused".into())
    }
}

#[test]
fn player_record_intent_opens_the_recording_target_without_starting_capture_or_changing_gallery() {
    struct RecordIntent(std::sync::Mutex<Option<PathBuf>>);
    impl photosuite_application::diary::AudioPlayer for RecordIntent {
        fn open(
            &self,
            _: &crate::domain::diary_timeline::DiaryTimeline,
            _: &std::path::Path,
            _: &[Media],
        ) -> Result<(), String> {
            panic!("record intent must not reopen playback")
        }
        fn revise(
            &self,
            _: &crate::domain::diary_timeline::DiaryTimeline,
            _: &photosuite_application::diary::VerificationProgress,
        ) -> Result<(), String> {
            panic!("record intent must not revise playback")
        }
        fn poll(&self) -> photosuite_application::diary::AudioPlayerUpdate {
            photosuite_application::diary::AudioPlayerUpdate {
                is_open: true,
                photo_to_reveal: None,
                photo_to_record: self.0.lock().unwrap().take(),
            }
        }
    }
    let ctx = egui::Context::default();
    let mut suite = test_suite();
    suite.root = Some("/Photos".into());
    suite.group = Some("2008".into());
    suite.items = vec![
        Media::from_path(
            "/Photos/2008/09/img_20080916_035811.jpg".into(),
            "2008/09".into(),
        )
        .unwrap(),
    ];
    suite.services.audio_player = Arc::new(RecordIntent(std::sync::Mutex::new(Some(
        suite.items[0].path().into(),
    ))));
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        suite.poll_audio_player(ctx)
    });
    assert_eq!(suite.record_photo, Some(0));
    assert_eq!(suite.group.as_deref(), Some("2008"));
    assert!(suite.selected.is_none());
    assert!(!suite.comments.has_unsaved());
    suite.record_photo = None;
    suite.services.audio_player = Arc::new(RecordIntent(std::sync::Mutex::new(Some(
        "/Other/a.jpg".into(),
    ))));
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        suite.poll_audio_player(ctx)
    });
    assert!(suite.record_photo.is_none());
    assert!(!suite.status.is_empty());
}

#[test]
fn changing_page_prepares_audio_ratios_without_play_or_date_io() {
    use photosuite_application::diary::{AudioProbe, AudioSource};
    struct Source(std::sync::atomic::AtomicUsize);
    impl AudioSource for Source {
        fn probe(&self, path: &std::path::Path) -> Result<AudioProbe, String> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(AudioProbe {
                duration: if path.to_string_lossy().contains("130000") {
                    3.
                } else {
                    1.
                },
                audio_only: true,
                file_created: None,
            })
        }
        fn capture_time(&self, _: &std::path::Path) -> Result<crate::domain::CaptureTime, String> {
            panic!("page preparation must not load date metadata")
        }
    }
    let source = Arc::new(Source(std::sync::atomic::AtomicUsize::new(0)));
    let mut suite = test_suite();
    suite.root = Some("/library".into());
    suite.group = Some("2026".into());
    suite.audio_preparation = photosuite_application::diary_preparation::AudioPreparation::new(
        source.clone(),
        Default::default(),
    );
    suite.items = [("2026", "120000"), ("2026", "130000"), ("2027", "120000")]
        .into_iter()
        .map(|(year, time)| {
            Media::from_audio_asset(
                format!("/library/{year}/audio/d/aud_{year}1113_{time}.wav").into(),
                format!("{year}/audio/d"),
            )
            .unwrap()
        })
        .collect();
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    for (year, expected) in [("2026", 2), ("2027", 3)] {
        suite.group = Some(year.into());
        let render = |suite: &mut Suite| {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| suite.page_heading(ui, year));
            });
        };
        render(&mut suite);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while suite.audio_preparation.is_loading() {
            suite.audio_preparation.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        render(&mut suite);
        assert_eq!(source.0.load(std::sync::atomic::Ordering::SeqCst), expected);
        let timeline = suite.audio_header.draft.timeline().unwrap();
        assert_eq!(timeline.year(), year);
        assert!(timeline.clips().iter().all(|c| c.is_verified()));
        if year == "2026" {
            assert_eq!(
                timeline.clips()[1].weight() / timeline.clips()[0].weight(),
                3.
            );
        }
        assert!(suite.diary.timeline().is_none());
    }
}
