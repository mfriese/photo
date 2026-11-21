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
    pub metadata_pending: bool,
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
            || self.metadata_pending
            || self.comments_pending
            || self.audio_active
    }
}
/// Ordering belongs to the use case; callers supply snapshots and render session state.
#[cfg(test)]
pub fn advance(gps: &mut geo_sweep::Session, queue: &mut QueueSession, readiness: Readiness) {
    gps.poll(readiness.allow_gps(queue.mode_loaded(), queue.mode()));
    queue.set_hold(readiness.hold_transcription(gps.complete()));
    queue.poll();
}
/// One maintenance worker at a time: audio, GPS/places, then delayed queue planning/inference.
/// Poll cancelled workers until they have exited before admitting the next stage.
pub fn advance_with_audio(
    audio: &mut crate::audio_library::Session,
    gps: &mut geo_sweep::Session,
    queue: &mut QueueSession,
    readiness: Readiness,
    model_busy: bool,
) {
    let running = queue.mode_loaded() && queue.mode() == Mode::Running;
    let base = readiness.library_open
        && !readiness.index_maintenance
        && readiness.comments_ready
        && !readiness.metadata_pending
        && !readiness.comments_pending
        && !readiness.audio_active
        && running;
    let preflight = audio.complete() && gps.complete() && readiness.inventory_complete;
    let admitted = audio.inference_ready(preflight && base);
    queue.hold_planning(!admitted || model_busy);
    queue.set_hold(!admitted || model_busy || readiness.hold_transcription(gps.complete()));
    queue.poll();
    let allow_gps =
        base && audio.complete() && !audio.busy() && !queue.busy() && readiness.inventory_complete;
    gps.poll(allow_gps && !model_busy);
    audio.poll(base && !model_busy && !gps.busy() && !queue.busy());
}
/// Stable full workflow projection, including short-lived stages that have already completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    AudioInventory,
    AudioDurations,
    Metadata,
    Years,
    Locations,
    Model,
    Planning,
    Transcription,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Active,
    Paused,
    Complete,
    Warning,
    Error,
}
#[derive(Clone, Debug)]
pub struct StepStatus {
    pub step: Step,
    pub state: StepState,
    pub checked: usize,
    pub total: usize,
    pub info: String,
    pub current: Option<std::path::PathBuf>,
}
pub fn steps(
    audio: &crate::audio_library::Session,
    gps: &geo_sweep::Session,
    queue: &QueueSession,
    models: &ModelSession,
) -> Vec<StepStatus> {
    use crate::audio_library::Stage;
    use StepState::*;
    let waiting = if queue.mode() == Mode::Running {
        Pending
    } else {
        Paused
    };
    let mut rows = Vec::new();
    for (stage, step) in [
        (Stage::Inventory, Step::AudioInventory),
        (Stage::Durations, Step::AudioDurations),
        (Stage::Metadata, Step::Metadata),
        (Stage::Years, Step::Years),
    ] {
        let (complete, progress) = audio.stage_progress(stage);
        let state = if complete {
            if stage == Stage::Durations && !audio.snapshot().warnings.is_empty() {
                Warning
            } else {
                Complete
            }
        } else if audio.stage() == Some(stage) && !audio.error().is_empty() {
            Error
        } else if audio.stage() == Some(stage) && audio.busy() && queue.mode() == Mode::Running {
            Active
        } else {
            waiting
        };
        rows.push(StepStatus {
            step,
            state,
            checked: progress.checked,
            total: progress.total,
            info: match stage {
                Stage::Inventory => "Audioordner und Dateistände abgleichen".into(),
                Stage::Durations => {
                    format!("{} gespeicherte Dauern", audio.snapshot().durations.len())
                }
                Stage::Metadata => "Tags und Foto-Kommentarzuordnungen".into(),
                Stage::Years => format!("{} Jahresblöcke für Alles", audio.snapshot().years.len()),
            },
            current: progress.current,
        });
    }
    let geo = gps.progress();
    rows.push(StepStatus {
        step: Step::Locations,
        state: if gps.complete() {
            if geo.failed + geo.location_failed > 0 || geo.data_warning.is_some() {
                Warning
            } else {
                Complete
            }
        } else if !gps.error().is_empty() {
            Error
        } else if gps.busy() && queue.mode() == Mode::Running {
            Active
        } else {
            waiting
        },
        checked: geo.checked,
        total: geo.total,
        info: format!(
            "{} mit GPS · {} ohne · {} Fehler",
            geo.available,
            geo.missing,
            geo.failed + geo.location_failed
        ),
        current: geo.current.as_ref().map(|file| file.as_str().into()),
    });
    let (done, total, failed) = queue.counts();
    rows.push(StepStatus {
        step: Step::Model,
        state: if !models.error().is_empty() {
            Error
        } else if models.busy() {
            Active
        } else if models.prepared().is_some_and(|p| p.ready_path.is_some())
            || (queue.planned() && total == 0)
        {
            Complete
        } else {
            waiting
        },
        checked: 0,
        total: 0,
        info: models
            .prepared()
            .map(|p| {
                format!(
                    "{} · Sprache {}",
                    p.settings.model().id,
                    p.settings.language()
                )
            })
            .unwrap_or_else(|| "Modell und lokale Gewichte prüfen".into()),
        current: None,
    });
    rows.push(StepStatus {
        step: Step::Planning,
        state: if queue.planned() {
            Complete
        } else if !queue.error().is_empty() {
            Error
        } else if audio.complete() && gps.complete() && queue.planning() {
            Active
        } else {
            waiting
        },
        checked: 0,
        total: 0,
        info: "Dateistände und vorhandene Transkripte abgleichen".into(),
        current: None,
    });
    rows.push(StepStatus {
        step: Step::Transcription,
        state: if queue.planned() && done + failed == total {
            if failed > 0 { Warning } else { Complete }
        } else if !queue.error().is_empty() {
            Error
        } else if queue.busy() && queue.current().is_some() && queue.mode() == Mode::Running {
            Active
        } else {
            waiting
        },
        checked: done + failed,
        total,
        info: format!(
            "{done} fertig · {} offen · {failed} Fehler",
            queue.pending()
        ),
        current: queue.current().map(|file| file.as_str().into()),
    });
    rows
}
/// Bounded, deduplicated per-step requests. Repeats wait for exclusive worker ownership.
#[derive(Default)]
pub struct Repeats {
    pending: std::collections::VecDeque<Step>,
}
impl Repeats {
    pub fn request(&mut self, step: Step) {
        if !self.pending.contains(&step) {
            self.pending.push_back(step);
        }
    }
    pub fn clear(&mut self) {
        self.pending.clear();
    }
    pub fn pending(&self) -> bool {
        !self.pending.is_empty()
    }
    pub fn contains(&self, step: Step) -> bool {
        self.pending.contains(&step)
    }
    pub fn advance(
        &mut self,
        audio: &mut crate::audio_library::Session,
        gps: &mut geo_sweep::Session,
        queue: &mut QueueSession,
        models: &mut ModelSession,
        readiness: Readiness,
    ) {
        if let Some(step) = self.pending.front().copied() {
            audio.inference_ready(false);
            queue.hold_planning(true);
            queue.set_hold(true);
            queue.poll();
            gps.poll(false);
            audio.poll(false);
            if queue.busy()
                || gps.busy()
                || audio.busy()
                || models.busy()
                || !readiness.library_open
                || readiness.index_maintenance
                || readiness.metadata_pending
                || readiness.comments_pending
                || readiness.audio_active
            {
                return;
            }
            use crate::audio_library::Stage;
            let mode = queue.mode();
            match step {
                Step::AudioInventory => audio.repeat_from(Stage::Inventory),
                Step::AudioDurations => audio.repeat_from(Stage::Durations),
                Step::Metadata => audio.repeat_from(Stage::Metadata),
                Step::Years => audio.repeat_from(Stage::Years),
                Step::Locations => gps.retry(),
                Step::Model => models.defer_reload(),
                Step::Planning => queue.rescan(),
                Step::Transcription => queue.retry_failed(),
            }
            if mode != Mode::Running {
                queue.set_mode(mode);
            }
            queue.rescan();
            self.pending.pop_front();
            return;
        }
        advance_with_audio(audio, gps, queue, readiness, models.busy());
    }
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
                models.defer_reload();
            }
        }
        Control::Rescan => {
            gps.retry();
            queue.rescan();
            if !models.busy() {
                models.defer_reload();
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
