//! Per-media YAML sources and the derived SQLite tag projection.
use crate::{
    domain::tags::{LibraryFile, TagCatalog},
    library_db::{self, Database, Section},
};
use photosuite_application::organization::{CatalogRepository, StoredCatalog};
use std::{path::Path, sync::Arc};
#[cfg_attr(test, derive(Default))]
pub struct SqliteTags {
    pub priority: Arc<photosuite_application::work_priority::WorkPriority>,
    pub database: Arc<Database>,
}

impl CatalogRepository for SqliteTags {
    fn supports_file_records(&self) -> bool {
        true
    }
    fn read_file_tags(
        &self,
        root: &Path,
        file: &crate::domain::tags::LibraryFile,
    ) -> Result<photosuite_application::organization::FileTags, String> {
        let path = crate::media_records_fs::path(root, file, false)?;
        let revision = crate::media_records_fs::read(&path)?;
        let tags = if let Some(bytes) = &revision {
            let record = crate::media_records_fs::parse(bytes)?;
            if record.file != file.as_str() {
                return Err("Medien-YAML gehört zu einer anderen Datei".into());
            }
            record.tags
        } else {
            None
        };
        Ok(photosuite_application::organization::FileTags {
            file: file.clone(),
            tags,
            revision,
        })
    }
    fn write_file_tags(
        &self,
        root: &Path,
        file: &crate::domain::tags::LibraryFile,
        tags: &std::collections::BTreeSet<String>,
        expected: Option<&[u8]>,
    ) -> Result<(), String> {
        let path = crate::media_records_fs::path(root, file, false)?;
        let current = crate::media_records_fs::read(&path)?;
        if current.as_deref() != expected {
            return Err("Medien-YAML wurde gleichzeitig verändert; erneut versuchen".into());
        }
        let mut record = if let Some(bytes) = &current {
            crate::media_records_fs::parse(bytes)?
        } else {
            crate::media_records_fs::Record::new(file)
        };
        record.tags = Some(tags.clone());
        crate::media_records_fs::write(root, file, &record, expected)
    }
    fn scan_file_tags(
        &self,
        root: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<photosuite_application::organization::FileTags>, String> {
        let mut records = Vec::new();
        crate::media_records_fs::visit(root, cancel, &self.priority, |file, record, _| {
            records.push(photosuite_application::organization::FileTags {
                file,
                tags: record.tags,
                revision: None,
            });
            Ok(())
        })?;
        Ok(records)
    }

    fn load(&self, root: &Path) -> Result<StoredCatalog, String> {
        self.database.transaction(root, |tx| {
            let revision = library_db::revision(tx, Section::Tags)?;
            let mut catalog = TagCatalog::default();
            let mut stmt = tx.prepare(
                "SELECT tag_name,media_path FROM media_tags ORDER BY tag_name,media_path",
            )?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (name, path) = row?;
                if !catalog.tags().contains_key(&name) {
                    catalog.create(&name).map_err(library_db::invalid)?;
                }
                catalog
                    .assign(
                        &name,
                        LibraryFile::new(path).map_err(library_db::invalid)?,
                        true,
                    )
                    .map_err(library_db::invalid)?;
            }
            Ok(StoredCatalog { catalog, revision })
        })
    }
    fn save(
        &self,
        root: &Path,
        catalog: &TagCatalog,
        expected: Option<&[u8]>,
    ) -> Result<Vec<u8>, String> {
        self.database.transaction(root,|tx| {
            let revision=library_db::publish(tx,Section::Tags,expected)?;
            tx.execute_batch("CREATE TEMP TABLE desired_tags(media_path TEXT NOT NULL,tag_name TEXT NOT NULL,PRIMARY KEY(media_path,tag_name)) WITHOUT ROWID;")?;
            {let mut stmt=tx.prepare("INSERT INTO desired_tags VALUES(?1,?2)")?;
            for (name,files) in catalog.tags() {for file in files {library_db::media(tx,file)?;stmt.execute((file.as_str(),name))?;}}}
            tx.execute("DELETE FROM media_tags WHERE NOT EXISTS(SELECT 1 FROM desired_tags d WHERE d.media_path=media_tags.media_path AND d.tag_name=media_tags.tag_name)",[])?;
            tx.execute("INSERT OR IGNORE INTO media_tags SELECT media_path,tag_name FROM desired_tags",[])?;
            Ok(revision)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photosuite_application::organization::index;
    #[test]
    fn single_yaml_is_authoritative_stale_sql_writers_are_rejected_and_empty_tags_disappear() {
        let root = tempfile::tempdir().unwrap();
        let store = SqliteTags::default();
        let file = LibraryFile::new("2023/01/a.jpg".into()).unwrap();
        let loaded = index::load(&store, root.path()).unwrap();
        let mut desired = loaded.catalog.clone();
        desired.create("SCAN").unwrap();
        desired.assign("scan", file.clone(), true).unwrap();
        index::save(
            &store,
            root.path(),
            &loaded.catalog,
            &desired,
            loaded.revision.as_deref(),
        )
        .unwrap();
        assert!(
            store
                .save(root.path(), &Default::default(), loaded.revision.as_deref())
                .is_err()
        );
        let previous = store.load(root.path()).unwrap();
        let mut empty = previous.catalog.clone();
        empty.assign("scan", file.clone(), false).unwrap();
        empty.prune_empty();
        index::save(
            &store,
            root.path(),
            &previous.catalog,
            &empty,
            previous.revision.as_deref(),
        )
        .unwrap();
        assert!(store.load(root.path()).unwrap().catalog.tags().is_empty());
        let record = crate::media_records_fs::read(
            &crate::media_records_fs::path(root.path(), &file, false).unwrap(),
        )
        .unwrap()
        .unwrap();
        assert!(
            crate::media_records_fs::parse(&record)
                .unwrap()
                .tags
                .unwrap()
                .is_empty()
        );
        assert!(!root.path().join("yaml/tag-definitions.yaml").exists());
        assert!(!root.path().join("yaml/tags.yaml").exists());
    }
}
