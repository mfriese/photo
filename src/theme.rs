//! Shared gallery design tokens. Presentation only; no file or domain operations.
use eframe::egui::{self, Color32, FontId, RichText, Stroke, TextStyle};
pub const CANVAS: Color32 = Color32::from_rgb(19, 22, 28);
pub const SURFACE: Color32 = Color32::from_rgb(27, 31, 39);
pub const RAISED: Color32 = Color32::from_rgb(36, 42, 52);
pub const BORDER: Color32 = Color32::from_rgb(49, 57, 70);
pub const TEXT: Color32 = Color32::from_rgb(242, 245, 250);
pub const MUTED: Color32 = Color32::from_rgb(214, 222, 233);
pub const ACCENT: Color32 = Color32::from_rgb(139, 181, 255);
pub const SELECTED: Color32 = Color32::from_rgb(43, 67, 103);

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
    v.widgets.hovered.bg_fill = Color32::from_rgb(48, 59, 76);
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

pub fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.).strong().color(MUTED));
}
pub fn panel(fill: Color32, margin: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::same(margin))
}
pub fn navigation(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    ui.add_sized(
        [ui.available_width(), 26.],
        egui::Button::new(RichText::new(label).color(TEXT))
            .right_text("")
            .selected(selected)
            .frame(selected)
            .wrap_mode(egui::TextWrapMode::Truncate),
    )
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
