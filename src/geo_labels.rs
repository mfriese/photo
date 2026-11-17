//! Bundled display names only; no network access or per-media persistence.
use std::{collections::BTreeMap, sync::OnceLock};
pub struct Labels {
    countries: BTreeMap<String, String>,
    regions: BTreeMap<String, String>,
}
pub fn labels() -> Result<&'static Labels, String> {
    static LABELS: OnceLock<Result<Labels, String>> = OnceLock::new();
    LABELS
        .get_or_init(|| {
            Ok(Labels {
                countries: serde_json::from_str(include_str!(
                    "../assets/geo-labels/countries.json"
                ))
                .map_err(|e| format!("Ungültige Ländernamen: {e}"))?,
                regions: serde_json::from_str(include_str!("../assets/geo-labels/regions.json"))
                    .map_err(|e| format!("Ungültige Regionsnamen: {e}"))?,
            })
        })
        .as_ref()
        .map_err(Clone::clone)
}
impl Labels {
    pub fn country(&self, code: &str) -> String {
        self.countries
            .get(code)
            .cloned()
            .unwrap_or_else(|| code.into())
    }
    pub fn region(
        &self,
        country: &str,
        code: &str,
    ) -> Option<photosuite_application::media_geo::AdministrativeRegion> {
        if code.is_empty() || code == "00" {
            return None;
        }
        Some(photosuite_application::media_geo::AdministrativeRegion {
            code: code.into(),
            name: self
                .regions
                .get(&format!("{country}.{code}"))
                .cloned()
                .unwrap_or_else(|| format!("Region {code}")),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_names_cover_countries_regions_and_unknown_codes_without_guessing() {
        let names = labels().unwrap();
        assert_eq!(names.country("DE"), "Deutschland");
        assert_eq!(names.country("US"), "Vereinigte Staaten");
        assert_eq!(names.country("ZZ-unknown"), "ZZ-unknown");
        assert_eq!(names.region("US", "CA").unwrap().name, "California");
        assert_eq!(names.region("DE", "16").unwrap().name, "State of Berlin");
        assert_eq!(
            names.region("DE", "unknown").unwrap().name,
            "Region unknown"
        );
        assert!(names.region("DE", "00").is_none());
        assert!(names.region("DE", "").is_none());
    }
}
