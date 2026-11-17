//! Complete index reconstruction from current per-media YAMLs; old aggregate YAMLs are ignored.
use crate::{
    domain::{
        comments::{Comment, CommentCatalog},
        tags::{LibraryFile, TagCatalog},
    },
    library_db::{self, Database, Section},
    media_records_fs,
};
use photosuite_application::{
    library_index::Rebuilder,
    transcription::queue::{Candidate, Job, JobState},
};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub struct SqliteRebuilder {
    pub database: Arc<Database>,
    pub priority: Arc<photosuite_application::work_priority::WorkPriority>,
}
impl Rebuilder for SqliteRebuilder {
    fn rebuild(
        &self,
        root: &Path,
        cancel: &AtomicBool,
        notify: &(dyn Fn(usize) + Sync),
    ) -> Result<(), String> {
        self.database.replace(root, cancel, |staging| {
            let mut tags = TagCatalog::default();
            let mut comments = CommentCatalog::default();
            let mut jobs = BTreeMap::<LibraryFile, Job>::new();
            let mut files = Vec::new();
            let mut revisions = Vec::new();
            let mut checked = 0;
            media_records_fs::visit(root, cancel, &self.priority, |file, record, _| {
                files.push(file.clone());
                let path=media_records_fs::path(root,&file,false)?;
                let metadata=std::fs::symlink_metadata(&path).map_err(|e|e.to_string())?;
                revisions.push((path,metadata.len(),metadata.modified().map_err(|e|e.to_string())?));
                for tag in record.tags.into_iter().flatten() {
                    if !tags.tags().contains_key(&tag) {
                        tags.create(&tag)?;
                    }
                    tags.assign(&tag, file.clone(), true)?;
                }
                for (id, audio) in record.comments {
                    comments.insert(Comment::new(id, LibraryFile::new(audio)?, file.clone())?)?;
                }
                for (source, value) in record.transcripts {
                    let source = LibraryFile::new(source)?;
                    let transcript = value.value()?;
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
                for (source, failure) in record.transcription_failures {
                    let source = LibraryFile::new(source)?;
                    let version = crate::domain::transcript::SourceVersion::new(
                        failure.source_bytes,
                        failure.source_modified,
                    )?;
                    if jobs.get(&source).is_none_or(|j| j.version != version) {
                        jobs.insert(
                            source.clone(),
                            Job {
                                candidate: Candidate {
                                    source,
                                    record: file.clone(),
                                },
                                version,
                                state: JobState::Failed(failure.error),
                            },
                        );
                    }
                }
                checked += 1;
                notify(checked);
                Ok(())
            })?;
            if cancel.load(Ordering::Relaxed) {
                return Err("Datenbank-Neuaufbau abgebrochen".into());
            }
            for (path,size,modified) in &revisions {
                let metadata=std::fs::symlink_metadata(path).map_err(|e|e.to_string())?;
                if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len()!=*size || metadata.modified().map_err(|e|e.to_string())?!=*modified {return Err("Einzel-YAML während des Neuaufbaus geändert; erneut versuchen".into());}
            }
            self.database.transaction(staging, |tx| {
                for file in &files {
                    library_db::media(tx, file)?;
                }
                for (name, files) in tags.tags() {
                    for file in files {
                        tx.execute(
                            "INSERT INTO media_tags(media_path,tag_name) VALUES(?1,?2)",
                            (file.as_str(), name),
                        )?;
                    }
                }
                for comment in comments.entries().values() {
                    library_db::media(tx, comment.photo())?;
                    library_db::media(tx, comment.audio())?;
                    tx.execute("UPDATE media SET reference_at=(SELECT reference_at FROM media WHERE path=?1) WHERE path=?2",(comment.photo().as_str(),comment.audio().as_str()))?;
                    tx.execute(
                        "INSERT INTO audio_comments(id,photo_path,audio_path) VALUES(?1,?2,?3)",
                        (
                            comment.id(),
                            comment.photo().as_str(),
                            comment.audio().as_str(),
                        ),
                    )?;
                }
                for job in jobs.values() {
                    library_db::media(tx, &job.candidate.source)?;
                    library_db::media(tx, &job.candidate.record)?;
                    let (status, error) = match &job.state {
                        JobState::Complete => ("complete", ""),
                        JobState::Failed(e) => ("failed", e.as_str()),
                        JobState::Pending => ("pending", ""),
                    };
                    tx.execute(
                        "INSERT INTO transcription_jobs VALUES(?1,?2,?3,?4,?5,?6)",
                        rusqlite::params![
                            job.candidate.source.as_str(),
                            job.candidate.record.as_str(),
                            job.version.bytes(),
                            job.version.modified(),
                            status,
                            error
                        ],
                    )?;
                }
                for section in [Section::Tags, Section::Comments, Section::Transcription] {
                    library_db::publish(tx, section, None)?;
                }
                Ok(())
            })?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        comments_fs::SqliteComments, tag_store::SqliteTags,
        transcription_queue_fs::SqliteTranscriptionQueue,
    };
    use photosuite_application::{
        comments::load_catalog,
        organization::{CatalogRepository, index},
        runtime::RuntimeState,
        transcription::queue::{Mode, QueueStore},
    };
    fn file(value: &str) -> LibraryFile {
        LibraryFile::new(value.into()).unwrap()
    }
    fn media(root: &Path, value: &str) -> LibraryFile {
        let file = file(value);
        let path = root.join(value);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"untouched original").unwrap();
        file
    }
    fn fixture(root: &Path) -> (Arc<Database>, LibraryFile, LibraryFile, LibraryFile) {
        let database = Arc::new(Database);
        let photo = media(root, "1980/09/img_19800916_035811.jpg");
        let audio = media(root, "2026/audio/comments/com_20261117_120000.m4a");
        let diary = media(root, "2026/audio/diary/aud_20261117_130000.wav");
        let mut record = media_records_fs::Record::new(&photo);
        record.tags = Some(["scan".into(), "commented".into()].into());
        record.comments.insert(
            "12345678-1234-1234-1234-123456789abc".into(),
            audio.as_str().into(),
        );
        let queue = SqliteTranscriptionQueue {
            database: database.clone(),
            priority: Arc::default(),
        };
        let transcript = crate::domain::transcript::Transcript::new(
            queue.version(root, &audio).unwrap(),
            crate::domain::transcript::TranscriptStatus::Complete,
            "tiny".into(),
            "de".into(),
            "Nur im Einzel-YAML: persönliche Erinnerung".into(),
            vec![],
        )
        .unwrap();
        record.transcripts.insert(
            audio.as_str().into(),
            crate::transcript_codec::TranscriptDto::from_value(&transcript),
        );
        media_records_fs::write(root, &photo, &record, None).unwrap();
        let mut failed = media_records_fs::Record::new(&diary);
        let version = queue.version(root, &diary).unwrap();
        failed.tags = Some(["diary".into()].into());
        failed.transcription_failures.insert(
            diary.as_str().into(),
            media_records_fs::Failure {
                source_bytes: version.bytes(),
                source_modified: version.modified().into(),
                error: "codec error".into(),
            },
        );
        media_records_fs::write(root, &diary, &failed, None).unwrap();
        let mut state = RuntimeState::default();
        state.transcription.mode = Mode::Paused;
        crate::runtime_state_fs::save(root, &state, None).unwrap();
        (database, photo, audio, diary)
    }
    #[test]
    fn complete_rebuild_and_database_loss_recover_tags_comments_jobs_without_text_duplication() {
        let root = tempfile::tempdir().unwrap();
        let (database, photo, audio, diary) = fixture(root.path());
        let settings = b"# Benutzerkonfiguration bleibt lesbar\nversion: 1\n";
        std::fs::write(root.path().join("yaml/settings.yaml"), settings).unwrap();
        for old in [
            "tags.yaml",
            "tag-definitions.yaml",
            "comments-index.yaml",
            "transcription-jobs.yaml",
        ] {
            std::fs::write(root.path().join("yaml").join(old), b"broken: [ ignored").unwrap();
        }
        let original = std::fs::read(root.path().join(photo.metadata_path().unwrap())).unwrap();
        let rebuild = SqliteRebuilder {
            database: database.clone(),
            priority: Arc::default(),
        };
        rebuild
            .rebuild(root.path(), &AtomicBool::new(false), &|_| {})
            .unwrap();
        let tags = SqliteTags {
            database: database.clone(),
            priority: Arc::default(),
        };
        let comments = SqliteComments {
            database: database.clone(),
            priority: Arc::default(),
        };
        let queue = SqliteTranscriptionQueue {
            database: database.clone(),
            priority: Arc::default(),
        };
        let before = tags.load(root.path()).unwrap().catalog;
        assert!(before.contains("scan", photo.as_str()));
        assert_eq!(
            load_catalog(&comments, root.path(), &AtomicBool::new(false))
                .unwrap()
                .catalog
                .entries()
                .len(),
            1
        );
        let jobs = queue.load(root.path()).unwrap();
        assert_eq!(jobs.mode, Mode::Paused);
        assert!(matches!(jobs.jobs[&audio].state, JobState::Complete));
        assert!(matches!(&jobs.jobs[&diary].state,JobState::Failed(e) if e=="codec error"));
        database.transaction(root.path(),|tx|{
            let text_columns:i64=tx.query_row("SELECT count(*) FROM pragma_table_info('transcription_jobs') WHERE name IN ('text','segments')",[],|r|r.get(0))?;assert_eq!(text_columns,0);
            let definitions:i64=tx.query_row("SELECT count(*) FROM sqlite_schema WHERE name IN ('albums','tag_definitions','application_state')",[],|r|r.get(0))?;assert_eq!(definitions,0);
            let (recorded,reference):(String,String)=tx.query_row("SELECT recorded_at,reference_at FROM media WHERE path=?1",[audio.as_str()],|r|Ok((r.get(0)?,r.get(1)?)))?;assert!(recorded.starts_with("2026-11"));assert!(reference.starts_with("1980-09"));
            let plan:String=tx.query_row("EXPLAIN QUERY PLAN SELECT media_path FROM media_tags WHERE tag_name=?1",["scan"],|r|r.get(3))?;assert!(plan.contains("media_tags_by_name"));Ok(())
        }).unwrap();
        std::fs::remove_file(root.path().join(library_db::FILE)).unwrap();
        assert_eq!(index::load(&tags, root.path()).unwrap().catalog, before);
        assert_eq!(
            load_catalog(&comments, root.path(), &AtomicBool::new(false))
                .unwrap()
                .catalog
                .entries()
                .len(),
            1
        );
        let recovered = queue.load(root.path()).unwrap();
        assert!(matches!(recovered.jobs[&audio].state, JobState::Complete));
        assert!(matches!(recovered.jobs[&diary].state, JobState::Failed(_)));
        assert_eq!(recovered.mode, Mode::Paused);
        assert_eq!(
            std::fs::read(root.path().join(photo.metadata_path().unwrap())).unwrap(),
            original
        );
        assert_eq!(
            std::fs::read(root.path().join("yaml/settings.yaml")).unwrap(),
            settings
        );
        for value in [&photo, &audio, &diary] {
            assert_eq!(
                std::fs::read(root.path().join(value.as_str())).unwrap(),
                b"untouched original"
            );
        }
    }
    #[test]
    fn interrupted_failed_or_stale_rebuild_does_not_publish_partial_database() {
        let root = tempfile::tempdir().unwrap();
        let (database, photo, _, _) = fixture(root.path());
        let rebuild = SqliteRebuilder {
            database: database.clone(),
            priority: Arc::default(),
        };
        rebuild
            .rebuild(root.path(), &AtomicBool::new(false), &|_| {})
            .unwrap();
        let path = root.path().join(library_db::FILE);
        let before = std::fs::read(&path).unwrap();
        assert!(
            rebuild
                .rebuild(root.path(), &AtomicBool::new(true), &|_| {})
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let sidecar = root.path().join(photo.metadata_path().unwrap());
        let bytes = std::fs::read(&sidecar).unwrap();
        std::fs::write(&sidecar, b"bad: [").unwrap();
        assert!(
            rebuild
                .rebuild(root.path(), &AtomicBool::new(false), &|_| {})
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::write(&sidecar, bytes).unwrap();
        let changed = AtomicBool::new(false);
        assert!(
            rebuild
                .rebuild(root.path(), &AtomicBool::new(false), &|_| {
                    if !changed.swap(true, Ordering::Relaxed) {
                        database
                            .transaction(root.path(), |tx| {
                                tx.execute(
                                    "UPDATE index_state SET change_sequence=change_sequence+1",
                                    [],
                                )?;
                                Ok(())
                            })
                            .unwrap();
                    }
                })
                .is_err()
        );
        assert!(changed.load(Ordering::Relaxed));
    }
    #[test]
    fn full_rebuild_replaces_corrupt_index_without_migrating_aggregate_yamls() {
        let root = tempfile::tempdir().unwrap();
        let (database, photo, _, _) = fixture(root.path());
        let path = root.path().join(library_db::FILE);
        std::fs::write(&path, b"broken index").unwrap();
        let rebuild = SqliteRebuilder {
            database: database.clone(),
            priority: Arc::default(),
        };
        rebuild
            .rebuild(root.path(), &AtomicBool::new(false), &|_| {})
            .unwrap();
        assert!(
            SqliteTags {
                database,
                priority: Arc::default()
            }
            .load(root.path())
            .unwrap()
            .catalog
            .contains("scan", photo.as_str())
        );
    }
}
