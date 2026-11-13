use crate::theme;
use eframe::egui;
#[derive(Clone, Copy)]
pub(super) enum Icon {
    Previous,
    Next,
    Reveal,
    Tag,
    Record,
    RotateLeft,
    RotateRight,
    Reset,
    Play,
    Reload,
}
pub(super) fn button(ui: &mut egui::Ui, icon: Icon, enabled: bool, help: &str) -> egui::Response {
    let response = theme::transport_button(ui, enabled, egui::vec2(34., 34.), "");
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, help));
    let c = response.rect.center();
    let color = theme::transport_symbol(enabled, matches!(icon, Icon::Record));
    let p = ui.painter();
    let stroke = egui::Stroke::new(1.5_f32, color);
    let point = |x, y| c + egui::vec2(x, y);
    match icon {
        Icon::Previous | Icon::Next | Icon::Play => {
            let d = if matches!(icon, Icon::Previous) {
                -1.
            } else {
                1.
            };
            p.add(egui::Shape::convex_polygon(
                vec![point(-4. * d, -6.), point(6. * d, 0.), point(-4. * d, 6.)],
                color,
                egui::Stroke::NONE,
            ));
        }
        Icon::Record => {
            p.circle_filled(c, 6., color);
        }
        Icon::Tag => {
            p.add(egui::Shape::closed_line(
                vec![
                    point(-7., -7.),
                    point(0., -7.),
                    point(8., 1.),
                    point(1., 8.),
                    point(-7., 0.),
                ],
                stroke,
            ));
            p.circle_stroke(point(-3., -3.), 1.5, stroke);
        }
        Icon::Reveal => {
            p.add(egui::Shape::closed_line(
                vec![
                    point(-8., -5.),
                    point(-3., -5.),
                    point(-1., -2.),
                    point(7., -2.),
                    point(7., 7.),
                    point(-8., 7.),
                ],
                stroke,
            ));
            p.circle_stroke(point(3., 0.), 4., stroke);
            p.line_segment([point(6., 3.), point(9., 6.)], stroke);
        }
        Icon::RotateLeft | Icon::RotateRight | Icon::Reset | Icon::Reload => {
            let d = if matches!(icon, Icon::RotateLeft | Icon::Reset) {
                -1.
            } else {
                1.
            };
            let points: Vec<_> = (0..25)
                .map(|i| {
                    let a = (0.3 + i as f32 / 24. * 1.5) * std::f32::consts::PI;
                    point(a.cos() * 7. * d, a.sin() * 7.)
                })
                .collect();
            let end = points[24];
            let tangent = (end - points[23]).normalized();
            let normal = egui::vec2(-tangent.y, tangent.x);
            p.add(egui::Shape::line(points, stroke));
            p.line_segment([end, end - tangent * 4. + normal * 3.], stroke);
            p.line_segment([end, end - tangent * 4. - normal * 3.], stroke);
            if matches!(icon, Icon::Reset) {
                p.circle_filled(c, 1.5, color);
            }
        }
    }
    response.on_hover_text(help)
}
