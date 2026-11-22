//! Render the application's counted location hierarchy; no query or grouping rules here.
use crate::theme;
use eframe::egui;
use photosuite_application::place_filter::{CountryOption, Selection};
use std::{collections::BTreeSet, path::Path};
pub(super) fn show(
    ui: &mut egui::Ui,
    root: Option<&Path>,
    countries: &[CountryOption],
    selected: &mut BTreeSet<Selection>,
) {
    ui.scope(|ui| {
        let mut country_ids = Vec::new();
        let mut bulk_open = None;
        for country in countries {
            let id = egui::Id::new(("photosuite_place_country", root, &country.code));
            let header = ui
                .add_enabled_ui(country.count > 0, |ui| {
                    theme::tree_header(
                        ui,
                        id,
                        &country.name,
                        selected.contains(&Selection::Country(country.code.clone())),
                        Some(country.count),
                        None,
                        |ui| {
                            for region in &country.regions {
                                let id = egui::Id::new((
                                    "photosuite_place_region",
                                    root,
                                    &country.code,
                                    &region.code,
                                ));
                                egui::collapsing_header::CollapsingState::load_with_default_open(
                                    ui.ctx(),
                                    id,
                                    false,
                                )
                                .show_header(ui, |ui| {
                                    toggle(
                                        ui,
                                        selected,
                                        Selection::Region {
                                            country: country.code.clone(),
                                            code: region.code.clone(),
                                        },
                                        &region.name,
                                        region.count,
                                    )
                                })
                                .body(|ui| {
                                    for place in &region.places {
                                        ui.indent(("place_leaf", place.id), |ui| {
                                            toggle(
                                                ui,
                                                selected,
                                                Selection::Place(place.id),
                                                &place.name,
                                                place.count,
                                            )
                                        });
                                    }
                                });
                            }
                            for place in &country.places {
                                ui.indent(("place_leaf", place.id), |ui| {
                                    toggle(
                                        ui,
                                        selected,
                                        Selection::Place(place.id),
                                        &place.name,
                                        place.count,
                                    )
                                });
                            }
                        },
                    )
                })
                .inner;
            country_ids.push(header.header_response.id);
            if header.header_response.clicked() {
                let choice = Selection::Country(country.code.clone());
                if !selected.remove(&choice) {
                    selected.insert(choice);
                }
            }
            header.header_response.context_menu(|ui| {
                for (label, open) in [
                    ("Alle Orte aufklappen", true),
                    ("Alle Orte einklappen", false),
                ] {
                    if ui.button(label).clicked() {
                        bulk_open = Some(open);
                        ui.close();
                    }
                }
            });
        }
        if let Some(open) = bulk_open {
            set_all_open(ui.ctx(), root, countries, &country_ids, open);
        }
    });
}
fn toggle(
    ui: &mut egui::Ui,
    selected: &mut BTreeSet<Selection>,
    choice: Selection,
    label: &str,
    count: usize,
) -> egui::Response {
    let response = ui
        .add_enabled_ui(count > 0, |ui| {
            theme::place_navigation(ui, selected.contains(&choice), label, count)
        })
        .inner;
    if response.clicked() && !selected.remove(&choice) {
        selected.insert(choice);
    }
    response
}
fn set_all_open(
    ctx: &egui::Context,
    root: Option<&Path>,
    countries: &[CountryOption],
    ids: &[egui::Id],
    open: bool,
) {
    for (country, id) in countries.iter().zip(ids) {
        let id = *id;
        let mut state =
            egui::collapsing_header::CollapsingState::load_with_default_open(ctx, id, false);
        state.set_open(open);
        state.store(ctx);
        for region in &country.regions {
            let id = egui::Id::new(("photosuite_place_region", root, &country.code, &region.code));
            let mut state =
                egui::collapsing_header::CollapsingState::load_with_default_open(ctx, id, false);
            state.set_open(open);
            state.store(ctx);
        }
    }
    ctx.request_repaint();
}
