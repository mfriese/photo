//! Library opening workflow, independent of any presentation framework.
use crate::{application::Services, comments::CommentSession, organization::OrganizationSession};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum OpenError {
    RecordingPending,
    Catalog(String),
}
pub struct Opened {
    pub tags_reloaded: bool,
}
/// Borrows existing sessions; the workflow does not create adapters or own UI state.
pub struct LibraryOpening<'a> {
    pub comments: &'a mut CommentSession,
    pub organization: &'a mut OrganizationSession,
    pub processing: &'a mut crate::processing::Repeats,
    pub transcription: &'a mut crate::transcription::queue::QueueSession,
    pub geo_sweep: &'a mut crate::geo_sweep::Session,
    pub places: &'a mut crate::place_filter::Session,
    pub audio_preparation: &'a mut crate::diary_preparation::AudioPreparation,
    pub dates: &'a mut crate::date_correction::Session,
    pub audio_library: &'a mut crate::audio_library::Session,
    pub diary: &'a mut crate::diary::DiarySession,
    pub diary_catalog: &'a mut crate::diary::DiaryCatalog,
    pub settings: &'a mut crate::preferences::LibrarySettings,
    pub collection: &'a mut crate::collection::CollectionSession,
    pub index: &'a mut crate::library_index::Session,
    pub location: &'a mut crate::photo_location::Session,
    pub startup: Option<&'a mut crate::preferences::StartupPreferences>,
}
impl LibraryOpening<'_> {
    pub fn open(
        &mut self,
        root: PathBuf,
        current: Option<&Path>,
        refresh_tags: bool,
        services: &Services,
    ) -> Result<Opened, OpenError> {
        // Rejected navigation must leave all running sessions intact.
        if self.comments.has_unsaved() || self.comments.is_saving() {
            return Err(OpenError::RecordingPending);
        }
        let switched = current != Some(root.as_path());
        let tags_reloaded = refresh_tags || switched || self.organization.catalog().is_none();
        if tags_reloaded {
            self.organization
                .open_cached(root.clone())
                .map_err(OpenError::Catalog)?;
        }
        if switched {
            self.index.cancel();
            self.location.cancel();
        }
        if let Some(startup) = self.startup.as_mut() {
            startup.cancel_for_navigation();
        }
        self.processing.clear();
        self.transcription.reset();
        self.geo_sweep.reset();
        self.places.reset();
        self.audio_preparation.reset();
        self.dates.open(&root);
        self.audio_library.open(&root);
        self.organization.begin_media_scan(&root);
        self.comments.load_cached(root.clone());
        self.diary_catalog.cancel();
        if switched {
            let _ = services
                .audio_player
                .control(crate::diary::AudioControl::Stop);
            self.diary.cancel();
        }
        self.settings
            .request(services.preferences.clone(), root.clone());
        self.collection.request(services.repository.clone(), root);
        Ok(Opened { tags_reloaded })
    }
}
