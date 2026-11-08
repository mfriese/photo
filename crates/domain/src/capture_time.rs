use crate::Media;
use std::path::{Path, PathBuf};
#[derive(Clone, Debug)]
pub struct CaptureTime {
    stamp: String,
    year: String,
    month: String,
    origin: String,
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
            stamp: date.format("%Y%m%d_%H%M%S").to_string(),
            year: date.format("%Y").to_string(),
            month: date.format("%m").to_string(),
            origin: origin.into(),
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
    pub fn year(&self) -> &str {
        &self.year
    }
    pub fn stamp(&self) -> &str {
        &self.stamp
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn from_photo_filename(stem: &str) -> Option<Self> {
        let base = stem.get(..19)?;
        if !base.get(..4)?.eq_ignore_ascii_case("img_") || base.as_bytes()[12] != b'_' {
            return None;
        }
        let date = base.get(4..12)?;
        let clock = base.get(13..19)?;
        if !date
            .bytes()
            .chain(clock.bytes())
            .all(|b| b.is_ascii_digit())
        {
            return None;
        }
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
    pub fn civil_seconds(&self) -> i64 {
        chrono::NaiveDateTime::parse_from_str(&self.stamp, "%Y%m%d_%H%M%S")
            .expect("validated capture time")
            .and_utc()
            .timestamp()
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
    }
    pub fn target(&self, root: &Path, media: &Media, suffix: usize) -> PathBuf {
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
            ("diary", "aud")
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
    fn audio_names_preserve_format_and_use_diary_instead_of_month() {
        let time = CaptureTime::parse("2008:09:16 03:58:11", "BWF").unwrap();
        let media = Media::from_path("a.WAV".into(), String::new()).unwrap();
        assert_eq!(
            time.target(Path::new("library"), &media, 0),
            PathBuf::from("library/2008/diary/aud_20080916_035811.wav")
        );
        assert_eq!(
            time.target(Path::new("library"), &media, 2),
            PathBuf::from("library/2008/diary/aud_20080916_035811_002.wav")
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
