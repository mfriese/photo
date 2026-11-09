//! Read-only collection discovery; no decoding on the browsing path.
use crate::domain::Media;
use crate::ports::MediaRepository;
use std::path::Path;
pub struct FileSystemRepository {
    pub priority: std::sync::Arc<photosuite_application::work_priority::WorkPriority>,
    pub audio: std::sync::Arc<dyn photosuite_application::diary::AudioSource>,
}
impl MediaRepository for FileSystemRepository {
    fn partitions(&self, root: &Path) -> Result<Option<Vec<String>>, String> {
        if !std::fs::symlink_metadata(root).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        {
            return Err("Bibliothek ist kein reguläres Verzeichnis".into());
        }
        let mut directories = Vec::new();
        for entry in std::fs::read_dir(root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                directories.push(
                    entry
                        .file_name()
                        .into_string()
                        .map_err(|_| "Nicht darstellbarer Verzeichnisname")?,
                );
            }
        }
        Ok(Some(directories))
    }
    fn scan_partition(
        &self,
        root: &Path,
        partition: &str,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> (Vec<Media>, usize) {
        if !partition.is_empty()
            && (Path::new(partition).components().count() != 1
                || !matches!(
                    Path::new(partition).components().next(),
                    Some(std::path::Component::Normal(_))
                ))
        {
            return (Vec::new(), 1);
        }
        self.scan_directory(
            root,
            &root.join(partition),
            partition.is_empty(),
            false,
            cancel,
        )
    }

    fn scan(&self, root: &Path) -> (Vec<Media>, usize) {
        self.scan_cancellable(root, &std::sync::atomic::AtomicBool::new(false))
    }
    fn scan_cancellable(
        &self,
        root: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> (Vec<Media>, usize) {
        self.scan_directory(root, root, false, true, cancel)
    }
}
impl FileSystemRepository {
    fn scan_directory(
        &self,
        root: &Path,
        directory: &Path,
        shallow: bool,
        inspect_containers: bool,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> (Vec<Media>, usize) {
        let mut items = Vec::new();
        let mut errors = 0;
        if !std::fs::symlink_metadata(directory)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        {
            return (items, 1);
        }
        let depth = if shallow { 1 } else { usize::MAX };
        for entry in walkdir::WalkDir::new(directory)
            .follow_links(false)
            .max_depth(depth)
        {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if !self.priority.wait_background(cancel) {
                break;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let parent = path
                .parent()
                .unwrap_or(root)
                .strip_prefix(root)
                .unwrap_or(Path::new(""));
            let group = if parent.as_os_str().is_empty() {
                "Ohne Unterordner".into()
            } else {
                parent.to_string_lossy().into_owned()
            };
            let media = if inspect_containers {
                Media::from_path(path.into(), group)
            } else {
                Media::from_library_path(path.into(), group)
            };
            if let Some(mut media) = media {
                if media.is_video()
                    && inspect_containers
                    && self.audio.probe(path).is_ok_and(|p| p.audio_only)
                {
                    media = Media::from_audio_asset(path.into(), media.group().to_owned()).unwrap();
                }
                if crate::domain::diary::diary_year(media.group()).is_some() && !media.is_audio() {
                    errors += 1;
                    continue;
                }
                items.push(media);
            }
        }
        items.sort_by(|a, b| a.path().cmp(b.path()));
        (items, errors)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shallow_year_index_and_partition_scans_preserve_all_source_bytes() {
        use crate::ports::MediaRepository;
        let directory = tempfile::tempdir().unwrap();
        for year in ["2023", "2024", "2025"] {
            std::fs::create_dir_all(directory.path().join(year).join("01")).unwrap();
            std::fs::write(
                directory.path().join(year).join("01/photo.jpg"),
                b"original",
            )
            .unwrap();
        }
        std::fs::create_dir(directory.path().join("2022")).unwrap(); // Empty year remains selectable.
        std::fs::write(directory.path().join("root.jpg"), b"root-original").unwrap();
        std::os::unix::fs::symlink(
            directory.path().join("2023"),
            directory.path().join("linked"),
        )
        .unwrap();
        let repository = FileSystemRepository {
            priority: Default::default(),
            audio: std::sync::Arc::new(crate::audio::NativeAudioSource {
                tool: crate::exiftool::ExifTool::development(),
            }),
        };
        let mut years = repository.partitions(directory.path()).unwrap().unwrap();
        years.sort();
        assert_eq!(years, ["2022", "2023", "2024", "2025"]);
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let (items, errors) = repository.scan_partition(directory.path(), "2024", &cancel);
        assert_eq!(errors, 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].group(), "2024/01");
        let (root_items, _) = repository.scan_partition(directory.path(), "", &cancel);
        assert_eq!(root_items.len(), 1);
        assert_eq!(root_items[0].path(), directory.path().join("root.jpg"));
        assert_eq!(
            repository
                .scan_partition(directory.path(), "../outside", &cancel)
                .1,
            1
        );
        assert_eq!(
            repository
                .scan_partition(directory.path(), "linked", &cancel)
                .1,
            1
        );
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(
            repository
                .scan_partition(directory.path(), "2025", &cancel)
                .0
                .is_empty()
        );
        for year in ["2023", "2024", "2025"] {
            assert_eq!(
                std::fs::read(directory.path().join(year).join("01/photo.jpg")).unwrap(),
                b"original"
            );
        }
        assert_eq!(
            std::fs::read(directory.path().join("root.jpg")).unwrap(),
            b"root-original"
        );
    }
    #[test]
    fn browsing_classifies_diary_by_names_without_any_container_or_metadata_reads() {
        struct NoReads;
        impl photosuite_application::diary::AudioSource for NoReads {
            fn probe(&self, _: &Path) -> Result<photosuite_application::diary::AudioProbe, String> {
                panic!("browse must not probe containers")
            }
            fn capture_time(&self, _: &Path) -> Result<crate::domain::CaptureTime, String> {
                panic!("browse must not read metadata")
            }
        }
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("2023/diary")).unwrap();
        std::fs::create_dir_all(directory.path().join("2023/12")).unwrap();
        let audio = directory.path().join("2023/diary/aud_20231227_225132.mp4");
        let video = directory.path().join("2023/12/img_20231227_225132.mp4");
        std::fs::write(&audio, b"audio-original").unwrap();
        std::fs::write(&video, b"video-original").unwrap();
        let sidecar = directory
            .path()
            .join("2023/diary/._aud_20231227_225132.wav");
        std::fs::write(&sidecar, [0, 5, 22, 7]).unwrap();
        let repository = FileSystemRepository {
            audio: std::sync::Arc::new(NoReads),
            priority: Default::default(),
        };
        let (items, errors) = repository.scan_partition(
            directory.path(),
            "2023",
            &std::sync::atomic::AtomicBool::new(false),
        );
        assert_eq!(errors, 0);
        assert_eq!(items.len(), 2);
        assert!(items.iter().find(|m| m.path() == audio).unwrap().is_audio());
        assert!(items.iter().find(|m| m.path() == video).unwrap().is_video());
        assert_eq!(std::fs::read(audio).unwrap(), b"audio-original");
        assert_eq!(std::fs::read(video).unwrap(), b"video-original");
        assert_eq!(std::fs::read(sidecar).unwrap(), [0, 5, 22, 7]);
    }
    #[test]
    fn library_root_must_be_a_regular_directory() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("file.jpg");
        std::fs::write(&file, b"original").unwrap();
        let repository = FileSystemRepository {
            priority: Default::default(),
            audio: std::sync::Arc::new(crate::audio::NativeAudioSource {
                tool: crate::exiftool::ExifTool::development(),
            }),
        };
        assert_eq!(repository.scan(&file).1, 1);
        assert_eq!(repository.scan(&directory.path().join("missing")).1, 1);
        assert_eq!(std::fs::read(file).unwrap(), b"original");
    }
    #[test]
    fn scan_filters_and_preserves_originals() {
        let dir = tempfile::tempdir().unwrap();
        let month = dir.path().join("2024/03");
        std::fs::create_dir_all(&month).unwrap();
        for name in ["a.JPG", "b.mov", "ignore.txt"] {
            std::fs::write(month.join(name), b"original").unwrap();
        }
        let repository = FileSystemRepository {
            priority: Default::default(),
            audio: std::sync::Arc::new(crate::audio::NativeAudioSource {
                tool: crate::exiftool::ExifTool::development(),
            }),
        };
        let (items, errors) = repository.scan(dir.path());
        assert_eq!(errors, 0);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].group(), "2024/03");
        assert!(items[1].is_video());
        assert_eq!(std::fs::read(month.join("a.JPG")).unwrap(), b"original");
    }
}
