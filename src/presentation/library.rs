use crate::theme;
use eframe::egui;
impl super::Suite {
    pub(super) fn library_sidebar(&mut self, ctx: &egui::Context) {
        if !self.show_library {
            return;
        }
        let previous_group = self.group.clone();
        egui::SidePanel::left("albums")
            .default_width(180.).min_width(120.).max_width((ctx.content_rect().width() * 0.4).clamp(120., 260.))
            .frame(theme::panel(theme::SURFACE, 8))
            .show(ctx, |ui| {
                ui.add_space(4.);
                ui.strong("Mediathek");
                ui.add_space(2.);
                if theme::navigation(ui, self.group.is_none(), "▦  Alle Erinnerungen").clicked() {
                    self.group = None;
                }

                ui.add_space(8.);
                theme::eyebrow(ui, "Jahre");
                ui.add_space(2.);
                let mut groups: Vec<_> = self.items.iter().map(|m| m.group().to_owned()).collect();
                groups.sort(); groups.dedup();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut years = std::collections::BTreeMap::<String, Vec<String>>::new();
                    for g in groups {
                        let year = g.split(std::path::MAIN_SEPARATOR).next().unwrap_or(&g).to_owned();
                        years.entry(year).or_default().push(g);
                    }
                    for (year, months) in years.into_iter().rev() {
                        egui::CollapsingHeader::new(egui::RichText::new(&year).size(14.).strong())
                            .id_salt((&self.root, &year)).default_open(false)
                            .show(ui, |ui| {
                                if theme::navigation(ui, self.group.as_ref() == Some(&year), "Ganzes Jahr").clicked() {
                                    self.group = Some(year.clone());
                                }
                                let diary = format!("{year}/diary");
                                if theme::navigation(ui, self.group.as_ref() == Some(&diary), "♪ Diary").clicked() {
                                    self.group = Some(diary);
                                }
                                for g in months {
                                    if g == year || crate::domain::diary::diary_year(&g).is_some() { continue; }
                                    let label = g.strip_prefix(&format!("{year}/")).unwrap_or(&g);
                                    if theme::navigation(ui, self.group.as_ref() == Some(&g), month_label(label)).clicked() {
                                        self.group = Some(g);
                                    }
                                }
                            });
                        ui.add_space(4.);
                    }
                    if self.root.is_none() {
                        ui.label(egui::RichText::new("Deine Ordnerstruktur erscheint hier, sobald du eine Sammlung öffnest.").small().color(theme::MUTED));
                    }
                });
            });
        if self.group != previous_group {
            self.selected = None;
            self.storyboard.clear();
            self.diary.cancel();
        }
    }
}
fn month_label(value: &str) -> &str {
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
