//! Shared formatting only; no I/O, persistence or workflow decisions.
use crate::theme;
use eframe::egui;
pub(super) fn file(ui: &mut egui::Ui, label: &str, path: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).small().color(theme::MUTED));
        ui.add(
            egui::Label::new(egui::RichText::new(path.rsplit('/').next().unwrap_or(path)).strong())
                .truncate(),
        )
        .on_hover_text(path);
    });
}
pub(super) fn value(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(theme::MUTED));
        ui.add(egui::Label::new(value).truncate())
            .on_hover_text(value);
    });
}
#[derive(Clone)]
struct StepExpansion {
    expanded: bool,
    previous: photosuite_application::processing::StepState,
    queued: bool,
}
#[derive(Default)]
pub(super) struct StepActions {
    pub repeat: bool,
    pub model_settings: bool,
    pub geo_settings: bool,
    pub reveal: bool,
}
pub(super) fn step(
    ui: &mut egui::Ui,
    row: &photosuite_application::processing::StepStatus,
    enabled: bool,
    queued: bool,
    last_result: Option<&photosuite_application::transcription::queue::Receipt>,
) -> StepActions {
    use photosuite_application::processing::{Step, StepState};
    let label = match row.step {
        Step::AudioInventory => "Audiobestand",
        Step::AudioDurations => "Audiodauern",
        Step::Metadata => "Medienzuordnungen",
        Step::Years => "Audio-Jahresübersicht",
        Step::Locations => "GPS & Orte",
        Step::Model => "Whisper-Modell",
        Step::Planning => "Transkriptionsprüfung",
        Step::Transcription => "Transkription",
    };
    let id = ui.id().with(("queue_card_expansion", row.step));
    let mut expansion = ui
        .data_mut(|data| data.get_temp::<StepExpansion>(id))
        .unwrap_or(StepExpansion {
            expanded: row.state != StepState::Complete,
            previous: row.state,
            queued: false,
        });
    if row.state == StepState::Complete && expansion.previous != StepState::Complete && !queued {
        expansion.expanded = false;
    }
    if row.state == StepState::Active
        || (queued && !expansion.queued)
        || (expansion.previous == StepState::Complete && row.state != StepState::Complete)
    {
        expansion.expanded = true;
    }
    expansion.previous = row.state;
    expansion.queued = queued;
    let (state, color) = if queued {
        ("Vorgemerkt", theme::ACCENT)
    } else {
        match row.state {
            StepState::Pending => ("Wartet", theme::MUTED),
            StepState::Active => ("Aktiv", theme::ACCENT),
            StepState::Paused => ("Pausiert", theme::MUTED),
            StepState::Complete => ("Fertig", theme::LEVEL_GREEN),
            StepState::Warning => ("Hinweis", theme::LEVEL_YELLOW),
            StepState::Error => ("Fehler", theme::DANGER),
        }
    };
    let mut actions = StepActions::default();
    theme::queue_step_group(ui, !expansion.expanded, |ui| {
        let title = if row.total > 0 {
            format!("{label}  {} / {}", row.checked, row.total)
        } else {
            label.into()
        };
        let height = if expansion.expanded {
            theme::QUEUE_HEADER_HEIGHT
        } else {
            theme::QUEUE_COLLAPSED_HEADER_HEIGHT
        };
        let response = ui.allocate_response(
            egui::vec2(ui.available_width(), height),
            egui::Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                format!("{title} · {state} · Ein-/Ausklappen"),
            )
        });
        let center = egui::pos2(response.rect.left() + 6., response.rect.center().y);
        let triangle = if expansion.expanded {
            vec![
                center + egui::vec2(-4., -2.),
                center + egui::vec2(4., -2.),
                center + egui::vec2(0., 3.),
            ]
        } else {
            vec![
                center + egui::vec2(-2., -4.),
                center + egui::vec2(-2., 4.),
                center + egui::vec2(3., 0.),
            ]
        };
        ui.painter().add(egui::Shape::convex_polygon(
            triangle,
            theme::MUTED,
            egui::Stroke::NONE,
        ));
        let status =
            ui.painter()
                .layout_no_wrap(state.into(), egui::FontId::proportional(13.), color);
        let status_pos = egui::pos2(
            response.rect.right() - status.size().x,
            response.rect.center().y - status.size().y / 2.,
        );
        ui.painter().galley(status_pos, status, color);
        let title_rect = egui::Rect::from_min_max(
            egui::pos2(response.rect.left() + 18., response.rect.top()),
            egui::pos2(
                (status_pos.x - 8.).max(response.rect.left() + 19.),
                response.rect.bottom(),
            ),
        );
        let count = if row.total > 0 {
            Some(ui.painter().layout_no_wrap(
                format!("{} / {}", row.checked, row.total),
                egui::FontId::proportional(13.),
                theme::MUTED,
            ))
        } else {
            None
        };
        let count_width = count.as_ref().map_or(0., |galley| galley.size().x + 8.);
        let title_galley = egui::WidgetText::from(egui::RichText::new(label).strong()).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (title_rect.width() - count_width).max(1.),
            egui::TextStyle::Body,
        );
        let count_x = title_rect.left() + title_galley.size().x + 8.;
        ui.painter().galley(
            egui::pos2(
                title_rect.left(),
                title_rect.center().y - title_galley.size().y / 2.,
            ),
            title_galley,
            theme::TEXT,
        );
        if let Some(count) = count {
            ui.painter().galley(
                egui::pos2(count_x, title_rect.center().y - count.size().y / 2.),
                count,
                theme::MUTED,
            );
        }
        if response.clicked() && row.state != StepState::Active {
            expansion.expanded = !expansion.expanded;
        }
        response
            .on_hover_text(&title)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if !expansion.expanded {
            return;
        }
        let fraction = if row.state == StepState::Complete {
            1.
        } else if row.total > 0 {
            row.checked as f32 / row.total as f32
        } else {
            0.
        };
        ui.add(
            egui::ProgressBar::new(fraction.clamp(0., 1.))
                .desired_height(5.)
                .fill(color),
        );
        let detail = match row.step {
            Step::Model => format!("Sprachmodell prüfen · {}", row.info),
            Step::Locations => format!("GPS/Orte · cities500 · {}", row.info),
            Step::AudioDurations => format!("Dauern prüfen · {}", row.info),
            Step::Years => format!("{} · nach Bezugsdatum", row.info),
            Step::Transcription => format!("Lokal verschriftlichen · {}", row.info),
            _ => row.info.clone(),
        };
        ui.add(
            egui::Label::new(egui::RichText::new(&detail).small().color(theme::MUTED)).truncate(),
        )
        .on_hover_text(&detail);
        if let Some(path) = &row.current {
            file(
                ui,
                if row.state == StepState::Active {
                    "Aktuell"
                } else {
                    "Zuletzt"
                },
                &path.to_string_lossy(),
            );
        }
        match row.step {
            Step::Model => {
                actions.model_settings = ui.button("Transkription …").clicked();
            }
            Step::Locations => {
                actions.geo_settings = ui.button("Geodaten …").clicked();
            }
            Step::Transcription => {
                if let Some(result) = last_result {
                    file(ui, "Zuletzt abgeschlossen", result.source.as_str());
                    actions.reveal = ui.button("Ergebnis im Finder anzeigen").clicked();
                }
                ui.small("Lokal auf diesem Mac · Ergebnisse bleiben erhalten.").on_hover_text("Während der Transkription bleibt der Mac wach; der Bildschirm darf ausgehen. Fertige Ergebnisse bleiben beim Beenden erhalten.");
            }
            _ => {}
        }
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), theme::TRANSPORT_EDGE),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                actions.repeat = super::icons::button(
                    ui,
                    super::icons::Icon::Reload,
                    enabled,
                    &format!("{label} wiederholen"),
                )
                .clicked();
            },
        );
    });
    ui.data_mut(|data| data.insert_temp(id, expansion));
    actions
}
// Pagination bounds even arbitrarily long error lists/text without a scroll area.
// State is transient presentation state; no library or application data is changed.
pub(super) fn page_selector(ui: &mut egui::Ui, salt: impl std::hash::Hash, count: usize) -> usize {
    let id = ui.id().with(salt);
    let mut page = ui
        .data_mut(|d| d.get_temp::<usize>(id).unwrap_or(0))
        .min(count.saturating_sub(1));
    if count > 1 {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(page > 0, egui::Button::new("Zurück"))
                .clicked()
            {
                page -= 1;
            }
            ui.label(format!("{} / {count}", page + 1));
            if ui
                .add_enabled(page + 1 < count, egui::Button::new("Weiter"))
                .clicked()
            {
                page += 1;
            }
        });
    }
    ui.data_mut(|d| d.insert_temp(id, page));
    page
}

pub(super) fn paged_text(ui: &mut egui::Ui, id: egui::Id, text: &str, error: bool) {
    // Count line breaks as a full line, avoiding tall pages from many short lines.
    let mut pages = vec![String::new()];
    let mut used = 0;
    // Leave room for both issue navigation and shared processing controls on compact windows.
    let budget = if ui.ctx().content_rect().height() < 480. {
        120
    } else {
        200
    };
    for character in text.chars() {
        let cost = if character == '\n' { 40 } else { 1 };
        if used + cost > budget && used > 0 {
            pages.push(String::new());
            used = 0;
        }
        pages.last_mut().unwrap().push(character);
        used += cost;
    }
    let page = page_selector(ui, id, pages.len());
    let label = egui::RichText::new(&pages[page]);
    ui.add(
        egui::Label::new(if error {
            label.color(crate::theme::DANGER)
        } else {
            label
        })
        .wrap(),
    );
}

#[cfg(test)]
mod step_tests {
    use super::*;
    use photosuite_application::processing::{Step, StepState, StepStatus};
    fn row(state: StepState) -> StepStatus {
        StepStatus {
            step: Step::Transcription,
            state,
            checked: 3,
            total: 10,
            info: "2 fertig · 7 offen · 1 Fehler".into(),
            current: None,
        }
    }
    fn frame(
        ctx: &egui::Context,
        row: &StepStatus,
        scope: &str,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(560., 700.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.push_id(scope, |ui| {
                        step(ui, row, true, false, None);
                    });
                });
            },
        )
    }
    fn text(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
    }
    fn repeat(output: &egui::FullOutput) -> Option<egui::Rect> {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.rect.size() == egui::vec2(34., 34.) => Some(rect.rect),
            _ => None,
        })
    }
    fn click(ctx: &egui::Context, row: &StepStatus, scope: &str, pos: egui::Pos2) {
        for pressed in [true, false] {
            let _ = frame(
                ctx,
                row,
                scope,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }
    #[test]
    fn counts_are_in_title_completed_cards_collapse_and_active_cards_stay_open() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let active = row(StepState::Active);
        let output = frame(&ctx, &active, "library-a", vec![]);
        let title = text(&output, "Transkription").unwrap();
        assert!(text(&output, "3 / 10").unwrap().x > title.x);
        let button = repeat(&output).unwrap();
        assert!(button.top() > title.y + 20.);
        assert!(button.center().x > 400.);
        click(&ctx, &active, "library-a", title);
        assert!(repeat(&frame(&ctx, &active, "library-a", vec![])).is_some());
        let complete = StepStatus {
            checked: 10,
            state: StepState::Complete,
            ..active.clone()
        };
        let output = frame(&ctx, &complete, "library-a", vec![]);
        let title = text(&output, "Transkription").unwrap();
        let count = text(&output, "10 / 10").unwrap();
        assert!(count.x > title.x);
        assert!(text(&output, "Fertig").unwrap().x > count.x);
        assert!(repeat(&output).is_none());
        assert!(text(&output, "Transkription").is_some());
        click(&ctx, &complete, "library-a", title);
        let output = frame(&ctx, &complete, "library-a", vec![]);
        assert!(text(&output, "Transkription").is_some());
        assert!(repeat(&output).is_some());
        assert!(
            repeat(&frame(&ctx, &complete, "library-a", vec![])).is_some(),
            "manual expansion must persist"
        );
        assert!(
            repeat(&frame(&ctx, &complete, "library-b", vec![])).is_none(),
            "expansion must not leak between libraries"
        );
        let _ = frame(&ctx, &active, "library-a", vec![]);
        assert!(
            repeat(&frame(&ctx, &complete, "library-a", vec![])).is_none(),
            "finishing a new run must collapse the card again"
        );
    }
    #[test]
    fn folded_header_keeps_counts_left_alignment_and_uses_about_two_thirds_height() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let complete = StepStatus {
            checked: 10,
            state: StepState::Complete,
            ..row(StepState::Complete)
        };
        let output = frame(&ctx, &complete, "library-a", vec![]);
        let title = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Transkription" => Some(text.pos),
                _ => None,
            })
            .unwrap();
        let rect = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.fill == theme::CANVAS
                        && rect.stroke.color == theme::BORDER
                        && rect.rect.width() > 400. =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .unwrap();
        assert!(
            (28. ..=32.).contains(&rect.height()),
            "collapsed card too tall: {rect:?}"
        );
        assert!(
            (title.x - rect.left() - 27.).abs() < 2.,
            "title must stay at the left edge: {title:?} {rect:?}"
        );
        let other = StepStatus {
            step: Step::Locations,
            ..complete
        };
        let output = frame(&ctx, &other, "library-a", vec![]);
        let other_title = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "GPS & Orte" => Some(text.pos),
                _ => None,
            })
            .unwrap();
        assert!(
            (title.x - other_title.x).abs() < 0.5,
            "different title lengths must not alter alignment"
        );
    }
    #[test]
    fn resource_settings_and_last_result_actions_live_in_the_matching_step_cards() {
        use crate::domain::tags::LibraryFile;
        use photosuite_application::transcription::queue::Receipt;
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let receipt = Receipt {
            source: LibraryFile::new("2026/audio/diary/aud_20261121_120000.wav".into()).unwrap(),
            record: LibraryFile::new("2026/audio/diary/aud_20261121_120000.wav".into()).unwrap(),
            excerpt: "Text".into(),
        };
        let render = |row: &StepStatus, events| {
            let mut actions = StepActions::default();
            let output = ctx.run(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(560., 700.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.push_id("resource-actions", |ui| {
                            actions = step(ui, row, true, false, Some(&receipt));
                        });
                    });
                },
            );
            (output, actions)
        };
        for (kind, label) in [
            (Step::Model, "Transkription …"),
            (Step::Locations, "Geodaten …"),
            (Step::Transcription, "Ergebnis im Finder anzeigen"),
        ] {
            let row = StepStatus {
                step: kind,
                ..row(StepState::Active)
            };
            let (output, _) = render(&row, vec![]);
            let pos = text(&output, label).unwrap();
            let mut selected = None;
            for pressed in [true, false] {
                let (_, action) = render(
                    &row,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
                selected = Some(action);
            }
            let selected = selected.unwrap();
            assert_eq!(selected.model_settings, kind == Step::Model);
            assert_eq!(selected.geo_settings, kind == Step::Locations);
            assert_eq!(selected.reveal, kind == Step::Transcription);
        }
    }
    #[test]
    fn model_and_place_details_are_small_single_lines_below_the_progress_bar() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        for (step_kind, info, prefix) in [
            (
                Step::Model,
                "large-v3-turbo · Sprache de",
                "Sprachmodell prüfen",
            ),
            (
                Step::Locations,
                "21 mit GPS · 4 ohne · 0 Fehler",
                "GPS/Orte · cities500",
            ),
        ] {
            let row = StepStatus {
                step: step_kind,
                info: info.into(),
                ..row(StepState::Active)
            };
            let output = frame(&ctx, &row, "library-a", vec![]);
            let detail = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text().starts_with(prefix) => Some(text),
                    _ => None,
                })
                .unwrap();
            assert!(detail.galley.text().contains(info));
            assert_eq!(detail.galley.rows.len(), 1);
            let button = repeat(&output).unwrap();
            assert!(button.top() > detail.visual_bounding_rect().bottom());
            let progress = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.rect.width() > 100. && rect.rect.height() < 15. =>
                    {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .max_by(|a, b| a.top().total_cmp(&b.top()))
                .unwrap();
            assert!(detail.visual_bounding_rect().top() >= progress.bottom());
        }
    }
}
