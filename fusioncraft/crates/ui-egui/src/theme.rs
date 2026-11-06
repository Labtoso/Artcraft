//! Colours and widget style: the dark Craft-suite look (flat, rounded, blue accent).

use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const CHROME: Color32 = Color32::from_rgb(0x3b, 0x44, 0x53);
pub const CHROME_DARK: Color32 = Color32::from_rgb(0x31, 0x38, 0x44);
pub const PANEL: Color32 = Color32::from_rgb(0x2c, 0x33, 0x3d);
pub const CONTROL: Color32 = Color32::from_rgb(0x50, 0x5a, 0x6d);
pub const CONTROL_HOVER: Color32 = Color32::from_rgb(0x5c, 0x67, 0x7c);
pub const BORDER: Color32 = Color32::from_rgb(0x23, 0x29, 0x30);
pub const CANVAS: Color32 = Color32::from_rgb(0x22, 0x28, 0x2f);
pub const TEXT: Color32 = Color32::from_rgb(0xd7, 0xdb, 0xe0);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xa3, 0xaa, 0xb4);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x87, 0x8d, 0x96);
pub const ICON: Color32 = Color32::from_rgb(0xcf, 0xd4, 0xdb);
pub const ACCENT: Color32 = Color32::from_rgb(0x3d, 0x8b, 0xfd);
pub const SELECTION: Color32 = Color32::from_rgb(0x4a, 0x8f, 0xff);
pub const HOVER: Color32 = Color32::from_rgb(0x9c, 0xc6, 0xff);
pub const SNAP: Color32 = Color32::from_rgb(0x2f, 0xd0, 0x62);
pub const ERROR: Color32 = Color32::from_rgb(0xe5, 0x5b, 0x5b);
pub const WARNING: Color32 = Color32::from_rgb(0xf0, 0xb4, 0x3c);
pub const SKETCH: Color32 = Color32::from_rgb(0x9c, 0xc6, 0xff);
pub const SKETCH_ACTIVE: Color32 = Color32::from_rgb(0xf2, 0xf4, 0xf7);
pub const CONSTRUCTION: Color32 = Color32::from_rgb(0xf0, 0x7a, 0x3a);
pub const PROFILE: Color32 = Color32::from_rgba_premultiplied(0x2a, 0x5c, 0xa8, 0x70);
pub const PROFILE_SELECTED: Color32 = Color32::from_rgba_premultiplied(0x3d, 0x8b, 0xfd, 0xa0);
pub const PLANE: Color32 = Color32::from_rgba_premultiplied(0x40, 0x5a, 0x80, 0x50);
pub const PLANE_HOVER: Color32 = Color32::from_rgba_premultiplied(0x3d, 0x8b, 0xfd, 0x90);

pub const RADIUS: u8 = 6;

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    let r = CornerRadius::same(RADIUS);
    v.panel_fill = PANEL;
    v.window_fill = CHROME_DARK;
    v.extreme_bg_color = CANVAS;
    v.faint_bg_color = CHROME_DARK;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.selection.bg_fill = ACCENT.linear_multiply(0.6);
    v.selection.stroke = Stroke::new(1.0, TEXT);
    v.hyperlink_color = ACCENT;
    v.override_text_color = Some(TEXT);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    v.widgets.inactive.bg_fill = CONTROL;
    v.widgets.inactive.weak_bg_fill = CHROME;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.bg_fill = CONTROL_HOVER;
    v.widgets.hovered.weak_bg_fill = CONTROL_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, HOVER.linear_multiply(0.5));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = ACCENT;
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 5.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.spacing.interact_size.y = 24.0;
    });
}
