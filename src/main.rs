mod application_icon;
mod date_correction_fs;
use photosuite_application::{application, import, ports};
use photosuite_domain as domain;
use photosuite_domain::commands;
mod audio;
mod audio_labels;
mod audio_library_fs;
mod audio_player;
mod command_bindings;
mod command_labels;
mod comment_recorder;
mod comments_fs;
mod context_photos;
mod desktop_menu;
mod duplicates_fs;
mod duplicates_ui;
mod duration_cache_fs;
mod exiftool;
mod file_dates_fs;
mod geo_labels;
mod geodata_db;
mod geodata_fs;
mod geodata_ui;
mod image_loader;
mod import_cleanup_fs;
mod import_cleanup_ui;
mod import_fs;
mod import_ui;
mod infrastructure;
mod library_db;
mod library_index_fs;
mod library_preferences_fs;
mod media_geo_fs;
mod media_records_fs;
mod media_trash_fs;
mod metadata_fs;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod muda_menu;
mod options_ui;
mod photo_coordinates_fs;
mod preferences_codec;
mod presentation;
mod runtime_state_fs;
mod staging_fs;
mod storyboard_ui;
mod tag_store;
mod theme;
mod transcript_codec;
mod transcription_fs;
mod transcription_native;
mod transcription_queue_fs;
mod transcription_ui;
mod video;
mod whisper_models_native;
use eframe::egui;
fn main() -> eframe::Result {
    let executable = std::env::current_exe().unwrap_or_default();
    let exiftool = exiftool::ExifTool::for_executable(&executable);
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--check-installation")
    {
        match exiftool.check() {
            Ok(version) => println!(
                "PhotoSuite {} · ExifTool {version} · Installation OK",
                env!("CARGO_PKG_VERSION")
            ),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let priority =
        std::sync::Arc::new(photosuite_application::work_priority::WorkPriority::default());
    let audio_source = std::sync::Arc::new(audio::NativeAudioSource {
        tool: exiftool.clone(),
    });
    let repository = std::sync::Arc::new(infrastructure::FileSystemRepository {
        priority: priority.clone(),
        audio: audio_source.clone(),
    });
    let preferences_path = match std::env::var_os("PHOTOSUITE_PREFERENCES_PATH")
        .map(std::path::PathBuf::from)
        .map(Ok)
        .unwrap_or_else(library_preferences_fs::LibraryPreferences::user_path)
    {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return Ok(());
        }
    };
    let video_preview = std::sync::Arc::new(video::NativeVideoSource);
    let importer = std::sync::Arc::new(import_fs::FileImportAdapter {
        audio: audio_source.clone(),
        repository: repository.clone(),
        tool: exiftool.clone(),
    });
    let model_transport = std::sync::Arc::new(whisper_models_native::NativeModelTransport);
    let database = std::sync::Arc::new(library_db::Database);
    let places = std::sync::Arc::new(geodata_db::SqlitePlaces);
    let coordinates = std::sync::Arc::new(photo_coordinates_fs::ExifCoordinates);
    let media_geo = std::sync::Arc::new(media_geo_fs::SqlitePhotoGeo {
        database: database.clone(),
        coordinates: coordinates.clone(),
    });
    let tag_repository = std::sync::Arc::new(tag_store::SqliteTags {
        database: database.clone(),
        priority: priority.clone(),
    });
    let comment_repository = std::sync::Arc::new(comments_fs::SqliteComments {
        database: database.clone(),
        priority: priority.clone(),
    });
    let services = application::Services {
        audio_library: Some(std::sync::Arc::new(audio_library_fs::AudioLibrary {
            tags: Some(tag_repository.clone()),
            comments: Some(comment_repository.clone()),
            database: database.clone(),
            durations: std::sync::Arc::new(duration_cache_fs::SqliteDurations {
                database: database.clone(),
            }),
            audio: audio_source.clone(),
            priority: priority.clone(),
            pending: std::sync::Mutex::new(None),
            issues: std::sync::Mutex::new(None),
        })),
        location_index: media_geo.clone(),
        media_geo,
        places: places.clone(),
        index_rebuilder: std::sync::Arc::new(library_index_fs::SqliteRebuilder {
            database: database.clone(),
            priority: priority.clone(),
        }),
        transcription_engine: std::sync::Arc::new(transcription_native::NativeEngine::new(
            model_transport.clone(),
        )),
        transcription_queue: std::sync::Arc::new(
            transcription_queue_fs::SqliteTranscriptionQueue {
                database: database.clone(),
                priority: priority.clone(),
            },
        ),
        transcription_power: std::sync::Arc::new(transcription_native::NativePower),
        transcription: std::sync::Arc::new(transcription_fs::WhisperModels::new(
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_default(),
            model_transport.clone(),
        )),
        geodata: std::sync::Arc::new(geodata_fs::GeoData::new(
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_default(),
            model_transport,
            places,
        )),
        duration_cache: Some(std::sync::Arc::new(duration_cache_fs::SqliteDurations {
            database: database.clone(),
        })),
        comment_media: std::sync::Arc::new(comments_fs::CommentFiles {
            audio: audio_source.clone(),
            copier: importer.clone(),
        }),
        comments: comment_repository,
        recorder: std::sync::Arc::new(comment_recorder::NativeCommentRecorder::new()),
        date_corrections: Some(std::sync::Arc::new(date_correction_fs::CaptureDates {
            database: database.clone(),
            priority: priority.clone(),
        })),
        media_dates: importer.clone(),
        organization: tag_repository,
        work_priority: priority.clone(),
        audio_source: audio_source.clone(),
        audio_player: std::sync::Arc::new(audio_player::NativeAudioPlayer::new(
            std::sync::Arc::new(context_photos::PhotoContextAdapter),
            priority.clone(),
        )),
        duplicates: std::sync::Arc::new(duplicates_fs::PixelDuplicates),
        video_player: std::sync::Arc::new(video::NativeVideoPlayer),
        video_preview: video_preview.clone(),
        preferences: std::sync::Arc::new(library_preferences_fs::LibraryPreferences::new(
            preferences_path,
        )),
        media_trash: std::sync::Arc::new(media_trash_fs::LibraryTrash {
            trash: std::sync::Arc::new(import_cleanup_fs::NativeTrash),
        }),
        import_cleanup: std::sync::Arc::new(import_cleanup_fs::SafeImportCleanup {
            trash: std::sync::Arc::new(import_cleanup_fs::NativeTrash),
        }),
        importer,
        orientation_writer: std::sync::Arc::new(infrastructure::ExifOrientationWriter {
            tool: exiftool,
        }),
        repository,
        decoder: std::sync::Arc::new(infrastructure::RasterDecoder {
            priority,
            videos: video_preview,
        }),
        opener: std::sync::Arc::new(infrastructure::SystemMediaOpener),
    };
    eframe::run_native(
        "PhotoSuite",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_icon(
                    application_icon::window_icon()
                        .expect("Eingebettetes PhotoSuite-Symbol ist beschädigt"),
                )
                .with_inner_size([1280., 850.])
                .with_min_inner_size([360., 400.]),
            event_loop_builder: Some(Box::new(|builder| {
                #[cfg(target_os = "macos")]
                {
                    use winit::platform::macos::EventLoopBuilderExtMacOS;
                    builder.with_default_menu(false);
                }
                #[cfg(target_os = "windows")]
                {
                    use winit::platform::windows::EventLoopBuilderExtWindows;
                    builder.with_msg_hook(muda_menu::windows_message_hook);
                }
                let _ = builder;
            })),
            ..Default::default()
        },
        Box::new(move |cc| {
            #[cfg(target_os = "macos")]
            application_icon::install_dock_icon().map_err(std::io::Error::other)?;
            #[cfg(not(any(target_os="macos",target_os="windows")))]
            return Err(std::io::Error::other("PhotoSuite's native muda menu requires a GTK host on Linux/BSD; this port is not implemented. No fallback menu is installed.").into());
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            {
                use raw_window_handle::HasWindowHandle;
                let menu =
                    muda_menu::MudaMenu::new(cc.egui_ctx.clone(), cc.window_handle()?.as_raw())
                        .map_err(std::io::Error::other)?;
                Ok(Box::new(presentation::create(cc, services, Box::new(menu)))
                    as Box<dyn eframe::App>)
            }
        }),
    )
}
