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
        let mut close = false;
        crate::theme::dialog("Ort des Fotos")
            .id(egui::Id::new("photo_location"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.)
            .max_width((ctx.content_rect().width() - 32.).max(260.))
            .vscroll(false)
            .show(ctx, |ui| {
                crate::options_ui::spacing(ui);
                if let Some(photo) = self.photo_location.photo() {
                    super::processing_ui::file(ui, "Foto", photo.as_str());
                }
                ui.add_space(8.);
                if self.photo_location.busy() {
                    theme::compact_group(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().size(16.).color(theme::ACCENT));
                            ui.strong("Ort wird ermittelt");
                        });
                        ui.label(match self.photo_location.phase() {
                            Phase::ReadingCoordinates => {
                                "GPS-Koordinaten werden aus dem Foto gelesen."
                            }
                            Phase::CheckingData => "Die lokalen Ortsdaten werden geprüft.",
                            Phase::Searching => "Der nächstgelegene Ort wird gesucht.",
                        });
                    });
                } else if let Some(outcome) = self.photo_location.outcome() {
                    location_result(ui, outcome);
                }
                ui.add_space(8.);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !self.photo_location.busy(),
                            egui::Button::new("Erneut prüfen"),
                        )
                        .clicked()
                    {
                        retry = true;
                    }
                    if ui.button("Geodaten-Einstellungen").clicked() {
                        settings = true;
                    }
                    if theme::default_dialog_button(ui, "OK") {
                        close = true;
                    }
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
        if !open || close {
            self.location_open = false;
            self.photo_location.cancel();
        }
    }
}

/// Render a read-only application result; state transitions stay in the session.
fn location_result(ui: &mut egui::Ui, result: &Result<Outcome, String>) {
    match result {
        Ok(Outcome::Found {
            coordinates,
            place,
            distance_meters,
        }) => {
            theme::compact_group(ui, |ui| {
                ui.label(
                    egui::RichText::new("Nächstgelegener Ort")
                        .small()
                        .color(theme::MUTED),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&place.name)
                            .size(theme::SECTION_HEADING_SIZE)
                            .strong()
                            .color(theme::TEXT),
                    )
                    .truncate(),
                )
                .on_hover_text(&place.name);
                ui.add_space(4.);
                let region = if place.admin1_code.is_empty() {
                    place.country_code.clone()
                } else {
                    format!("{} · {}", place.country_code, place.admin1_code)
                };
                super::processing_ui::value(ui, "Land / Region", &region);
                super::processing_ui::value(
                    ui,
                    "Luftlinie",
                    &if *distance_meters < 1000. {
                        format!("{distance_meters:.0} m")
                    } else {
                        format!("{:.1} km", distance_meters / 1000.)
                    },
                );
            });
            ui.add_space(8.);
            coordinate_group(ui, *coordinates);
            ui.add_space(4.);
            ui.label(
                egui::RichText::new("GeoNames · Orientierung, keine Grenzprüfung.")
                    .small()
                    .color(theme::MUTED),
            );
        }
        Ok(Outcome::NoPlace { coordinates }) => {
            message(
                ui,
                "Kein Ort gefunden",
                "Für diese Koordinaten wurde in den lokalen Ortsdaten kein Ort gefunden.",
            );
            ui.add_space(8.);
            coordinate_group(ui, *coordinates);
        }
        Ok(Outcome::MissingCoordinates) => message(
            ui,
            "Keine GPS-Daten",
            "Dieses Bild enthält keine GPS-Koordinaten. Ein Ort kann deshalb nicht ermittelt werden.",
        ),
        Ok(Outcome::DataUnavailable) => message(
            ui,
            "Offline-Geodaten fehlen",
            "Die Ortsdaten sind noch nicht bereit oder deaktiviert. In den Geodaten-Einstellungen lassen sie sich vorbereiten.",
        ),
        Err(error) => {
            theme::compact_group(ui, |ui| {
                ui.label(
                    egui::RichText::new("Ort konnte nicht ermittelt werden")
                        .strong()
                        .color(theme::DANGER),
                );
                ui.add_space(4.);
                super::processing_ui::paged_text(
                    ui,
                    egui::Id::new(("photo_location_error", error)),
                    error,
                    true,
                );
            });
        }
    }
}
fn message(ui: &mut egui::Ui, title: &str, text: &str) {
    theme::compact_group(ui, |ui| {
        ui.label(egui::RichText::new(title).strong().color(theme::TEXT));
        ui.label(text);
    });
}

fn coordinate_group(ui: &mut egui::Ui, coordinates: crate::domain::geo::Coordinates) {
    theme::compact_group(ui, |ui| {
        ui.strong("GPS-Koordinaten");
        let text = format!(
            "{:.6}, {:.6}",
            coordinates.latitude(),
            coordinates.longitude()
        );
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(&text).monospace().color(theme::TEXT));
            if ui
                .button("Kopieren")
                .on_hover_text("GPS-Koordinaten kopieren (Breite, Länge)")
                .clicked()
            {
                ui.ctx().copy_text(text);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_location_results_fit_compact_windows_and_coordinates_can_be_copied() {
        use crate::domain::geo::Coordinates;
        use photosuite_application::geodata::Place;
        for (width, height) in [(360., 400.), (600., 400.), (1280., 720.)] {
            let coordinates = Coordinates::new(52.52, 13.41).unwrap();
            let states = [
                Ok(Outcome::Found {
                    coordinates,
                    place: Place {
                        id: 1,
                        name: "Ein sehr langer Ortsname ".repeat(30),
                        ascii_name: "Long name".into(),
                        latitude: 52.5,
                        longitude: 13.4,
                        country_code: "DE".into(),
                        admin1_code: "16".into(),
                    },
                    distance_meters: 1234.,
                }),
                Ok(Outcome::MissingCoordinates),
                Ok(Outcome::DataUnavailable),
                Ok(Outcome::NoPlace { coordinates }),
                Err("Ungültige Metadaten 日本語\n".repeat(1000)),
            ];
            for (state, result) in states.iter().enumerate() {
                let ctx = egui::Context::default();
                theme::install(&ctx);
                ctx.style_mut(|s| s.animation_time = 0.);
                let mut copy = None;
                for frame in 0..5 {
                    let mut events = vec![];
                    if state == 0 && frame >= 3 {
                        let pos = copy.expect("copy action remains visible");
                        events = vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed: frame == 3,
                                modifiers: Default::default(),
                            },
                        ];
                    }
                    let mut bounds = None;
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, height),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ctx| {
                            let shown = crate::theme::dialog("Ort des Fotos")
                                .default_width(420.)
                                .max_width(width - 32.)
                                .resizable(false)
                                .vscroll(false)
                                .show(ctx, |ui| {
                                    crate::options_ui::spacing(ui);
                                    super::super::processing_ui::file(
                                        ui,
                                        "Foto",
                                        &format!("2023/01/{}.jpg", "Foto".repeat(100)),
                                    );
                                    ui.add_space(8.);
                                    location_result(ui, result);
                                    ui.add_space(8.);
                                    ui.horizontal_wrapped(|ui| {
                                        let _ = ui.button("Erneut prüfen");
                                        let _ = ui.button("Geodaten-Einstellungen");
                                    });
                                    ui.min_rect()
                                })
                                .unwrap();
                            bounds = Some((shown.response.rect, shown.inner.unwrap()));
                        },
                    );
                    if frame >= 2 {
                        let (outer, inner) = bounds.unwrap();
                        assert!(
                            outer.contains_rect(inner),
                            "clipped state {state}: {outer:?} {inner:?}"
                        );
                        assert!(
                            outer.width() <= width && outer.height() < height - 24.,
                            "state {state}: {outer:?}"
                        );
                    }
                    for shape in &output.shapes {
                        if let egui::Shape::Text(t) = &shape.shape
                            && t.galley.job.text == "Kopieren"
                        {
                            copy = Some(t.visual_bounding_rect().center());
                        }
                    }
                    if state == 0 && frame == 4 {
                        assert!(output.platform_output.commands.iter().any(|command|matches!(command,egui::OutputCommand::CopyText(text) if text=="52.520000, 13.410000")));
                    }
                }
            }
        }
    }
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
