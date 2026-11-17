//! Pure painted overlays: shared compact video/tag badges without new hit targets.
use crate::theme;
use eframe::egui;
use std::sync::Arc;
const INSET: f32 = 4.;
const GAP: f32 = 3.;
struct Badge {
    galley: Option<Arc<egui::Galley>>,
    size: egui::Vec2,
}
fn badge(ui: &egui::Ui, label: &str, width: f32) -> Badge {
    let galley = theme::tag_galley(ui, label, width);
    let size = theme::tag_size(&galley);
    Badge {
        galley: Some(galley),
        size,
    }
}
fn used_width(row: &[Badge]) -> f32 {
    row.iter().map(|b| b.size.x).sum::<f32>() + GAP * row.len().saturating_sub(1) as f32
}
pub(super) fn paint_cloud(
    ui: &egui::Ui,
    image: egui::Rect,
    labels: &[&str],
    has_geo: bool,
) -> Option<egui::Rect> {
    if (labels.is_empty() && !has_geo) || image.width() < 24. || image.height() < 24. {
        return None;
    }
    let width = image.width() - 2. * INSET;
    let row_height = theme::BADGE_HEIGHT;
    let max_rows = (((image.height() * 0.5 - 2. * INSET + GAP) / (row_height + GAP)).floor()
        as usize)
        .clamp(1, 3);
    let mut rows: Vec<Vec<Badge>> = vec![Vec::new()];
    if has_geo {
        rows[0].push(Badge {
            galley: None,
            size: egui::vec2(theme::BADGE_HEIGHT, theme::BADGE_HEIGHT),
        });
    }
    let mut shown = 0;
    for label in labels {
        let next = badge(ui, label, width);
        if !rows.last().unwrap().is_empty()
            && used_width(rows.last().unwrap()) + GAP + next.size.x > width
        {
            if rows.len() == max_rows {
                break;
            }
            rows.push(Vec::new());
        }
        rows.last_mut().unwrap().push(next);
        shown += 1;
    }
    if shown < labels.len() {
        let last = rows.last_mut().unwrap();
        let mut hidden = labels.len() - shown;
        loop {
            let more = badge(ui, &format!("+{hidden}"), width);
            if last.is_empty() || used_width(last) + GAP + more.size.x <= width {
                last.push(more);
                break;
            }
            if last.len() == 1 && last[0].galley.is_none() {
                break;
            }
            last.pop();
            hidden += 1;
        }
    }
    let height = row_height * rows.len() as f32 + GAP * rows.len().saturating_sub(1) as f32;
    let top = image.bottom() - INSET - height;
    let painter = ui.painter_at(image);
    let mut geo_rect = None;
    for (index, row) in rows.iter().enumerate() {
        let mut x = image.left() + INSET;
        let y = top + index as f32 * (row_height + GAP);
        for badge in row {
            let rect = egui::Rect::from_min_size(
                egui::pos2(x, y + (row_height - badge.size.y) * 0.5),
                badge.size,
            );
            painter.rect_filled(rect, 3., theme::BADGE_BACKGROUND);
            if let Some(galley) = &badge.galley {
                let text_origin = theme::tag_text_origin(rect, galley);
                painter.galley(text_origin, galley.clone(), theme::TEXT);
            } else {
                geo_rect = Some(rect);
                let center = rect.center();
                let radius = 6.;
                let stroke = egui::Stroke::new(1_f32, theme::TEXT);
                painter.circle_stroke(center, radius, stroke);
                painter.line_segment(
                    [
                        center + egui::vec2(-radius, 0.),
                        center + egui::vec2(radius, 0.),
                    ],
                    stroke,
                );
                let meridian: Vec<_> = (0..=24)
                    .map(|i| {
                        let angle = i as f32 / 24. * std::f32::consts::TAU;
                        center + egui::vec2(angle.cos() * 3., angle.sin() * radius)
                    })
                    .collect();
                painter.add(egui::Shape::closed_line(meridian, stroke));
            }
            x += badge.size.x + GAP;
        }
    }
    geo_rect
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn video_and_tags_have_equal_ink_padding_and_stay_inside_the_image() {
        for width in [90., 130., 240.] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let image = egui::Rect::from_min_size(egui::pos2(10., 10.), egui::vec2(width, width));
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    paint_cloud(
                        ui,
                        image,
                        &[
                            "m",
                            "a",
                            "v",
                            "b",
                            "p",
                            "g",
                            "ÄÖÜ",
                            "Video",
                            "Familie",
                            "Sommer",
                            "Ein sehr sehr langer Tagname",
                            "Reise",
                            "Arbeit",
                        ],
                        false,
                    )
                });
            });
            let blocks: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(r) if r.fill == theme::BADGE_BACKGROUND => Some(r.rect),
                    _ => None,
                })
                .collect();
            assert!(!blocks.is_empty());
            for block in &blocks {
                assert!(image.contains_rect(*block));
                assert_eq!(block.height(), theme::BADGE_HEIGHT);
            }
            let mut texts = 0;
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    theme::assert_text_glyphs(&ctx, text);
                    let ink = text.visual_bounding_rect();
                    let block = blocks.iter().find(|rect| rect.contains_rect(ink)).unwrap();
                    for padding in [ink.left() - block.left(), block.right() - ink.right()] {
                        assert!(
                            (padding - theme::BADGE_PADDING).abs() < 0.1,
                            "unequal padding {padding}"
                        );
                    }
                    texts += 1;
                }
            }
            assert_eq!(texts, blocks.len());
            if width == 90. {
                assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.job.text.starts_with('+'))));
            }
        }
    }
    #[test]
    fn globe_badge_is_drawn_without_unicode_and_keeps_the_same_height_as_tags() {
        for width in [32., 90., 240.] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let image = egui::Rect::from_min_size(egui::pos2(10., 10.), egui::vec2(width, width));
            let mut globe = None;
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    globe = paint_cloud(ui, image, &["scan", "reise"], true);
                });
            });
            let globe = globe.expect("GPS badge remains visible even if tags overflow");
            assert!(image.contains_rect(globe));
            assert_eq!(
                globe.size(),
                egui::vec2(theme::BADGE_HEIGHT, theme::BADGE_HEIGHT)
            );
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|s| matches!(&s.shape,egui::Shape::Circle(c) if globe.contains(c.center)))
            );
            assert!(!output.shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text.contains('🌐'))
            ));
        }
    }
}
