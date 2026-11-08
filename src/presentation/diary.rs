use crate::theme;
use eframe::egui;
fn columns(width: f32, zoom: usize) -> usize {
    let desired = [10, 8, 6, 5, 4][zoom.min(4)];
    desired.min(((width + 8.) / 86.).floor().max(1.) as usize)
}
fn audio_preview(ui: &mut egui::Ui, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    ui.painter().rect_filled(rect, 8., theme::RAISED);
    let levels = [0.24, 0.48, 0.72, 1., 0.60, 0.82, 0.42, 0.24];
    let band = rect.width() * 0.54;
    for (i, level) in levels.iter().enumerate() {
        let width = band / 12.;
        let height = size * 0.36 * level;
        let x = rect.center().x - band / 2. + i as f32 * band / 7.;
        let bar = egui::Rect::from_center_size(
            egui::pos2(x, rect.center().y),
            egui::vec2(width.max(2.), height),
        );
        ui.painter().rect_filled(bar, 2., theme::ACCENT);
    }
    if response.hovered() {
        ui.painter().rect_stroke(
            rect.shrink(1.),
            8.,
            egui::Stroke::new(2_f32, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
impl super::Suite {
    pub(super) fn diary_wall(&mut self, ctx: &egui::Context, visible: &[usize]) {
        let year = crate::domain::diary::diary_year(self.group.as_deref().unwrap_or(""))
            .unwrap_or("")
            .to_owned();
        self.diary_catalog
            .ensure(&self.items, &year, self.services.audio_source.clone());
        if self.diary_catalog.is_loading() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let mut ordered = visible.to_vec();
        ordered.sort_by(|&a, &b| {
            let date = |i: usize| {
                self.diary_catalog
                    .date(self.items[i].path())
                    .and_then(|r| r.as_ref().ok())
                    .map(|d| d.stamp())
            };
            date(a)
                .unwrap_or("99999999_999999")
                .cmp(date(b).unwrap_or("99999999_999999"))
                .then(self.items[a].path().cmp(self.items[b].path()))
        });
        let mut chosen = None;
        egui::CentralPanel::default().frame(theme::panel(theme::CANVAS,12)).show(ctx,|ui| {
            ui.heading(format!("Diary · {year}"));
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!ordered.is_empty()&&!self.diary.is_loading(),egui::Button::new("Jahresverlauf öffnen")).clicked(){chosen=ordered.first().copied();}
                if self.diary.is_loading()&&ui.button("Laden abbrechen").clicked(){self.diary.cancel();}
                ui.label(format!("{} Aufnahmen",ordered.len()));
            });
            if !self.diary.status().is_empty(){ui.add(egui::Label::new(self.diary.status()).wrap());}
            if ordered.is_empty(){ui.label("Noch keine Aufnahmen. Audio über Import hinzufügen oder den Suchfilter leeren.");return;}
            ui.add_space(6.);
            ui.spacing_mut().item_spacing=egui::vec2(8.,4.);
            let width=(ui.available_width()-ui.spacing().scroll.allocated_width()).max(1.);
            let count=columns(width,self.size);
            let tile=((width-8.*(count-1) as f32)/count as f32).max(1.);
            let row_height=tile+44.;
            egui::ScrollArea::vertical().id_salt("diary_grid").show_rows(ui,row_height,ordered.len().div_ceil(count),|ui,rows| {
                for row in rows {
                    ui.horizontal_top(|ui| {
                        ui.set_min_height(row_height);
                        for &index in ordered.iter().skip(row*count).take(count) {
                            let media=&self.items[index];
                            ui.vertical(|ui| {
                                ui.set_width(tile);
                                let date=self.diary_catalog.date(media.path());
                                let (day,clock,detail)=match date {
                                    Some(Ok(date))=>(date.date_label(),date.time_label(),format!("{}\n{}",media.path().display(),date.origin())),
                                    Some(Err(error))=>("Datum fehlt".into(),"Nicht verfügbar".into(),format!("{}\n{error}",media.path().display())),
                                    None=>("Datum wird geladen".into(),String::new(),media.path().display().to_string()),
                                };
                                if audio_preview(ui,tile).on_hover_text(&detail).clicked(){chosen=Some(index);}
                                ui.add_sized([tile,18.],egui::Label::new(egui::RichText::new(day).size(12.).color(theme::TEXT)).truncate()).on_hover_text(&detail);
                                ui.add_sized([tile,18.],egui::Label::new(egui::RichText::new(clock).size(12.).color(theme::MUTED)).truncate()).on_hover_text(detail);
                            });
                        }
                    });
                }
            });
        });
        if chosen.is_some() {
            ctx.request_repaint();
        }
        if let Some(index) = chosen
            && let Err(error) = self.diary.start(
                &self.items,
                &self.items[index],
                self.services.audio_source.clone(),
            )
        {
            self.status = error;
        }
    }
}
impl super::Suite {
    pub(super) fn poll_audio_player(&mut self, ctx: &egui::Context) {
        let audio_update = self.services.audio_player.poll();
        if audio_update.is_open {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(path) = audio_update.photo_to_open {
            if let Some(index) = self
                .items
                .iter()
                .position(|media| media.is_photo() && media.path() == path)
            {
                self.selected = Some(index);
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            } else {
                self.status = "Foto gehört nicht mehr zur geöffneten Bibliothek".into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn five_zoom_steps_range_from_ten_to_four_columns_and_adapt_to_small_windows() {
        assert_eq!(
            (0..5).map(|zoom| columns(1080., zoom)).collect::<Vec<_>>(),
            vec![10, 8, 6, 5, 4]
        );
        for width in [120., 200., 360., 600.] {
            for zoom in 0..5 {
                let n = columns(width, zoom);
                assert!((1..=10).contains(&n));
                assert!(n as f32 * 78. + (n - 1) as f32 * 8. <= width);
            }
        }
    }
}
