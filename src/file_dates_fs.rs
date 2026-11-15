//! Read-only filesystem dates in the local OS timezone, without launching an external tool.
use crate::domain::{
    CaptureTime,
    import_date::{FileDates, FileTimestamp},
};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
fn seconds(time: SystemTime) -> Option<i64> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_secs()).ok(),
        Err(error) => {
            let duration = error.duration();
            i64::try_from(duration.as_secs())
                .ok()?
                .checked_neg()?
                .checked_sub(i64::from(duration.subsec_nanos() > 0))
        }
    }
}
#[cfg(unix)]
fn local(time: SystemTime, origin: &str) -> Option<FileTimestamp> {
    let seconds = seconds(time)?;
    let raw: libc::time_t = seconds;
    let mut result = std::mem::MaybeUninit::<libc::tm>::uninit();
    // localtime_r initializes caller-owned storage and is safe for parallel background workers.
    if unsafe { libc::localtime_r(&raw, result.as_mut_ptr()) }.is_null() {
        return None;
    }
    let result = unsafe { result.assume_init() };
    let text = format!(
        "{:04}:{:02}:{:02} {:02}:{:02}:{:02}",
        result.tm_year.checked_add(1900)?,
        result.tm_mon + 1,
        result.tm_mday,
        result.tm_hour,
        result.tm_min,
        result.tm_sec
    );
    let local = CaptureTime::parse(&text, origin).ok()?;
    Some(FileTimestamp {
        unix_seconds: seconds,
        local,
    })
}
#[cfg(not(unix))]
fn local(_: SystemTime, _: &str) -> Option<FileTimestamp> {
    None
}
pub fn read(path: &Path) -> Result<FileDates, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Dateisystem-Fallback benötigt eine reguläre Datei ohne Symlink".into());
    }
    Ok(FileDates{
        created:metadata.created().ok().and_then(|time|local(time,"FALLBACK Dateisystem-Erstellungsdatum (lokale Zeit; kein belegtes Aufnahmedatum)")),
        modified:metadata.modified().ok().and_then(|time|local(time,"FALLBACK Dateisystem-Inhaltsänderungsdatum (lokale Zeit; kein belegtes Aufnahmedatum)")),
    })
}

/// Local calendar day for disposable-cache maintenance, not a media capture date.
pub(crate) fn today() -> Result<String, String> {
    let value =
        local(SystemTime::now(), "Cachetag").ok_or("Lokales Kalenderdatum nicht verfügbar")?;
    Ok(value.local.stamp()[..8].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seconds_handle_pre_epoch_and_subseconds_without_truncating_toward_zero() {
        assert_eq!(
            seconds(UNIX_EPOCH - std::time::Duration::from_millis(500)),
            Some(-1)
        );
        assert_eq!(
            seconds(UNIX_EPOCH + std::time::Duration::from_millis(500)),
            Some(0)
        );
    }
    #[test]
    fn fallback_reader_rejects_links_and_does_not_modify_file_content_or_mtime() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("photo.jpg");
        std::fs::write(&path, b"unchanged").unwrap();
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        let dates = read(&path).unwrap();
        assert!(dates.modified.is_some());
        assert!(dates.oldest().unwrap().is_fallback());
        assert_eq!(std::fs::read(&path).unwrap(), b"unchanged");
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before
        );
        assert!(read(root.path()).is_err());
        #[cfg(unix)]
        {
            let link = root.path().join("link.jpg");
            std::os::unix::fs::symlink(path, &link).unwrap();
            assert!(read(&link).is_err());
        }
    }
}
