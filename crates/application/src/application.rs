use crate::domain::{Media, matches_group};
use crate::ports::MediaRepository;
pub fn visible(items: &[Media], query: &str, group: Option<&str>, videos_only: bool) -> Vec<usize> {
    let query = query.to_lowercase();
    let diary = crate::domain::diary::audio_year(group.unwrap_or("")).is_some();
    let mut result: Vec<_> = items
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            (if diary { m.is_audio() } else { !m.is_audio() })
                && (diary || !videos_only || m.is_video())
                && (if diary {
                    crate::domain::diary::AudioScope::parse(group.unwrap_or(""))
                        .is_some_and(|scope| scope.includes(m))
                } else {
                    matches_group(m.group(), group)
                })
                && m.path().to_string_lossy().to_lowercase().contains(&query)
        })
        .map(|(i, _)| i)
        .collect();
    if !diary {
        sort_newest_by(items, &mut result, capture_stamp);
    }
    result
}

/// Borrow canonical filename timestamps without metadata reads or per-comparison parsing.
pub(crate) fn capture_stamp(media: &Media) -> Option<&str> {
    if let Some(date) = media.reference_time() {
        return Some(date.stamp());
    }
    crate::domain::CaptureTime::photo_filename_stamp(media.path().file_stem()?.to_str()?)
}
pub(crate) fn sort_newest_by<'a>(
    items: &'a [Media],
    indices: &mut [usize],
    key: impl Fn(&'a Media) -> Option<&'a str>,
) {
    // Resolve each key once, rather than validating/looking it up for every comparison.
    indices
        .sort_by_cached_key(|&index| (std::cmp::Reverse(key(&items[index])), items[index].path()));
}

/// Gallery destination for a contextual photo; never performs I/O or changes playback.
pub struct PhotoLocation {
    pub index: usize,
    pub year: Option<String>,
}
pub fn locate_photo(items: &[Media], path: &std::path::Path) -> Result<PhotoLocation, String> {
    let index = items
        .iter()
        .position(|media| media.is_photo() && media.path() == path)
        .ok_or("Foto gehört nicht mehr zur geöffneten Bibliothek")?;
    let year = items[index]
        .group()
        .split(std::path::MAIN_SEPARATOR)
        .next()
        .filter(|year| {
            year.len() == 4
                && year.bytes().all(|b| b.is_ascii_digit())
                && year.parse::<u16>().is_ok_and(|n| n > 0)
        })
        .map(str::to_owned);
    Ok(PhotoLocation { index, year })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diary_audio_stays_out_of_photo_wall_and_video_filter_does_not_hide_diary() {
        let items = vec![
            Media::from_path("a.wav".into(), "2026/audio/diary".into()).unwrap(),
            Media::from_path("a.jpg".into(), "2026/01".into()).unwrap(),
        ];
        assert_eq!(visible(&items, "", None, false), vec![1]);
        assert_eq!(visible(&items, "", Some("2026/audio/diary"), true), vec![0]);
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
    pub date_corrections: Option<std::sync::Arc<dyn crate::date_correction::Repository>>,
    pub audio_library: Option<std::sync::Arc<dyn crate::audio_library::Repository>>,
    pub location_index: std::sync::Arc<dyn crate::media_geo::LocationIndex>,
    pub media_geo: std::sync::Arc<dyn crate::media_geo::Repository>,
    pub places: std::sync::Arc<dyn crate::geodata::PlaceIndex>,
    pub index_rebuilder: std::sync::Arc<dyn crate::library_index::Rebuilder>,
    pub transcription_engine: std::sync::Arc<dyn crate::transcription::queue::Engine>,
    pub transcription_queue: std::sync::Arc<dyn crate::transcription::queue::QueueStore>,
    pub transcription_power: std::sync::Arc<dyn crate::transcription::queue::PowerManager>,
    pub geodata: std::sync::Arc<dyn crate::geodata::Store>,
    pub transcription: std::sync::Arc<dyn crate::transcription::ModelStore>,
    pub duration_cache: Option<std::sync::Arc<dyn crate::duration_cache::DurationCache>>,
    pub media_trash: std::sync::Arc<dyn crate::media_trash::MediaTrash>,
    pub comment_media: std::sync::Arc<dyn crate::comments::CommentMediaStore>,
    pub comments: std::sync::Arc<dyn crate::comments::CommentRepository>,
    pub recorder: std::sync::Arc<dyn crate::comments::CommentRecorder>,
    pub media_dates: std::sync::Arc<dyn crate::media_dates::MediaDateSource>,
    pub organization: std::sync::Arc<dyn crate::organization::CatalogRepository>,
    pub work_priority: std::sync::Arc<crate::work_priority::WorkPriority>,
    pub import_cleanup: std::sync::Arc<dyn crate::import_cleanup::CleanupPort>,
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

#[cfg(test)]
mod photo_location_tests {
    use super::*;
    #[test]
    fn contextual_photo_uses_its_own_year_and_rejects_nonphotos_or_stale_paths() {
        let items = vec![
            Media::from_path("/Photos/2021/12/a.jpg".into(), "2021/12".into()).unwrap(),
            Media::from_path(
                "/Photos/2023/audio/diary/a.wav".into(),
                "2023/audio/diary".into(),
            )
            .unwrap(),
        ];
        let location = locate_photo(&items, items[0].path()).unwrap();
        assert_eq!(location.year.as_deref(), Some("2021"));
        assert_eq!(location.index, 0);
        assert!(locate_photo(&items, items[1].path()).is_err());
        assert!(locate_photo(&items, std::path::Path::new("/Other/a.jpg")).is_err());
        let loose = Media::from_path("/Photos/Misc/a.jpg".into(), "Misc".into()).unwrap();
        assert!(
            locate_photo(std::slice::from_ref(&loose), loose.path())
                .unwrap()
                .year
                .is_none()
        );
    }
}

#[cfg(test)]
mod flat_audio_tests {
    use super::*;
    #[test]
    fn canonical_folders_are_filtered_by_kind() {
        let items: Vec<_> = [
            ("aud_20230101_120000.wav", "2023/audio/diary"),
            ("com_20230101_120000.mp4", "2023/audio/comments"),
            ("unknown.wav", "2023/audio"),
            ("old.wav", "2023/audio/diary"),
            ("img_20230101_120000.jpg", "2023/01"),
        ]
        .into_iter()
        .map(|(file, group)| {
            Media::from_library_path(std::path::Path::new(group).join(file), group.into()).unwrap()
        })
        .collect();
        assert_eq!(
            visible(&items, "", Some("2023/audio"), true),
            vec![0, 1, 2, 3]
        );
        assert_eq!(
            visible(&items, "", Some("2023/audio/diary"), true),
            vec![0, 3]
        );
        assert_eq!(
            visible(&items, "", Some("2023/audio/comments"), false),
            vec![1]
        );
        assert_eq!(visible(&items, "com_", Some("2023/audio"), false), vec![1]);
        assert_eq!(visible(&items, "", Some("2023"), false), vec![4]);
    }
}

#[cfg(test)]
mod newest_tests {
    use super::*;
    #[test]
    fn grid_order_is_newest_first_independent_of_scan_order_and_filters() {
        let items = [
            "2020/01/img_20200102_120000.jpg",
            "2024/01/IMG_20240101_120000.mp4",
            "2020/01/img_20200103_120000.jpg",
            "2020/01/unknown.jpg",
        ]
        .map(|file| {
            Media::from_library_path(file.into(), file.rsplit_once('/').unwrap().0.into()).unwrap()
        });
        assert_eq!(visible(&items, "", None, false), vec![1, 2, 0, 3]);
        assert_eq!(visible(&items, "", Some("2020"), false), vec![2, 0, 3]);
        assert_eq!(visible(&items, "", None, true), vec![1]);
        let mut corrected = items.to_vec();
        corrected[0] = corrected[0].clone().with_capture_correction(Some(
            crate::domain::CaptureTime::parse("2026:01:01 12:00:00", "test").unwrap(),
        ));
        assert_eq!(visible(&corrected, "", None, false), vec![0, 1, 2, 3]);
    }
}
