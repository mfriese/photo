//! Recording UI only: session owns capture transitions, publishing and reference persistence.
use super::Suite;
use eframe::egui;
use photosuite_application::comments::{
    CaptureCommand as Command, CapturePhase as Phase, PhotoTarget,
};
#[derive(Default)]
pub(super) struct CommentView {
    confirm_close: bool,
    confirm_quit: bool,
    allow_quit: bool,
}
impl Suite {
    pub(super) fn poll_comments(&mut self, ctx: &egui::Context) {
        self.comments.poll();
        if !self.comments.is_open() && !self.comments.error().is_empty() {
            self.status = format!("Audiokommentare: {}", self.comments.error());
        }

        if self.comments.take_changed() {
            for media in &mut self.items {
                *media = self.comments.project(media.clone());
            }
            for media in self.comments.comment_media() {
                if self.known_media.insert(media.path().to_owned()) {
                    self.items.push(media);
                }
            }
            self.diary_catalog.cancel();
            self.audio_header.draft = Default::default();
            self.media_dates.clear();
        }
        if self.comments.busy() || self.comments.is_open() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.comment_view.allow_quit
            && (self.comments.has_unsaved() || self.comments.is_saving())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.comment_view.confirm_quit = true;
            self.comments.show();
        }
    }
    pub(super) fn comment_dialog(&mut self, ctx: &egui::Context) {
        if let Some(index) = self.record_photo.take() {
            let result = (|| {
                let media = self
                    .items
                    .get(index)
                    .filter(|m| m.is_photo())
                    .ok_or("Foto nicht mehr verfügbar")?;
                let root = self.root.clone().ok_or("Keine Bibliothek geöffnet")?;
                let file = self.organization.file(media.path())?;
                let time = self
                    .media_dates
                    .get(media)
                    .and_then(|date| date.as_ref().ok())
                    .cloned();
                self.comments.open_photo(PhotoTarget { root, file, time })
            })();
            if let Err(error) = result {
                self.status = error;
            }
        }
        if !self.comments.is_open() {
            return;
        }
        let mut close = false;
        let mut listen = None;
        let width = (ctx.content_rect().width() - 40.).clamp(260., 540.);
        egui::Modal::new(egui::Id::new("photo_comment_recording")).show(ctx, |ui| {
            ui.set_width(width);
            crate::options_ui::spacing(ui);
            ui.horizontal(|ui| {
                ui.heading("Audiokommentar");
                close = ui
                    .add_enabled(!self.comments.is_saving(), egui::Button::new("Schließen"))
                    .clicked();
            });
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 110.).max(180.))
                .show(ui, |ui| {
                    if let Some(target) = self.comments.target() {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Image::from_uri(super::preview_uri(
                                    &target.root.join(target.file.as_str()),
                                    self.editor.rotations(),
                                    96.,
                                ))
                                .max_size(egui::vec2(96., 96.))
                                .corner_radius(4.),
                            );
                            ui.vertical(|ui| {
                                ui.label("Kommentar zu diesem Foto");
                                if let Some(date) = &target.time {
                                    ui.label(date.date_label());
                                    ui.label(date.time_label());
                                } else {
                                    ui.label("Fotodatum noch unbekannt");
                                }
                            });
                        });
                    }
                    ui.separator();
                    capture_controls(ui, &mut self.comments, self.services.audio_player.as_ref());
                    if let Some(target) = self.comments.target() {
                        let comments = self.comments.for_photo(&target.file);
                        if !comments.is_empty() {
                            ui.separator();
                            ui.strong("Gespeicherte Kommentare");
                            for comment in comments {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(format!(
                                        "{} {}",
                                        comment.recorded_at().time().date_label(),
                                        comment.recorded_at().time().time_label()
                                    ));
                                    if ui
                                        .add_enabled(
                                            !self.comments.has_unsaved()
                                                && !self.comments.is_saving(),
                                            egui::Button::new("Anhören"),
                                        )
                                        .clicked()
                                    {
                                        listen = Some(target.root.join(comment.audio().as_str()));
                                    }
                                });
                            }
                        }
                    }
                });
        });
        if let Some(path) = listen
            && let Some(media) = self.items.iter().find(|m| m.path() == path)
            && let Err(error) =
                self.diary
                    .start(&self.items, media, self.services.audio_source.clone())
        {
            self.status = error;
        }
        if close {
            if self.comments.has_unsaved() {
                self.comment_view.confirm_close = true;
            } else {
                self.comments.discard();
            }
        }
        if self.comment_view.confirm_close || self.comment_view.confirm_quit {
            egui::Modal::new(egui::Id::new("discard_photo_comment")).show(ctx, |ui| {
                ui.set_width(width);
                ui.heading("Aufnahme noch nicht gespeichert");
                ui.label("Aufnahme behalten und zurückkehren oder die ungespeicherte Aufnahme verwerfen?");
                if self.comments.is_saving() {
                    ui.label("Speichern läuft. Bitte Abschluss abwarten.");
                }
                if ui.button("Zurück zur Aufnahme").clicked() {
                    self.comment_view.confirm_close = false;
                    self.comment_view.confirm_quit = false;
                }
                if ui.add_enabled(!self.comments.is_saving(), egui::Button::new("Aufnahme verwerfen")).clicked() {
                    self.comments.discard();
                    self.comment_view.confirm_close = false;
                    if self.comment_view.confirm_quit {
                        self.comment_view.allow_quit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    self.comment_view.confirm_quit = false;
                }
            });
        }
    }
}
fn capture_controls(
    ui: &mut egui::Ui,
    session: &mut photosuite_application::comments::CommentSession,
    player: &dyn photosuite_application::diary::AudioPlayer,
) {
    let phase = session.snapshot().phase;
    let idle = matches!(phase, Phase::Idle | Phase::Ready | Phase::Error) && !session.is_saving();
    let mut selected_device = session.device().to_owned();
    ui.add_enabled_ui(idle, |ui| {
        ui.label("Mikrofon");
        let selected = session
            .devices()
            .iter()
            .find(|d| d.id == session.device())
            .map(|d| d.name.as_str())
            .unwrap_or("Systemstandard");
        egui::ComboBox::from_id_salt("comment_microphone")
            .width((ui.available_width() - 24.).max(140.))
            .truncate()
            .selected_text(selected)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected_device, String::new(), "Systemstandard");
                for device in session.devices() {
                    ui.selectable_value(&mut selected_device, device.id.clone(), &device.name);
                }
            });
        if ui.small_button("Geräte aktualisieren").clicked() {
            session.refresh_devices();
        }
    });
    if selected_device != session.device()
        && let Err(error) = session.set_device(selected_device)
    {
        session.report_error(error);
    }
    ui.add_space(6.);
    let status = match phase {
        Phase::Idle => "Bereit zur Aufnahme",
        Phase::Preparing => "Mikrofon wird vorbereitet …",
        Phase::Recording => "Aufnahme läuft",
        Phase::Paused => "Aufnahme pausiert",
        Phase::Finishing => "Aufnahme wird abgeschlossen …",
        Phase::Ready => "Aufnahme bereit zum Speichern",
        Phase::Error => "Aufnahme nicht möglich",
    };
    ui.horizontal_wrapped(|ui| {
        ui.strong(status);
        let seconds = session.snapshot().seconds as u64;
        ui.label(format!("{:02}:{:02}", seconds / 60, seconds % 60));
    });
    waveform(ui, &session.snapshot().wave, session.snapshot().level);
    ui.add(egui::ProgressBar::new(session.snapshot().level).desired_height(8.))
        .on_hover_text("Mikrofonpegel; bei dauerhaft vollem Ausschlag Abstand vergrößern");
    ui.horizontal_wrapped(|ui| {
        let label = if phase == Phase::Ready {
            "Neu aufnehmen"
        } else {
            "Aufnehmen"
        };
        if ui
            .add_enabled(session.can_start(), egui::Button::new(label))
            .clicked()
            && let Err(error) = session.start(player)
        {
            session.report_error(error);
        }
        if phase == Phase::Recording && ui.button("⏸ Pause").clicked() {
            session.command(Command::Pause);
        }
        if phase == Phase::Paused && ui.button("▶ Fortsetzen").clicked() {
            session.command(Command::Resume);
        }
        if matches!(phase, Phase::Recording | Phase::Paused)
            && ui.button("Aufnahme beenden").clicked()
        {
            session.command(Command::Finish);
        }
        if phase == Phase::Ready {
            let label = if session.snapshot().playing {
                "⏸ Probehören"
            } else {
                "▶ Probehören"
            };
            if ui
                .add_enabled(!session.is_saving(), egui::Button::new(label))
                .clicked()
            {
                session.command(Command::Preview);
            }
            if ui
                .add_enabled(!session.is_saving(), egui::Button::new("Speichern"))
                .clicked()
                && let Err(error) = session.save()
            {
                session.report_error(error);
            }
        }
    });
    if matches!(phase, Phase::Preparing | Phase::Finishing) || session.busy() {
        ui.spinner();
    }
    if !session.snapshot().message.is_empty() {
        ui.label(&session.snapshot().message);
    }
    if !session.error().is_empty() {
        ui.strong(session.error());
    }
    if !session.is_loaded()
        && !session.is_loading()
        && ui.button("Zuordnungen erneut laden").clicked()
    {
        session.reload();
    }
    if !session.message().is_empty() {
        ui.label(session.message());
    }
    if !session.has_unsaved() {
        ui.label("Mikrofonzugriff erst nach „Aufnehmen“. Speicherung als M4A/AAC; das Foto bleibt unverändert.");
    }
}

fn waveform(ui: &mut egui::Ui, values: &[f32], level: f32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 80.), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4., crate::theme::CANVAS);
    let count = 120;
    let step = rect.width() / count as f32;
    for i in 0..count {
        let value = values.get(i).copied().unwrap_or(0.);
        let bar = egui::Rect::from_center_size(
            egui::pos2(rect.left() + (i as f32 + 0.5) * step, rect.center().y),
            egui::vec2((step - 1.).max(1.), (value * 64.).max(1.)),
        );
        ui.painter().rect_filled(
            bar,
            1.,
            if level > 0.98 {
                egui::Color32::from_rgb(255, 150, 130)
            } else {
                crate::theme::ACCENT
            },
        );
    }
}
