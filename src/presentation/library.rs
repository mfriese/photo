use crate::theme;
use eframe::egui;
impl super::Suite {
    pub(super) fn library_sidebar(&mut self, ctx: &egui::Context) {
        if !self.show_library {
            return;
        }
        let previous_group = self.group.clone();
        let previous_tag = self.tag_view.active.clone();
        egui::SidePanel::left("albums")
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
                    if theme::navigation(ui,self.group.is_none() && self.tag_view.active.is_none(),"Alles").clicked() {
                        self.group=None;self.tag_view.active=None;
                    }
                    theme::eyebrow(ui,"Jahre");ui.add_space(2.);
                    for (year, months) in years.into_iter().rev() {
                        let response = egui::CollapsingHeader::new(egui::RichText::new(&year).size(14.).strong())
                            .id_salt((&self.root, &year)).default_open(false)
                            .open((self.scroll_to_photo.is_some() && self.group.as_deref()==Some(year.as_str())).then_some(true))
                            .show(ui, |ui| {
                                for g in months {
                                    if g == year || crate::domain::diary::audio_year(&g).is_some() {continue;}
                                    let label=g.strip_prefix(&format!("{year}/")).unwrap_or(&g);
                                    if theme::navigation(ui,self.group.as_ref()==Some(&g),month_label(label)).clicked() {
                                        self.group=Some(g);self.tag_view.active=None;
                                    }
                                }
                            });
                        if response.header_response.clicked()
                            && egui::collapsing_header::CollapsingState::load(ctx, response.header_response.id)
                                .is_some_and(|state| state.is_open()) {
                            self.collection.prioritize(&year);
                            self.group = Some(year.clone());self.tag_view.active=None;
                        }
                        ui.add_space(2.);
                    }
                    let tags:Vec<_>=self.organization.catalog().map(|catalog|catalog.tags().keys().cloned().collect()).unwrap_or_default();
                    if !tags.is_empty() {
                        ui.add_space(8.);theme::eyebrow(ui,"Tags");ui.add_space(4.);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing=egui::vec2(4.,4.);
                            for tag in tags {
                                let selected=self.tag_view.active.as_ref()==Some(&tag);
                                let response=ui.add(egui::Button::new(&tag).selected(selected).wrap_mode(egui::TextWrapMode::Truncate)).on_hover_text(&tag);
                                if response.clicked(){self.tag_view.active=(!selected).then_some(tag);self.group=None;}
                            }
                        });
                    }
                    if self.root.is_none() {
                        ui.label(egui::RichText::new("Deine Ordnerstruktur erscheint hier, sobald du eine Sammlung öffnest.").small().color(theme::MUTED));
                    }
                });
            });
        if self.group != previous_group || self.tag_view.active != previous_tag {
            self.selected = None;
            self.highlighted_photo = None;
            self.scroll_to_photo = None;
            self.storyboard.clear();
            self.diary_catalog.cancel();
            self.audio_preparation.cancel();
        }
    }
}
pub(super) fn month_label(value: &str) -> &str {
    match value {
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
