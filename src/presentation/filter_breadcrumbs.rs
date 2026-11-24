use eframe::egui;
use photosuite_application::place_filter::Selection;

pub(super) fn show(suite: &mut super::Suite, ui: &mut egui::Ui) {
    let group = suite.group.clone();
    let parts: Vec<_> = group
        .as_deref()
        .unwrap_or("")
        .split(std::path::MAIN_SEPARATOR)
        .filter(|part| !part.is_empty())
        .collect();
    let year = parts
        .first()
        .copied()
        .filter(|value| value.len() == 4 && value.bytes().all(|byte| byte.is_ascii_digit()));
    let month = parts.get(1).copied().filter(|value| {
        value.len() == 2
            && value
                .parse::<u8>()
                .is_ok_and(|number| (1..=12).contains(&number))
    });
    let scans = parts.get(1).copied() == Some("scans");
    let mut clear_period = false;
    let mut select_year = false;
    let mut select_month = false;
    let mut filter_changed = false;

    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(group.is_none(), "Sammlung")
            .on_hover_text("Zeitraumfilter entfernen")
            .clicked()
        {
            clear_period = true;
            filter_changed = true;
        }
        if let Some(year) = year {
            ui.label("›");
            if ui
                .selectable_label(month.is_none() && !scans, year)
                .on_hover_text("Monats- und Scanfilter entfernen")
                .clicked()
            {
                select_year = true;
                filter_changed = true;
            }
            if let Some(month) = month {
                ui.label("›");
                if ui
                    .selectable_label(true, super::library::month_label(month))
                    .clicked()
                {
                    select_month = true;
                    filter_changed = true;
                }
            } else if scans {
                ui.label("›");
                if ui
                    .selectable_label(true, "Scans")
                    .on_hover_text("Scanfilter entfernen")
                    .clicked()
                {
                    select_year = true;
                    filter_changed = true;
                }
            }
        }
        if !suite.query.is_empty() {
            ui.label("›");
            if ui.button(format!("Suche: {} ×", suite.query)).clicked() {
                suite.query.clear();
                filter_changed = true;
            }
        }
        if suite.videos_only {
            ui.label("›");
            if ui.button("Nur Videos ×").clicked() {
                suite.videos_only = false;
                filter_changed = true;
            }
        }
        for tag in suite.tag_view.filters.iter().cloned().collect::<Vec<_>>() {
            ui.label("›");
            if ui.button(format!("Tag: {tag} ×")).clicked() {
                suite.tag_view.filters.remove(&tag);
                filter_changed = true;
            }
        }
        for selection in suite
            .place_filter
            .selected
            .iter()
            .cloned()
            .collect::<Vec<_>>()
        {
            ui.label("›");
            let label = place_label(suite, &selection);
            if ui.button(format!("Ort: {label} ×")).clicked() {
                suite.place_filter.selected.remove(&selection);
                filter_changed = true;
            }
        }
    });

    if clear_period {
        suite.group = None;
    } else if select_year && let Some(year) = year {
        suite.group = Some(year.to_owned());
    } else if select_month
        && let Some(year) = year
        && let Some(month) = month
    {
        suite.group = Some(format!("{year}/{month}"));
    }
    if filter_changed {
        suite.selected = None;
        suite.gallery.selection.clear();
        suite.highlighted_photo = None;
        suite.storyboard.clear();
    }
}

fn place_label(suite: &super::Suite, selection: &Selection) -> String {
    suite
        .place_options
        .as_ref()
        .and_then(|cache| cache.label_for(selection))
        .map(str::to_owned)
        .unwrap_or_else(|| match selection {
            Selection::Country(code) => code.clone(),
            Selection::Region { code, .. } => code.clone(),
            Selection::Place(id) => format!("{id}"),
        })
}
