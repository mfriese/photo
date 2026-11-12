//! Pure painted overlays: shared compact video/tag badges without new hit targets.
use crate::theme;
use eframe::egui;
use std::sync::Arc;
const INSET: f32 = 4.;
const GAP: f32 = 3.;
struct Badge {
    galley: Arc<egui::Galley>,
    size: egui::Vec2,
}
fn badge(ui: &egui::Ui, label: &str, width: f32) -> Badge {
    let mut job = egui::text::LayoutJob::simple_singleline(
        label.into(),
        egui::FontId::proportional(12.),
        theme::TEXT,
    );
    job.wrap.max_width = (width - 2. * theme::BADGE_PADDING).max(1.);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let size = galley.mesh_bounds.size() + egui::Vec2::splat(2. * theme::BADGE_PADDING);
    Badge { galley, size }
}
fn used_width(row: &[Badge]) -> f32 {
    row.iter().map(|b| b.size.x).sum::<f32>() + GAP * row.len().saturating_sub(1) as f32
}
pub(super) fn paint_cloud(ui: &egui::Ui, image: egui::Rect, labels: &[&str]) {
    if labels.is_empty() || image.width() < 24. || image.height() < 24. {
        return;
    }
    let width = image.width() - 2. * INSET;
    let row_height = ui.fonts_mut(|fonts| fonts.row_height(&egui::FontId::proportional(12.)))
        + 2. * theme::BADGE_PADDING;
    let max_rows = (((image.height() * 0.5 - 2. * INSET + GAP) / (row_height + GAP)).floor()
        as usize)
        .clamp(1, 3);
    let mut rows: Vec<Vec<Badge>> = vec![Vec::new()];
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
            last.pop();
            hidden += 1;
        }
    }
    let height = row_height * rows.len() as f32 + GAP * rows.len().saturating_sub(1) as f32;
    let top = image.bottom() - INSET - height;
    let painter = ui.painter_at(image);
    for (index, row) in rows.iter().enumerate() {
        let mut x = image.left() + INSET;
        let y = top + index as f32 * (row_height + GAP);
        for badge in row {
            let rect = egui::Rect::from_min_size(
                egui::pos2(x, y + (row_height - badge.size.y) * 0.5),
                badge.size,
            );
            painter.rect_filled(rect, 3., theme::BADGE_BACKGROUND);
            let text_origin = rect.min + egui::Vec2::splat(theme::BADGE_PADDING)
                - badge.galley.mesh_bounds.min.to_vec2();
            painter.galley(text_origin, badge.galley.clone(), theme::TEXT);
            x += badge.size.x + GAP;
        }
    }
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
                            "Video",
                            "Familie",
                            "Sommer",
                            "Ein sehr sehr langer Tagname",
                            "Reise",
                            "Arbeit",
                        ],
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
            }
            let mut texts = 0;
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    theme::assert_text_glyphs(&ctx, text);
                    let ink = text.visual_bounding_rect();
                    let block = blocks.iter().find(|rect| rect.contains_rect(ink)).unwrap();
                    for padding in [
                        ink.left() - block.left(),
                        block.right() - ink.right(),
                        ink.top() - block.top(),
                        block.bottom() - ink.bottom(),
                    ] {
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
}
