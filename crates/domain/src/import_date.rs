//! Explicit import fallback; compare absolute timestamps before choosing a local civil name.
use crate::{CaptureTime, DateSource};
#[derive(Clone, Debug)]
pub struct FileTimestamp {
    pub unix_seconds: i64,
    pub local: CaptureTime,
}
#[derive(Clone, Debug, Default)]
pub struct FileDates {
    pub created: Option<FileTimestamp>,
    pub modified: Option<FileTimestamp>,
}
impl FileDates {
    pub fn oldest(self) -> Result<CaptureTime, String> {
        [self.created, self.modified]
            .into_iter()
            .flatten()
            .min_by_key(|date| date.unix_seconds)
            .map(|date| date.local.with_source(DateSource::FileSystem))
            .ok_or_else(|| {
                "Kein gültiges Dateierstellungs- oder Inhaltsänderungsdatum verfügbar".into()
            })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn date(seconds: i64, local: &str, origin: &str) -> FileTimestamp {
        FileTimestamp {
            unix_seconds: seconds,
            local: CaptureTime::parse(local, origin).unwrap(),
        }
    }
    #[test]
    fn oldest_uses_absolute_order_even_during_daylight_saving_clock_repetition() {
        let created = date(
            2,
            "2026:10:25 02:10:00",
            "FALLBACK Dateisystem-Erstellungsdatum",
        );
        let modified = date(
            1,
            "2026:10:25 02:50:00",
            "FALLBACK Dateisystem-Inhaltsänderung",
        );
        let selected = FileDates {
            created: Some(created),
            modified: Some(modified),
        }
        .oldest()
        .unwrap();
        assert_eq!(selected.time_label(), "02:50:00");
        assert!(selected.origin().contains("Inhaltsänderung"));
        assert!(selected.is_fallback());
        assert!(FileDates::default().oldest().is_err());
        assert!(
            FileDates {
                created: None,
                modified: Some(date(1, "2020:01:01 00:00:00", "modified"))
            }
            .oldest()
            .is_ok()
        );
    }
}
