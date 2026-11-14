//! Comment catalog use cases. Adapters provide codecs and file operations, never workflow policy.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Kommentarauftrag abgebrochen; Gesamtdatei unverändert".into())
    } else {
        Ok(())
    }
}
pub fn load<R: CommentRepository + ?Sized>(
    repository: &R,
    root: &Path,
    cancel: &AtomicBool,
) -> Result<StoredComments, String> {
    check_cancel(cancel)?;
    match repository.read_cache(root)? {
        CommentCache::Current(stored) => {
            check_cancel(cancel)?;
            Ok(stored)
        }
        CommentCache::Missing => {
            let mut catalog = CommentCatalog::default();
            for comment in repository.scan_photo_records(root, cancel, &|_| {})? {
                check_cancel(cancel)?;
                catalog.insert(comment)?;
            }
            check_cancel(cancel)?;
            Ok(StoredComments {
                catalog,
                revision: None,
            })
        }
        CommentCache::Obsolete => {
            rebuild(repository, root, cancel, &|_| {}).map(|(stored, _)| stored)
        }
    }
}
pub(super) fn rebuild<R: CommentRepository + ?Sized>(
    repository: &R,
    root: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(RebuildProgress),
) -> Result<(StoredComments, String), String> {
    check_cancel(cancel)?;
    let base = repository.rebuild_base(root)?;
    let entries = repository.scan_photo_records(root, cancel, progress)?;
    let mut catalog = CommentCatalog::default();
    for comment in entries {
        check_cancel(cancel)?;
        catalog.insert(comment)?;
    }
    check_cancel(cancel)?;
    let revision = repository.replace_index(root, &catalog, base.revision.as_deref())?;
    let message = format!(
        "Kommentarindex neu aufgebaut: {} Zuordnungen. {}",
        catalog.entries().len(),
        base.warning
    );
    Ok((
        StoredComments {
            catalog,
            revision: Some(revision),
        },
        message,
    ))
}
pub fn commit<R: CommentRepository + ?Sized>(
    repository: &R,
    root: &Path,
    comment: &Comment,
    catalog: &CommentCatalog,
    expected: Option<&[u8]>,
) -> Result<StoredComments, String> {
    let record = repository.write_photo_record(root, comment)?;
    if record.by_audio(comment.audio()) != Some(comment) {
        return Err("Foto-Datensatz bestätigt die neue Aufnahme nicht".into());
    }
    let mut next = catalog.clone();
    next.replace_photo(comment.photo(), record.entries().values().cloned())?;
    let revision = repository.save(root, &next, expected)?;
    Ok(StoredComments {
        catalog: next,
        revision: Some(revision),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Repository {
        cancel_scan: bool,
        record: CommentCatalog,
        writes: AtomicUsize,
    }
    impl CommentRepository for Repository {
        fn read_cache(&self, _: &Path) -> Result<CommentCache, String> {
            Ok(CommentCache::Obsolete)
        }
        fn write_photo_record(&self, _: &Path, _: &Comment) -> Result<CommentCatalog, String> {
            Ok(self.record.clone())
        }
        fn rebuild_base(&self, _: &Path) -> Result<RebuildBase, String> {
            Ok(RebuildBase {
                revision: None,
                warning: String::new(),
            })
        }
        fn scan_photo_records(
            &self,
            _: &Path,
            cancel: &AtomicBool,
            _: &dyn Fn(RebuildProgress),
        ) -> Result<Vec<Comment>, String> {
            if self.cancel_scan {
                cancel.store(true, Ordering::Relaxed);
            }
            Ok(Vec::new())
        }
        fn save(&self, _: &Path, _: &CommentCatalog, _: Option<&[u8]>) -> Result<Vec<u8>, String> {
            self.writes.fetch_add(1, Ordering::Relaxed);
            Ok(vec![1])
        }
    }
    fn comment() -> Comment {
        Comment::new(
            "12345678-1234-1234-1234-123456789abc".into(),
            LibraryFile::new("2026/audio/comments/com_20261113_120000.m4a".into()).unwrap(),
            LibraryFile::new("2008/09/img_20080916_035811.jpg".into()).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn cancellation_after_source_scan_prevents_cache_replacement() {
        let repository = Repository {
            cancel_scan: true,
            record: Default::default(),
            writes: AtomicUsize::new(0),
        };
        assert!(load(&repository, Path::new("/library"), &AtomicBool::new(false)).is_err());
        assert_eq!(repository.writes.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn confirmed_photo_record_is_required_before_saving_derived_index() {
        let mut repository = Repository {
            cancel_scan: false,
            record: Default::default(),
            writes: AtomicUsize::new(0),
        };
        let comment = comment();
        assert!(
            commit(
                &repository,
                Path::new("/library"),
                &comment,
                &CommentCatalog::default(),
                None
            )
            .is_err()
        );
        assert_eq!(repository.writes.load(Ordering::Relaxed), 0);
        repository.record.insert(comment.clone()).unwrap();
        let stored = commit(
            &repository,
            Path::new("/library"),
            &comment,
            &CommentCatalog::default(),
            None,
        )
        .unwrap();
        assert_eq!(stored.catalog.by_audio(comment.audio()), Some(&comment));
        assert_eq!(repository.writes.load(Ordering::Relaxed), 1);
    }
}
