//! Media processing use case: authoritative comment targets and preflight admission.
use crate::{
    domain::{Media, comments::CommentCatalog, tags::LibraryFile},
    geo_sweep,
    transcription::{
        ModelSession,
        queue::{Candidate, Mode, QueueSession},
    },
};
use std::path::Path;
pub struct Inventory {
    pub photos: Vec<LibraryFile>,
    pub transcriptions: Vec<Candidate>,
}
/// Never silently omit an invalid library member or write a comment transcript to its audio YAML.
pub fn inventory(
    root: &Path,
    items: &[Media],
    comments: &CommentCatalog,
) -> Result<Inventory, String> {
    let mut photos = Vec::new();
    let mut transcriptions = Vec::new();
    for media in items {
        let relative = media
            .path()
            .strip_prefix(root)
            .map_err(|_| "Medium gehört nicht zur Bibliothek")?
            .to_str()
            .ok_or("Medienpfad ist kein gültiges UTF-8")?;
        let source = LibraryFile::new(relative.into())?;
        if media.is_photo() {
            photos.push(source);
        } else if media.is_audio() || media.is_video() {
            let record = comments
                .by_audio(&source)
                .map(|c| c.photo().clone())
                .unwrap_or_else(|| source.clone());
            transcriptions.push(Candidate { source, record });
        }
    }
    photos.sort();
    photos.dedup();
    transcriptions.sort_by(|a, b| a.source.cmp(&b.source));
    transcriptions.dedup_by(|a, b| a.source == b.source);
    Ok(Inventory {
        photos,
        transcriptions,
    })
}
#[derive(Default, Clone, Copy)]
pub struct Readiness {
    pub library_open: bool,
    pub inventory_complete: bool,
    pub index_maintenance: bool,
    pub comments_pending: bool,
    pub comments_ready: bool,
    pub audio_active: bool,
}
impl Readiness {
    pub fn allow_gps(self, mode_loaded: bool, mode: Mode) -> bool {
        self.library_open
            && self.inventory_complete
            && mode_loaded
            && mode == Mode::Running
            && !self.index_maintenance
    }
    pub fn hold_transcription(self, gps_complete: bool) -> bool {
        !self.library_open
            || !self.inventory_complete
            || !gps_complete
            || self.index_maintenance
            || !self.comments_ready
            || self.comments_pending
            || self.audio_active
    }
}
/// Ordering belongs to the use case; callers supply snapshots and render session state.
pub fn advance(gps: &mut geo_sweep::Session, queue: &mut QueueSession, readiness: Readiness) {
    gps.poll(readiness.allow_gps(queue.mode_loaded(), queue.mode()));
    queue.set_hold(readiness.hold_transcription(gps.complete()));
    queue.poll();
}
/// All activity-panel controls share this application workflow, independent of UI widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Pause,
    Stop,
    Resume,
    Retry,
    Rescan,
}
pub fn primary_control(
    gps: &geo_sweep::Session,
    queue: &QueueSession,
    models: &ModelSession,
) -> Control {
    if queue.mode() != Mode::Running {
        return Control::Resume;
    }
    if !queue.error().is_empty() || !gps.error().is_empty() || !models.error().is_empty() {
        return Control::Resume;
    }
    if queue.pending() == 0 && queue.counts().2 > 0 {
        return Control::Retry;
    }
    Control::Pause
}
pub fn control(
    action: Control,
    gps: &mut geo_sweep::Session,
    queue: &mut QueueSession,
    models: &mut ModelSession,
) {
    match action {
        Control::Pause => queue.set_mode(Mode::Paused),
        Control::Stop => queue.set_mode(Mode::Stopped),
        Control::Resume | Control::Retry => {
            if action == Control::Retry {
                queue.retry_failed();
            } else {
                queue.resume();
            }
            let p = gps.progress();
            if !gps.error().is_empty()
                || (action == Control::Retry
                    && (p.failed > 0 || p.location_failed > 0 || p.data_warning.is_some()))
            {
                gps.retry();
            }
            if !models.busy()
                && (!models.error().is_empty()
                    || models.prepared().is_none_or(|p| p.ready_path.is_none()))
            {
                models.reload();
            }
        }
        Control::Rescan => {
            gps.retry();
            queue.rescan();
            if !models.busy() {
                models.reload();
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::comments::Comment;
    #[test]
    fn inventory_preserves_photo_targets_deduplicates_and_rejects_foreign_media() {
        let root = Path::new("/photos");
        let photo = LibraryFile::new("1980/01/img_19800101_120000.jpg".into()).unwrap();
        let audio = LibraryFile::new("2026/audio/comments/com_20261117_120000.m4a".into()).unwrap();
        let mut comments = CommentCatalog::default();
        comments
            .replace_photo(
                &photo,
                [Comment::new(
                    "893d3899-9d2d-4241-8a3c-9f4226353b67".into(),
                    audio.clone(),
                    photo.clone(),
                )
                .unwrap()],
            )
            .unwrap();
        let paths = [
            photo.as_str(),
            audio.as_str(),
            audio.as_str(),
            "2020/01/img_20200101_120000.mp4",
        ];
        let items = paths
            .into_iter()
            .map(|p| Media::from_path(root.join(p), String::new()).unwrap())
            .collect::<Vec<_>>();
        let plan = inventory(root, &items, &comments).unwrap();
        assert_eq!(plan.photos, vec![photo.clone()]);
        assert_eq!(plan.transcriptions.len(), 2);
        assert!(
            plan.transcriptions
                .iter()
                .any(|c| c.source == audio && c.record == photo)
        );
        let foreign = Media::from_path("/other/file.jpg".into(), String::new()).unwrap();
        assert!(inventory(root, &[foreign], &comments).is_err());
    }
    #[test]
    fn preflight_obeys_persisted_mode_complete_inventory_and_exclusive_work() {
        let ready = Readiness {
            library_open: true,
            inventory_complete: true,
            comments_ready: true,
            ..Readiness::default()
        };
        assert!(!ready.allow_gps(false, Mode::Running));
        for mode in [Mode::Paused, Mode::Stopped] {
            assert!(!ready.allow_gps(true, mode));
        }
        assert!(ready.allow_gps(true, Mode::Running));
        assert!(ready.hold_transcription(false));
        assert!(!ready.hold_transcription(true));
        for blocked in [
            Readiness {
                library_open: false,
                ..ready
            },
            Readiness {
                inventory_complete: false,
                ..ready
            },
            Readiness {
                index_maintenance: true,
                ..ready
            },
            Readiness {
                comments_ready: false,
                ..ready
            },
            Readiness {
                comments_pending: true,
                ..ready
            },
            Readiness {
                audio_active: true,
                ..ready
            },
        ] {
            assert!(blocked.hold_transcription(true));
        }
        assert!(
            !Readiness {
                inventory_complete: false,
                ..ready
            }
            .allow_gps(true, Mode::Running)
        );
        assert!(
            !Readiness {
                index_maintenance: true,
                ..ready
            }
            .allow_gps(true, Mode::Running)
        );
    }
}
