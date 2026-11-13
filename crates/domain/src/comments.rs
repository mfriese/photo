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
            .join(format!("comment_{}{suffix}.m4a", self.time.stamp()))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    id: String,
    audio: LibraryFile,
    photo: LibraryFile,
    recorded_at: RecordedAt,
    photo_time: Option<CaptureTime>,
}
impl Comment {
    pub fn new(
        id: String,
        audio: LibraryFile,
        photo: LibraryFile,
        recorded_at: RecordedAt,
        photo_time: Option<CaptureTime>,
    ) -> Result<Self, String> {
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
                    Path::new(audio.as_str())
                        .file_stem()
                        .is_some_and(|s| s.to_string_lossy().to_lowercase().starts_with("comment_"))
                },
            )
        {
            return Err("Kommentar benötigt eine comment_-Audiodatei".into());
        }
        // Persisted photo time is a snapshot: normalize provenance so reload/retry
        // compares the same reference value rather than transient metadata labels.
        let photo_time = photo_time
            .map(|time| CaptureTime::parse(&time.exif_value(), "Fotozuordnung"))
            .transpose()?;
        Ok(Self {
            id: id.to_ascii_lowercase(),
            audio,
            photo,
            recorded_at,
            photo_time,
        })
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
    pub fn photo_time(&self) -> Option<&CaptureTime> {
        self.photo_time.as_ref()
    }
    pub fn logical_year(&self) -> &str {
        self.photo_time
            .as_ref()
            .map(CaptureTime::year)
            .or_else(|| {
                self.photo
                    .as_str()
                    .split(std::path::MAIN_SEPARATOR)
                    .next()
                    .filter(|year| {
                        crate::diary::AudioScope::for_year(year, crate::diary::AudioKind::All)
                            .is_some()
                    })
            })
            .unwrap_or_else(|| self.recorded_at.time.year())
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
            RecordedAt::parse("2026-11-12T15:30:00+01:00").unwrap(),
            Some(CaptureTime::parse("2008:09:16 03:58:11", "Foto").unwrap()),
        )
        .unwrap()
    }
    #[test]
    fn recording_date_and_photo_placement_are_distinct_and_survive_refinement() {
        let c = comment();
        assert_eq!(c.recorded_at().iso(), "2026-11-12T15:30:00+01:00");
        assert_eq!(c.logical_year(), "2008");
        assert_eq!(
            c.recorded_at().target(Path::new("/library"), 1),
            PathBuf::from("/library/2026/audio/comment_20261112_153000_001.m4a")
        );
        let m = Media::from_audio_asset(PathBuf::from(c.audio().as_str()), "2026/audio".into())
            .unwrap()
            .place_comment(Some(&c));
        assert_eq!(m.group(), "2008/audio");
        assert_eq!(
            crate::capture_from_filename(&m).unwrap().stamp(),
            "20080916_035811"
        );
        let mut timeline = crate::diary_timeline::DiaryTimeline::new(
            "2008".into(),
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
            "20080916_035811"
        );
        assert_eq!(timeline.clips()[0].duration(), Some(4.));
        assert!(
            crate::diary::AudioScope::parse("2008/audio/comments")
                .unwrap()
                .includes(&m)
        );
        assert!(
            !crate::diary::AudioScope::parse("2026/audio")
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
        assert!(
            Comment::new(
                "bad".into(),
                c.audio.clone(),
                c.photo.clone(),
                c.recorded_at.clone(),
                None
            )
            .is_err()
        );
        assert!(
            Comment::new(
                c.id,
                c.audio,
                LibraryFile::new("movie.mov".into()).unwrap(),
                c.recorded_at,
                None
            )
            .is_err()
        );
    }
    #[test]
    fn uuid_casing_does_not_create_distinct_comment_identities() {
        let c = comment();
        let upper = Comment::new(
            c.id().to_uppercase(),
            c.audio().clone(),
            c.photo().clone(),
            c.recorded_at().clone(),
            c.photo_time().cloned(),
        )
        .unwrap();
        assert_eq!(upper, c);
        let mut catalog = CommentCatalog::default();
        catalog.insert(c).unwrap();
        catalog.insert(upper).unwrap();
        assert_eq!(catalog.entries().len(), 1);
    }
}
