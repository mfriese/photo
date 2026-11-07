use crate::domain::{Media, matches_group};
use crate::ports::MediaRepository;
use std::{path::PathBuf, sync::mpsc, thread};
pub fn load_collection(
    repository: std::sync::Arc<dyn MediaRepository>,
    root: PathBuf,
) -> mpsc::Receiver<(Vec<Media>, usize)> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(repository.scan(&root));
    });
    rx
}
pub fn visible(items: &[Media], query: &str, group: Option<&str>, videos_only: bool) -> Vec<usize> {
    let query = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            (!videos_only || m.is_video())
                && matches_group(m.group(), group)
                && m.path().to_string_lossy().to_lowercase().contains(&query)
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn year_selection_includes_months_without_matching_other_years() {
        let items: Vec<_> = ["2024/01", "2024/12", "20240/01", "2025/01"]
            .into_iter()
            .map(|g| Media::from_path(format!("{g}/Holiday.MOV").into(), g.into()).unwrap())
            .collect();
        assert_eq!(visible(&items, "holiday", Some("2024"), true), vec![0, 1]);
        assert_eq!(visible(&items, "", Some("2024/01"), false), vec![0]);
        assert!(visible(&items, "missing", None, false).is_empty());
    }
}

/// Ports are supplied exclusively by the composition root.
#[derive(Clone)]
pub struct Services {
    pub duplicates: std::sync::Arc<dyn crate::duplicates::DuplicateImages>,
    pub video_player: std::sync::Arc<dyn crate::ports::VideoPlayer>,
    pub video_preview: std::sync::Arc<dyn crate::ports::VideoPreviewSource>,
    pub preferences: std::sync::Arc<dyn crate::preferences::PreferencesStore>,
    pub importer: std::sync::Arc<dyn crate::import::ImportAdapter>,
    pub orientation_writer: std::sync::Arc<dyn crate::ports::OrientationWriter>,
    pub repository: std::sync::Arc<dyn MediaRepository>,
    pub decoder: std::sync::Arc<dyn crate::ports::ImageDecoder>,
    pub opener: std::sync::Arc<dyn crate::ports::MediaOpener>,
}

#[cfg(test)]
mod port_tests {
    use super::*;
    struct MemoryRepository;
    impl MediaRepository for MemoryRepository {
        fn scan(&self, _: &std::path::Path) -> (Vec<Media>, usize) {
            (
                vec![Media::from_path("2024/01/a.jpg".into(), "2024/01".into()).unwrap()],
                2,
            )
        }
    }
    #[test]
    fn collection_use_case_works_without_filesystem_or_gui() {
        let receiver = load_collection(std::sync::Arc::new(MemoryRepository), "unused".into());
        let (items, errors) = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(errors, 2);
        assert_eq!(visible(&items, "a.jpg", Some("2024"), false), vec![0]);
    }
}
