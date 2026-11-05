//! Dark and light themes.

use crate::core::Settings;
use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(0x26, 0x80, 0xEB);

pub fn apply(ctx: &egui::Context, s: &Settings) {
    let mut v = if s.dark { Visuals::dark() } else { Visuals::light() };
    if s.dark {
        v.panel_fill = Color32::from_gray(50);
        v.window_fill = Color32::from_gray(44);
        v.extreme_bg_color = Color32::from_gray(30);
        v.faint_bg_color = Color32::from_gray(56);
        v.widgets.noninteractive.bg_fill = Color32::from_gray(50);
        v.widgets.inactive.bg_fill = Color32::from_gray(62);
        v.widgets.inactive.weak_bg_fill = Color32::from_gray(58);
        v.widgets.hovered.weak_bg_fill = Color32::from_gray(72);
    } else {
        v.panel_fill = Color32::from_gray(238);
        v.window_fill = Color32::from_gray(244);
        v.extreme_bg_color = Color32::WHITE;
    }
    v.selection.bg_fill = ACCENT.gamma_multiply(if s.dark { 0.55 } else { 0.35 });
    v.selection.stroke = Stroke::new(1.0, if s.dark { Color32::WHITE } else { Color32::from_rgb(0x0b, 0x4f, 0xa8) });
    v.hyperlink_color = ACCENT;
    v.window_corner_radius = CornerRadius::same(6);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(3);
    }
    ctx.set_visuals(v);
    ctx.set_zoom_factor(s.ui_scale.clamp(0.5, 3.0));
}
