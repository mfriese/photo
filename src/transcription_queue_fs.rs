//! Durable queue index and merged transcript updates in the shared media YAML format.
use crate::{
    domain::{
        tags::LibraryFile,
        transcript::{SourceVersion, Transcript},
    },
    media_records_fs::{self, Record},
    metadata_fs,
};
use photosuite_application::transcription::queue::{
    Candidate, Job, JobState, Journal, Mode, QueueStore,
};
use std::{collections::BTreeMap, fs, io::Read, path::Path, time::UNIX_EPOCH};
pub struct YamlTranscriptionQueue;
const FILE: &str = "yaml/transcription-jobs.yaml";
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    mode: String,
    jobs: BTreeMap<String, JobDto>,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JobDto {
    record: String,
    source_bytes: u64,
    source_modified: String,
    state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    error: String,
}
fn source(root: &Path, file: &LibraryFile) -> Result<std::path::PathBuf, String> {
    if crate::domain::Media::from_audio_asset(file.as_str().into(), String::new()).is_none() {
        return Err("Transkriptionsquelle ist kein Audio/Video".into());
    }
    file.metadata_path()?;
    let path = metadata_fs::library_path(root, file.as_str(), false)?;
    if !metadata_fs::check_file(&path)? {
        return Err(format!("Transkriptionsquelle fehlt: {}", file.as_str()));
    }
    Ok(path)
}
impl QueueStore for YamlTranscriptionQueue {
    fn load(&self, root: &Path) -> Result<Journal, String> {
        let path = metadata_fs::library_path(root, FILE, false)?;
        if !metadata_fs::check_file(&path)? {
            return Ok(Journal::default());
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("Transkriptionswarteschlange überschreitet 32 MiB".into());
        }
        let document: Document = metadata_fs::parse_large_yaml(&bytes, 32 * 1024 * 1024)
            .map_err(|e| format!("Transkriptionswarteschlange bleibt unverändert: {e}"))?;
        if document.version != 1 {
            return Err("Unbekannte Transkriptionswarteschlange".into());
        }
        let mode = match document.mode.as_str() {
            "running" => Mode::Running,
            "paused" => Mode::Paused,
            "stopped" => Mode::Stopped,
            _ => return Err("Ungültiger Transkriptionsmodus".into()),
        };
        let mut jobs = BTreeMap::new();
        for (path, value) in document.jobs {
            let file = LibraryFile::new(path)?;
            if crate::domain::Media::from_audio_asset(file.as_str().into(), String::new()).is_none()
            {
                return Err("Warteschlange enthält ungültige Quelle".into());
            }
            let record = LibraryFile::new(value.record)?;
            record.metadata_path()?;
            if record != file
                && !crate::domain::Media::from_path(record.as_str().into(), String::new())
                    .is_some_and(|m| m.is_photo())
            {
                return Err("Ungültige Transkriptzuordnung".into());
            }
            let state = match value.state.as_str() {
                "pending" => JobState::Pending,
                "complete" => JobState::Complete,
                "failed" => JobState::Failed(value.error),
                _ => return Err("Ungültiger Auftragsstatus".into()),
            };
            jobs.insert(
                file.clone(),
                Job {
                    candidate: Candidate {
                        source: file,
                        record,
                    },
                    version: SourceVersion::new(value.source_bytes, value.source_modified)?,
                    state,
                },
            );
        }
        Ok(Journal {
            jobs,
            mode,
            revision: Some(bytes),
        })
    }
    fn save(&self, root: &Path, journal: &Journal) -> Result<Vec<u8>, String> {
        let jobs = journal
            .jobs
            .iter()
            .map(|(source, job)| {
                (
                    source.as_str().to_owned(),
                    JobDto {
                        record: job.candidate.record.as_str().into(),
                        source_bytes: job.version.bytes(),
                        source_modified: job.version.modified().into(),
                        state: match job.state {
                            JobState::Pending => "pending",
                            JobState::Complete => "complete",
                            JobState::Failed(_) => "failed",
                        }
                        .into(),
                        error: if let JobState::Failed(e) = &job.state {
                            e.chars().take(8192).collect()
                        } else {
                            String::new()
                        },
                    },
                )
            })
            .collect();
        let document = Document {
            version: 1,
            mode: match journal.mode {
                Mode::Running => "running",
                Mode::Paused => "paused",
                Mode::Stopped => "stopped",
            }
            .into(),
            jobs,
        };
        let bytes = serde_saphyr::to_string(&document)
            .map_err(|e| e.to_string())?
            .into_bytes();
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("Transkriptionswarteschlange überschreitet 32 MiB".into());
        }
        metadata_fs::atomic_write_if_current(
            &metadata_fs::library_path(root, FILE, true)?,
            &bytes,
            journal.revision.as_deref(),
        )?;
        Ok(bytes)
    }
    fn version(&self, root: &Path, file: &LibraryFile) -> Result<SourceVersion, String> {
        let path = source(root, file)?;
        let m = fs::metadata(path).map_err(|e| e.to_string())?;
        let modified = m.modified().map_err(|e| e.to_string())?;
        let stamp = match modified.duration_since(UNIX_EPOCH) {
            Ok(t) => format!("{}.{:09}", t.as_secs(), t.subsec_nanos()),
            Err(e) => format!(
                "-{}.{:09}",
                e.duration().as_secs(),
                e.duration().subsec_nanos()
            ),
        };
        SourceVersion::new(m.len(), stamp)
    }
    fn existing(&self, root: &Path, candidate: &Candidate) -> Result<Option<Transcript>, String> {
        let path = media_records_fs::path(root, &candidate.record, false)?;
        let Some(bytes) = media_records_fs::read(&path)? else {
            return Ok(None);
        };
        let record = media_records_fs::parse(&bytes)?;
        if record.file != candidate.record.as_str() {
            return Err("Medien-YAML gehört zu einer anderen Datei".into());
        }
        record
            .transcripts
            .get(candidate.source.as_str())
            .map(|t| t.value())
            .transpose()
    }
    fn write_transcript(
        &self,
        root: &Path,
        candidate: &Candidate,
        transcript: &Transcript,
    ) -> Result<(), String> {
        let mut last_error = String::new();
        for _ in 0..4 {
            if self.version(root, &candidate.source)? != *transcript.source() {
                return Err("Quelle wurde geändert; Transkript nicht veröffentlicht".into());
            }
            let record_path = metadata_fs::library_path(root, candidate.record.as_str(), false)?;
            if !metadata_fs::check_file(&record_path)? {
                return Err("Bezugsmedium fehlt; Transkript bleibt unveröffentlicht".into());
            }
            let path = media_records_fs::path(root, &candidate.record, false)?;
            let previous = media_records_fs::read(&path)?;
            let mut record = previous
                .as_deref()
                .map(media_records_fs::parse)
                .transpose()?
                .unwrap_or_else(|| Record::new(&candidate.record));
            if record.file != candidate.record.as_str() {
                return Err("Medien-YAML gehört zu einer anderen Datei".into());
            }
            if candidate.record != candidate.source
                && !record
                    .comments
                    .values()
                    .any(|p| p == candidate.source.as_str())
            {
                return Err(
                    "Foto-Kommentarzuordnung wurde verändert; keine fremde YAML beschreiben".into(),
                );
            }
            record.transcripts.insert(
                candidate.source.as_str().into(),
                crate::transcript_codec::TranscriptDto::from_value(transcript),
            );
            match media_records_fs::write(root, &candidate.record, &record, previous.as_deref()) {
                Ok(()) => return Ok(()),
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        comments::Comment,
        transcript::{Segment, TranscriptStatus},
    };
    use photosuite_application::{comments::CommentRepository, organization::CatalogRepository};
    fn file(value: &str) -> LibraryFile {
        LibraryFile::new(value.into()).unwrap()
    }
    fn media(root: &Path, name: &str) -> LibraryFile {
        let file = file(name);
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"unchanged original media").unwrap();
        file
    }
    fn value(store: &YamlTranscriptionQueue, root: &Path, source: &LibraryFile) -> Transcript {
        Transcript::new(
            store.version(root, source).unwrap(),
            TranscriptStatus::Complete,
            "tiny".into(),
            "de".into(),
            "Er sagt: \"Grüße!\" # Erinnerung\n\nZweite Zeile: äöü".into(),
            vec![Segment::new(0, 1000, "Grüße".into()).unwrap()],
        )
        .unwrap()
    }
    #[test]
    fn transcript_at_supported_segment_limit_can_be_saved_reloaded_and_tagged() {
        let root = tempfile::tempdir().unwrap();
        let store = YamlTranscriptionQueue;
        let source = media(root.path(), "2017/04/img_20170407_040311.mp4");
        let candidate = Candidate {
            source: source.clone(),
            record: source.clone(),
        };
        let segments = (0..50000)
            .map(|i| Segment::new(i * 1000, i * 1000 + 500, "x".into()).unwrap())
            .collect();
        let transcript = Transcript::new(
            store.version(root.path(), &source).unwrap(),
            TranscriptStatus::Complete,
            "tiny".into(),
            "de".into(),
            "x ".repeat(50000),
            segments,
        )
        .unwrap();
        store
            .write_transcript(root.path(), &candidate, &transcript)
            .unwrap();
        assert_eq!(
            store.existing(root.path(), &candidate).unwrap().as_ref(),
            Some(&transcript)
        );
        let tags = crate::tag_store::YamlTags::default();
        let old = tags.read_file_tags(root.path(), &source).unwrap();
        tags.write_file_tags(
            root.path(),
            &source,
            &["video".into()].into(),
            old.revision.as_deref(),
        )
        .unwrap();
        assert_eq!(
            store.existing(root.path(), &candidate).unwrap().as_ref(),
            Some(&transcript)
        );
        assert_eq!(
            fs::read(root.path().join(source.as_str())).unwrap(),
            b"unchanged original media"
        );
    }

    #[test]
    fn large_valid_queue_can_be_reopened_after_saving() {
        let root = tempfile::tempdir().unwrap();
        let store = YamlTranscriptionQueue;
        let mut journal = Journal::default();
        for i in 0..40000 {
            let source = file(&format!("2017/04/img_20170407_040311_{i:05}.mp4"));
            journal.jobs.insert(
                source.clone(),
                Job {
                    candidate: Candidate {
                        source: source.clone(),
                        record: source,
                    },
                    version: SourceVersion::new(123, "123.456".into()).unwrap(),
                    state: JobState::Pending,
                },
            );
        }
        store.save(root.path(), &journal).unwrap();
        let restored = store
            .load(root.path())
            .expect("valid saved large queue must reload");
        assert_eq!(restored.jobs.len(), journal.jobs.len());
    }

    #[test]
    fn photo_comment_transcript_round_trip_preserves_tags_links_and_source_bytes() {
        let root = tempfile::tempdir().unwrap();
        let photo = media(root.path(), "1980/09/img_19800916_035811.jpg");
        let audio = media(root.path(), "2026/audio/comments/com_20261115_120000.m4a");
        let comments = crate::comments_fs::YamlComments {
            priority: Default::default(),
        };
        let comment = Comment::new(
            "12345678-1234-1234-1234-123456789abc".into(),
            audio.clone(),
            photo.clone(),
        )
        .unwrap();
        comments.write_photo_record(root.path(), &comment).unwrap();
        let tags = crate::tag_store::YamlTags::default();
        let old = tags.read_file_tags(root.path(), &photo).unwrap();
        tags.write_file_tags(
            root.path(),
            &photo,
            &["scan".into()].into(),
            old.revision.as_deref(),
        )
        .unwrap();
        let queue = YamlTranscriptionQueue;
        let candidate = Candidate {
            source: audio.clone(),
            record: photo.clone(),
        };
        let transcript = value(&queue, root.path(), &audio);
        queue
            .write_transcript(root.path(), &candidate, &transcript)
            .unwrap();
        let restored = queue.existing(root.path(), &candidate).unwrap().unwrap();
        assert_eq!(restored, transcript);
        let path = media_records_fs::path(root.path(), &photo, false).unwrap();
        let record = media_records_fs::parse(&fs::read(&path).unwrap()).unwrap();
        assert!(record.tags.unwrap().contains("scan"));
        assert_eq!(record.comments[comment.id()], audio.as_str());
        assert!(
            !root.path().join(audio.metadata_path().unwrap()).exists(),
            "comment text belongs to photo record"
        );
        let old = tags.read_file_tags(root.path(), &photo).unwrap();
        tags.write_file_tags(
            root.path(),
            &photo,
            &["scan".into(), "dia".into()].into(),
            old.revision.as_deref(),
        )
        .unwrap();
        let audio2 = media(root.path(), "2026/audio/comments/com_20261115_120001.m4a");
        comments
            .write_photo_record(
                root.path(),
                &Comment::new(
                    "12345678-1234-1234-1234-123456789abd".into(),
                    audio2,
                    photo.clone(),
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            queue.existing(root.path(), &candidate).unwrap().unwrap(),
            transcript
        );
        assert_eq!(
            fs::read(root.path().join(photo.as_str())).unwrap(),
            b"unchanged original media"
        );
        assert_eq!(
            fs::read(root.path().join(audio.as_str())).unwrap(),
            b"unchanged original media"
        );
        let record = media_records_fs::parse(&fs::read(path).unwrap()).unwrap();
        assert_eq!(record.comments.len(), 2);
        assert!(record.tags.unwrap().contains("dia"));
    }
    #[test]
    fn diary_and_video_use_identical_media_payload_and_changed_or_corrupt_sources_are_not_overwritten()
     {
        let root = tempfile::tempdir().unwrap();
        let store = YamlTranscriptionQueue;
        for path in [
            "2026/audio/diary/aud_20261115_120000.wav",
            "2026/11/img_20261115_120000.mp4",
        ] {
            let source = media(root.path(), path);
            let candidate = Candidate {
                record: source.clone(),
                source: source.clone(),
            };
            let transcript = value(&store, root.path(), &source);
            store
                .write_transcript(root.path(), &candidate, &transcript)
                .unwrap();
            assert_eq!(
                store.existing(root.path(), &candidate).unwrap().unwrap(),
                transcript
            );
            let yaml = media_records_fs::path(root.path(), &source, false).unwrap();
            let before = fs::read(&yaml).unwrap();
            fs::write(root.path().join(path), b"changed media").unwrap();
            assert!(
                store
                    .write_transcript(root.path(), &candidate, &transcript)
                    .is_err()
            );
            assert_eq!(fs::read(&yaml).unwrap(), before);
            let transcript = value(&store, root.path(), &source);
            fs::write(&yaml, b"broken: [").unwrap();
            assert!(
                store
                    .write_transcript(root.path(), &candidate, &transcript)
                    .is_err()
            );
            assert_eq!(fs::read(yaml).unwrap(), b"broken: [");
        }
    }
    #[test]
    fn queue_modes_and_failures_round_trip_and_external_or_symbolic_metadata_remains_intact() {
        let root = tempfile::tempdir().unwrap();
        let store = YamlTranscriptionQueue;
        assert!(store.load(root.path()).unwrap().jobs.is_empty());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        let source = media(root.path(), "2026/audio/diary/aud_20261115_120000.wav");
        let mut journal = Journal {
            mode: Mode::Paused,
            ..Default::default()
        };
        journal.jobs.insert(
            source.clone(),
            Job {
                candidate: Candidate {
                    record: source.clone(),
                    source: source.clone(),
                },
                version: store.version(root.path(), &source).unwrap(),
                state: JobState::Failed("codec error".into()),
            },
        );
        journal.revision = Some(store.save(root.path(), &journal).unwrap());
        let loaded = store.load(root.path()).unwrap();
        assert_eq!(loaded.mode, Mode::Paused);
        assert!(matches!(&loaded.jobs[&source].state,JobState::Failed(e) if e=="codec error"));
        let path = root.path().join(FILE);
        fs::write(&path, b"outside change").unwrap();
        assert!(store.save(root.path(), &loaded).is_err());
        assert!(store.load(root.path()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"outside change");
        #[cfg(unix)]
        {
            let root = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(outside.path(), root.path().join("yaml")).unwrap();
            assert!(store.save(root.path(), &Journal::default()).is_err());
            assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
        }
    }
}
