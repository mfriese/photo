use crate::Media;
use std::path::{Path, PathBuf};
/// Physical photo placement only; naming and capture-time selection stay unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PhotoImportLayout {
    #[default]
    YearMonth,
    Scans,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateSource {
    Metadata,
    FileName,
    FileSystem,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureTime {
    stamp: String,
    civil_seconds: i64,
    year: String,
    month: String,
    origin: String,
    source: DateSource,
}
impl CaptureTime {
    pub fn parse(value: &str, origin: &str) -> Result<Self, String> {
        let date = chrono::NaiveDateTime::parse_from_str(value, "%Y:%m:%d %H:%M:%S")
            .map_err(|_| format!("Ungültiges Aufnahmedatum: {value}"))?;
        use chrono::Datelike;
        if !(1..=9999).contains(&date.year()) {
            return Err("Aufnahmejahr muss vierstellig und größer als null sein".into());
        }
        Ok(Self {
            civil_seconds: date.and_utc().timestamp(),
            stamp: date.format("%Y%m%d_%H%M%S").to_string(),
            year: date.format("%Y").to_string(),
            month: date.format("%m").to_string(),
            origin: origin.into(),
            source: DateSource::Metadata,
        })
    }
    pub fn parse_audio(value: &str, origin: &str) -> Result<Self, String> {
        let value = value.trim();
        let date = chrono::DateTime::parse_from_rfc3339(value)
            .map(|d| d.naive_local())
            .or_else(|_| {
                chrono::DateTime::parse_from_str(value, "%Y:%m:%d %H:%M:%S%.f%:z")
                    .map(|d| d.naive_local())
            })
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y:%m:%d %H:%M:%S%.f"))
            .map_err(|_| format!("Kein vollständiger Audiozeitstempel: {value}"))?;
        Self::parse(
            &date.format("%Y:%m:%d %H:%M:%S").to_string(),
            &format!("{origin}: {value}"),
        )
    }
    pub fn month(&self) -> &str {
        &self.month
    }
    pub fn year(&self) -> &str {
        &self.year
    }
    pub fn stamp(&self) -> &str {
        &self.stamp
    }
    pub fn source(&self) -> DateSource {
        self.source
    }
    pub fn with_source(mut self, source: DateSource) -> Self {
        self.source = source;
        self
    }
    pub fn is_fallback(&self) -> bool {
        self.source != DateSource::Metadata
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    /// Validated, borrowed filename key for allocation-free chronological comparison.
    pub fn photo_filename_stamp(stem: &str) -> Option<&str> {
        if !stem.get(..4)?.eq_ignore_ascii_case("img_") {
            return None;
        }
        let stamp = stem.get(4..19)?;
        if stamp.as_bytes()[8] != b'_'
            || !stamp
                .bytes()
                .enumerate()
                .all(|(i, byte)| i == 8 || byte.is_ascii_digit())
        {
            return None;
        }
        let year = stamp[..4].parse::<i32>().ok()?;
        if year == 0 {
            return None;
        }
        chrono::NaiveDate::from_ymd_opt(
            year,
            stamp[4..6].parse().ok()?,
            stamp[6..8].parse().ok()?,
        )?;
        let hour = stamp[9..11].parse::<u8>().ok()?;
        let minute = stamp[11..13].parse::<u8>().ok()?;
        let second = stamp[13..15].parse::<u8>().ok()?;
        // Match chrono's existing parser, which also accepts leap seconds.
        if hour > 23 || minute > 59 || second > 60 {
            return None;
        }
        Some(stamp)
    }
    pub fn from_photo_filename(stem: &str) -> Option<Self> {
        let stamp = Self::photo_filename_stamp(stem)?;
        let date = &stamp[..8];
        let clock = &stamp[9..];
        Self::parse(
            &format!(
                "{}:{}:{} {}:{}:{}",
                &date[..4],
                &date[4..6],
                &date[6..],
                &clock[..2],
                &clock[2..4],
                &clock[4..]
            ),
            "Foto-Importdateiname",
        )
        .ok()
    }
    pub fn exif_value(&self) -> String {
        format!(
            "{}:{}:{} {}",
            self.year,
            self.month,
            &self.stamp[6..8],
            self.time_label()
        )
    }
    pub fn date_label(&self) -> String {
        format!(
            "{}.{}.{}",
            &self.stamp[6..8],
            &self.stamp[4..6],
            &self.stamp[..4]
        )
    }
    pub fn time_label(&self) -> String {
        format!(
            "{}:{}:{}",
            &self.stamp[9..11],
            &self.stamp[11..13],
            &self.stamp[13..15]
        )
    }
    /// Civil clock seconds, deliberately without implicit timezone conversion.
    pub fn from_civil_seconds(seconds: i64, origin: &str) -> Result<Self, String> {
        let date =
            chrono::DateTime::from_timestamp(seconds, 0).ok_or("Ungültiger ziviler Zeitpunkt")?;
        Self::parse(&date.format("%Y:%m:%d %H:%M:%S").to_string(), origin)
    }
    pub fn civil_seconds(&self) -> i64 {
        self.civil_seconds
    }
    pub fn shifted(&self, seconds: f64) -> Result<Self, String> {
        if !seconds.is_finite() || seconds < 0. || seconds > i64::MAX as f64 {
            return Err("Ungültige Zeitverschiebung".into());
        }
        let base = chrono::NaiveDateTime::parse_from_str(&self.stamp, "%Y%m%d_%H%M%S")
            .map_err(|e| e.to_string())?;
        let shifted = base
            .checked_add_signed(
                chrono::Duration::try_seconds(seconds.floor() as i64)
                    .ok_or("Zeitverschiebung zu groß")?,
            )
            .ok_or("Datum außerhalb des gültigen Bereichs")?;
        Self::parse(
            &shifted.format("%Y:%m:%d %H:%M:%S").to_string(),
            &self.origin,
        )
        .map(|date| date.with_source(self.source))
    }
    pub fn target(&self, root: &Path, media: &Media, suffix: usize) -> PathBuf {
        self.target_with_layout(root, media, suffix, PhotoImportLayout::YearMonth)
    }
    pub fn target_with_layout(
        &self,
        root: &Path,
        media: &Media,
        suffix: usize,
        layout: PhotoImportLayout,
    ) -> PathBuf {
        let ext = media
            .path()
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let ext = if ext == "jpeg" { "jpg".into() } else { ext };
        let suffix = if suffix == 0 {
            String::new()
        } else {
            format!("_{suffix:03}")
        };
        let (folder, prefix) = if media.is_audio() {
            ("audio/diary", "aud")
        } else if media.is_photo() && layout == PhotoImportLayout::Scans {
            ("scans", "img")
        } else {
            (self.month.as_str(), "img")
        };
        root.join(&self.year)
            .join(folder)
            .join(format!("{prefix}_{}{suffix}.{ext}", self.stamp))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_names_preserve_format_and_use_flat_year_audio_instead_of_month() {
        let time = CaptureTime::parse("2008:09:16 03:58:11", "BWF").unwrap();
        let media = Media::from_path("a.WAV".into(), String::new()).unwrap();
        assert_eq!(
            time.target(Path::new("library"), &media, 0),
            PathBuf::from("library/2008/audio/diary/aud_20080916_035811.wav")
        );
        assert_eq!(
            time.target(Path::new("library"), &media, 2),
            PathBuf::from("library/2008/audio/diary/aud_20080916_035811_002.wav")
        );
    }
    #[test]
    fn naming_preserves_format_and_validates_calendar() {
        let time = CaptureTime::parse("2008:09:16 03:58:11", "EXIF").unwrap();
        let media = Media::from_path("original.JPEG".into(), String::new()).unwrap();
        assert_eq!(
            time.target(Path::new("library"), &media, 0),
            PathBuf::from("library/2008/09/img_20080916_035811.jpg")
        );
        assert_eq!(
            time.target(Path::new("library"), &media, 2),
            PathBuf::from("library/2008/09/img_20080916_035811_002.jpg")
        );
        assert!(CaptureTime::parse("2009:02:29 03:58:11", "EXIF").is_err());
        assert!(CaptureTime::parse("0000:01:01 00:00:00", "EXIF").is_err());
        assert!(CaptureTime::parse("2008:02:29 03:58:11", "EXIF").is_ok());
    }
}

#[cfg(test)]
mod scan_layout_tests {
    use super::*;
    #[test]
    fn scans_changes_only_photo_folder_and_retains_date_name_format_and_suffix() {
        let date = CaptureTime::parse("2026:11:14 12:13:14", "EXIF").unwrap();
        let root = Path::new("library");
        let photo = Media::from_path("source.JPEG".into(), String::new()).unwrap();
        assert_eq!(
            date.target_with_layout(root, &photo, 0, PhotoImportLayout::Scans),
            Path::new("library/2026/scans/img_20261114_121314.jpg")
        );
        assert_eq!(
            date.target_with_layout(root, &photo, 2, PhotoImportLayout::Scans),
            Path::new("library/2026/scans/img_20261114_121314_002.jpg")
        );
        assert_eq!(
            date.target(root, &photo, 0),
            Path::new("library/2026/11/img_20261114_121314.jpg")
        );
        for name in ["video.mp4", "diary.wav"] {
            let media = Media::from_path(name.into(), String::new()).unwrap();
            assert_eq!(
                date.target_with_layout(root, &media, 0, PhotoImportLayout::Scans),
                date.target(root, &media, 0)
            );
        }
        let file =
            crate::tags::LibraryFile::new("2026/scans/img_20261114_121314.jpg".into()).unwrap();
        assert_eq!(
            file.metadata_path().unwrap(),
            "yaml/2026/11/img_20261114_121314.jpg.yaml"
        );
    }
}

#[cfg(test)]
mod filename_sort_tests {
    use super::*;
    #[test]
    fn borrowed_sort_keys_reject_invalid_dates_and_keep_valid_filename_variants() {
        for name in [
            "img_20230229_120000",
            "img_20241301_120000",
            "img_20240101_240000",
            "img_00000101_120000",
            "img_20240101_12é000",
        ] {
            assert!(CaptureTime::photo_filename_stamp(name).is_none(), "{name}");
        }
        for name in ["IMG_20240229_035811_002", "img_20161231_235960"] {
            let key = CaptureTime::photo_filename_stamp(name).unwrap();
            assert_eq!(CaptureTime::from_photo_filename(name).unwrap().stamp(), key);
        }
    }
}
