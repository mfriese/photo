//! Application tick for background processing; no window, input or repaint APIs.
use super::{
    inventory_sync::{InventorySync, Revision},
    *,
};
pub struct Input<'a> {
    pub root: Option<&'a Path>,
    pub items: &'a [Media],
    pub inventory_complete: bool,
    pub index_busy: bool,
    pub audio_active: bool,
}
pub enum UpdateError {
    Inventory(String),
    Metadata(String),
}
#[derive(Default)]
pub struct Update {
    pub audio_changed: bool,
    pub error: Option<UpdateError>,
    pub pending: bool,
}
pub struct Maintenance<'a> {
    pub audio: &'a mut crate::audio_library::Session,
    pub geo: &'a mut crate::geo_sweep::Session,
    pub queue: &'a mut QueueSession,
    pub model: &'a mut ModelSession,
    pub repeats: &'a mut Repeats,
    pub comments: &'a mut crate::comments::CommentSession,
    pub organization: &'a mut crate::organization::OrganizationSession,
    pub preparation: &'a mut crate::diary_preparation::AudioPreparation,
}
impl Maintenance<'_> {
    pub fn advance(
        &mut self,
        input: Input<'_>,
        seen: &mut Option<Revision>,
        audio_revision: &mut u64,
    ) -> Update {
        let mut update = Update::default();
        if let Some(root) = input.root.filter(|_| self.comments.is_loaded())
            && let Err(error) = (InventorySync {
                previous: seen,
                audio: self.audio,
                geo: self.geo,
                queue: self.queue,
            })
            .sync(
                root,
                input.items,
                self.comments.catalog(),
                input.inventory_complete,
            )
        {
            advance_with_audio(
                self.audio,
                self.geo,
                self.queue,
                Readiness {
                    library_open: true,
                    ..Default::default()
                },
                self.model.busy(),
            );
            return Update {
                error: Some(UpdateError::Inventory(error)),
                pending: self.geo.busy() || self.queue.busy(),
                ..Default::default()
            };
        }
        if self.audio.complete()
            && self.geo.complete()
            && input.inventory_complete
            && !self.repeats.pending()
            && self.queue.mode() == Mode::Running
            && !self.queue.busy()
            && !self.geo.busy()
            && !self.audio.busy()
        {
            self.model.ensure_loaded();
        }
        let model = self
            .model
            .prepared()
            .filter(|_| !self.model.busy() && self.model.error().is_empty())
            .and_then(|p| {
                p.ready_path
                    .as_ref()
                    .map(|path| crate::transcription::queue::ModelChoice {
                        path: path.clone(),
                        settings: p.settings.clone(),
                    })
            });
        self.queue.set_model(model);
        self.repeats.advance(
            self.audio,
            self.geo,
            self.queue,
            self.model,
            Readiness {
                library_open: input.root.is_some(),
                inventory_complete: input.inventory_complete,
                index_maintenance: input.index_busy,
                comments_ready: self.comments.is_loaded(),
                comments_pending: self.comments.has_unsaved() || self.comments.is_saving(),
                metadata_pending: self.organization.loading()
                    || self.organization.saving()
                    || self.organization.dirty(),
                audio_active: input.audio_active,
            },
        );
        if self.audio.revision() != *audio_revision {
            *audio_revision = self.audio.revision();
            update.audio_changed = true;
            if let Some(root) = input.root.filter(|_| self.audio.complete()) {
                self.comments.load_cached(root.to_owned());
                update.error = self
                    .organization
                    .open_cached(root.to_owned())
                    .err()
                    .map(UpdateError::Metadata);
                self.organization
                    .observe_media(root, input.items, input.inventory_complete);
                *seen = None;
            }
            self.preparation
                .display_hints(&self.audio.snapshot().durations);
        }
        update.pending = self.repeats.pending()
            || self.queue.busy()
            || self.geo.busy()
            || self.audio.busy()
            || (input.root.is_some()
                && self.queue.mode() == Mode::Running
                && self.queue.held()
                && self.audio.complete()
                && self.geo.complete());
        update
    }
}
