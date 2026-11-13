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
    awaiting_save: bool,
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
        let mut save = false;
        let mut listen = None;
        let width = (ctx.content_rect().width() - 40.).clamp(260., 760.);
        egui::Modal::new(egui::Id::new("photo_comment_recording")).show(ctx, |ui| {
            ui.set_width(width);
            crate::options_ui::spacing(ui);
            ui.horizontal(|ui| {
                ui.heading("Audiokommentar");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let response = ui.add_enabled(
                        !self.comments.is_saving(),
                        egui::Button::new("")
                            .frame(false)
                            .min_size(egui::vec2(24., 24.)),
                    );
                    let center = response.rect.center();
                    for sign in [-1., 1.] {
                        ui.painter().line_segment(
                            [
                                center + egui::vec2(-4., -4. * sign),
                                center + egui::vec2(4., 4. * sign),
                            ],
                            egui::Stroke::new(1.5_f32, crate::theme::TEXT),
                        );
                    }
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            response.enabled(),
                            "Dialog schließen",
                        )
                    });
                    close = response.on_hover_text("Schließen").clicked();
                });
            });
            capture_controls(ui, &mut self.comments, self.services.audio_player.as_ref());
            egui::ScrollArea::vertical()
                .max_height(
                    (ctx.content_rect().height() - 240. - if width < 560. { 132. } else { 62. })
                        .max(20.),
                )
                .show(ui, |ui| {
                    capture_details(ui, &mut self.comments);
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
            if let Some(target) = self.comments.target() {
                let image = egui::Image::from_uri(super::preview_uri(
                    &target.root.join(target.file.as_str()),
                    self.editor.rotations(),
                    128.,
                ));
                let busy = self.comments.is_saving() || self.comments.is_save_pending();
                match photo_footer(
                    ui,
                    image,
                    target.time.as_ref(),
                    !busy,
                    self.comments.has_unsaved() && !busy,
                ) {
                    Some(FooterAction::Save) => save = true,
                    Some(FooterAction::Cancel) => close = true,
                    None => {}
                }
            }
        });
        if save {
            match self.comments.request_save() {
                Ok(()) => self.comment_view.awaiting_save = true,
                Err(error) => self.comments.report_error(error),
            }
        }
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
        if self.comment_view.awaiting_save
            && !self.comments.is_saving()
            && !self.comments.is_save_pending()
        {
            self.comment_view.awaiting_save = false;
            if !self.comments.has_unsaved() {
                self.finish_comment_close(ctx);
            }
        }
        if self.comment_view.confirm_close || self.comment_view.confirm_quit {
            egui::Modal::new(egui::Id::new("discard_photo_comment")).show(ctx, |ui| {
                let busy = self.comments.is_saving() || self.comments.is_save_pending();
                let action = unsaved_confirmation(ui, width, busy, self.comments.error());
                match action {
                    Some(ConfirmationAction::Save) => match self.comments.request_save() {
                        Ok(()) => self.comment_view.awaiting_save = true,
                        Err(error) => self.comments.report_error(error),
                    },
                    Some(ConfirmationAction::Discard) => {
                        self.finish_comment_close(ctx);
                    }
                    None => {}
                }
            });
        }
    }
}
impl Suite {
    fn finish_comment_close(&mut self, ctx: &egui::Context) {
        self.comments.discard();
        self.comment_view.confirm_close = false;
        self.comment_view.awaiting_save = false;
        if self.comment_view.confirm_quit {
            self.comment_view.allow_quit = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.comment_view.confirm_quit = false;
    }
}
#[derive(Clone, Copy)]
enum ConfirmationAction {
    Save,
    Discard,
}
fn unsaved_confirmation(
    ui: &mut egui::Ui,
    width: f32,
    busy: bool,
    error: &str,
) -> Option<ConfirmationAction> {
    ui.set_width(width.min(440.));
    crate::options_ui::spacing(ui);
    ui.heading("Aufnahme noch nicht gespeichert");
    ui.add_space(12.);
    ui.label(if busy {
        "Aufnahme wird abgeschlossen und gespeichert …"
    } else {
        "Möchtest du die Aufnahme speichern oder verwerfen?"
    });
    if !error.is_empty() {
        ui.add_space(6.);
        ui.strong(error);
    }
    ui.add_space(18.);
    let mut action = None;
    ui.horizontal(|ui| {
        let button_width = 100.;
        let gap = 12.;
        ui.add_space(((ui.available_width() - button_width * 2. - gap) / 2.).max(0.));
        ui.spacing_mut().item_spacing.x = gap;
        if ui
            .add_enabled(
                !busy,
                egui::Button::new("Speichern").min_size(egui::vec2(button_width, 32.)),
            )
            .clicked()
        {
            action = Some(ConfirmationAction::Save);
        }
        if ui
            .add_enabled(
                !busy,
                egui::Button::new("Verwerfen").min_size(egui::vec2(button_width, 32.)),
            )
            .clicked()
        {
            action = Some(ConfirmationAction::Discard);
        }
    });
    ui.add_space(4.);
    action
}
#[derive(Clone, Copy)]
enum FooterAction {
    Save,
    Cancel,
}
fn photo_footer(
    ui: &mut egui::Ui,
    image: egui::Image<'_>,
    time: Option<&crate::domain::CaptureTime>,
    can_cancel: bool,
    can_save: bool,
) -> Option<FooterAction> {
    let height =
        ui.text_style_height(&egui::TextStyle::Body) * 3. + ui.spacing().item_spacing.y * 2.;
    let narrow = ui.available_width() < 560.;
    let reserve = if narrow {
        140.
    } else {
        140. + height * 2. + ui.spacing().item_spacing.x * 3.
    };
    let source = image
        .load_for_size(ui.ctx(), egui::vec2(256., 128.))
        .ok()
        .and_then(|loaded| loaded.size())
        .unwrap_or(egui::vec2(1., 1.));
    let size = photo_thumbnail_size(source, (ui.available_width() - reserve).max(24.), height);
    let mut action = None;
    ui.horizontal(|ui| {
        ui.add(image.fit_to_exact_size(size).corner_radius(4.));
        ui.vertical(|ui| {
            ui.label("Kommentar zu diesem Foto");
            if let Some(date) = time {
                ui.label(date.date_label());
                ui.label(date.time_label());
            } else {
                ui.label("Fotodatum noch unbekannt");
            }
        });
        if !narrow {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                action = footer_actions(ui, size.y, can_cancel, can_save);
            });
        }
    });
    if narrow {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            action = footer_actions(ui, size.y, can_cancel, can_save);
        });
    }
    action
}
fn footer_actions(
    ui: &mut egui::Ui,
    edge: f32,
    can_cancel: bool,
    can_save: bool,
) -> Option<FooterAction> {
    let mut action = None;
    // Right-to-left layout: save is the far-right action, cancel immediately beside it.
    if footer_button(ui, edge, true, can_save).clicked() {
        action = Some(FooterAction::Save);
    }
    if footer_button(ui, edge, false, can_cancel).clicked() {
        action = Some(FooterAction::Cancel);
    }
    action
}
fn footer_button(ui: &mut egui::Ui, edge: f32, save: bool, enabled: bool) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("").min_size(egui::vec2(edge, edge)),
    );
    let c = response.rect.center();
    let r = (edge * 0.14).clamp(4., 9.);
    let color = if enabled {
        crate::theme::MUTED
    } else {
        crate::theme::MUTED.gamma_multiply(0.5)
    };
    let stroke = egui::Stroke::new(1.5_f32, color);
    if save {
        ui.painter().line_segment(
            [c + egui::vec2(-r, 0.), c + egui::vec2(-r * 0.2, r * 0.7)],
            stroke,
        );
        ui.painter().line_segment(
            [
                c + egui::vec2(-r * 0.2, r * 0.7),
                c + egui::vec2(r, -r * 0.7),
            ],
            stroke,
        );
    } else {
        for sign in [-1., 1.] {
            ui.painter().line_segment(
                [c + egui::vec2(-r, -r * sign), c + egui::vec2(r, r * sign)],
                stroke,
            );
        }
    }
    let label = if save {
        "Aufnahme speichern"
    } else {
        "Aufnahme abbrechen"
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response.on_hover_text(label)
}
fn photo_thumbnail_size(source: egui::Vec2, max_width: f32, height: f32) -> egui::Vec2 {
    let aspect = if source.x > 0. && source.y > 0. {
        source.x / source.y
    } else {
        1.
    };
    let width = (height * aspect).min(max_width);
    egui::vec2(width, width / aspect)
}
fn capture_controls(
    ui: &mut egui::Ui,
    session: &mut photosuite_application::comments::CommentSession,
    player: &dyn photosuite_application::diary::AudioPlayer,
) {
    let phase = session.snapshot().phase;
    let idle = matches!(phase, Phase::Idle | Phase::Ready | Phase::Error) && !session.is_saving();
    let status = match phase {
        Phase::Idle => "Bereit zur Aufnahme",
        Phase::Preparing => "",
        Phase::Recording => "Aufnahme läuft",
        Phase::Paused => "Aufnahme pausiert",
        Phase::Finishing => "",
        Phase::Ready => "Aufnahme bereit zum Speichern",
        Phase::Error => "Aufnahme nicht möglich",
    };
    if let Some(position) = recording_waveform(
        ui,
        session.snapshot(),
        matches!(phase, Phase::Paused | Phase::Ready)
            && !session.is_saving()
            && !session.is_save_pending(),
    ) {
        session.command(Command::Seek(position));
    }
    level_meter(ui, session.snapshot().level, session.snapshot().peak);
    ui.horizontal(|ui| {
        let can_play = matches!(phase, Phase::Paused | Phase::Ready)
            && session.snapshot().seconds > 0.
            && !session.is_saving()
            && !session.is_save_pending();
        if preview_button(ui, session.snapshot().playing, can_play).clicked() {
            session.command(Command::Preview);
        }
        let active = matches!(phase, Phase::Recording | Phase::Paused);
        let primary = transport_button(
            ui,
            phase == Phase::Recording,
            session.can_start() || active,
            false,
        );
        let tip = match phase {
            Phase::Recording => "Aufnahme pausieren",
            Phase::Paused => "Ab der Markierung aufnehmen; bisherigen Rest ersetzen",
            Phase::Ready => "Neu aufnehmen (ungespeicherte Aufnahme ersetzen)",
            _ => "Aufnehmen",
        };
        primary.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, primary.enabled(), tip)
        });
        if primary.on_hover_text(tip).clicked() {
            match phase {
                Phase::Recording => session.command(Command::Pause),
                Phase::Paused => session.command(Command::Resume),
                _ => {
                    if let Err(error) = session.start(player) {
                        session.report_error(error);
                    }
                }
            }
        }
        if transport_button(ui, false, active && !session.is_saving(), true)
            .on_hover_text("Aufnahme beenden")
            .clicked()
        {
            session.command(Command::Finish);
        }
        let remaining = ui.available_width();
        let microphone_width = (remaining * 0.38).clamp(88., 190.);
        let status_width = (remaining - microphone_width - ui.spacing().item_spacing.x).max(1.);
        ui.allocate_ui_with_layout(
            egui::vec2(status_width, 34.),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(egui::vec2(status_width, 34.));
                let seconds = session.snapshot().seconds as u64;
                let position = session.snapshot().position as u64;
                let message = if status.is_empty() {
                    format!("{:02}:{:02}", seconds / 60, seconds % 60)
                } else if matches!(phase, Phase::Paused | Phase::Ready) {
                    format!(
                        "{status} · {:02}:{:02} / {:02}:{:02}",
                        position / 60,
                        position % 60,
                        seconds / 60,
                        seconds % 60
                    )
                } else {
                    format!("{status} · {:02}:{:02}", seconds / 60, seconds % 60)
                };
                ui.add(egui::Label::new(egui::RichText::new(&message).strong()).truncate())
                    .on_hover_text(message);
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(microphone_width, 34.),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.set_min_size(egui::vec2(microphone_width, 34.));
                microphone_picker(ui, session, idle)
            },
        );
    });
    ui.separator();
}
fn preview_button(ui: &mut egui::Ui, playing: bool, enabled: bool) -> egui::Response {
    let response = if playing {
        transport_button(ui, true, enabled, false)
    } else {
        let response = ui.add_enabled(
            enabled,
            egui::Button::new("").min_size(egui::vec2(34., 34.)),
        );
        let c = response.rect.center();
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(-4., -6.),
                c + egui::vec2(-4., 6.),
                c + egui::vec2(6., 0.),
            ],
            crate::theme::MUTED,
            egui::Stroke::NONE,
        ));
        response
    };
    let label = if playing {
        "Vorhören pausieren"
    } else {
        "Ab der Markierung abspielen"
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response.on_hover_text(label)
}
fn recording_waveform(
    ui: &mut egui::Ui,
    snapshot: &photosuite_application::comments::CaptureSnapshot,
    seekable: bool,
) -> Option<f64> {
    let (rect, response) = waveform(ui, &snapshot.wave);
    let span = snapshot.wave_span.max(0.001);
    let fraction = ((snapshot.position - snapshot.wave_start) / span).clamp(0., 1.) as f32;
    let x = rect.left() + rect.width() * fraction;
    ui.painter().line_segment(
        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
        egui::Stroke::new(1_f32, crate::theme::TEXT),
    );
    ui.painter()
        .circle_filled(egui::pos2(x, rect.bottom() - 3.), 3., crate::theme::TEXT);
    let result = if seekable && (response.clicked() || response.dragged()) {
        response.interact_pointer_pos().map(|point| {
            (snapshot.wave_start
                + (((point.x - rect.left()) / rect.width()).clamp(0., 1.) as f64) * span)
                .clamp(0., snapshot.seconds)
        })
    } else {
        None
    };
    response.on_hover_text(if seekable {"Klicken oder ziehen: Abspielposition wählen. Play hört ab dort vor; Aufnahme ersetzt den Rest ab der Markierung."}else{"Während der Aufnahme ist die Position gesperrt. Zum Vorhören oder Ersetzen pausieren."});
    result
}
fn microphone_picker(
    ui: &mut egui::Ui,
    session: &mut photosuite_application::comments::CommentSession,
    enabled: bool,
) {
    let mut selected_device = session.device().to_owned();
    ui.add_enabled_ui(enabled, |ui| {
        ui.spacing_mut().interact_size.y = 34.;
        let selected = session
            .devices()
            .iter()
            .find(|d| d.id == session.device())
            .map(|d| d.name.as_str())
            .unwrap_or("Systemstandard");
        let combo = egui::ComboBox::from_id_salt("comment_microphone")
            .width(ui.available_width())
            .truncate()
            .selected_text(selected)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected_device, String::new(), "Systemstandard");
                for device in session.devices() {
                    ui.selectable_value(&mut selected_device, device.id.clone(), &device.name);
                }
            });
        // Refresh once on each open, not on every frame while the popup is visible.
        if combo.response.clicked() && egui::ComboBox::is_open(ui.ctx(), combo.response.id) {
            session.refresh_devices();
        }
        combo
            .response
            .on_hover_text("Eingabegerät wählen; Geräteliste wird beim Öffnen aktualisiert");
    });
    if selected_device != session.device()
        && let Err(error) = session.set_device(selected_device)
    {
        session.report_error(error);
    }
}
fn capture_details(
    ui: &mut egui::Ui,
    session: &mut photosuite_application::comments::CommentSession,
) {
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
}
// Draw transport shapes instead of font glyphs, keeping size and alignment stable.
fn transport_button(ui: &mut egui::Ui, pause: bool, enabled: bool, stop: bool) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("").min_size(egui::vec2(34., 34.)),
    );
    let center = response.rect.center();
    let color = if enabled {
        if !pause && !stop {
            crate::theme::RECORD_RED
        } else {
            crate::theme::TEXT
        }
    } else {
        crate::theme::MUTED
    };
    if stop {
        ui.painter().rect_filled(
            egui::Rect::from_center_size(center, egui::vec2(11., 11.)),
            1.,
            color,
        );
    } else if pause {
        for dx in [-3.5, 3.5] {
            ui.painter().rect_filled(
                egui::Rect::from_center_size(center + egui::vec2(dx, 0.), egui::vec2(3., 13.)),
                0.,
                color,
            );
        }
    } else {
        ui.painter().circle_filled(center, 6., color);
    }
    response
}
const YELLOW_DB: f32 = -18.;
const RED_DB: f32 = -6.;
fn db_position(db: f32) -> f32 {
    ((db + 48.) / 48.).clamp(0., 1.)
}
fn amplitude_position(amplitude: f32) -> f32 {
    if amplitude.is_finite() && amplitude > 0. {
        db_position(20. * amplitude.log10())
    } else {
        0.
    }
}
fn level_meter(ui: &mut egui::Ui, amplitude: f32, peak: f32) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.), egui::Sense::hover());
    let track = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 9.));
    ui.painter().rect_filled(track, 2., crate::theme::CANVAS);
    let x = |db| track.left() + track.width() * db_position(db);
    ui.painter().rect_filled(
        egui::Rect::from_min_max(
            track.min,
            egui::pos2(
                track.left() + track.width() * amplitude_position(amplitude),
                track.bottom(),
            ),
        ),
        2.,
        crate::theme::ACCENT,
    );
    for (db, color) in [
        (YELLOW_DB, crate::theme::LEVEL_YELLOW),
        (RED_DB, crate::theme::LEVEL_RED),
    ] {
        ui.painter().line_segment(
            [
                egui::pos2(x(db), track.top()),
                egui::pos2(x(db), track.bottom() + 2.),
            ],
            egui::Stroke::new(1. / ui.ctx().pixels_per_point(), color.gamma_multiply(0.45)),
        );
    }
    for db in [-48., YELLOW_DB, RED_DB, 0.] {
        let align = if db == 0. {
            egui::Align2::RIGHT_TOP
        } else if db == -48. {
            egui::Align2::LEFT_TOP
        } else {
            egui::Align2::CENTER_TOP
        };
        ui.painter().text(
            egui::pos2(x(db), track.bottom() + 3.),
            align,
            format!("{db:.0}"),
            egui::FontId::proportional(12.),
            crate::theme::MUTED,
        );
    }
    if peak.is_finite() && peak > -48. {
        ui.painter().line_segment(
            [
                egui::pos2(x(peak), track.top()),
                egui::pos2(x(peak), track.bottom()),
            ],
            egui::Stroke::new(2_f32, crate::theme::TEXT),
        );
    }
    response.on_hover_text("Pegel in dBFS: Ziel für Sprachpegel ungefähr −18 bis −6 dBFS. Oberhalb der roten Linie bleiben 6 dB Reserve bis zur digitalen Grenze bei 0 dBFS. Weißer Strich: Spitzenpegel. Keine automatische Lautstärkeänderung.");
}
fn wave_color(amplitude: f32) -> egui::Color32 {
    let position = amplitude_position(amplitude);
    if position > db_position(RED_DB) {
        crate::theme::LEVEL_RED
    } else if position >= db_position(YELLOW_DB) {
        crate::theme::LEVEL_YELLOW
    } else {
        crate::theme::LEVEL_GREEN
    }
}
// Fit the fixed time window to every physical pixel, preserving brief peaks when
// multiple measurements share a column; repeat values when pixels outnumber samples.
fn wave_column(values: &[f32], column: usize, columns: usize) -> f32 {
    let history = photosuite_application::comments::CAPTURE_WAVE_SAMPLES;
    let start = column * history / columns;
    let end = ((column + 1) * history)
        .div_ceil(columns)
        .min(values.len())
        .min(history);
    values
        .get(start..end)
        .unwrap_or(&[])
        .iter()
        .copied()
        .filter(|n| n.is_finite())
        .fold(0., f32::max)
}
fn waveform(ui: &mut egui::Ui, values: &[f32]) -> (egui::Rect, egui::Response) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 68.),
        egui::Sense::click_and_drag(),
    );
    response.clone().on_hover_text("Pegelverlauf: ungefähr 32 Sekunden. Bewegung auf ein Achtel verlangsamt; kurze Ausschläge bleiben sichtbar.");
    ui.painter().rect_filled(rect, 4., crate::theme::CANVAS);
    let pixel = 1. / ui.ctx().pixels_per_point();
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.top()),
            egui::pos2(rect.center().x, rect.bottom()),
        ],
        egui::Stroke::new(pixel, crate::theme::MUTED.gamma_multiply(0.35)),
    );
    let half = rect.height() / 2. - 3.;
    let pixels = ui.ctx().pixels_per_point();
    let columns = (rect.width() * pixels).ceil().max(1.) as usize;
    let history = photosuite_application::comments::CAPTURE_WAVE_SAMPLES;
    let visible_columns = (values.len().min(history) * columns).div_ceil(history);
    for column in 0..visible_columns {
        let value = wave_column(values, column, columns);
        let position = amplitude_position(value);
        let color = wave_color(value);
        let x = (rect.left() * pixels).floor() / pixels + (column as f32 + 0.5) / pixels;
        let extent = (position * half).max(0.5 / pixels);
        ui.painter().line_segment(
            [
                egui::pos2(x, rect.center().y - extent),
                egui::pos2(x, rect.center().y + extent),
            ],
            egui::Stroke::new(1. / pixels, color),
        );
    }
    for (db, color) in [
        (YELLOW_DB, crate::theme::LEVEL_YELLOW),
        (RED_DB, crate::theme::LEVEL_RED),
    ] {
        let offset = half * db_position(db);
        for sign in [-1., 1.] {
            let y = rect.center().y + sign * offset;
            ui.painter().line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                egui::Stroke::new(1. / ui.ctx().pixels_per_point(), color.gamma_multiply(0.35)),
            );
        }
    }
    (rect, response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn meter_preserves_six_db_headroom_and_marks_the_same_waveform_scale() {
        for db in [-48., YELLOW_DB, RED_DB, 0.] {
            let amplitude = 10_f32.powf(db / 20.);
            assert!((amplitude_position(amplitude) - db_position(db)).abs() < 0.00001);
        }
        assert!(db_position(RED_DB) < 1.);
        assert_eq!(db_position(RED_DB), 0.875);
        assert_eq!(amplitude_position(0.), 0.);
        assert_eq!(amplitude_position(f32::NAN), 0.);
    }
    #[test]
    fn transport_shapes_keep_identical_bounds_for_record_pause_and_stop() {
        let ctx = egui::Context::default();
        for (pause, stop) in [(false, false), (true, false), (false, true)] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let response = transport_button(ui, pause, true, stop);
                    assert_eq!(response.rect.size(), egui::vec2(34., 34.));
                });
            });
        }
    }
    #[test]
    fn photo_thumbnail_uses_text_height_and_variable_proportional_width() {
        let portrait = photo_thumbnail_size(egui::vec2(100., 300.), 240., 60.);
        let landscape = photo_thumbnail_size(egui::vec2(300., 100.), 240., 60.);
        assert_eq!(portrait, egui::vec2(20., 60.));
        assert_eq!(landscape, egui::vec2(180., 60.));
        let panorama = photo_thumbnail_size(egui::vec2(1000., 100.), 180., 60.);
        assert_eq!(panorama, egui::vec2(180., 18.));
    }
    #[test]
    fn waveform_uses_physical_pixel_strokes_and_a_vertical_centerline() {
        for pixels in [1., 2.] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(pixels);
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    waveform(
                        ui,
                        &vec![0.05; photosuite_application::comments::CAPTURE_WAVE_SAMPLES],
                    )
                });
            });
            let mut bars = 0;
            let mut middle = false;
            for shape in output.shapes {
                if let egui::Shape::LineSegment { points, stroke } = shape.shape {
                    if stroke.color == crate::theme::LEVEL_GREEN {
                        assert!((stroke.width * ctx.pixels_per_point() - 1.).abs() < 0.0001);
                        bars += 1;
                    } else if points[0].x == points[1].x
                        && stroke.color == crate::theme::MUTED.gamma_multiply(0.35)
                    {
                        middle = true;
                    }
                }
            }
            assert!(bars >= 500);
            assert!(middle);
        }
    }
    #[test]
    fn save_discard_confirmation_has_spaced_centered_buttons() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                unsaved_confirmation(ui, 440., false, "");
            });
        });
        let rect = |label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
                    }
                    _ => None,
                })
                .unwrap()
        };
        let save = rect("Speichern");
        let discard = rect("Verwerfen");
        let text = rect("Möchtest du die Aufnahme speichern oder verwerfen?");
        let title = rect("Aufnahme noch nicht gespeichert");
        assert!((save.center().y - discard.center().y).abs() < 1.);
        assert!(save.right() < discard.left());
        assert!(save.top() - text.bottom() >= 16.);
        assert!(text.top() - title.bottom() >= 10.);
    }
    #[test]
    fn waveform_colors_and_dense_resampling_preserve_thresholds_and_short_peaks() {
        assert_eq!(
            wave_color(10_f32.powf(-24. / 20.)),
            crate::theme::LEVEL_GREEN
        );
        assert_eq!(
            wave_color(10_f32.powf(-12. / 20.)),
            crate::theme::LEVEL_YELLOW
        );
        assert_eq!(wave_color(10_f32.powf(-3. / 20.)), crate::theme::LEVEL_RED);
        let mut values = vec![0.01; photosuite_application::comments::CAPTURE_WAVE_SAMPLES];
        values[7] = 0.8;
        assert_eq!(wave_column(&values, 0, 120), 0.8);
        assert_eq!(wave_column(&values, 1, 120), 0.01);
        assert_eq!(wave_column(&values, 14, 1920), 0.8);
        assert_eq!(wave_column(&values, 15, 1920), 0.8);
    }
    #[test]
    fn clicking_a_paused_waveform_selects_the_corresponding_time() {
        for seekable in [true, false] {
            let ctx = egui::Context::default();
            let snapshot = photosuite_application::comments::CaptureSnapshot {
                seconds: 10.,
                wave_span: 10.,
                position: 2.,
                wave: vec![0.05; 960],
                ..Default::default()
            };
            let input = |events| egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400., 200.),
                )),
                events,
                ..Default::default()
            };
            let output = ctx.run(input(vec![]), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    recording_waveform(ui, &snapshot, seekable);
                });
            });
            let rect = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(r) if (r.rect.height() - 68.).abs() < 0.1 => Some(r.rect),
                    _ => None,
                })
                .unwrap();
            let point = rect.center();
            let mut selected = None;
            let _ = ctx.run(
                input(vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]),
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        selected = recording_waveform(ui, &snapshot, seekable);
                    });
                },
            );
            if seekable {
                assert!((selected.unwrap() - 5.).abs() < 0.01);
            } else {
                assert!(selected.is_none());
            }
        }
    }
}
