use crate::domain::{Media, matches_group};
use crate::ports::MediaRepository;
pub fn visible(items: &[Media], query: &str, group: Option<&str>, videos_only: bool) -> Vec<usize> {
    let query = query.to_lowercase();
    let diary = crate::domain::diary::diary_year(group.unwrap_or("")).is_some();
    items
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            (if diary { m.is_audio() } else { !m.is_audio() })
                && (diary || !videos_only || m.is_video())
                && (if diary {
                    crate::domain::diary::diary_year(m.group())
                        == crate::domain::diary::diary_year(group.unwrap_or(""))
                } else {
                    matches_group(m.group(), group)
                })
                && m.path().to_string_lossy().to_lowercase().contains(&query)
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diary_audio_stays_out_of_photo_wall_and_video_filter_does_not_hide_diary() {
        let items = vec![
            Media::from_path("a.wav".into(), "2026/diary".into()).unwrap(),
            Media::from_path("a.jpg".into(), "2026/01".into()).unwrap(),
        ];
        assert_eq!(visible(&items, "", None, false), vec![1]);
        assert_eq!(visible(&items, "", Some("2026/diary"), true), vec![0]);
    }
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
    pub audio_source: std::sync::Arc<dyn crate::diary::AudioSource>,
    pub audio_player: std::sync::Arc<dyn crate::diary::AudioPlayer>,
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
