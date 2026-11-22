//! Read-only shortcut help derived from enabled menus and the existing binding catalog.
use super::Suite;
use crate::{command_labels::CommandLabel, desktop_menu::Action};
use eframe::egui;
use photosuite_application::preferences::{
    Shortcut,
    bindings::{BindingContext, Scope, fixed_bindings},
};
impl Suite {
    fn shortcut_help_rows(&self, ctx: &egui::Context) -> Vec<(String, Shortcut)> {
        let fixed = fixed_bindings();
        let mut rows = Vec::new();
        for entry in self
            .menu_entries(ctx)
            .into_iter()
            .filter(|entry| entry.enabled)
        {
            let Some(shortcut) = entry.shortcut else {
                continue;
            };
            let title = match entry.action {
                Action::Edit(command) => Some(command.title().to_owned()),
                _ => fixed
                    .iter()
                    .find(|binding| {
                        binding.shortcut == shortcut
                            && matches!(binding.scope, Scope::Application | Scope::Text)
                    })
                    .map(|binding| binding.title.to_owned()),
            };
            if let Some(title) = title {
                rows.push((title, shortcut));
            }
        }
        let context = BindingContext {
            focused: ctx.input(|i| i.focused),
            modal: (self.comments.is_open() && !self.comment_view.inline)
                || self.options_open
                || self.tag_view.open
                || self.bindings.visible
                || self.location_open
                || self.date_editor.is_some()
                || self.search_open,
            text_input: ctx.wants_keyboard_input(),
            viewer: self.selected.is_some(),
            ..Default::default()
        };
        for binding in fixed
            .into_iter()
            .filter(|binding| binding.scope == Scope::Viewer && context.allows(binding.scope))
        {
            let enabled = match binding.id {
                "viewer_previous" | "viewer_next" => {
                    let visible = self.viewer_items(&self.visible());
                    let position = self
                        .selected
                        .and_then(|selected| visible.iter().position(|index| *index == selected));
                    position.is_some_and(|position| {
                        if binding.id == "viewer_previous" {
                            position > 0
                        } else {
                            position + 1 < visible.len()
                        }
                    }) && !self.comments.is_switching_photo()
                }
                _ => true,
            };
            if enabled {
                rows.push((binding.title.into(), binding.shortcut));
            }
        }
        if self.search_open || self.location_open {
            rows.push((
                "Dialog schließen".into(),
                Shortcut {
                    key: "Escape".into(),
                    alt: false,
                    ctrl: false,
                    shift: false,
                    command: false,
                    mac_cmd: false,
                },
            ));
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows.dedup();
        rows
    }
    pub(super) fn shortcut_overlay(&self, ctx: &egui::Context) {
        let held = ctx.input(|input| {
            input.focused
                && input.modifiers.shift
                && !input.modifiers.alt
                && !input.modifiers.ctrl
                && !input.modifiers.command
                && !input.modifiers.mac_cmd
                // Native menu shortcuts can leave a stale pressed-key entry when
                // focus moves to the search dialog. Only a new keypress suppresses help.
                && !input.events.iter().any(|event| matches!(event, egui::Event::Key { pressed: true, .. }))
        });
        if !held || ctx.wants_keyboard_input() || self.bindings.visible {
            return;
        }
        let rows = self.shortcut_help_rows(ctx);
        egui::Area::new(egui::Id::new("shortcut_help_overlay"))
            .order(egui::Order::Tooltip)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .interactable(true)
            .movable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::BLACK)
                    .inner_margin(20)
                    .corner_radius(8.)
                    .show(ui, |ui| {
                        paint_help(ui, ctx, &rows);
                    });
            });
    }
}

/// Measured, non-scrolling layout. Scale only if the available window is too small.
fn paint_help(ui: &mut egui::Ui, ctx: &egui::Context, rows: &[(String, Shortcut)]) {
    let lines: Vec<_> = rows
        .iter()
        .filter_map(|(title, shortcut)| {
            let key = egui::Key::from_name(&shortcut.key)?;
            let shortcut = egui::KeyboardShortcut::new(
                egui::Modifiers {
                    alt: shortcut.alt,
                    ctrl: shortcut.ctrl,
                    shift: shortcut.shift,
                    command: shortcut.command,
                    mac_cmd: shortcut.mac_cmd,
                },
                key,
            );
            Some(format!("{}   {title}", ctx.format_shortcut(&shortcut)))
        })
        .collect();
    let max_size = egui::vec2(
        (ctx.content_rect().width() - 72.).max(1.),
        (ctx.content_rect().height() * 0.75 - 40.).max(1.),
    );
    let columns = if lines.len() > 8 || 64. + lines.len() as f32 * 24. > max_size.y {
        2
    } else {
        1
    };
    let per_column = lines.len().max(1).div_ceil(columns);
    let measure = |text: &str, size| {
        ui.painter()
            .layout_no_wrap(
                text.into(),
                egui::FontId::proportional(size),
                egui::Color32::WHITE,
            )
            .size()
            .x
    };
    let mut widths = vec![0_f32; columns];
    for (index, line) in lines.iter().enumerate() {
        widths[index / per_column] = widths[index / per_column].max(measure(line, 14.));
    }
    let heading = "Verfügbare Tastenkombinationen";
    let desired = egui::vec2(
        (widths.iter().sum::<f32>() + (columns - 1) as f32 * 36.)
            .max(measure(heading, 18.))
            .max(200.),
        64. + per_column as f32 * 24.,
    );
    let scale = (max_size.x / desired.x).min(max_size.y / desired.y).min(1.);
    let (rect, _) = ui.allocate_exact_size(desired * scale, egui::Sense::hover());
    let text = |position, text: &str, size| {
        ui.painter().text(
            position,
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::proportional(size * scale),
            egui::Color32::WHITE,
        )
    };
    text(rect.min, heading, 18.);
    text(
        rect.min + egui::vec2(0., 28. * scale),
        "Shift loslassen zum Ausblenden",
        12.,
    );
    let mut x = 0.;
    for (column, width) in widths.iter().enumerate() {
        for (row, line) in lines
            .iter()
            .skip(column * per_column)
            .take(per_column)
            .enumerate()
        {
            text(
                rect.min + egui::vec2(x, 64. + row as f32 * 24.) * scale,
                line,
                14.,
            );
        }
        x += width + 36.;
    }
    if lines.is_empty() {
        text(
            rect.min + egui::vec2(0., 64.) * scale,
            "Keine Tastenkombinationen verfügbar.",
            14.,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn help_uses_configured_keys_and_viewer_boundaries() {
        let mut suite = crate::presentation::layout_tests::test_suite();
        suite.items = ["20211115_102505.mp4", "20211115_102506.mp4"]
            .into_iter()
            .map(|name| {
                crate::domain::Media::from_path(
                    std::path::Path::new("/fixture").join(name),
                    "2021/11".into(),
                )
                .unwrap()
            })
            .collect();
        suite.selected = Some(0);
        let mut saved = suite.bindings.export();
        saved.get_mut("play_video").unwrap().as_mut().unwrap().key = "P".into();
        suite.bindings.restore(&saved).unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            let rows = suite.shortcut_help_rows(ctx);
            assert!(
                rows.iter()
                    .any(|(title, key)| title == "Video abspielen" && key.key == "P")
            );
            assert!(rows.iter().any(|(title, _)| title == "Nächstes Medium"));
            assert!(!rows.iter().any(|(title, _)| title == "Vorheriges Medium"));
        });
        suite.selected = Some(1);
        let _ = ctx.run(Default::default(), |ctx| {
            let rows = suite.shortcut_help_rows(ctx);
            assert!(rows.iter().any(|(title, _)| title == "Vorheriges Medium"));
            assert!(!rows.iter().any(|(title, _)| title == "Nächstes Medium"));
        });
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn overlay_content_fits_a_frame_capped_at_three_quarters_height() {
        for height in [400., 900.] {
            let suite = crate::presentation::layout_tests::test_suite();
            let ctx = egui::Context::default();
            ctx.style_mut(|style| style.animation_time = 0.);
            let mut frame = None;
            for _ in 0..4 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800., height),
                        )),
                        modifiers: egui::Modifiers::SHIFT,
                        ..Default::default()
                    },
                    |ctx| suite.shortcut_overlay(ctx),
                );
                frame = output.shapes.iter().find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Rect(rect) if rect.fill == egui::Color32::BLACK => {
                        Some(rect.rect)
                    }
                    _ => None,
                });
            }
            let frame = frame.expect("visible black overlay");
            assert!(frame.height() <= height * 0.75 + 1., "{frame:?}");
            assert!(frame.top() >= 0. && frame.bottom() <= height);
            assert!(frame.height() > 100., "overlay must grow to its content");
        }
    }
}

#[cfg(test)]
mod search_regression {
    use super::*;
    #[test]
    fn shift_help_returns_after_search_even_with_a_stale_native_shortcut_key() {
        let mut suite = crate::presentation::layout_tests::test_suite();
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.);
        suite.search_open = true;
        suite.focus_search = true;
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::F,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
                ..Default::default()
            },
            |ctx| suite.search_dialog(ctx),
        );
        // Also cover closing from outside the dialog's own close-button path.
        suite.search_open = false;
        let mut shown = false;
        for _ in 0..4 {
            let output = ctx.run(
                egui::RawInput {
                    modifiers: egui::Modifiers::SHIFT,
                    ..Default::default()
                },
                |ctx| {
                    suite.search_dialog(ctx);
                    suite.shortcut_overlay(ctx);
                },
            );
            shown |= output.shapes.iter().any(|shape| {
                matches!(&shape.shape,
                egui::epaint::Shape::Rect(rect) if rect.fill == egui::Color32::BLACK)
            });
        }
        assert!(shown, "Shift help must remain available after using search");
        assert_ne!(
            ctx.memory(|memory| memory.focused()),
            Some(egui::Id::new("library_search"))
        );
    }
}

#[cfg(test)]
mod columns_tests {
    use super::*;
    #[test]
    fn long_lists_paint_every_entry_in_two_columns_without_overflow() {
        for width in [360., 1200.] {
            let ctx = egui::Context::default();
            let rows: Vec<_> = (0..20)
                .map(|index| {
                    (
                        format!("Aktion {index:02}"),
                        Shortcut {
                            key: "Space".into(),
                            alt: false,
                            ctrl: false,
                            shift: false,
                            command: false,
                            mac_cmd: false,
                        },
                    )
                })
                .collect();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 400.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| paint_help(ui, ctx, &rows));
                },
            );
            let entries: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.job.text.contains("Aktion") => {
                        Some(text)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(entries.len(), 20);
            let first_x = entries[0].pos.x;
            assert_eq!(
                entries
                    .iter()
                    .filter(|text| (text.pos.x - first_x).abs() < 0.5)
                    .count(),
                10
            );
            for text in entries {
                let rect = text.visual_bounding_rect();
                assert!(rect.left() >= 0. && rect.right() <= width + 1., "{rect:?}");
                assert!(rect.top() >= 0. && rect.bottom() <= 300., "{rect:?}");
            }
        }
    }
}
