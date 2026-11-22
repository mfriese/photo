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
