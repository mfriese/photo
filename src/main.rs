use photosuite_application::{application, import, ports};
use photosuite_domain as domain;
use photosuite_domain::commands;
mod audio;
mod audio_player;
mod command_bindings;
mod command_labels;
mod context_photos;
mod duplicates_fs;
mod duplicates_ui;
mod exiftool;
mod image_loader;
mod import_cleanup_fs;
mod import_cleanup_ui;
mod import_fs;
mod import_ui;
mod infrastructure;
mod options_ui;
mod preferences_fs;
mod presentation;
mod storyboard_ui;
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
    let services = application::Services {
        work_priority: priority.clone(),
        audio_source: audio_source.clone(),
        audio_player: std::sync::Arc::new(audio_player::NativeAudioPlayer::new(
            std::sync::Arc::new(context_photos::PhotoContextAdapter),
            priority.clone(),
        )),
        duplicates: std::sync::Arc::new(duplicates_fs::PixelDuplicates),
        video_player: std::sync::Arc::new(video::NativeVideoPlayer),
        video_preview: video_preview.clone(),
        preferences: std::sync::Arc::new(preferences_fs::PlistPreferences::at(preferences_path)),
        import_cleanup: std::sync::Arc::new(import_cleanup_fs::SafeImportCleanup {
            trash: std::sync::Arc::new(import_cleanup_fs::NativeTrash),
        }),
        importer: std::sync::Arc::new(import_fs::FileImportAdapter {
            audio: audio_source,
            repository: repository.clone(),
            tool: exiftool.clone(),
        }),
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
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(presentation::create(cc, services)))),
    )
}
