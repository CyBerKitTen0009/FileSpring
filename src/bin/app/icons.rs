//! Original vector icons, painted at the display's native scale. No bundled symbol font.
use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Folder,
    File,
    Back,
    Up,
    Refresh,
    Search,
    History,
    Grid,
    Sun,
    Moon,
    Close,
    Link,
    Check,
}

pub(super) fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height()) / 20.0;
    let origin = rect.center() - Vec2::splat(10.0 * s);
    let pt = |x, y| origin + egui::vec2(x * s, y * s);
    let stroke = Stroke::new(1.45 * s, color);
    let line = |points: &[(f32, f32)]| {
        painter.add(egui::Shape::line(
            points.iter().map(|&(x, y)| pt(x, y)).collect(),
            stroke,
        ));
    };
    match icon {
        Icon::Folder => {
            let points = [
                (2., 5.),
                (8., 5.),
                (10., 7.),
                (18., 7.),
                (18., 16.),
                (2., 16.),
            ];
            painter.add(egui::Shape::convex_polygon(
                points.into_iter().map(|(x, y)| pt(x, y)).collect(),
                color.gamma_multiply(0.18),
                stroke,
            ));
            line(&[(2., 8.), (18., 8.)]);
        }
        Icon::File => {
            line(&[
                (5., 2.),
                (12., 2.),
                (16., 6.),
                (16., 18.),
                (5., 18.),
                (5., 2.),
            ]);
            line(&[(12., 2.), (12., 6.), (16., 6.)]);
            line(&[(8., 10.), (13., 10.)]);
            line(&[(8., 13.), (13., 13.)]);
        }
        Icon::Back => line(&[(12., 4.), (6., 10.), (12., 16.)]),
        Icon::Up => {
            line(&[(4., 10.), (10., 4.), (16., 10.)]);
            line(&[(10., 4.), (10., 17.)]);
        }
        Icon::Refresh => {
            line(&[
                (16., 7.),
                (14., 4.),
                (9., 3.),
                (5., 5.),
                (3., 10.),
                (5., 15.),
                (10., 17.),
                (15., 15.),
            ]);
            line(&[(16., 2.), (16., 7.), (11., 7.)]);
        }
        Icon::Search => {
            painter.circle_stroke(pt(8., 8.), 5.0 * s, stroke);
            line(&[(12., 12.), (17., 17.)]);
        }
        Icon::History => {
            painter.circle_stroke(pt(10., 10.), 7.0 * s, stroke);
            line(&[(10., 5.), (10., 10.), (13., 12.)]);
        }
        Icon::Grid => {
            for y in [3., 11.] {
                for x in [3., 11.] {
                    painter.rect_stroke(
                        Rect::from_min_size(pt(x, y), Vec2::splat(6. * s)),
                        1,
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
        Icon::Sun => {
            painter.circle_stroke(pt(10., 10.), 3.5 * s, stroke);
            for (x, y) in [
                (0., -1.),
                (1., 0.),
                (0., 1.),
                (-1., 0.),
                (0.7, 0.7),
                (-0.7, 0.7),
                (0.7, -0.7),
                (-0.7, -0.7),
            ] {
                line(&[(10. + x * 6., 10. + y * 6.), (10. + x * 8., 10. + y * 8.)]);
            }
        }
        Icon::Moon => line(&[
            (12., 2.),
            (6., 3.),
            (3., 7.),
            (3., 12.),
            (6., 16.),
            (11., 18.),
            (16., 15.),
            (18., 11.),
            (13., 12.),
            (9., 9.),
            (9., 5.),
            (12., 2.),
        ]),
        Icon::Close => {
            line(&[(5., 5.), (15., 15.)]);
            line(&[(15., 5.), (5., 15.)]);
        }
        Icon::Link => {
            line(&[(4., 13.), (4., 16.), (16., 16.), (16., 10.)]);
            line(&[(8., 12.), (16., 4.), (10., 4.)]);
            line(&[(16., 4.), (16., 10.)]);
        }
        Icon::Check => line(&[(4., 10.), (8., 14.), (16., 5.)]),
    }
}

pub(super) fn button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    color: Color32,
) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(28., 28.), Sense::click());
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, 6, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        paint(
            ui.painter(),
            rect.shrink(5.),
            icon,
            if ui.is_enabled() {
                color
            } else {
                color.gamma_multiply(0.35)
            },
        );
        response.on_hover_text(label)
    })
    .inner
}

pub(super) fn label(ui: &mut Ui, icon: Icon, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(20., 20.), Sense::hover());
    paint(ui.painter(), rect, icon, color);
}

pub(super) fn text(painter: &egui::Painter, pos: Pos2, text: &str, size: f32, color: Color32) {
    painter.text(
        pos,
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(size),
        color,
    );
}
