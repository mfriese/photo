//! Explicit reliability order. Never confuse release years with recording times.
use crate::CaptureTime;
use std::collections::BTreeMap;
pub fn select_audio_date(
    tags: &BTreeMap<String, String>,
    imported_stem: Option<&str>,
    file_created: Option<&str>,
) -> Result<CaptureTime, String> {
    let mut rejected = Vec::new();
    for key in [
        "RIFF:DateTimeOriginal",
        "ID3:RecordingTime",
        "XMP-exif:DateTimeOriginal",
        "QuickTime:CreationDate",
        "Keys:CreationDate",
        "RIFF:DateCreated",
        "QuickTime:CreateDate",
        "XMP-xmp:CreateDate",
    ] {
        if let Some(value) = tags.get(key) {
            let combined = if key == "RIFF:DateCreated" && value.len() == 10 {
                tags.get("RIFF:TimeCreated")
                    .map(|time| format!("{value} {time}"))
                    .unwrap_or_else(|| value.clone())
            } else {
                value.clone()
            };
            match CaptureTime::parse_audio(&combined, key) {
                Ok(date) if date.stamp() != "19040101_000000" => return Ok(date),
                _ => rejected.push(format!("{key}={value}")),
            }
        }
    }
    // Recording filenames retain their timestamp when copying changes filesystem birth dates.
    if let Some(stem) = imported_stem {
        if let Some(date) = date_from_filename(stem) {
            return Ok(date);
        }
        rejected.push(format!(
            "Dateiname={stem} (kein gültiger vollständiger Zeitstempel)"
        ));
    }
    let origin = format!(
        "FALLBACK Dateisystem-Erstellungsdatum (kein belegtes Aufnahmedatum; lokale macOS-Zeit){}",
        if rejected.is_empty() {
            String::new()
        } else {
            format!("; verworfene Angaben: {}", rejected.join(", "))
        }
    );
    CaptureTime::parse_audio(
        file_created.ok_or(
            "Weder vollständiges Audio-Aufnahmedatum noch Dateisystem-Erstellungsdatum verfügbar",
        )?,
        &origin,
    )
}
/// Anchored YYYY[-_]MM[-_]DD[-_]hh[-_]mm[-_]ss, also without separators.
/// Optional aud_ / comment_ prefix and a separated numeric collision suffix are supported.
pub fn date_from_filename(stem: &str) -> Option<CaptureTime> {
    let imported = stem
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("aud_"));
    let comment = stem
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("comment_"));
    let value = if imported {
        stem.get(4..)?
    } else if comment {
        stem.get(8..)?
    } else {
        stem
    };
    let mut position = 0;
    let mut parts = Vec::with_capacity(6);
    for length in [4, 2, 2, 2, 2, 2] {
        if position > 0 && matches!(value.as_bytes().get(position), Some(b'-' | b'_')) {
            position += 1;
        }
        let part = value.get(position..position + length)?;
        if !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        parts.push(part);
        position += length;
    }
    let suffix = value.get(position..)?;
    if !suffix.is_empty() {
        let number = suffix
            .strip_prefix('_')
            .or_else(|| suffix.strip_prefix('-'))?;
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
    }
    let date = format!(
        "{}:{}:{} {}:{}:{}",
        parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]
    );
    let origin = if comment {
        "FALLBACK Fotokommentar-Dateiname"
    } else if imported {
        "FALLBACK Diary-Importdateiname"
    } else {
        "FALLBACK Audio-Dateiname"
    };
    CaptureTime::parse(
        &date,
        &format!("{origin}: {stem} (keine vollständige eingebettete Aufnahmezeit)"),
    )
    .ok()
}
/// A comment's logical time belongs to the photograph, not to its later recording session.
pub fn comment_time(stem: &str) -> Option<CaptureTime> {
    stem.get(..8)
        .filter(|prefix| prefix.eq_ignore_ascii_case("comment_"))?;
    date_from_filename(stem)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorder_filenames_accept_hyphens_underscores_and_missing_separators() {
        for name in [
            "2023-12-27_22-51-32",
            "2023_12_27_22_51_32",
            "2023-12-27-22-51-32",
            "20231227_225132",
            "20231227225132",
            "2023_1227-2251_32",
            "2023-12-27_22-51-32_001",
            "aud_20231227_225132",
            "aud_20231227_225132_002",
            "AUD_20231227_225132",
            "comment_20231227_225132",
            "COMMENT_20231227_225132_002",
        ] {
            let date =
                select_audio_date(&BTreeMap::new(), Some(name), Some("2026-11-07T08:00:00Z"))
                    .unwrap();
            assert_eq!(date.stamp(), "20231227_225132", "{name}");
            assert!(date.origin().contains("dateiname") || date.origin().contains("Dateiname"));
            assert!(date.origin().contains(name));
        }
    }
    #[test]
    fn filename_parsing_does_not_guess_incomplete_invalid_or_embedded_dates() {
        for name in [
            "2023-02-29_22-51-32",
            "2023-12-27_24-51-32",
            "2023-12-27_22-61-32",
            "2023-12-27",
            "202312272251",
            "recording20231227225132",
            "20231227225132extra",
            "2023--12-27_22-51-32",
            "20231227225132001",
            "2023-12-27_22-51-32_",
            "aud_äöü",
            "録音20231227225132",
        ] {
            assert!(date_from_filename(name).is_none(), "{name}");
            let date =
                select_audio_date(&BTreeMap::new(), Some(name), Some("2026-11-07T08:00:00Z"))
                    .unwrap();
            assert!(date.origin().contains("Dateisystem"), "{name}");
        }
        assert!(date_from_filename("2024-02-29_22-51-32").is_some());
    }
    #[test]
    fn embedded_recording_time_keeps_priority_over_filename() {
        let tags = BTreeMap::from([(
            "ID3:RecordingTime".into(),
            "2008-09-16T03:58:11+02:00".into(),
        )]);
        let date = select_audio_date(&tags, Some("2023-12-27_22-51-32"), None).unwrap();
        assert_eq!(date.stamp(), "20080916_035811");
        assert!(date.origin().contains("ID3:RecordingTime"));
    }
    #[test]
    fn recording_metadata_beats_container_and_birth_date_without_timezone_shift() {
        let tags = BTreeMap::from([
            (
                "ID3:RecordingTime".into(),
                "2008-09-16T03:58:11+02:00".into(),
            ),
            ("QuickTime:CreateDate".into(), "2026:01:01 00:00:00".into()),
        ]);
        let date = select_audio_date(&tags, None, Some("2026-11-07T08:00:00Z")).unwrap();
        assert_eq!(date.stamp(), "20080916_035811");
        assert!(date.origin().contains("ID3:RecordingTime"));
        let tags = BTreeMap::from([
            ("RIFF:DateCreated".into(), "2024:02:29".into()),
            ("RIFF:TimeCreated".into(), "12:30:00".into()),
        ]);
        assert_eq!(
            select_audio_date(&tags, None, None).unwrap().stamp(),
            "20240229_123000"
        );
    }
    #[test]
    fn incomplete_invalid_and_zero_container_dates_are_not_invented() {
        for value in [
            "2024",
            "2024-01-01",
            "1904:01:01 00:00:00",
            "2024:02:30 12:00:00",
        ] {
            let tags = BTreeMap::from([("QuickTime:CreateDate".into(), value.into())]);
            assert!(select_audio_date(&tags, None, None).is_err());
            let date = select_audio_date(&tags, None, Some("2026-11-07T09:00:00+01:00")).unwrap();
            assert!(date.origin().contains("FALLBACK"));
            assert!(date.origin().contains(value));
        }
    }
}
