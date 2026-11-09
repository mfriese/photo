//! Virtual organization. Membership never moves or deletes an original file.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LibraryFile(String);
impl LibraryFile {
    pub fn new(value: String) -> Result<Self, String> {
        if value.is_empty()
            || value.contains('\0')
            || !Path::new(&value)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(
                "Dateizuordnungen benötigen einen relativen Bibliothekspfad ohne ..".into(),
            );
        }
        let normalized: std::path::PathBuf = Path::new(&value).components().collect();
        if normalized.to_str() != Some(value.as_str()) {
            return Err("Dateizuordnungen benötigen normalisierte relative Pfade".into());
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::borrow::Borrow<str> for LibraryFile {
    fn borrow(&self) -> &str {
        &self.0
    }
}
fn name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 100 || value.chars().any(char::is_control) {
        return Err("Tagname benötigt 1 bis 100 lesbare Zeichen".into());
    }
    Ok(value.into())
}
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct TagCatalog {
    tags: BTreeMap<String, BTreeSet<LibraryFile>>,
    albums: BTreeSet<String>,
}
impl TagCatalog {
    pub fn from_entries(
        tags: BTreeMap<String, Vec<String>>,
        albums: Vec<String>,
    ) -> Result<Self, String> {
        let mut catalog = Self::default();
        for (tag, files) in tags {
            if name(&tag)? != tag {
                return Err(
                    "Gespeicherte Tagnamen dürfen keine äußeren Leerzeichen enthalten".into(),
                );
            }
            catalog.create(&tag)?;
            for file in files {
                catalog.assign(&tag, LibraryFile::new(file)?, true)?;
            }
        }
        for album in albums {
            catalog.set_album(&album, true)?;
        }
        Ok(catalog)
    }
    pub fn tags(&self) -> &BTreeMap<String, BTreeSet<LibraryFile>> {
        &self.tags
    }
    pub fn partitions(&self, tag: &str) -> BTreeSet<&str> {
        self.tags
            .get(tag)
            .into_iter()
            .flat_map(|files| files.iter())
            .filter_map(|file| {
                file.as_str()
                    .split_once('/')
                    .map(|(partition, _)| partition)
            })
            .collect()
    }
    pub fn albums(&self) -> &BTreeSet<String> {
        &self.albums
    }
    pub fn contains(&self, tag: &str, file: &str) -> bool {
        self.tags.get(tag).is_some_and(|files| files.contains(file))
    }
    pub fn create(&mut self, tag: &str) -> Result<String, String> {
        let tag = name(tag)?;
        if self.tags.contains_key(&tag) {
            return Err("Dieser Tag existiert bereits".into());
        }
        self.tags.insert(tag.clone(), BTreeSet::new());
        Ok(tag)
    }
    pub fn assign(&mut self, tag: &str, file: LibraryFile, present: bool) -> Result<(), String> {
        let files = self.tags.get_mut(tag).ok_or("Tag nicht gefunden")?;
        if present {
            files.insert(file);
        } else {
            files.remove(&file);
        }
        Ok(())
    }
    pub fn set_album(&mut self, tag: &str, album: bool) -> Result<(), String> {
        if !self.tags.contains_key(tag) {
            return Err("Album verweist auf einen unbekannten Tag".into());
        }
        if album {
            self.albums.insert(tag.into());
        } else {
            self.albums.remove(tag);
        }
        Ok(())
    }
    pub fn remove(&mut self, tag: &str) -> Result<(), String> {
        self.tags.remove(tag).ok_or("Tag nicht gefunden")?;
        self.albums.remove(tag);
        Ok(())
    }
    pub fn rename(&mut self, old: &str, new: &str) -> Result<String, String> {
        let new = name(new)?;
        if !self.tags.contains_key(old) {
            return Err("Tag nicht gefunden".into());
        }
        if old == new {
            return Ok(new);
        }
        if self.tags.contains_key(&new) {
            return Err("Dieser Tag existiert bereits".into());
        }
        let files = self.tags.remove(old).ok_or("Tag nicht gefunden")?;
        self.tags.insert(new.clone(), files);
        if self.albums.remove(old) {
            self.albums.insert(new.clone());
        }
        Ok(new)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn membership_is_unique_and_rename_delete_preserve_album_invariants() {
        let mut catalog = TagCatalog::default();
        catalog.create("Urlaub").unwrap();
        let file = LibraryFile::new("2023/12/img.jpg".into()).unwrap();
        catalog.assign("Urlaub", file.clone(), true).unwrap();
        catalog.assign("Urlaub", file, true).unwrap();
        assert_eq!(catalog.tags()["Urlaub"].len(), 1);
        catalog.set_album("Urlaub", true).unwrap();
        catalog.rename("Urlaub", "Reisen").unwrap();
        assert!(catalog.albums().contains("Reisen"));
        assert!(catalog.contains("Reisen", "2023/12/img.jpg"));
        catalog.remove("Reisen").unwrap();
        assert!(catalog.tags().is_empty());
        assert!(catalog.albums().is_empty());
    }
    #[test]
    fn invalid_paths_and_orphan_albums_are_rejected() {
        for path in [
            "",
            "../photo.jpg",
            "/photo.jpg",
            "2023/../photo.jpg",
            "2023//photo.jpg",
            "./photo.jpg",
        ] {
            assert!(LibraryFile::new(path.into()).is_err());
        }
        assert!(TagCatalog::from_entries(BTreeMap::new(), vec!["missing".into()]).is_err());
        let mut catalog = TagCatalog::default();
        assert!(catalog.create(" ").is_err());
        catalog.create("a").unwrap();
        catalog.create("b").unwrap();
        assert!(catalog.rename("a", "b").is_err());
        assert_eq!(catalog.tags().len(), 2);
    }
}
