//! Photo comments retain their real recording time and an explicit relative photo reference.
use crate::{CaptureTime, Media, tags::LibraryFile};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedAt {
    iso: String,
    time: CaptureTime,
}
impl RecordedAt {
    pub fn parse(value: &str) -> Result<Self, String> {
        let date = chrono::DateTime::parse_from_rfc3339(value)
            .map_err(|_| "Aufnahmestart benötigt Datum, Uhrzeit und Zeitzone")?;
        let time = CaptureTime::parse(
            &date.format("%Y:%m:%d %H:%M:%S").to_string(),
            "Tatsächlicher Aufnahmestart",
        )?;
        Ok(Self {
            iso: date.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            time,
        })
    }
    pub fn from_audio_name(path: &Path) -> Result<Self, String> {
        let time = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(crate::audio_date::date_from_filename)
            .ok_or("Kommentar-Dateiname benötigt Aufnahmedatum und Uhrzeit")?;
        let iso = time.exif_value().replacen(':', "-", 2).replace(' ', "T");
        Ok(Self { iso, time })
    }
    pub fn iso(&self) -> &str {
        &self.iso
    }
    pub fn time(&self) -> &CaptureTime {
        &self.time
    }
    pub fn target(&self, root: &Path, suffix: usize) -> PathBuf {
        let suffix = if suffix == 0 {
            String::new()
        } else {
            format!("_{suffix:03}")
        };
        root.join(self.time.year())
            .join("audio")
            .join("c")
            .join(format!("com_{}{suffix}.m4a", self.time.stamp()))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    id: String,
    audio: LibraryFile,
    photo: LibraryFile,
    recorded_at: RecordedAt,
}
impl Comment {
    pub fn new(id: String, audio: LibraryFile, photo: LibraryFile) -> Result<Self, String> {
        if id.len() != 36
            || !id.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit()
                }
            })
        {
            return Err("Kommentar benötigt eine gültige UUID".into());
        }
        if !Media::from_path(PathBuf::from(photo.as_str()), String::new())
            .is_some_and(|m| m.is_photo())
        {
            return Err("Kommentarziel muss ein Foto sein".into());
        }
        if audio == photo
            || !Media::from_audio_asset(PathBuf::from(audio.as_str()), String::new()).is_some_and(
                |_| {
                    Path::new(audio.as_str()).file_stem().is_some_and(|s| {
                        let name = s.to_string_lossy().to_lowercase();
                        name.starts_with("comment_") || name.starts_with("com_")
                    })
                },
            )
        {
            return Err("Kommentar benötigt eine com_- oder ältere comment_-Audiodatei".into());
        }
        Self::metadata_path(&photo)?;
        let recorded_at = RecordedAt::from_audio_name(Path::new(audio.as_str()))?;
        Ok(Self {
            id: id.to_ascii_lowercase(),
            audio,
            photo,
            recorded_at,
        })
    }
    /// Mirror the image path under its year's audio/y folder, keeping its extension.
    pub fn metadata_path(photo: &LibraryFile) -> Result<LibraryFile, String> {
        if !Media::from_path(photo.as_str().into(), String::new()).is_some_and(|m| m.is_photo()) {
            return Err("Foto-Metadaten benötigen einen Bildverweis".into());
        }
        let path = Path::new(photo.as_str());
        let mut parts = path.components();
        let year = parts
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .filter(|year| {
                crate::diary::AudioScope::for_year(year, crate::diary::AudioKind::All).is_some()
            })
            .ok_or("Foto muss in einem gültigen Bibliotheksjahr liegen")?;
        let mut target = Path::new(year)
            .join("audio")
            .join("y")
            .join(parts.as_path());
        let name = target
            .file_name()
            .ok_or("Foto ohne Dateiname")?
            .to_string_lossy();
        target.set_file_name(format!("{name}.yaml"));
        LibraryFile::new(target.to_string_lossy().into_owned())
    }
    pub fn sidecar(&self) -> LibraryFile {
        Self::metadata_path(&self.photo).expect("validated photo reference")
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn audio(&self) -> &LibraryFile {
        &self.audio
    }
    pub fn photo(&self) -> &LibraryFile {
        &self.photo
    }
    pub fn recorded_at(&self) -> &RecordedAt {
        &self.recorded_at
    }
    pub fn photo_year(&self) -> &str {
        self.photo
            .as_str()
            .split(std::path::MAIN_SEPARATOR)
            .next()
            .filter(|year| {
                crate::diary::AudioScope::for_year(year, crate::diary::AudioKind::All).is_some()
            })
            .unwrap_or(self.recorded_at.time().year())
    }
}
#[derive(Clone, Default, Debug)]
pub struct CommentCatalog {
    entries: BTreeMap<String, Comment>,
    audio: BTreeMap<LibraryFile, String>,
    photos: BTreeMap<LibraryFile, Vec<String>>,
}
impl CommentCatalog {
    pub fn entries(&self) -> &BTreeMap<String, Comment> {
        &self.entries
    }
    /// Replace only this photo's derived index entries with its authoritative record.
    pub fn replace_photo(
        &mut self,
        photo: &LibraryFile,
        comments: impl IntoIterator<Item = Comment>,
    ) -> Result<(), String> {
        let mut next = Self::default();
        for c in self.entries.values().filter(|c| c.photo() != photo) {
            next.insert(c.clone())?;
        }
        for c in comments {
            if c.photo() != photo {
                return Err("Foto-Datensatz enthält eine fremde Zuordnung".into());
            }
            next.insert(c)?;
        }
        *self = next;
        Ok(())
    }
    pub fn insert(&mut self, comment: Comment) -> Result<(), String> {
        if let Some(existing) = self.entries.get(comment.id()) {
            return if existing == &comment {
                Ok(())
            } else {
                Err("Kommentar-ID bereits anders belegt".into())
            };
        }
        if self.audio.contains_key(comment.audio()) {
            return Err("Audiodatei bereits einem Kommentar zugeordnet".into());
        }
        self.audio.insert(comment.audio.clone(), comment.id.clone());
        self.photos
            .entry(comment.photo.clone())
            .or_default()
            .push(comment.id.clone());
        self.entries.insert(comment.id.clone(), comment);
        Ok(())
    }
    pub fn by_audio(&self, path: &LibraryFile) -> Option<&Comment> {
        self.audio.get(path).and_then(|id| self.entries.get(id))
    }
    pub fn for_photo(&self, path: &LibraryFile) -> Vec<&Comment> {
        self.photos
            .get(path)
            .into_iter()
            .flatten()
            .filter_map(|id| self.entries.get(id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn comment() -> Comment {
        Comment::new(
            "12345678-1234-1234-1234-123456789abc".into(),
            LibraryFile::new("2026/audio/comment_20261112_153000.m4a".into()).unwrap(),
            LibraryFile::new("2008/09/img_20080916_035811.jpg".into()).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn recording_date_and_physical_tree_location_survive_refinement() {
        let c = comment();
        assert_eq!(c.recorded_at().iso(), "2026-11-12T15:30:00");
        assert_eq!(c.photo_year(), "2008");
        assert_eq!(
            c.recorded_at().target(Path::new("/library"), 1),
            PathBuf::from("/library/2026/audio/c/com_20261112_153000_001.m4a")
        );
        let m = Media::from_audio_asset(PathBuf::from(c.audio().as_str()), "2026/audio".into())
            .unwrap()
            .with_comment_recording(Some(&c));
        assert_eq!(m.group(), "2026/audio");
        assert_eq!(
            crate::capture_from_filename(&m).unwrap().stamp(),
            "20261112_153000"
        );
        let mut timeline = crate::diary_timeline::DiaryTimeline::new(
            "2026".into(),
            vec![crate::diary_timeline::DiaryClip::from_media(&m)],
        )
        .unwrap();
        timeline
            .refine(
                crate::diary::Recording::new(m.path().into(), c.recorded_at().time().clone(), 4.)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            timeline.clips()[0].captured().unwrap().stamp(),
            "20261112_153000"
        );
        assert_eq!(timeline.clips()[0].duration(), Some(4.));
        assert!(
            crate::diary::AudioScope::parse("2026/audio/comments")
                .unwrap()
                .includes(&m)
        );
        assert!(
            !crate::diary::AudioScope::parse("2008/audio")
                .unwrap()
                .includes(&m)
        );
    }
    #[test]
    fn indexes_reject_conflicts_and_keep_multiple_comments_for_one_photo() {
        let mut catalog = CommentCatalog::default();
        let c = comment();
        catalog.insert(c.clone()).unwrap();
        catalog.insert(c.clone()).unwrap();
        let mut other = c.clone();
        other.id = "22345678-1234-1234-1234-123456789abc".into();
        assert!(catalog.insert(other.clone()).is_err());
        other.audio =
            LibraryFile::new("2026/audio/comment_20261112_153000_001.m4a".into()).unwrap();
        catalog.insert(other).unwrap();
        assert_eq!(catalog.for_photo(c.photo()).len(), 2);
        assert_eq!(catalog.by_audio(c.audio()), Some(&c));
    }
    #[test]
    fn invalid_times_references_and_ids_are_rejected() {
        assert!(RecordedAt::parse("2026-11-12T15:30:00").is_err());
        assert!(RecordedAt::parse("2026-02-30T15:30:00Z").is_err());
        assert!(LibraryFile::new("../outside.jpg".into()).is_err());
        let c = comment();
        assert!(Comment::new("bad".into(), c.audio.clone(), c.photo.clone()).is_err());
        assert!(
            Comment::new(c.id, c.audio, LibraryFile::new("movie.mov".into()).unwrap()).is_err()
        );
    }
    #[test]
    fn uuid_casing_does_not_create_distinct_comment_identities() {
        let c = comment();
        let upper =
            Comment::new(c.id().to_uppercase(), c.audio().clone(), c.photo().clone()).unwrap();
        assert_eq!(upper, c);
        let mut catalog = CommentCatalog::default();
        catalog.insert(c).unwrap();
        catalog.insert(upper).unwrap();
        assert_eq!(catalog.entries().len(), 1);
    }
}
