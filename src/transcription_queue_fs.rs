//! SQLite scheduling projection; transcripts and failures remain in authoritative media YAMLs.
use crate::{
    domain::{
        tags::LibraryFile,
        transcript::{SourceVersion, Transcript},
    },
    library_db::{self, Database, Section},
    media_records_fs::{self, Record},
    metadata_fs,
};
use photosuite_application::transcription::queue::{Candidate, Job, JobState, Journal, QueueStore};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::UNIX_EPOCH,
};
#[cfg_attr(test, derive(Default))]
pub struct SqliteTranscriptionQueue {
    pub database: Arc<Database>,
    pub priority: Arc<photosuite_application::work_priority::WorkPriority>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Revision {
    index: Option<Vec<u8>>,
    runtime: Option<Vec<u8>>,
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
fn pack(index: Option<Vec<u8>>, runtime: Option<Vec<u8>>) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&Revision { index, runtime }).map_err(|e| e.to_string())
}
impl SqliteTranscriptionQueue {
    fn failure(
        &self,
        root: &Path,
        candidate: &Candidate,
        failure: Option<media_records_fs::Failure>,
    ) -> Result<(), String> {
        let path = media_records_fs::path(root, &candidate.record, false)?;
        let previous = media_records_fs::read(&path)?;
        let mut record = previous
            .as_deref()
            .map(media_records_fs::parse)
            .transpose()?
            .unwrap_or_else(|| Record::new(&candidate.record));
        if record.file != candidate.record.as_str() {
            return Err("Falsche Medien-YAML für Transkriptionsfehler".into());
        }
        if record.transcription_failures.get(candidate.source.as_str()) == failure.as_ref() {
            return Ok(());
        }
        if let Some(failure) = failure {
            record
                .transcription_failures
                .insert(candidate.source.as_str().into(), failure);
        } else {
            record
                .transcription_failures
                .remove(candidate.source.as_str());
        }
        media_records_fs::write(root, &candidate.record, &record, previous.as_deref())
    }
    pub(crate) fn reconstruct(
        &self,
        root: &Path,
        cancel: &AtomicBool,
    ) -> Result<BTreeMap<LibraryFile, Job>, String> {
        let mut jobs = BTreeMap::new();
        media_records_fs::visit(root, cancel, &self.priority, |file, record, _| {
            for (source, value) in record.transcription_failures {
                let source = LibraryFile::new(source)?;
                jobs.insert(
                    source.clone(),
                    Job {
                        candidate: Candidate {
                            source,
                            record: file.clone(),
                        },
                        version: SourceVersion::new(value.source_bytes, value.source_modified)?,
                        state: JobState::Failed(value.error),
                    },
                );
            }
            for (source, value) in record.transcripts {
                let source = LibraryFile::new(source)?;
                let transcript = value.value()?;
                if jobs
                    .get(&source)
                    .is_none_or(|job| job.version == *transcript.source())
                {
                    jobs.insert(
                        source.clone(),
                        Job {
                            candidate: Candidate {
                                source,
                                record: file.clone(),
                            },
                            version: transcript.source().clone(),
                            state: JobState::Complete,
                        },
                    );
                }
            }
            Ok(())
        })?;
        Ok(jobs)
    }
}
impl QueueStore for SqliteTranscriptionQueue {
    fn load(&self, root: &Path) -> Result<Journal, String> {
        self.load_cancellable(root, &AtomicBool::new(false))
    }
    fn load_cancellable(&self, root: &Path, cancel: &AtomicBool) -> Result<Journal, String> {
        let (state, runtime) = crate::runtime_state_fs::read(root)?;
        let (mut jobs,index)=self.database.transaction(root,|tx|{
   let revision=library_db::revision(tx,Section::Transcription)?;let mut jobs=BTreeMap::new();
   let mut stmt=tx.prepare("SELECT source_path,record_path,source_bytes,source_modified,status,error FROM transcription_jobs ORDER BY source_path")?;
   for row in stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u64>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?)))? {
    let (source,record,bytes,modified,status,error)=row?;let source=LibraryFile::new(source).map_err(library_db::invalid)?;
    let state=match status.as_str(){"pending"=>JobState::Pending,"complete"=>JobState::Complete,"failed"=>JobState::Failed(error),_=>return Err(library_db::invalid("Ungültiger Auftragsstatus"))};
    jobs.insert(source.clone(),Job {candidate:Candidate {source,record:LibraryFile::new(record).map_err(library_db::invalid)?},version:SourceVersion::new(bytes,modified).map_err(library_db::invalid)?,state});
   }Ok((jobs,revision))
  })?;
        if index.is_none() {
            jobs = self.reconstruct(root, cancel)?;
        }
        Ok(Journal {
            jobs,
            mode: state.transcription.mode,
            revision: Some(pack(index, runtime)?),
        })
    }
    fn save(&self, root: &Path, journal: &Journal) -> Result<Vec<u8>, String> {
        let expected: Revision = journal
            .revision
            .as_deref()
            .map(serde_json::from_slice)
            .transpose()
            .map_err(|e| format!("Ungültige Queue-Revision: {e}"))?
            .unwrap_or(Revision {
                index: None,
                runtime: None,
            });
        let (mut state, runtime) = crate::runtime_state_fs::read(root)?;
        if runtime != expected.runtime {
            return Err("runtime.yaml extern geändert; erneut laden".into());
        }
        let previous=self.database.transaction(root,|tx|{
   if library_db::revision(tx,Section::Transcription)?!=expected.index{return Err(library_db::invalid("Warteschlange extern geändert"));}
   let mut previous=BTreeMap::new();let mut stmt=tx.prepare("SELECT source_path,record_path,source_bytes,source_modified,error FROM transcription_jobs WHERE status='failed'")?;
   for row in stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u64>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?)))?{let (source,record,source_bytes,source_modified,error)=row?;previous.insert(source,(record,media_records_fs::Failure{source_bytes,source_modified,error}));}Ok(previous)
  })?;
        for (source, (record, _)) in &previous {
            if journal
                .jobs
                .get(source.as_str())
                .is_none_or(|j| !matches!(j.state, JobState::Failed(_)))
            {
                self.failure(
                    root,
                    &Candidate {
                        source: LibraryFile::new(source.clone())?,
                        record: LibraryFile::new(record.clone())?,
                    },
                    None,
                )?;
            }
        }
        for job in journal.jobs.values() {
            if let JobState::Failed(error) = &job.state {
                let failure = media_records_fs::Failure {
                    source_bytes: job.version.bytes(),
                    source_modified: job.version.modified().into(),
                    error: error.chars().take(8192).collect(),
                };
                if previous
                    .get(job.candidate.source.as_str())
                    .is_none_or(|(record, old)| {
                        record != job.candidate.record.as_str() || old != &failure
                    })
                {
                    self.failure(root, &job.candidate, Some(failure))?;
                }
            }
        }
        if state.transcription.mode != journal.mode || runtime.is_none() {
            state.transcription.mode = journal.mode;
        }
        let new_runtime = if runtime.is_none() || crate::runtime_state_fs::read(root)?.0 != state {
            Some(crate::runtime_state_fs::save(
                root,
                &state,
                runtime.as_deref(),
            )?)
        } else {
            runtime
        };
        let index = self.database.transaction(root, |tx| {
            let revision = library_db::publish(tx, Section::Transcription, expected.index.as_deref())?;
            tx.execute_batch("CREATE TEMP TABLE desired_jobs(source_path TEXT PRIMARY KEY,record_path TEXT,source_bytes INTEGER,source_modified TEXT,status TEXT,error TEXT) WITHOUT ROWID;")?;
            {
                let mut insert = tx.prepare("INSERT INTO desired_jobs VALUES(?1,?2,?3,?4,?5,?6)")?;
                for job in journal.jobs.values() {
                    library_db::media(tx, &job.candidate.source)?;
                    library_db::media(tx, &job.candidate.record)?;
                    let (status, error) = match &job.state {
                        JobState::Pending => ("pending", String::new()),
                        JobState::Complete => ("complete", String::new()),
                        JobState::Failed(error) => ("failed", error.chars().take(8192).collect()),
                    };
                    insert.execute(rusqlite::params![job.candidate.source.as_str(), job.candidate.record.as_str(), job.version.bytes(), job.version.modified(), status, error])?;
                }
            }
            tx.execute("DELETE FROM transcription_jobs WHERE source_path NOT IN(SELECT source_path FROM desired_jobs)", [])?;
            tx.execute("INSERT INTO transcription_jobs SELECT * FROM desired_jobs WHERE 1 ON CONFLICT(source_path) DO UPDATE SET record_path=excluded.record_path,source_bytes=excluded.source_bytes,source_modified=excluded.source_modified,status=excluded.status,error=excluded.error WHERE transcription_jobs.record_path!=excluded.record_path OR transcription_jobs.source_bytes!=excluded.source_bytes OR transcription_jobs.source_modified!=excluded.source_modified OR transcription_jobs.status!=excluded.status OR transcription_jobs.error!=excluded.error", [])?;
            Ok(revision)
        })?;
        pack(Some(index), new_runtime)
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
            record
                .transcription_failures
                .remove(candidate.source.as_str());
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
    use photosuite_application::transcription::queue::Mode;
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
    fn value(store: &SqliteTranscriptionQueue, root: &Path, source: &LibraryFile) -> Transcript {
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
        let store = SqliteTranscriptionQueue::default();
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
        let tags = crate::tag_store::SqliteTags::default();
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
        let store = SqliteTranscriptionQueue::default();
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
        let comments = crate::comments_fs::SqliteComments {
            database: std::sync::Arc::default(),
            priority: Default::default(),
        };
        let comment = Comment::new(
            "12345678-1234-1234-1234-123456789abc".into(),
            audio.clone(),
            photo.clone(),
        )
        .unwrap();
        comments.write_photo_record(root.path(), &comment).unwrap();
        let tags = crate::tag_store::SqliteTags::default();
        let old = tags.read_file_tags(root.path(), &photo).unwrap();
        tags.write_file_tags(
            root.path(),
            &photo,
            &["scan".into()].into(),
            old.revision.as_deref(),
        )
        .unwrap();
        let queue = SqliteTranscriptionQueue::default();
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
        let store = SqliteTranscriptionQueue::default();
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
        let store = SqliteTranscriptionQueue::default();
        assert!(store.load(root.path()).unwrap().jobs.is_empty());

        let source = media(root.path(), "2026/audio/diary/aud_20261115_120000.wav");
        let initial = store.load(root.path()).unwrap();
        let mut journal = Journal {
            revision: initial.revision,
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
        let path = root.path().join(crate::library_db::FILE);
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
