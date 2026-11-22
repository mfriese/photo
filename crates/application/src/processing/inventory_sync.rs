//! Coordinate inventory changes without presentation state or GUI callbacks.
use super::*;
#[derive(Clone, PartialEq, Eq)]
pub struct Revision {
    root: std::path::PathBuf,
    items: usize,
    comments: usize,
    complete: bool,
}
pub struct InventorySync<'a> {
    pub previous: &'a mut Option<Revision>,
    pub audio: &'a mut crate::audio_library::Session,
    pub geo: &'a mut crate::geo_sweep::Session,
    pub queue: &'a mut QueueSession,
}
impl InventorySync<'_> {
    pub fn sync(
        &mut self,
        root: &Path,
        items: &[Media],
        comments: &CommentCatalog,
        complete: bool,
    ) -> Result<(), String> {
        let observed = Revision {
            root: root.into(),
            items: items.len(),
            comments: comments.entries().len(),
            complete,
        };
        if self.previous.as_ref() == Some(&observed) {
            return Ok(());
        }
        if complete
            && self.previous.as_ref().is_some_and(|old| {
                old.complete && (old.items != observed.items || old.comments != observed.comments)
            })
        {
            self.audio.retry();
        }
        let inventory = inventory(root, items, comments)?;
        self.queue.sync(root, inventory.transcriptions, complete);
        if complete {
            self.geo.sync(root, inventory.photos);
        }
        *self.previous = Some(observed);
        Ok(())
    }
}
