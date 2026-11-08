//! Five chronological neighbours around the photograph nearest a recording instant.
use crate::CaptureTime;
use std::path::{Path, PathBuf};
#[derive(Clone, Debug)]
pub struct TimedPhoto {
    path: PathBuf,
    captured: CaptureTime,
}
impl TimedPhoto {
    pub fn new(path: PathBuf, captured: CaptureTime) -> Self {
        Self { path, captured }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn captured(&self) -> &CaptureTime {
        &self.captured
    }
}
#[derive(Default)]
pub struct PhotoTimeline {
    photos: Vec<TimedPhoto>,
}
impl PhotoTimeline {
    pub fn new(mut photos: Vec<TimedPhoto>) -> Self {
        photos.sort_by(|a, b| {
            a.captured
                .civil_seconds()
                .cmp(&b.captured.civil_seconds())
                .then(a.path.cmp(&b.path))
        });
        Self { photos }
    }
    pub fn len(&self) -> usize {
        self.photos.len()
    }
    pub fn is_empty(&self) -> bool {
        self.photos.is_empty()
    }
    pub fn around(&self, target: i64) -> [Option<TimedPhoto>; 5] {
        if self.photos.is_empty() {
            return std::array::from_fn(|_| None);
        }
        let right = self
            .photos
            .partition_point(|photo| photo.captured.civil_seconds() < target);
        let anchor = if right == 0 {
            0
        } else if right == self.photos.len() {
            right - 1
        } else {
            let left_distance =
                (target as i128 - self.photos[right - 1].captured.civil_seconds() as i128).abs();
            let right_distance =
                (self.photos[right].captured.civil_seconds() as i128 - target as i128).abs();
            if left_distance <= right_distance {
                right - 1
            } else {
                right
            }
        };
        std::array::from_fn(|slot| {
            anchor
                .checked_add_signed(slot as isize - 2)
                .and_then(|index| self.photos.get(index))
                .cloned()
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn photo(second: u32) -> TimedPhoto {
        TimedPhoto::new(
            format!("{second:02}.jpg").into(),
            CaptureTime::parse(&format!("2023:12:27 22:51:{second:02}"), "test").unwrap(),
        )
    }
    #[test]
    fn closest_is_centered_with_earlier_left_and_later_right() {
        let timeline =
            PhotoTimeline::new(vec![photo(50), photo(10), photo(40), photo(20), photo(30)]);
        let target = photo(31).captured.civil_seconds();
        let selection = timeline.around(target);
        assert_eq!(
            selection.map(|p| p.unwrap().path),
            ["10.jpg", "20.jpg", "30.jpg", "40.jpg", "50.jpg"].map(PathBuf::from)
        );
        assert_eq!(
            timeline.around(photo(25).captured.civil_seconds())[2]
                .as_ref()
                .unwrap()
                .path(),
            Path::new("20.jpg")
        );
    }
    #[test]
    fn edges_do_not_duplicate_or_move_the_center_photo() {
        let timeline = PhotoTimeline::new(vec![photo(10), photo(20)]);
        let first = timeline.around(i64::MIN);
        assert!(first[0].is_none() && first[1].is_none());
        assert_eq!(first[2].as_ref().unwrap().path(), Path::new("10.jpg"));
        let last = timeline.around(i64::MAX);
        assert_eq!(last[2].as_ref().unwrap().path(), Path::new("20.jpg"));
        assert!(last[3].is_none() && last[4].is_none());
        assert!(
            PhotoTimeline::default()
                .around(0)
                .iter()
                .all(Option::is_none)
        );
    }
}
