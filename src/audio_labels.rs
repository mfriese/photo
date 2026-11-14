//! Shared presentation labels for audio categories, independent of native playback.
pub fn kind_label(kind: photosuite_domain::diary::AudioKind) -> &'static str {
    match kind {
        photosuite_domain::diary::AudioKind::All => "Audio",
        photosuite_domain::diary::AudioKind::Diary => "Tagebuch",
        photosuite_domain::diary::AudioKind::Comments => "Kommentare",
    }
}

pub fn timeline_label(timeline: &photosuite_domain::diary_timeline::DiaryTimeline) -> String {
    format!(
        "{}{}",
        timeline.year(),
        timeline
            .month()
            .map(|m| format!("/{m}"))
            .unwrap_or_default()
    )
}
