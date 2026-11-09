use eframe::egui::{self, Color32, FontId, Stroke};

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub background: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub subtle: Color32,
    pub line: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub selected_text: Color32,
}

impl Palette {
    pub fn new(light: bool) -> Self {
        let rgb = Color32::from_rgb;
        if light {
            Self {
                background: rgb(255, 255, 255),
                sidebar: rgb(242, 242, 245),
                surface: rgb(250, 250, 252),
                subtle: rgb(246, 246, 248),
                line: rgb(222, 222, 227),
                text: rgb(35, 35, 39),
                secondary: rgb(112, 112, 119),
                accent: rgb(0, 102, 218),
                selection: rgb(0, 102, 218),
                selected_text: Color32::WHITE,
            }
        } else {
            Self {
                background: rgb(30, 30, 32),
                sidebar: rgb(39, 39, 42),
                surface: rgb(35, 35, 38),
                subtle: rgb(35, 35, 37),
                line: rgb(57, 57, 61),
                text: rgb(239, 239, 242),
                secondary: rgb(161, 161, 169),
                accent: rgb(64, 156, 255),
                selection: rgb(29, 96, 177),
                selected_text: Color32::WHITE,
            }
        }
    }
}

pub(super) fn install(ctx: &egui::Context) {
    for (theme, light) in [(egui::Theme::Dark, false), (egui::Theme::Light, true)] {
        let p = Palette::new(light);
        let mut style = (*ctx.style_of(theme)).clone();
        style.visuals = if light {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.interact_size.y = 28.0;
        style.visuals.panel_fill = p.background;
        style.visuals.window_fill = p.surface;
        style.visuals.extreme_bg_color = p.background;
        style.visuals.faint_bg_color = p.subtle;
        style.visuals.override_text_color = Some(p.text);
        style.visuals.selection.bg_fill = p.selection;
        style.visuals.selection.stroke = Stroke::new(1.0, p.selected_text);
        style.visuals.window_corner_radius = 12.into();
        for widget in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = 6.into();
            widget.fg_stroke = Stroke::new(1.0, p.text);
            widget.bg_stroke = Stroke::new(1.0, p.line);
            widget.expansion = 0.0;
        }
        style.visuals.widgets.inactive.weak_bg_fill = p.surface;
        style.visuals.widgets.inactive.bg_fill = p.subtle;
        style.visuals.widgets.hovered.weak_bg_fill = p.line;
        style.visuals.widgets.active.weak_bg_fill = p.selection;
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(22.0));
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(11.0));
        ctx.set_style_of(theme, style);
    }
    ctx.set_theme(egui::Theme::Dark);
}
