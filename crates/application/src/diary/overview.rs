//! Open/reuse the shared overview player from any presentation.
use super::*;
pub struct Overview<'a> {
    pub root: &'a Path,
    pub items: &'a [Media],
    pub group: Option<&'a str>,
    pub cached_audio: Option<&'a crate::audio_library::Snapshot>,
}
pub fn open(
    input: Overview<'_>,
    draft: &mut GalleryAudio,
    session: &mut DiarySession,
    source: Arc<dyn AudioSource>,
    player: &dyn AudioPlayer,
) -> Result<(), String> {
    let audio_items = input
        .cached_audio
        .map_or(input.items, |snapshot| snapshot.media.as_slice());
    let group = header_audio_group(input.items, input.group);
    draft.ensure(audio_items, input.root, &group);
    if player.transport().active
        && session
            .timeline_for_view(&group)
            .is_some_and(|timeline| draft.accepts_active(timeline))
    {
        return player.control(AudioControl::ShowWindow);
    }
    if let Some(selected) = draft
        .selected()
        .and_then(|path| audio_items.iter().find(|media| media.path() == path))
    {
        let items = input.cached_audio.map_or_else(
            || input.items.to_vec(),
            |snapshot| snapshot.with_photos(input.items),
        );
        session.request_window(&items, selected, source, &group)?;
    }
    Ok(())
}
