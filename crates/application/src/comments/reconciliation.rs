//! Apply authoritative comment changes to dependent application projections.
use super::{CommentChanges, CommentSession};
use crate::{audio_library, diary, domain::Media, media_dates, organization};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
pub struct Reconciliation<'a> {
    pub comments: &'a CommentSession,
    pub audio: &'a mut audio_library::Session,
    pub organization: &'a mut organization::OrganizationSession,
    pub diary_catalog: &'a mut diary::DiaryCatalog,
    pub dates: &'a mut media_dates::MediaDates,
}
impl Reconciliation<'_> {
    pub fn apply(
        &mut self,
        active_root: Option<&Path>,
        changes: &CommentChanges,
        items: &mut Vec<Media>,
        known: &mut HashSet<PathBuf>,
    ) -> bool {
        if active_root != Some(changes.root.as_path()) {
            return false;
        }
        let affected: std::collections::BTreeSet<_> = changes
            .media
            .iter()
            .map(|file| changes.root.join(file.as_str()))
            .collect();
        if let Some((root, catalog)) = self.comments.authoritative_catalog() {
            self.audio.observe_comments(catalog);
            if self.audio.complete() {
                self.organization.reconcile_comments(root, catalog);
            }
        }
        for media in items.iter_mut() {
            if affected.contains(media.path()) {
                *media = self.comments.project(media.clone());
            }
        }
        for media in self.comments.comment_media() {
            if known.insert(media.path().to_owned()) {
                items.push(media);
            }
        }
        self.diary_catalog.cancel();
        self.dates.invalidate(&affected);
        true
    }
}
