//! Pure in-memory location filtering and facet projection.
use crate::{
    domain::{Media, tags::LibraryFile},
    media_geo::LocatedPhoto,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Selection {
    Country(String),
    Region { country: String, code: String },
    Place(i64),
}
#[derive(Default)]
pub struct Catalog {
    pub(super) entries: BTreeMap<LibraryFile, LocatedPhoto>,
}
impl Catalog {
    pub fn new(entries: Vec<LocatedPhoto>) -> Self {
        Self {
            entries: entries.into_iter().map(|e| (e.photo.clone(), e)).collect(),
        }
    }
    fn entry(&self, media: &Media, root: &Path) -> Option<&LocatedPhoto> {
        let path = media.path().strip_prefix(root).ok()?.to_str()?;
        self.entries.get(path)
    }
    /// An empty selection preserves photos without GPS. Otherwise any selected location matches.
    pub fn filter(
        &self,
        items: &[Media],
        root: &Path,
        candidates: &[usize],
        selected: &BTreeSet<Selection>,
    ) -> Vec<usize> {
        if selected.is_empty() {
            return candidates.to_vec();
        }
        candidates
            .iter()
            .copied()
            .filter(|&i| {
                items
                    .get(i)
                    .and_then(|m| self.entry(m, root))
                    .is_some_and(|entry| {
                        selected.iter().any(|s| match s {
                            Selection::Country(c) => *c == entry.country,
                            Selection::Region { country, code } => {
                                *country == entry.country
                                    && entry.region.as_ref().is_some_and(|r| r.code == *code)
                            }
                            Selection::Place(id) => *id == entry.place_id,
                        })
                    })
            })
            .collect()
    }
    /// Preserve the library hierarchy while projecting counts for the current period.
    pub fn options_in_scope(
        &self,
        items: &[Media],
        root: &Path,
        candidates: &[usize],
    ) -> Vec<CountryOption> {
        let all: Vec<_> = (0..items.len()).collect();
        let mut rows = self.options(items, root, &all);
        let scoped = self.options(items, root, candidates);
        for country in &mut rows {
            let current = scoped.iter().find(|c| c.code == country.code);
            country.count = current.map_or(0, |c| c.count);
            for region in &mut country.regions {
                let current =
                    current.and_then(|c| c.regions.iter().find(|r| r.code == region.code));
                region.count = current.map_or(0, |r| r.count);
                for place in &mut region.places {
                    place.count = current
                        .and_then(|r| r.places.iter().find(|p| p.id == place.id))
                        .map_or(0, |p| p.count);
                }
            }
            for place in &mut country.places {
                place.count = current
                    .and_then(|c| c.places.iter().find(|p| p.id == place.id))
                    .map_or(0, |p| p.count);
            }
        }
        rows
    }
    /// Counts cover distinct media in the supplied period, before place/tag selection.
    pub fn options(
        &self,
        items: &[Media],
        root: &Path,
        candidates: &[usize],
    ) -> Vec<CountryOption> {
        let mut countries: BTreeMap<String, CountryBuilder> = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for i in candidates {
            let Some(entry) = items.get(*i).and_then(|m| self.entry(m, root)) else {
                continue;
            };
            if !seen.insert(&entry.photo) {
                continue;
            }
            let country =
                countries
                    .entry(entry.country.clone())
                    .or_insert_with(|| CountryBuilder {
                        code: entry.country.clone(),
                        name: entry.country_name.clone(),
                        count: 0,
                        regions: BTreeMap::new(),
                        places: BTreeMap::new(),
                    });
            country.count += 1;
            let places = if let Some(region) = &entry.region {
                let region = country
                    .regions
                    .entry(region.code.clone())
                    .or_insert_with(|| RegionBuilder {
                        code: region.code.clone(),
                        name: region.name.clone(),
                        count: 0,
                        places: BTreeMap::new(),
                    });
                region.count += 1;
                &mut region.places
            } else {
                &mut country.places
            };
            let place = places.entry(entry.place_id).or_insert_with(|| PlaceOption {
                id: entry.place_id,
                name: entry.place_name.clone(),
                count: 0,
            });
            place.count += 1;
        }
        let mut result: Vec<_> = countries
            .into_values()
            .map(|country| {
                let mut regions: Vec<_> = country
                    .regions
                    .into_values()
                    .map(|region| RegionOption {
                        code: region.code,
                        name: region.name,
                        count: region.count,
                        places: sorted_places(region.places),
                    })
                    .collect();
                regions.sort_by(|a, b| compare_name(&a.name, &b.name).then(a.code.cmp(&b.code)));
                CountryOption {
                    code: country.code,
                    name: country.name,
                    count: country.count,
                    regions,
                    places: sorted_places(country.places),
                }
            })
            .collect();
        result.sort_by(|a, b| compare_name(&a.name, &b.name).then(a.code.cmp(&b.code)));
        result
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceOption {
    pub id: i64,
    pub name: String,
    pub count: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionOption {
    pub code: String,
    pub name: String,
    pub count: usize,
    pub places: Vec<PlaceOption>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountryOption {
    pub code: String,
    pub name: String,
    pub count: usize,
    pub regions: Vec<RegionOption>,
    pub places: Vec<PlaceOption>,
}
struct RegionBuilder {
    code: String,
    name: String,
    count: usize,
    places: BTreeMap<i64, PlaceOption>,
}
struct CountryBuilder {
    code: String,
    name: String,
    count: usize,
    regions: BTreeMap<String, RegionBuilder>,
    places: BTreeMap<i64, PlaceOption>,
}
fn compare_name(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b))
}
fn sorted_places(places: BTreeMap<i64, PlaceOption>) -> Vec<PlaceOption> {
    let mut places: Vec<_> = places.into_values().collect();
    places.sort_by(|a, b| compare_name(&a.name, &b.name).then(a.id.cmp(&b.id)));
    places
}
