use eframe::egui;
impl super::Suite {
    pub(super) fn library_sidebar(&mut self, ctx: &egui::Context) {
        if self.show_library {
            egui::SidePanel::left("albums")
                .default_width(190.)
                .show(ctx, |ui| {
                    ui.add_space(12.);
                    ui.heading("Mediathek");
                    if ui
                        .selectable_label(self.group.is_none(), "Alle Fotos & Videos")
                        .clicked()
                    {
                        self.group = None;
                    }
                    ui.separator();
                    ui.label("Ordner · Jahr / Monat");
                    let mut groups: Vec<_> =
                        self.items.iter().map(|m| m.group().to_owned()).collect();
                    groups.sort();
                    groups.dedup();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut years = std::collections::BTreeMap::<String, Vec<String>>::new();
                        for g in groups {
                            let year = g
                                .split(std::path::MAIN_SEPARATOR)
                                .next()
                                .unwrap_or(&g)
                                .to_owned();
                            years.entry(year).or_default().push(g);
                        }
                        for (year, months) in years {
                            egui::CollapsingHeader::new(&year)
                                .id_salt((&self.root, &year))
                                .show(ui, |ui| {
                                    if ui
                                        .selectable_label(
                                            self.group.as_ref() == Some(&year),
                                            "Ganzes Jahr",
                                        )
                                        .clicked()
                                    {
                                        self.group = Some(year.clone());
                                    }
                                    for g in months {
                                        if g == year {
                                            continue;
                                        }
                                        let label =
                                            g.strip_prefix(&format!("{year}/")).unwrap_or(&g);
                                        if ui
                                            .selectable_label(
                                                self.group.as_ref() == Some(&g),
                                                label,
                                            )
                                            .clicked()
                                        {
                                            self.group = Some(g);
                                        }
                                    }
                                });
                        }
                    });
                });
        }
    }
}
