//! Presentation state for one video; retains at most six frame textures.
use crate::ports::VideoPreviewSource;
use eframe::egui;
use photosuite_application::storyboard::{self, Event, FRAME_COUNT};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};
enum Tile {
    Loading,
    Ready(egui::TextureHandle, f64),
    Failed(String),
}
struct State {
    path: PathBuf,
    receiver: Option<mpsc::Receiver<Event>>,
    tiles: [Tile; FRAME_COUNT],
    duration: Option<f64>,
    error: Option<String>,
}
#[derive(Default)]
pub struct StoryboardView {
    state: Option<State>,
}
impl StoryboardView {
    pub fn clear(&mut self) {
        self.state = None;
    }
    pub fn show(&mut self, ui: &mut egui::Ui, path: &Path, source: Arc<dyn VideoPreviewSource>) {
        if self.state.as_ref().is_none_or(|s| s.path != path) {
            self.state = Some(State {
                path: path.into(),
                receiver: Some(storyboard::load(source.clone(), path.into())),
                tiles: std::array::from_fn(|_| Tile::Loading),
                duration: None,
                error: None,
            });
        }
        let state = self.state.as_mut().unwrap();
        let mut done = false;
        if let Some(receiver) = &state.receiver {
            loop {
                match receiver.try_recv() {
                    Ok(Event::Started(duration)) => state.duration = Some(duration),
                    Ok(Event::Frame(index, result)) => {
                        state.tiles[index] = match result {
                            Ok(frame) => {
                                let image = egui::ColorImage::from_rgba_unmultiplied(
                                    [frame.image.width as usize, frame.image.height as usize],
                                    &frame.image.rgba,
                                );
                                Tile::Ready(
                                    ui.ctx().load_texture(
                                        format!("storyboard:{}:{index}", path.display()),
                                        image,
                                        egui::TextureOptions::LINEAR,
                                    ),
                                    frame.seconds,
                                )
                            }
                            Err(error) => Tile::Failed(error),
                        }
                    }
                    Ok(Event::Failed(error)) => {
                        state.error = Some(error);
                        done = true;
                    }
                    Ok(Event::Done) => {
                        done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if !done {
                            state.error = Some("Videovorschau unerwartet beendet".into());
                        }
                        done = true;
                        break;
                    }
                }
            }
        }
        if done {
            state.receiver = None;
        } else {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        let ready = state
            .tiles
            .iter()
            .filter(|t| matches!(t, Tile::Ready(..)))
            .count();
        ui.horizontal_wrapped(|ui| {
            ui.heading("Videoübersicht");
            if let Some(duration) = state.duration {
                ui.label(format!("Dauer {}", timestamp(duration)));
            }
            ui.label(format!("{ready} / {FRAME_COUNT} Vorschaubilder"));
        });
        if let Some(error) = &state.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        let columns = if ui.available_width() >= 720. {
            3
        } else if ui.available_width() >= 440. {
            2
        } else {
            1
        };
        let gap = ui.spacing().item_spacing.x;
        let width = ((ui.available_width() - gap * (columns - 1) as f32) / columns as f32).max(1.);
        let height = width * 9. / 16.;
        egui::ScrollArea::vertical()
            .id_salt("video_storyboard")
            .max_height(ui.available_height().max(180.))
            .show(ui, |ui| {
                for row in 0..FRAME_COUNT.div_ceil(columns) {
                    ui.horizontal_top(|ui| {
                        for index in row * columns..((row + 1) * columns).min(FRAME_COUNT) {
                            ui.vertical(|ui| {
                                ui.set_width(width);
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(width, height),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    rect,
                                    6.,
                                    egui::Color32::from_rgb(20, 24, 30),
                                );
                                let label = match &state.tiles[index] {
                                    Tile::Ready(texture, seconds) => {
                                        let dimensions = texture.size_vec2();
                                        let scale =
                                            (width / dimensions.x).min(height / dimensions.y);
                                        let target = egui::Rect::from_center_size(
                                            rect.center(),
                                            dimensions * scale,
                                        );
                                        ui.painter().image(
                                            texture.id(),
                                            target,
                                            egui::Rect::from_min_max(
                                                egui::pos2(0., 0.),
                                                egui::pos2(1., 1.),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                        timestamp(*seconds)
                                    }
                                    Tile::Loading => {
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            if state.receiver.is_some() {
                                                "Wird geladen …"
                                            } else {
                                                "Keine Vorschau"
                                            },
                                            egui::FontId::proportional(14.),
                                            egui::Color32::LIGHT_GRAY,
                                        );
                                        state
                                            .duration
                                            .and_then(|d| storyboard::sample_times(d).ok())
                                            .map(|times| timestamp(times[index]))
                                            .unwrap_or_else(|| format!("Bild {}", index + 1))
                                    }
                                    Tile::Failed(error) => {
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "Vorschau nicht verfügbar",
                                            egui::FontId::proportional(13.),
                                            egui::Color32::LIGHT_GRAY,
                                        );
                                        ui.interact(
                                            rect,
                                            ui.id().with(index),
                                            egui::Sense::hover(),
                                        )
                                        .on_hover_text(error);
                                        format!("Bild {} · Fehler", index + 1)
                                    }
                                };
                                ui.label(egui::RichText::new(label).monospace());
                            });
                        }
                    });
                    ui.add_space(6.);
                }
            });
    }
}
fn timestamp(seconds: f64) -> String {
    let millis = (seconds.max(0.) * 1000.).round() as u64;
    let hours = millis / 3_600_000;
    let minutes = millis / 60_000 % 60;
    let seconds = millis / 1000 % 60;
    let fraction = millis % 1000;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}.{fraction:03}")
    } else {
        format!("{minutes:02}:{seconds:02}.{fraction:03}")
    }
}
