use crate::theme;
use eframe::egui;
pub(super) struct PlaceOptionsCache {
    revision: u64,
    count: usize,
    group: Option<String>,
    rows: Vec<(String, Vec<(i64, String)>)>,
}
impl super::Suite {
    pub(super) fn library_sidebar(&mut self, ctx: &egui::Context) {
        if !self.show_library {
            return;
        }
        let previous_group = self.group.clone();
        let previous_tags = self.tag_view.filters.clone();
        let previous_places = self.place_filter.selected.clone();
        egui::SidePanel::left("library")
            .default_width(180.).min_width(120.).max_width((ctx.content_rect().width() * 0.4).clamp(120., 260.))
            .frame(theme::panel(theme::SURFACE, 8))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 1.;
                ui.spacing_mut().interact_size.y = 20.;
                ui.spacing_mut().button_padding.y = 1.;
                ui.visuals_mut().collapsing_header_frame = true;
                let mut years = self.collection.groups().clone();
                // Tests and a manually supplied collection may have no loading session.
                if years.is_empty() {
                    for media in &self.items {
                        let group = media.group();
                        let year = group.split(std::path::MAIN_SEPARATOR).next().unwrap_or(group);
                        years.entry(year.into()).or_default().insert(group.into());
                    }
                }
                egui::ScrollArea::vertical()
                    .id_salt("library_years")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    if theme::root_navigation(ui,self.group.is_none(),"Alles").clicked() {
                        self.group=None;self.tag_view.filters.clear();self.place_filter.selected.clear();
                    }
                    ui.add_space(8.);theme::sidebar_heading(ui,"Jahre");ui.add_space(8.);
                    for (year, months) in years.into_iter().rev() {
                        let response = egui::CollapsingHeader::new(egui::RichText::new(&year).size(14.).strong())
                            .id_salt((&self.root, &year)).default_open(false)
                            .open((self.scroll_to_photo.is_some() && self.group.as_deref()==Some(year.as_str())).then_some(true))
                            .show(ui, |ui| {
                                for g in months {
                                    if g == year || crate::domain::diary::audio_year(&g).is_some() {continue;}
                                    let label=g.strip_prefix(&format!("{year}/")).unwrap_or(&g);
                                    if theme::navigation(ui,self.group.as_ref()==Some(&g),month_label(label)).clicked() {
                                        self.group=Some(g);self.tag_view.filters.clear();self.place_filter.selected.clear();
                                    }
                                }
                            });
                        if response.header_response.clicked() {
                            self.collection.prioritize(&year);
                            self.group = Some(year.clone());self.tag_view.filters.clear();self.place_filter.selected.clear();
                        }
                        ui.add_space(2.);
                    }
                    self.prepare_tag_filter();
                    let scope=(self.root.clone(),self.group.clone());
                    if self.place_scope.as_ref()!=Some(&scope) {
                        self.place_filter.selected.clear();
                        self.place_scope=Some(scope);
                    }
                    let tags = self.tag_view.options_cache.as_ref().map(|c| c.tags.clone()).unwrap_or_default();
                    ui.add_space(12.);
                    ui.horizontal(|ui| {
                        theme::sidebar_heading(ui,"Tags");
                        if ui.add_enabled(!self.tag_view.filters.is_empty(), egui::Button::new("Reset").min_size(egui::vec2(0.,theme::BADGE_HEIGHT))).clicked() { self.tag_view.filters.clear(); }
                    });
                    ui.add_space(8.);
                    for tag in tags {
                        let selected = self.tag_view.filters.contains(&tag);
                        if theme::tag_navigation(ui, selected, &tag).on_hover_text(format!("{tag}\nNur Medien mit allen ausgewählten Tags anzeigen.")).clicked()
                            && !self.tag_view.filters.remove(&tag) {
                            self.tag_view.filters.insert(tag);
                        }
                    }
                    ui.add_space(12.);
                    ui.push_id("location_facets", |ui| {
                        ui.horizontal(|ui| {
                            theme::sidebar_heading(ui,"Orte");
                            if ui.add_enabled(!self.place_filter.selected.is_empty(),egui::Button::new("Reset").min_size(egui::vec2(0.,theme::BADGE_HEIGHT))).clicked() {self.place_filter.selected.clear();}
                        });
                        ui.add_space(8.);
                        if self.place_options.as_ref().is_none_or(|cache|cache.revision!=self.place_filter.revision() || cache.count!=self.items.len() || cache.group!=self.group) {
                            let candidates=crate::application::visible(&self.items,"",self.group.as_deref(),false);
                            let rows=self.root.as_ref().map(|root|self.place_filter.catalog().options(&self.items,root,&candidates)).unwrap_or_default();
                            self.place_options=Some(PlaceOptionsCache {revision:self.place_filter.revision(),count:self.items.len(),group:self.group.clone(),rows});
                        }
                        let options=self.place_options.as_ref().map(|cache|cache.rows.clone()).unwrap_or_default();
                        if options.is_empty() {
                            ui.label(egui::RichText::new("Noch keine Orte ermittelt").small().color(theme::MUTED))
                                .on_hover_text("Die Hintergrundprüfung ergänzt Orte aus den vorbereiteten Offline-Geodaten. Unter Settings → Geodaten vorbereiten.");
                        }
                        for (country,places) in options {
                            use photosuite_application::place_filter::Selection;
                            let choice=Selection::Country(country.clone());
                            if theme::tag_navigation(ui,self.place_filter.selected.contains(&choice),&country).on_hover_text("Land des nächstgelegenen Ortes. Alle gewählten Orts- und Tagfilter müssen zutreffen.").clicked() && !self.place_filter.selected.remove(&choice) {self.place_filter.selected.insert(choice);}
                            ui.indent(("country",&country),|ui| {
                                for (id,name) in places {
                                    let choice=Selection::Place(id);
                                    ui.push_id(id,|ui| {
                                        if theme::tag_navigation(ui,self.place_filter.selected.contains(&choice),&name).on_hover_text(format!("{country} · {name}\nErneut klicken, um den Filter auszuschalten.")).clicked() && !self.place_filter.selected.remove(&choice) {self.place_filter.selected.insert(choice);}
                                    });
                                }
                            });
                        }
                        if !self.place_filter.error().is_empty() {ui.colored_label(theme::DANGER,self.place_filter.error());}
                    });
                    if self.root.is_none() {
                        ui.label(egui::RichText::new("Deine Ordnerstruktur erscheint hier, sobald du eine Sammlung öffnest.").small().color(theme::MUTED));
                    }
                });
            });
        if self.group != previous_group
            || self.tag_view.filters != previous_tags
            || self.place_filter.selected != previous_places
        {
            self.selected = None;
            self.gallery.selection.clear();
            self.highlighted_photo = None;
            self.scroll_to_photo = None;
            self.storyboard.clear();
            if self.group != previous_group {
                self.diary_catalog.cancel();
                self.audio_preparation.cancel();
            }
        }
    }
}
pub(super) fn month_label(value: &str) -> &str {
    match value {
        "scans" => "Scans",
        "01" | "1" => "Januar",
        "02" | "2" => "Februar",
        "03" | "3" => "März",
        "04" | "4" => "April",
        "05" | "5" => "Mai",
        "06" | "6" => "Juni",
        "07" | "7" => "Juli",
        "08" | "8" => "August",
        "09" | "9" => "September",
        "10" => "Oktober",
        "11" => "November",
        "12" => "Dezember",
        _ => value,
    }
}
