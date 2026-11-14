//! Shared presentation labels for audio categories, independent of native playback.
pub fn kind_label(kind: photosuite_domain::diary::AudioKind) -> &'static str {
    match kind {
        photosuite_domain::diary::AudioKind::All => "Audio",
        photosuite_domain::diary::AudioKind::Diary => "Tagebuch",
        photosuite_domain::diary::AudioKind::Comments => "Kommentare",
    }
}

pub fn timeline_label(timeline: &photosuite_domain::diary_timeline::DiaryTimeline) -> String {
    if timeline.is_library() {
        return "Alles".into();
    }
    format!(
        "{}{}",
        timeline.year(),
        timeline
            .month()
            .map(|m| format!("/{m}"))
            .unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn library_player_shows_all_instead_of_an_arbitrary_year() {
        let t = photosuite_domain::diary_timeline::DiaryTimeline::for_library(vec![
            photosuite_domain::diary_timeline::DiaryClip::from_filename(
                "aud_20261114_120000.wav".into(),
            ),
        ])
        .unwrap();
        assert_eq!(super::timeline_label(&t), "Alles");
    }
}
