//! Shared gallery design tokens. Presentation only; no file or domain operations.
use eframe::egui::{self, Color32, FontId, RichText, Stroke, TextStyle};
pub const CANVAS: Color32 = Color32::from_rgb(19, 22, 28);
pub const SURFACE: Color32 = Color32::from_rgb(27, 31, 39);
pub const RAISED: Color32 = Color32::from_rgb(36, 42, 52);
pub const BORDER: Color32 = Color32::from_rgb(49, 57, 70);
pub const TEXT: Color32 = Color32::from_rgb(242, 245, 250);
pub const MUTED: Color32 = Color32::from_rgb(214, 222, 233);
pub const IMPORT_SUCCESS: Color32 = Color32::from_rgb(125, 211, 151);
pub const IMPORT_WARNING: Color32 = Color32::from_rgb(255, 215, 95);
pub const IMPORT_ERROR: Color32 = Color32::from_rgb(255, 145, 145);
pub const DANGER: Color32 = Color32::from_rgb(255, 125, 125);
pub const RECORD_RED: Color32 = Color32::from_rgb(255, 100, 105);
pub const LEVEL_GREEN: Color32 = Color32::from_rgb(125, 211, 151);
pub const LEVEL_YELLOW: Color32 = Color32::from_rgb(255, 215, 95);
pub const LEVEL_RED: Color32 = Color32::from_rgb(255, 125, 125);
pub const ACCENT: Color32 = Color32::from_rgb(139, 181, 255);
pub const BADGE_BACKGROUND: Color32 = Color32::from_black_alpha(220);
pub const BADGE_FONT_SIZE: f32 = 12.;
pub const BADGE_HEIGHT: f32 = 20.;
pub const SECTION_HEADING_SIZE: f32 = 18.;
pub const TRANSPORT_EDGE: f32 = 34.;
pub const BADGE_PADDING: f32 = 3.;
pub const SELECTED: Color32 = Color32::from_rgb(43, 67, 103);

pub const HOVER: Color32 = Color32::from_rgb(48, 59, 76);
pub const DISABLED_TEXT: Color32 = Color32::from_rgb(100, 110, 125);
pub const DISABLED_BG: Color32 = Color32::from_rgb(24, 28, 35);
pub const DISABLED_BORDER: Color32 = Color32::from_rgb(34, 40, 50);
pub const AUDIO_PENDING: Color32 = Color32::from_rgb(71, 77, 87);
pub const AUDIO_ERROR: Color32 = Color32::from_rgb(166, 64, 51);
pub const AUDIO_ODD: Color32 = Color32::from_rgb(92, 140, 201);
pub const AUDIO_EVEN: Color32 = Color32::from_rgb(59, 102, 166);

/// Shared transport geometry, border and interaction states, mirrored by native/theme.h.
pub fn transport_button(
    ui: &mut egui::Ui,
    enabled: bool,
    size: egui::Vec2,
    text: &str,
) -> egui::Response {
    ui.scope(|ui| {
        ui.visuals_mut().disabled_alpha = 1.;
        ui.visuals_mut().widgets.hovered.weak_bg_fill = HOVER;
        let visuals = ui.visuals_mut();
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
        ] {
            widget.expansion = 0.;
        }
        if !enabled {
            let visuals = ui.visuals_mut();
            visuals.override_text_color = Some(DISABLED_TEXT);
            for widget in [
                &mut visuals.widgets.inactive,
                &mut visuals.widgets.noninteractive,
            ] {
                widget.bg_fill = DISABLED_BG;
                widget.weak_bg_fill = DISABLED_BG;
                widget.bg_stroke = Stroke::new(1_f32, DISABLED_BORDER);
                widget.fg_stroke = Stroke::new(1_f32, DISABLED_TEXT);
            }
        }
        ui.add_enabled(
            enabled,
            egui::Button::new(text)
                .min_size(size)
                .wrap_mode(egui::TextWrapMode::Truncate),
        )
    })
    .inner
}
/// Audio wells share the transport button's normal outline and corners.
/// Keep samples/segments inside the rim without changing the allocated widget size.
pub fn audio_track(ui: &egui::Ui, rect: egui::Rect, fill: Color32, enabled: bool) -> egui::Rect {
    let visuals = &ui.visuals().widgets.inactive;
    ui.painter().rect(
        rect,
        visuals.corner_radius,
        fill,
        if enabled {
            visuals.bg_stroke
        } else {
            Stroke::new(1_f32, DISABLED_BORDER)
        },
        egui::StrokeKind::Inside,
    );
    rect.shrink((rect.width().min(rect.height()) * 0.25).min(2.))
}

pub fn transport_symbol(enabled: bool, record: bool) -> Color32 {
    if !enabled {
        DISABLED_TEXT
    } else if record {
        RECORD_RED
    } else {
        TEXT
    }
}

pub fn install(ctx: &egui::Context) {
    // Our gallery surfaces are always dark. Keep egui's selected style in sync,
    // including when a light OS theme arrives after application initialization.
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style()).clone();
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(20.));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.));
    style.spacing.item_spacing = egui::vec2(8., 4.);
    style.spacing.button_padding = egui::vec2(8., 3.);
    style.spacing.interact_size.y = 24.;
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.slider_width = 160.;
    style.visuals = egui::Visuals::dark();
    let v = &mut style.visuals;
    v.override_text_color = Some(TEXT);
    v.weak_text_color = Some(MUTED);
    v.disabled_alpha = 0.85;
    v.error_fg_color = Color32::from_rgb(255, 180, 180);
    v.warn_fg_color = Color32::from_rgb(255, 210, 140);
    v.panel_fill = CANVAS;
    v.window_fill = SURFACE;
    v.extreme_bg_color = CANVAS;
    v.faint_bg_color = RAISED;
    v.code_bg_color = CANVAS;
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = SELECTED;
    v.selection.stroke = Stroke::new(1_f32, TEXT);
    v.window_corner_radius = egui::CornerRadius::same(8);
    v.menu_corner_radius = egui::CornerRadius::same(6);
    v.window_stroke = Stroke::new(1_f32, BORDER);
    v.widgets.noninteractive.bg_fill = SURFACE;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1_f32, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1_f32, MUTED);
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.inactive.weak_bg_fill = RAISED;
    v.widgets.inactive.bg_stroke = Stroke::new(1_f32, BORDER);
    v.widgets.inactive.fg_stroke = Stroke::new(1_f32, TEXT);
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.weak_bg_fill = RAISED;
    v.widgets.hovered.bg_stroke = Stroke::new(1_f32, ACCENT);
    v.widgets.hovered.fg_stroke = Stroke::new(1_f32, TEXT);
    v.widgets.active.bg_fill = SELECTED;
    v.widgets.active.weak_bg_fill = SELECTED;
    v.widgets.active.bg_stroke = Stroke::new(1_f32, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1_f32, TEXT);
    v.widgets.open.bg_fill = SELECTED;
    v.widgets.open.weak_bg_fill = SELECTED;
    v.widgets.open.bg_stroke = Stroke::new(1_f32, ACCENT);
    v.widgets.open.fg_stroke = Stroke::new(1_f32, TEXT);
    for widget in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(5);
    }
    ctx.set_style(style);
}

pub fn panel(fill: Color32, margin: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::same(margin))
}
/// Top-level library action, framed and aligned like the year headers even when inactive.
pub fn root_navigation(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding.x = ui.spacing().indent;
        ui.add_sized(
            [ui.available_width(), 20.],
            egui::Button::new(RichText::new(label).size(14.).strong().color(TEXT))
                .right_text("")
                .selected(selected)
                .frame(true)
                .wrap_mode(egui::TextWrapMode::Truncate),
        )
    })
    .inner
    .on_hover_text(label)
}

pub fn navigation(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    navigation_row(ui, selected, label, 20.)
}
pub fn tag_navigation(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    navigation_row(ui, selected, label, BADGE_HEIGHT)
}
/// Same framed disclosure row for library years and countries; optional count reserves its space.
pub fn tree_header<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    label: &str,
    selected: bool,
    count: Option<usize>,
    open: Option<bool>,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::collapsing_header::CollapsingResponse<R> {
    let count = count.map(|count| {
        egui::WidgetText::from(RichText::new(count.to_string()).monospace().color(MUTED))
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Button,
            )
    });
    let reserve = count.as_ref().map_or(0., |g| g.size().x + 12.);
    let width =
        (ui.available_width() - ui.spacing().indent - ui.spacing().button_padding.x - reserve)
            .max(1.);
    let label_galley =
        egui::WidgetText::from(RichText::new(label).size(14.).strong().color(if selected {
            ACCENT
        } else {
            TEXT
        }))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            width,
            TextStyle::Button,
        );
    let mut result = egui::CollapsingHeader::new(label_galley)
        .id_salt(id)
        .default_open(false)
        .open(open)
        .show_background(true)
        .show(ui, body);
    if let Some(galley) = count {
        let rect = result.header_response.rect;
        let pos = egui::pos2(
            rect.right() - galley.size().x,
            rect.center().y - galley.size().y / 2.,
        );
        ui.painter().galley(pos, galley, MUTED);
    }
    result.header_response = result.header_response.on_hover_text(label);
    result
}
/// Count stays at the trailing edge while long place labels truncate independently.
pub fn place_navigation(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    count: usize,
) -> egui::Response {
    ui.add_sized([ui.available_width(),BADGE_HEIGHT],egui::Button::new(RichText::new(label).color(TEXT))
        .right_text(RichText::new(count.to_string()).monospace().color(MUTED))
        .selected(selected).frame(selected).wrap_mode(egui::TextWrapMode::Truncate))
        .on_hover_text(format!("{label} · {count} Medien im gewählten Zeitraum\nKlick: Ortsfilter umschalten. Mehrere Ortsfilter werden mit ODER verknüpft; Tags gelten zusätzlich. Pfeil: Unterebenen auf-/zuklappen. Rechtsklick auf ein Land: alle Orte auf-/zuklappen."))
}
fn navigation_row(ui: &mut egui::Ui, selected: bool, label: &str, height: f32) -> egui::Response {
    ui.add_sized(
        [ui.available_width(), height],
        egui::Button::new(RichText::new(label).color(TEXT))
            .right_text("")
            .selected(selected)
            .frame(selected)
            .wrap_mode(egui::TextWrapMode::Truncate),
    )
    .on_hover_text(label)
}

/// Loading indicators occupy one third of the image's shorter edge.
pub fn image_spinner(ui: &egui::Ui, rect: egui::Rect) {
    let side = rect.width().min(rect.height()) / 3.;
    egui::Spinner::new().paint_at(
        ui,
        egui::Rect::from_center_size(rect.center(), egui::vec2(side, side)),
    );
}
pub fn paint_image(ui: &mut egui::Ui, image: egui::Image<'_>, rect: egui::Rect) {
    let pending = matches!(
        image.load_for_size(ui.ctx(), rect.size()),
        Ok(egui::load::TexturePoll::Pending { .. })
    );
    image.show_loading_spinner(false).paint_at(ui, rect);
    if pending {
        image_spinner(ui, rect);
    }
}

pub fn import_log_color(level: photosuite_application::import::LogLevel) -> Color32 {
    use photosuite_application::import::LogLevel;
    match level {
        LogLevel::Neutral => TEXT,
        LogLevel::Success => IMPORT_SUCCESS,
        LogLevel::Warning => IMPORT_WARNING,
        LogLevel::Error => IMPORT_ERROR,
    }
}
/// Shared restrained card surface for related controls.
pub fn content_group<R>(
    ui: &mut egui::Ui,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1_f32, BORDER))
        .corner_radius(8)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui)
        })
}
/// Compact counterpart of content_group for information popovers.
pub const QUEUE_HEADER_HEIGHT: f32 = 26.;
pub const QUEUE_COLLAPSED_HEADER_HEIGHT: f32 = 20.;

pub fn compact_group<R>(
    ui: &mut egui::Ui,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    group_with_margin(ui, egui::Margin::same(8), body)
}
/// Preserve generous open cards; collapsed queue rows use about two thirds of their old height.
pub fn queue_step_group<R>(
    ui: &mut egui::Ui,
    collapsed: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    group_with_margin(
        ui,
        egui::Margin::symmetric(8, if collapsed { 4 } else { 8 }),
        body,
    )
}
fn group_with_margin<R>(
    ui: &mut egui::Ui,
    margin: egui::Margin,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(CANVAS)
        .stroke(Stroke::new(1_f32, BORDER))
        .corner_radius(6)
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui)
        })
}
/// Non-interactive, high-contrast status pill; all popup colors come from this theme.
pub fn status_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(RAISED)
        .stroke(Stroke::new(1_f32, BORDER))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().strong().color(color));
        });
}
pub fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(SECTION_HEADING_SIZE)
            .strong()
            .color(TEXT),
    );
}
/// Shared compact section strip; navigation and filter entries remain visually subordinate.
fn sidebar_section(ui: &mut egui::Ui, title: &str, reset: Option<bool>) -> bool {
    let section = egui::Frame::new()
        .fill(RAISED)
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(4, 3))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .size(SECTION_HEADING_SIZE)
                        .strong()
                        .color(ACCENT),
                );
                reset.is_some_and(|enabled| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled(
                            enabled,
                            egui::Button::new("Reset").min_size(egui::vec2(0., BADGE_HEIGHT)),
                        )
                        .clicked()
                    })
                    .inner
                })
            })
            .inner
        });
    let rect = section.response.rect;
    ui.painter().line_segment(
        [
            rect.left_top() + egui::vec2(1., 3.),
            rect.left_bottom() + egui::vec2(1., -3.),
        ],
        Stroke::new(2_f32, ACCENT),
    );
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1_f32, BORDER));
    section.inner
}
/// Filter heading with a stable trailing reset action across sidebar sections.
pub fn filter_heading(ui: &mut egui::Ui, title: &str, can_reset: bool) -> bool {
    sidebar_section(ui, title, Some(can_reset))
}
pub fn sidebar_heading(ui: &mut egui::Ui, text: &str) {
    sidebar_section(ui, text, None);
}
/// Shared single-line badge typography; glyph bounds determine width, never height.
pub fn tag_galley(ui: &egui::Ui, label: &str, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(
        label.into(),
        FontId::proportional(BADGE_FONT_SIZE),
        TEXT,
    );
    job.wrap.max_width = (width - 2. * BADGE_PADDING).max(1.);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}
pub fn tag_size(galley: &egui::Galley) -> egui::Vec2 {
    egui::vec2(
        galley.mesh_bounds.width() + 2. * BADGE_PADDING,
        BADGE_HEIGHT,
    )
}
pub fn tag_text_origin(rect: egui::Rect, galley: &egui::Galley) -> egui::Pos2 {
    egui::pos2(
        rect.left() + BADGE_PADDING - galley.mesh_bounds.min.x,
        rect.center().y - galley.size().y * 0.5,
    )
}
/// Interactive counterpart of photo badges, with one fixed compact height and common baseline.
pub fn tag_chip(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let galley = tag_galley(ui, label, ui.available_width());
    let size = tag_size(&galley);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            if selected {
                format!("{label}, ausgewählt")
            } else {
                label.into()
            },
        )
    });
    ui.painter()
        .rect_filled(rect, 3., if selected { SELECTED } else { BADGE_BACKGROUND });
    if selected {
        ui.painter().rect_stroke(
            rect,
            3.,
            Stroke::new(1_f32, ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter()
        .galley(tag_text_origin(rect, &galley), galley, TEXT);
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label)
}

#[cfg(test)]
pub fn assert_text_glyphs(ctx: &egui::Context, text: &egui::epaint::TextShape) {
    for section in &text.galley.job.sections {
        for character in text.galley.job.text[section.byte_range.clone()]
            .chars()
            .filter(|c| !c.is_control())
        {
            assert!(
                ctx.fonts_mut(|fonts| fonts.has_glyph(&section.format.font_id, character)),
                "Missing glyph {character:?} in {:?}",
                text.galley.job.text
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(color: Color32) -> f32 {
        let linear = |v: u8| {
            let c = v as f32 / 255.;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }
    #[test]
    fn text_and_secondary_labels_have_readable_contrast() {
        for (foreground, background) in [
            (TEXT, CANVAS),
            (TEXT, SELECTED),
            (MUTED, SURFACE),
            (MUTED, RAISED),
            (ACCENT, SURFACE),
            (IMPORT_SUCCESS, SURFACE),
            (IMPORT_WARNING, SURFACE),
            (IMPORT_ERROR, SURFACE),
        ] {
            let ratio = (luminance(foreground) + 0.05) / (luminance(background) + 0.05);
            assert!(ratio >= 7.0, "Insufficient text contrast: {ratio}");
        }
    }
    #[test]
    fn all_widget_states_and_secondary_text_keep_high_contrast() {
        let ctx = egui::Context::default();
        install(&ctx);
        let style = ctx.style();
        let v = &style.visuals;
        for widget in [
            &v.widgets.inactive,
            &v.widgets.hovered,
            &v.widgets.active,
            &v.widgets.open,
        ] {
            for color in [TEXT, MUTED] {
                let ratio = (luminance(color) + 0.05) / (luminance(widget.bg_fill) + 0.05);
                assert!(ratio >= 7., "Widget contrast: {ratio}");
            }
        }
        assert_eq!(v.weak_text_color, Some(MUTED));
        assert!(v.disabled_alpha >= 0.8);
        assert!(style.spacing.interact_size.y <= 24.);
        assert!(style.spacing.button_padding.y <= 3.);
    }
    #[test]
    fn rendered_text_stays_readable_when_macos_reports_a_light_theme() {
        for initial in [egui::Theme::Dark, egui::Theme::Light] {
            let ctx = egui::Context::default();
            let _ = ctx.run(
                egui::RawInput {
                    system_theme: Some(initial),
                    ..Default::default()
                },
                |_| {},
            );
            install(&ctx);
            for system_theme in [egui::Theme::Light, egui::Theme::Dark, egui::Theme::Light] {
                let output = ctx.run(
                    egui::RawInput {
                        system_theme: Some(system_theme),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .frame(panel(SURFACE, 8))
                            .show(ctx, |ui| {
                                ui.strong("Mediathek");
                                ui.heading("Sammlung");
                                egui::CollapsingHeader::new(RichText::new("2026").strong()).show(
                                    ui,
                                    |ui| {
                                        ui.label("September");
                                    },
                                );
                                ui.label("Dateiname");
                                ui.label(RichText::new("Suchhinweis").weak());
                                let _ = ui.button("Öffnen");
                                ui.menu_button("Ansicht", |ui| {
                                    ui.label("Optionen");
                                });
                                let _ = ui.checkbox(&mut false, "Quadratische Vorschau");
                            });
                    },
                );
                let mut year_found = false;
                for shape in output.shapes {
                    if let egui::Shape::Text(text) = shape.shape {
                        year_found |= text.galley.job.text == "2026";
                        for section in &text.galley.job.sections {
                            let color = text.override_text_color.unwrap_or_else(|| {
                                if section.format.color == Color32::PLACEHOLDER {
                                    text.fallback_color
                                } else {
                                    section.format.color
                                }
                            });
                            let ratio = (luminance(color) + 0.05) / (luminance(SURFACE) + 0.05);
                            assert!(
                                ratio >= 7.,
                                "Unreadable rendered text {:?}: {color:?}, contrast={ratio}, system={system_theme:?}",
                                text.galley.job.text
                            );
                        }
                    }
                }
                assert!(year_found, "The actual year header must be checked");
            }
        }
    }
}

#[cfg(test)]
mod tag_height_tests {
    use super::*;
    #[test]
    fn all_tag_letters_and_selection_states_share_height_and_text_baseline() {
        for scale in [1., 2.] {
            for selected in [false, true] {
                let ctx = egui::Context::default();
                install(&ctx);
                ctx.set_pixels_per_point(scale);
                let mut bounds = Vec::new();
                let output = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            for label in ["m", "a", "v", "b", "p", "g", "ÄÖÜ", "Video"] {
                                bounds.push((label, tag_chip(ui, label, selected).rect));
                            }
                        });
                    });
                });
                assert!(bounds.iter().all(|(_, rect)| rect.height() == BADGE_HEIGHT));
                let texts: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some(text),
                        _ => None,
                    })
                    .collect();
                assert_eq!(texts.len(), bounds.len());
                let y = texts[0].pos.y;
                for text in texts {
                    assert!(
                        (text.pos.y - y).abs() < 0.1,
                        "different logical text baseline"
                    );
                    let rect = bounds
                        .iter()
                        .find(|(label, _)| *label == text.galley.job.text)
                        .unwrap()
                        .1;
                    assert!(
                        rect.contains_rect(text.visual_bounding_rect()),
                        "glyph extends outside its tag: {}",
                        text.galley.job.text
                    );
                }
            }
        }
    }
}
