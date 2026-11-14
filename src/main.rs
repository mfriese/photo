use photosuite_application::{application, import, ports};
use photosuite_domain as domain;
use photosuite_domain::commands;
mod audio;
mod audio_labels;
mod audio_player;
mod command_bindings;
mod command_labels;
mod comment_recorder;
mod comments_fs;
mod context_photos;
mod desktop_menu;
mod duplicates_fs;
mod duplicates_ui;
mod exiftool;
mod image_loader;
mod import_cleanup_fs;
mod import_cleanup_ui;
mod import_fs;
mod import_ui;
mod infrastructure;
mod library_preferences_fs;
mod media_records_fs;
mod metadata_fs;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod muda_menu;
mod options_ui;
mod preferences_codec;
mod preferences_fs;
mod presentation;
mod staging_fs;
mod storyboard_ui;
mod tag_store;
mod theme;
mod video;
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
        .unwrap_or_else(preferences_fs::PlistPreferences::user_path)
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
    let services = application::Services {
        comment_media: std::sync::Arc::new(comments_fs::CommentFiles {
            audio: audio_source.clone(),
            copier: importer.clone(),
        }),
        comments: std::sync::Arc::new(comments_fs::YamlComments {
            priority: priority.clone(),
        }),
        recorder: std::sync::Arc::new(comment_recorder::NativeCommentRecorder::new()),
        media_dates: importer.clone(),
        organization: std::sync::Arc::new(tag_store::YamlTags {
            priority: priority.clone(),
        }),
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
            library_preferences_fs::LibraryPreferences::locator_path(&preferences_path),
            std::sync::Arc::new(preferences_fs::PlistPreferences::at(preferences_path)),
        )),
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
