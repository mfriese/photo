//! Catalog use cases: authoritative media records first, derived cache last.
use super::*;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Tagauftrag abgebrochen; Einzeldateien bleiben erhalten".into())
    } else {
        Ok(())
    }
}
fn tags(catalog: &TagCatalog, file: &LibraryFile) -> BTreeSet<String> {
    catalog
        .tags_for(file)
        .into_iter()
        .map(str::to_owned)
        .collect()
}
pub fn load(repository: &dyn CatalogRepository, root: &Path) -> Result<StoredCatalog, String> {
    load_cancel(repository, root, &AtomicBool::new(false))
}
fn load_cancel(
    repository: &dyn CatalogRepository,
    root: &Path,
    cancel: &AtomicBool,
) -> Result<StoredCatalog, String> {
    check(cancel)?;
    let stored = repository.load(root)?;
    if stored.revision.is_some() || !repository.supports_file_records() {
        return Ok(stored);
    }
    Ok(StoredCatalog {
        catalog: from_records(repository, root, cancel)?,
        revision: None,
    })
}
/// One-time, explicitly supported upgrade of existing central memberships to per-file sources.
pub fn load_with_backfill(
    repository: &dyn CatalogRepository,
    root: &Path,
) -> Result<StoredCatalog, String> {
    load_with_backfill_cancel(repository, root, &AtomicBool::new(false))
}
pub(super) fn load_with_backfill_cancel(
    repository: &dyn CatalogRepository,
    root: &Path,
    cancel: &AtomicBool,
) -> Result<StoredCatalog, String> {
    let mut stored = load_cancel(repository, root, cancel)?;

    if repository.supports_file_records()
        && stored.revision.is_some()
        && repository.load_definitions(root)?.revision.is_none()
    {
        let mut desired = stored.catalog.clone();
        for record in repository.scan_file_tags(root, cancel)? {
            if let Some(actual) = record.tags {
                for old in tags(&desired, &record.file) {
                    desired.assign(&old, record.file.clone(), false)?;
                }
                for name in actual {
                    if !desired.tags().contains_key(&name) {
                        desired.create(&name)?;
                    }
                    desired.assign(&name, record.file.clone(), true)?;
                }
            }
        }
        stored.revision = Some(save_cancel(
            repository,
            root,
            &stored.catalog,
            &desired,
            stored.revision.as_deref(),
            true,
            cancel,
        )?);
        stored.catalog = desired;
    }
    Ok(stored)
}
fn from_records(
    repository: &dyn CatalogRepository,
    root: &Path,
    cancel: &AtomicBool,
) -> Result<TagCatalog, String> {
    let definitions = repository.load_definitions(root)?;
    let mut catalog = if definitions.revision.is_none() {
        repository
            .load(root)
            .map(|s| s.catalog.definitions())
            .unwrap_or_default()
    } else {
        definitions.catalog.definitions()
    };
    for record in repository.scan_file_tags(root, cancel)? {
        for name in record.tags.into_iter().flatten() {
            if !catalog.tags().contains_key(&name) {
                catalog.create(&name)?;
            }
            catalog.assign(&name, record.file.clone(), true)?;
        }
    }
    Ok(catalog)
}
pub fn save(
    repository: &dyn CatalogRepository,
    root: &Path,
    previous: &TagCatalog,
    desired: &TagCatalog,
    revision: Option<&[u8]>,
    seed: bool,
) -> Result<Vec<u8>, String> {
    save_cancel(
        repository,
        root,
        previous,
        desired,
        revision,
        seed,
        &AtomicBool::new(false),
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn save_cancel(
    repository: &dyn CatalogRepository,
    root: &Path,
    previous: &TagCatalog,
    desired: &TagCatalog,
    revision: Option<&[u8]>,
    seed: bool,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    check(cancel)?;
    if !repository.supports_file_records() {
        return repository.save(root, desired, revision);
    }
    if repository.index_revision(root)?.as_deref() != revision {
        return Err("Tagindex wurde extern verändert; Einzeldateien bleiben erhalten".into());
    }
    if repository.supports_file_records() {
        let definitions = repository.load_definitions(root)?;
        if definitions.revision.is_some()
            && definitions.catalog != previous.definitions()
            && definitions.catalog != desired.definitions()
        {
            return Err("Tagdefinitionen wurden extern verändert".into());
        }
        for file in previous.files().union(&desired.files()) {
            check(cancel)?;
            let old = tags(previous, file);
            let new = tags(desired, file);
            if !seed && old == new {
                continue;
            }
            let current = repository.read_file_tags(root, file)?;
            if let Some(actual) = &current.tags
                && actual != &old
                && actual != &new
            {
                return Err(format!(
                    "Tags der Einzeldatei wurden extern verändert: {}",
                    file.as_str()
                ));
            }
            if current.tags.as_ref() != Some(&new) {
                repository.write_file_tags(root, file, &new, current.revision.as_deref())?;
            }
        }
        repository.save_definitions(
            root,
            &desired.definitions(),
            definitions.revision.as_deref(),
        )?;
    }
    check(cancel)?;
    repository.save(root, desired, revision)
}
pub fn rebuild(repository: &dyn CatalogRepository, root: &Path) -> Result<StoredCatalog, String> {
    rebuild_cancel(repository, root, &AtomicBool::new(false))
}
pub(super) fn rebuild_cancel(
    repository: &dyn CatalogRepository,
    root: &Path,
    cancel: &AtomicBool,
) -> Result<StoredCatalog, String> {
    check(cancel)?;
    let revision = repository.index_revision(root)?;
    let catalog = from_records(repository, root, cancel)?;
    check(cancel)?;
    let revision = repository.replace_index(root, &catalog, revision.as_deref())?;
    Ok(StoredCatalog {
        catalog,
        revision: Some(revision),
    })
}
/// One RAM catalog per import, per-file sources immediately; flush derived index on completion/abort.
pub struct ImportTags {
    repository: Arc<dyn CatalogRepository>,
    root: PathBuf,
    stored: StoredCatalog,
    names: BTreeSet<String>,
    changed: bool,
}
impl ImportTags {
    pub fn new(
        repository: Arc<dyn CatalogRepository>,
        root: PathBuf,
        names: &[String],
    ) -> Result<Self, String> {
        Self::new_with_cancel(repository, root, names, &AtomicBool::new(false))
    }
    pub fn new_with_cancel(
        repository: Arc<dyn CatalogRepository>,
        root: PathBuf,
        names: &[String],
        cancel: &AtomicBool,
    ) -> Result<Self, String> {
        let names = names
            .iter()
            .map(|n| crate::domain::tags::normalize_tag_name(n))
            .collect::<Result<_, _>>()?;
        let stored = load_with_backfill_cancel(repository.as_ref(), &root, cancel)?;
        Ok(Self {
            repository,
            root,
            stored,
            names,
            changed: false,
        })
    }
    pub fn assign(&mut self, path: &Path) -> Result<(), String> {
        let relative = path
            .strip_prefix(&self.root)
            .map_err(|_| "Importziel liegt außerhalb der Bibliothek")?
            .to_str()
            .ok_or("Ungültiger Medienpfad")?;
        let file = LibraryFile::new(relative.into())?;
        let current = self.repository.read_file_tags(&self.root, &file)?;
        let mut wanted = current
            .tags
            .unwrap_or_else(|| tags(&self.stored.catalog, &file));
        wanted.extend(self.names.iter().cloned());
        self.repository
            .write_file_tags(&self.root, &file, &wanted, current.revision.as_deref())?;
        for name in wanted {
            if !self.stored.catalog.tags().contains_key(&name) {
                self.stored.catalog.create(&name)?;
            }
            self.stored.catalog.assign(&name, file.clone(), true)?;
        }
        self.changed = true;
        Ok(())
    }
    pub fn finish(&mut self) -> Result<(), String> {
        if !self.changed {
            return Ok(());
        }
        let definitions = self.repository.load_definitions(&self.root)?;
        let mut merged = definitions.catalog;
        for name in self.stored.catalog.tags().keys() {
            if !merged.tags().contains_key(name) {
                merged.create(name)?;
            }
        }
        self.repository.save_definitions(
            &self.root,
            &merged.definitions(),
            definitions.revision.as_deref(),
        )?;
        self.repository.save(
            &self.root,
            &self.stored.catalog,
            self.stored.revision.as_deref(),
        )?;
        self.changed = false;
        Ok(())
    }
}
