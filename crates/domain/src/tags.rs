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
pub fn normalize_tag_name(value: &str) -> Result<String, String> {
    let value = value.trim().to_lowercase();
    if value.is_empty() || value.chars().count() > 100 || value.chars().any(char::is_control) {
        return Err("Tagname benötigt 1 bis 100 lesbare Zeichen".into());
    }
    Ok(value)
}
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct TagCatalog {
    tags: BTreeMap<String, BTreeSet<LibraryFile>>,
    albums: BTreeSet<String>,
}
impl TagCatalog {
    /// Derive managed tags without removing unseen files during a partial collection scan.
    pub fn reconcile_automatic(
        &mut self,
        comments: Option<&crate::comments::CommentCatalog>,
        audio: &BTreeMap<LibraryFile, crate::diary::AudioKind>,
        complete: bool,
    ) {
        use crate::diary::AudioKind;
        let mut photo_files = BTreeSet::new();
        let mut comment_files = BTreeSet::new();
        let mut diary_files = BTreeSet::new();
        if let Some(comments) = comments {
            for comment in comments.entries().values() {
                photo_files.insert(comment.photo().clone());
                comment_files.insert(comment.audio().clone());
            }
        }
        for (file, kind) in audio {
            match kind {
                AudioKind::Comments => {
                    comment_files.insert(file.clone());
                }
                AudioKind::Diary => {
                    diary_files.insert(file.clone());
                }
                AudioKind::All => {}
            }
        }
        if !complete {
            for (tag, kind, files) in [
                ("comment", AudioKind::Comments, &mut comment_files),
                ("diary", AudioKind::Diary, &mut diary_files),
            ] {
                for file in self.tags.get(tag).into_iter().flatten() {
                    let path = Path::new(file.as_str());
                    let group = path
                        .parent()
                        .unwrap_or(Path::new(""))
                        .to_string_lossy()
                        .into_owned();
                    if crate::Media::from_library_path(path.into(), group)
                        .as_ref()
                        .and_then(crate::diary::audio_kind)
                        == Some(kind)
                    {
                        files.insert(file.clone());
                    }
                }
            }
        }
        if comments.is_some() {
            self.replace_derived("commented", photo_files);
        }
        self.replace_derived("comment", comment_files);
        self.replace_derived("diary", diary_files);
    }
    fn replace_derived(&mut self, tag: &str, files: BTreeSet<LibraryFile>) {
        if files.is_empty() {
            self.tags.remove(tag);
            self.albums.remove(tag);
        } else {
            self.tags.insert(tag.into(), files);
        }
    }

    pub fn from_entries(
        tags: BTreeMap<String, Vec<String>>,
        albums: Vec<String>,
    ) -> Result<Self, String> {
        let mut catalog = Self::default();
        for (tag, files) in tags {
            let tag = normalize_tag_name(&tag)?;
            if !catalog.tags.contains_key(&tag) {
                catalog.create(&tag)?;
            }
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
    pub fn files_for(&self, tag: &str) -> Option<&BTreeSet<LibraryFile>> {
        self.tags.get(&normalize_tag_name(tag).ok()?)
    }
    pub fn partitions(&self, tag: &str) -> BTreeSet<&str> {
        self.files_for(tag)
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
    pub fn tags_for(&self, file: &LibraryFile) -> Vec<&str> {
        self.tags
            .iter()
            .filter(|(_, files)| files.contains(file))
            .map(|(name, _)| name.as_str())
            .collect()
    }
    pub fn contains(&self, tag: &str, file: &str) -> bool {
        self.files_for(tag)
            .is_some_and(|files| files.contains(file))
    }
    pub fn create(&mut self, tag: &str) -> Result<String, String> {
        let tag = normalize_tag_name(tag)?;
        if self.tags.contains_key(&tag) {
            return Err("Dieser Tag existiert bereits".into());
        }
        self.tags.insert(tag.clone(), BTreeSet::new());
        Ok(tag)
    }
    pub fn assign(&mut self, tag: &str, file: LibraryFile, present: bool) -> Result<(), String> {
        let tag = normalize_tag_name(tag)?;
        let files = self.tags.get_mut(&tag).ok_or("Tag nicht gefunden")?;
        if present {
            files.insert(file);
        } else {
            files.remove(&file);
        }
        Ok(())
    }
    pub fn set_album(&mut self, tag: &str, album: bool) -> Result<(), String> {
        let tag = normalize_tag_name(tag)?;
        if !self.tags.contains_key(&tag) {
            return Err("Album verweist auf einen unbekannten Tag".into());
        }
        if album {
            self.albums.insert(tag);
        } else {
            self.albums.remove(&tag);
        }
        Ok(())
    }
    pub fn remove(&mut self, tag: &str) -> Result<(), String> {
        let tag = normalize_tag_name(tag)?;
        self.tags.remove(&tag).ok_or("Tag nicht gefunden")?;
        self.albums.remove(&tag);
        Ok(())
    }
    pub fn rename(&mut self, old: &str, new: &str) -> Result<String, String> {
        let old = normalize_tag_name(old)?;
        let new = normalize_tag_name(new)?;
        if !self.tags.contains_key(&old) {
            return Err("Tag nicht gefunden".into());
        }
        if old == new {
            return Ok(new);
        }
        if self.tags.contains_key(&new) {
            return Err("Dieser Tag existiert bereits".into());
        }
        let files = self.tags.remove(&old).ok_or("Tag nicht gefunden")?;
        self.tags.insert(new.clone(), files);
        if self.albums.remove(&old) {
            self.albums.insert(new.clone());
        }
        Ok(new)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_media_tags_split_photos_from_audio_and_preserve_unseen_partial_years() {
        use crate::{
            comments::{Comment, CommentCatalog},
            diary::AudioKind,
        };
        let photo = LibraryFile::new("2008/09/img_20080916_035811.jpg".into()).unwrap();
        let comment_audio =
            LibraryFile::new("2026/audio/c/com_20261113_120000.m4a".into()).unwrap();
        let diary = LibraryFile::new("2026/audio/d/aud_20261113_130000.wav".into()).unwrap();
        let old = LibraryFile::new("2025/audio/d/aud_20251113_130000.wav".into()).unwrap();
        let mut comments = CommentCatalog::default();
        comments
            .insert(
                Comment::new(
                    "12345678-1234-1234-1234-123456789abc".into(),
                    comment_audio.clone(),
                    photo.clone(),
                )
                .unwrap(),
            )
            .unwrap();
        let mut tags = TagCatalog::default();
        for name in ["comment", "diary", "urlaub"] {
            tags.create(name).unwrap();
        }
        tags.assign("comment", photo.clone(), true).unwrap();
        tags.assign("diary", old.clone(), true).unwrap();
        tags.assign("urlaub", photo.clone(), true).unwrap();
        let media = BTreeMap::from([
            (comment_audio.clone(), AudioKind::Comments),
            (diary.clone(), AudioKind::Diary),
        ]);
        tags.reconcile_automatic(Some(&comments), &media, false);
        assert!(tags.contains("commented", photo.as_str()));
        assert!(!tags.contains("comment", photo.as_str()));
        assert!(tags.contains("comment", comment_audio.as_str()));
        assert!(tags.contains("diary", old.as_str()));
        assert!(tags.contains("diary", diary.as_str()));
        assert!(tags.contains("urlaub", photo.as_str()));
        tags.reconcile_automatic(None, &media, false);
        assert!(tags.contains("commented", photo.as_str()));
        tags.reconcile_automatic(Some(&CommentCatalog::default()), &media, true);
        assert!(!tags.tags().contains_key("commented"));
        assert!(!tags.contains("diary", old.as_str()));
        assert!(tags.contains("comment", comment_audio.as_str()));
        assert!(tags.contains("urlaub", photo.as_str()));
    }
    #[test]
    fn all_tag_boundaries_normalize_case_and_merge_legacy_variants_without_loss() {
        let mut catalog = TagCatalog::from_entries(
            BTreeMap::from([
                ("Comment".into(), vec!["2023/01/a.jpg".into()]),
                ("COMMENT".into(), vec!["2023/01/b.jpg".into()]),
                ("comment".into(), vec!["2023/01/a.jpg".into()]),
                ("ÄPFEL".into(), vec![]),
            ]),
            vec!["cOmMeNt".into()],
        )
        .unwrap();
        assert_eq!(catalog.tags()["comment"].len(), 2);
        assert_eq!(catalog.tags().len(), 2);
        assert!(catalog.albums().contains("comment"));
        assert!(catalog.tags().contains_key("äpfel"));
        assert!(catalog.create("COMMENT").is_err());
        let file = LibraryFile::new("2023/01/c.jpg".into()).unwrap();
        catalog.assign("CoMmEnT", file.clone(), true).unwrap();
        assert!(catalog.contains("COMMENT", file.as_str()));
        assert_eq!(catalog.tags_for(&file), vec!["comment"]);
        assert_eq!(catalog.partitions("COMMENT"), BTreeSet::from(["2023"]));
        assert_eq!(catalog.rename("COMMENT", "CoMmEnT").unwrap(), "comment");
        assert!(catalog.rename("COMMENT", "Äpfel").is_err());
        assert_eq!(catalog.rename("COMMENT", "NOTIZEN").unwrap(), "notizen");
        assert!(catalog.albums().contains("notizen"));
        catalog.set_album("NOTIZEN", false).unwrap();
        catalog.remove("NoTiZeN").unwrap();
        assert!(!catalog.contains("notizen", file.as_str()));
        assert!(catalog.albums().is_empty());
        assert!(catalog.create(&"İ".repeat(100)).is_err());
    }
    #[test]
    fn membership_is_unique_and_rename_delete_preserve_album_invariants() {
        let mut catalog = TagCatalog::default();
        catalog.create("Urlaub").unwrap();
        let file = LibraryFile::new("2023/12/img.jpg".into()).unwrap();
        catalog.assign("Urlaub", file.clone(), true).unwrap();
        catalog.assign("Urlaub", file, true).unwrap();
        assert_eq!(catalog.tags()["urlaub"].len(), 1);
        catalog.set_album("Urlaub", true).unwrap();
        catalog.rename("Urlaub", "Reisen").unwrap();
        assert!(catalog.albums().contains("reisen"));
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
