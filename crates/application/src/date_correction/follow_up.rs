//! Coordinate dependent sessions after an authoritative capture-date save.
use std::path::Path;

/// A playback failure must not undo the committed date or prevent refreshing.
pub struct SavedDate {
    pub playback_error: Option<String>,
}
pub struct FollowUp<'a> {
    pub diary: &'a mut crate::diary::DiarySession,
    pub diary_catalog: &'a mut crate::diary::DiaryCatalog,
    pub audio_preparation: &'a mut crate::diary_preparation::AudioPreparation,
    pub comments: &'a mut crate::comments::CommentSession,
    pub audio_library: &'a mut crate::audio_library::Session,
}
impl FollowUp<'_> {
    pub fn after_save(&mut self, root: &Path, player: &dyn crate::diary::AudioPlayer) -> SavedDate {
        self.diary.cancel();
        let playback_error = player.control(crate::diary::AudioControl::Stop).err();
        self.diary_catalog.cancel();
        self.audio_preparation.reset();
        self.comments.load_cached(root.to_owned());
        self.audio_library.open(root);
        SavedDate { playback_error }
    }
}

/// Data-side follow-up of a confirmed physical relocation. Selection and scroll
/// state are intentionally left to the presenter.
pub struct Relocation<'a> {
    pub editing: &'a mut crate::editing::EditingSession,
    pub places: &'a mut crate::place_filter::Session,
    pub transcription: &'a mut crate::transcription::queue::QueueSession,
    pub organization: &'a mut crate::organization::OrganizationSession,
}
impl Relocation<'_> {
    pub fn apply(
        &mut self,
        root: &Path,
        moved: &super::Relocated,
        items: &mut [crate::domain::Media],
        known: &mut std::collections::HashSet<std::path::PathBuf>,
    ) -> Result<(), String> {
        let old = root.join(moved.from.as_str());
        let new = root.join(moved.to.as_str());
        if let Some(media) = items.iter_mut().find(|media| media.path() == old) {
            let group = Path::new(moved.to.as_str())
                .parent()
                .unwrap_or(Path::new(""))
                .to_string_lossy()
                .into_owned();
            if let Some(replacement) = crate::domain::Media::from_library_path(new.clone(), group) {
                *media = replacement;
            }
        }
        known.remove(&old);
        known.insert(new.clone());
        self.editing.relocate(&old, &new);
        self.places.relocate(&moved.from, &moved.to);
        self.transcription.media_paths_changed();
        self.organization.refresh_relocated(&moved.from, &moved.to)
    }
}

/// Refresh the in-memory media projection from authoritative capture corrections.
pub fn project_media(
    root: &Path,
    catalog: &super::Catalog,
    items: &mut [crate::domain::Media],
    dates: &mut crate::media_dates::MediaDates,
) {
    for media in items {
        *media = super::project(root, catalog, media.clone());
    }
    dates.clear();
}
