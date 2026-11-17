//! Read-only location popup. The application session owns EXIF and nearest-place lookup.
use crate::{
    domain::{Media, tags::LibraryFile},
    theme,
};
use eframe::egui;
use photosuite_application::photo_location::{Outcome, Phase};
impl super::Suite {
    pub(super) fn open_photo_location(&mut self, media: &Media) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let file = media
            .path()
            .strip_prefix(&root)
            .ok()
            .and_then(|p| p.to_str())
            .ok_or_else(|| "Foto gehört nicht zur aktuellen Bibliothek".to_owned())
            .and_then(|path| LibraryFile::new(path.into()));
        match file {
            Ok(file) => {
                self.photo_location.open(root, file);
                self.location_open = true;
            }
            Err(error) => self.status = error,
        }
    }
    pub(super) fn photo_location_dialog(&mut self, ctx: &egui::Context) {
        if !self.location_open {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.location_open = false;
            self.photo_location.cancel();
            return;
        }
        let mut open = true;
        let mut retry = false;
        let mut settings = false;
        egui::Window::new("Ort des Fotos").id(egui::Id::new("photo_location")).open(&mut open).collapsible(false)
   .default_width(420.).max_width((ctx.content_rect().width()-32.).max(260.)).max_height((ctx.content_rect().height()-48.).max(240.)).vscroll(true).show(ctx,|ui|{
    crate::options_ui::spacing(ui);
    if let Some(photo)=self.photo_location.photo(){ui.add(egui::Label::new(photo.as_str()).truncate()).on_hover_text(photo.as_str());}
    ui.add_space(8.);
    if self.photo_location.busy(){ui.horizontal(|ui|{ui.spinner();ui.label(match self.photo_location.phase(){Phase::ReadingCoordinates=>"GPS-Koordinaten werden gelesen …",Phase::CheckingData=>"Lokale Ortsdaten werden geprüft …",Phase::Searching=>"Nächstgelegener Ort wird gesucht …"});});}
    else if let Some(outcome)=self.photo_location.outcome(){match outcome {
     Ok(Outcome::MissingCoordinates)=>{ui.label("Dieses Bild enthält keine GPS-Koordinaten.");},
     Ok(Outcome::DataUnavailable)=>{ui.label("Die Offline-Geodaten sind noch nicht bereit oder deaktiviert. Bitte in den Geodaten-Einstellungen vorbereiten.");},
     Ok(Outcome::Found {coordinates,place,distance_meters})=>{
      ui.small("Nächstgelegener Ort");theme::section_heading(ui,&place.name);
      let mut region=place.country_code.clone();if !place.admin1_code.is_empty(){region.push_str(&format!(" · Region {}",place.admin1_code));}
      ui.label(region).on_hover_text("Länder- und Regionscodes des gefundenen Ortes, keine geografische Grenzprüfung.");
      ui.label(if *distance_meters<1000.{format!("Luftlinie zum Ortsmittelpunkt: {:.0} m",distance_meters)}else{format!("Luftlinie zum Ortsmittelpunkt: {:.1} km",distance_meters/1000.)});
      ui.add_space(8.);let text=format!("{:.6}, {:.6}",coordinates.latitude(),coordinates.longitude());
      ui.label(format!("GPS: {text}"));if ui.small_button("Koordinaten kopieren").clicked(){ui.ctx().copy_text(text);}
      ui.small("Lokale GeoNames-Ortsdaten · Orientierung anhand des nächstgelegenen Ortes.");
     },
     Ok(Outcome::NoPlace {coordinates})=>{ui.label("In den lokalen Ortsdaten wurde kein Ort gefunden.");ui.small(format!("GPS: {:.6}, {:.6}",coordinates.latitude(),coordinates.longitude()));},
     Err(error)=>{ui.colored_label(theme::DANGER,error);},
    }}
    ui.add_space(10.);ui.horizontal_wrapped(|ui|{
     if ui.add_enabled(!self.photo_location.busy(),egui::Button::new("Erneut prüfen")).clicked(){retry=true;}
     if ui.button("Geodaten-Einstellungen").clicked(){settings=true;}
    });
   });
        if retry
            && let (Some(root), Some(photo)) =
                (self.root.clone(), self.photo_location.photo().cloned())
        {
            self.photo_location.open(root, photo);
        }
        if settings {
            self.options_open = true;
            self.settings_view.page = crate::options_ui::SettingsPage::Geodata;
            open = false;
        }
        if !open {
            self.location_open = false;
            self.photo_location.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_coordinates_popup_fits_small_windows_and_escape_closes_only_the_popup() {
        for width in [360., 600., 1280.] {
            let mut suite = super::super::layout_tests::test_suite();
            suite.root = Some("/Fotos".into());
            let media = Media::from_path(
                "/Fotos/2023/01/img_20230101_000000.jpg".into(),
                "2023/01".into(),
            )
            .unwrap();
            suite.open_photo_location(&media);
            let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while suite.photo_location.busy() {
                suite.photo_location.poll();
                assert!(std::time::Instant::now() < until);
                std::thread::yield_now();
            }
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let mut output = None;
            for _ in 0..3 {
                output = Some(ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 600.),
                        )),
                        ..Default::default()
                    },
                    |ctx| suite.photo_location_dialog(ctx),
                ));
            }
            let output = output.unwrap();
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text.contains("keine GPS-Koordinaten"))));
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ctx| suite.photo_location_dialog(ctx),
            );
            assert!(!suite.location_open);
        }
    }
}
