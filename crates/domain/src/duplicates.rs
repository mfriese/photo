//! Collision families follow the canonical import timestamp, with any suffix.
use crate::{CaptureTime, Media};
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Clone, Debug)]
pub struct CollisionGroup {
    pub name: String,
    pub files: Vec<PathBuf>,
}
fn family(media: &Media) -> Option<(String, bool)> {
    if media.is_video() {
        return None;
    }
    let stem = media.path().file_stem()?.to_str()?;
    let base = stem.get(..19)?;
    if !base.get(..4)?.eq_ignore_ascii_case("img_") {
        return None;
    }
    let date = base.get(4..12)?;
    let time = base.get(13..19)?;
    if base.as_bytes()[12] != b'_' || !date.bytes().chain(time.bytes()).all(|b| b.is_ascii_digit())
    {
        return None;
    }
    CaptureTime::parse(
        &format!(
            "{}:{}:{} {}:{}:{}",
            &date[..4],
            &date[4..6],
            &date[6..],
            &time[..2],
            &time[2..4],
            &time[4..]
        ),
        "filename",
    )
    .ok()?;
    Some((base.to_lowercase(), stem.len() > 19))
}
pub fn collision_groups(media: &[Media]) -> Vec<CollisionGroup> {
    let mut groups = BTreeMap::<(PathBuf, String), Vec<(PathBuf, bool)>>::new();
    for m in media {
        if let Some((base, suffix)) = family(m) {
            groups
                .entry((
                    m.path().parent().unwrap_or(std::path::Path::new("")).into(),
                    base,
                ))
                .or_default()
                .push((m.path().into(), suffix));
        }
    }
    groups
        .into_iter()
        .filter_map(|((parent, name), mut files)| {
            if files.len() < 2 || !files.iter().any(|(_, suffix)| *suffix) {
                return None;
            }
            files.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
            Some(CollisionGroup {
                name: format!("{} / {name}", parent.display()),
                files: files.into_iter().map(|(p, _)| p).collect(),
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn any_suffix_and_all_extensions_but_no_other_dates_or_folders() {
        let media = [
            "2008/09/img_20080916_035811.jpg",
            "2008/09/img_20080916_035811abc.JPG",
            "2008/09/img_20080916_035811_001.png",
            "2008/09/img_20080917_035811.jpg",
            "other/img_20080916_035811.jpg",
            "2008/09/img_20080916_035811_002.mp4",
        ]
        .into_iter()
        .map(|p| Media::from_path(p.into(), String::new()).unwrap())
        .collect::<Vec<_>>();
        let groups = collision_groups(&media);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 3);
        assert!(groups[0].files[0].ends_with("img_20080916_035811.jpg"));
    }
}
