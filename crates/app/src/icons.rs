//! Original vector icons drawn with the egui painter (no bitmap assets).

use crate::tools::Tool;
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Tool(Tool),
    NewLayer,
    NewFolder,
    Delete,
    Eye,
    EyeOff,
    Lock,
    Unlock,
    Outline,
    Keyframe,
    BlankKeyframe,
    Frame,
    Play,
    Stop,
    First,
    Last,
    StepBack,
    StepForward,
    Loop,
    Onion,
    Camera,
    Parent,
    Graph,
    Layer,
    Folder,
    Mask,
    Masked,
    Guide,
    Armature,
    Symbol,
    Bitmap,
    Sound,
    AutoKey,
    Swap,
    Plus,
}

fn line(p: &Painter, a: Pos2, b: Pos2, s: Stroke) {
    p.line_segment([a, b], s);
}

/// Paint an icon into `r` with color `c`.
pub fn paint(p: &Painter, r: Rect, icon: Icon, c: Color32) {
    let s = Stroke::new(1.4, c);
    let thin = Stroke::new(1.0, c);
    let cx = r.center();
    let w = r.width().min(r.height());
    let u = w / 16.0;
    let at = |x: f32, y: f32| pos2(cx.x + (x - 8.0) * u, cx.y + (y - 8.0) * u);
    match icon {
        Icon::Tool(t) => tool_icon(p, t, &at, c, s, thin, u),
        Icon::NewLayer | Icon::Plus => {
            p.rect_stroke(Rect::from_min_max(at(2.5, 2.5), at(13.5, 13.5)), 1.0, thin, egui::StrokeKind::Middle);
            line(p, at(8.0, 5.0), at(8.0, 11.0), s);
            line(p, at(5.0, 8.0), at(11.0, 8.0), s);
        }
        Icon::NewFolder | Icon::Folder => {
            let pts = vec![at(1.5, 4.0), at(6.0, 4.0), at(7.5, 5.5), at(14.5, 5.5), at(14.5, 13.0), at(1.5, 13.0)];
            p.add(Shape::closed_line(pts, thin));
            if icon == Icon::NewFolder {
                line(p, at(8.0, 7.0), at(8.0, 12.0), s);
                line(p, at(5.5, 9.5), at(10.5, 9.5), s);
            }
        }
        Icon::Delete => {
            line(p, at(3.0, 4.0), at(13.0, 4.0), s);
            line(p, at(6.0, 4.0), at(6.5, 2.5), thin);
            line(p, at(10.0, 4.0), at(9.5, 2.5), thin);
            p.add(Shape::closed_line(vec![at(4.0, 4.5), at(12.0, 4.5), at(11.0, 14.0), at(5.0, 14.0)], thin));
            line(p, at(7.0, 6.5), at(7.0, 12.0), thin);
            line(p, at(9.0, 6.5), at(9.0, 12.0), thin);
        }
        Icon::Eye | Icon::EyeOff => {
            let pts: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let t = i as f32 / 20.0 * std::f32::consts::TAU;
                    at(8.0 + 6.0 * t.cos(), 8.0 + 3.5 * t.sin())
                })
                .collect();
            p.add(Shape::closed_line(pts, thin));
            p.circle_filled(at(8.0, 8.0), 1.8 * u, c);
            if icon == Icon::EyeOff {
                line(p, at(2.5, 13.5), at(13.5, 2.5), s);
            }
        }
        Icon::Lock | Icon::Unlock => {
            p.rect_filled(Rect::from_min_max(at(4.0, 7.5), at(12.0, 14.0)), 1.0, c);
            let arc: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let t = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                    at(8.0 + 3.0 * t.cos(), 7.5 + 4.0 * t.sin())
                })
                .collect();
            let arc = if icon == Icon::Unlock { arc.into_iter().map(|q| q + vec2(3.0 * u, -1.0 * u)).collect() } else { arc };
            p.add(Shape::line(arc, s));
        }
        Icon::Outline => {
            p.rect_stroke(Rect::from_min_max(at(4.0, 3.0), at(12.0, 13.0)), 0.0, s, egui::StrokeKind::Middle);
        }
        Icon::Keyframe => {
            p.rect_stroke(Rect::from_min_max(at(3.0, 3.0), at(13.0, 13.0)), 1.0, thin, egui::StrokeKind::Middle);
            p.circle_filled(at(8.0, 8.0), 2.5 * u, c);
        }
        Icon::BlankKeyframe => {
            p.rect_stroke(Rect::from_min_max(at(3.0, 3.0), at(13.0, 13.0)), 1.0, thin, egui::StrokeKind::Middle);
            p.circle_stroke(at(8.0, 8.0), 2.5 * u, thin);
        }
        Icon::Frame => {
            p.rect_stroke(Rect::from_min_max(at(3.0, 3.0), at(13.0, 13.0)), 1.0, thin, egui::StrokeKind::Middle);
            p.rect_filled(Rect::from_min_max(at(8.0, 3.0), at(13.0, 13.0)), 0.0, c.gamma_multiply(0.6));
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(vec![at(5.0, 3.0), at(13.0, 8.0), at(5.0, 13.0)], c, Stroke::NONE));
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_min_max(at(4.5, 3.5), at(7.0, 12.5)), 0.0, c);
            p.rect_filled(Rect::from_min_max(at(9.0, 3.5), at(11.5, 12.5)), 0.0, c);
        }
        Icon::First | Icon::Last | Icon::StepBack | Icon::StepForward => {
            let flip = matches!(icon, Icon::First | Icon::StepBack);
            let fx = |x: f32| if flip { 16.0 - x } else { x };
            let tri = vec![at(fx(4.0), 4.0), at(fx(10.0), 8.0), at(fx(4.0), 12.0)];
            p.add(Shape::convex_polygon(if flip { tri.into_iter().rev().collect() } else { tri }, c, Stroke::NONE));
            if matches!(icon, Icon::First | Icon::Last) {
                p.rect_filled(Rect::from_two_pos(at(fx(11.0), 4.0), at(fx(12.5), 12.0)), 0.0, c);
            }
        }
        Icon::Loop => {
            let arc: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let t = std::f32::consts::PI * 0.2 + i as f32 / 16.0 * std::f32::consts::PI * 1.6;
                    at(8.0 + 5.0 * t.cos(), 8.0 + 4.5 * t.sin())
                })
                .collect();
            let end = *arc.last().unwrap();
            p.add(Shape::line(arc, s));
            p.add(Shape::convex_polygon(vec![end + vec2(-2.5 * u, -1.5 * u), end + vec2(2.0 * u, -1.0 * u), end + vec2(0.0, 2.5 * u)], c, Stroke::NONE));
        }
        Icon::Onion => {
            p.circle_stroke(at(6.0, 8.0), 4.0 * u, thin);
            p.circle_stroke(at(10.0, 8.0), 4.0 * u, Stroke::new(1.0, c.gamma_multiply(0.6)));
            p.circle_filled(at(8.0, 8.0), 3.5 * u, c);
        }
        Icon::Camera => {
            p.rect_stroke(Rect::from_min_max(at(1.5, 5.0), at(10.5, 12.0)), 1.0, thin, egui::StrokeKind::Middle);
            p.add(Shape::convex_polygon(vec![at(11.0, 8.5), at(14.5, 5.5), at(14.5, 11.5)], c, Stroke::NONE));
        }
        Icon::Parent => {
            p.rect_stroke(Rect::from_min_max(at(2.0, 2.0), at(7.0, 6.0)), 0.0, thin, egui::StrokeKind::Middle);
            p.rect_stroke(Rect::from_min_max(at(9.0, 10.0), at(14.0, 14.0)), 0.0, thin, egui::StrokeKind::Middle);
            p.add(Shape::line(vec![at(4.5, 6.0), at(4.5, 12.0), at(9.0, 12.0)], thin));
        }
        Icon::Graph => {
            line(p, at(2.0, 14.0), at(14.0, 14.0), thin);
            line(p, at(2.0, 14.0), at(2.0, 2.0), thin);
            let pts: Vec<Pos2> = (0..=12).map(|i| {
                let t = i as f32 / 12.0;
                at(2.0 + 12.0 * t, 14.0 - 12.0 * (t * t * (3.0 - 2.0 * t)))
            }).collect();
            p.add(Shape::line(pts, s));
        }
        Icon::Layer => {
            p.add(Shape::closed_line(vec![at(2.0, 6.0), at(8.0, 3.0), at(14.0, 6.0), at(8.0, 9.0)], thin));
            p.add(Shape::line(vec![at(2.0, 9.5), at(8.0, 12.5), at(14.0, 9.5)], thin));
        }
        Icon::Mask => {
            p.add(Shape::closed_line(vec![at(2.0, 6.0), at(8.0, 3.0), at(14.0, 6.0), at(8.0, 9.0)], thin));
            p.circle_filled(at(8.0, 6.0), 1.8 * u, c);
        }
        Icon::Masked => {
            p.add(Shape::closed_line(vec![at(2.0, 6.0), at(8.0, 3.0), at(14.0, 6.0), at(8.0, 9.0)], Stroke::new(1.0, c.gamma_multiply(0.5))));
            line(p, at(4.0, 12.0), at(12.0, 12.0), thin);
        }
        Icon::Guide => {
            for i in 0..4 {
                let x = 2.0 + i as f32 * 3.5;
                line(p, at(x, 12.0 - i as f32 * 2.0), at(x + 2.0, 11.0 - i as f32 * 2.0), s);
            }
        }
        Icon::Armature => {
            p.circle_stroke(at(4.0, 12.0), 1.8 * u, thin);
            p.add(Shape::convex_polygon(vec![at(4.0, 12.0), at(9.0, 6.0), at(12.5, 3.5), at(7.0, 8.5)], c.gamma_multiply(0.6), thin));
        }
        Icon::Symbol => {
            p.add(Shape::convex_polygon(vec![at(8.0, 2.0), at(14.0, 8.0), at(8.0, 14.0), at(2.0, 8.0)], c.gamma_multiply(0.3), thin));
        }
        Icon::Bitmap => {
            p.rect_stroke(Rect::from_min_max(at(2.0, 3.0), at(14.0, 13.0)), 0.0, thin, egui::StrokeKind::Middle);
            p.add(Shape::convex_polygon(vec![at(3.0, 12.0), at(7.0, 7.0), at(10.0, 10.0), at(13.0, 12.0)], c.gamma_multiply(0.6), Stroke::NONE));
            p.circle_filled(at(11.0, 6.0), 1.3 * u, c);
        }
        Icon::Sound => {
            p.add(Shape::convex_polygon(vec![at(2.0, 6.0), at(5.0, 6.0), at(9.0, 3.0), at(9.0, 13.0), at(5.0, 10.0), at(2.0, 10.0)], c, Stroke::NONE));
            line(p, at(11.0, 6.0), at(11.5, 10.0), thin);
            line(p, at(13.0, 4.5), at(13.5, 11.5), thin);
        }
        Icon::AutoKey => {
            p.rect_stroke(Rect::from_min_max(at(2.0, 4.0), at(12.0, 14.0)), 1.0, thin, egui::StrokeKind::Middle);
            p.circle_filled(at(7.0, 9.0), 2.0 * u, c);
            p.text(at(13.0, 4.0), egui::Align2::CENTER_CENTER, "A", egui::FontId::proportional(7.0 * u), c);
        }
        Icon::Swap => {
            line(p, at(3.0, 5.0), at(13.0, 5.0), s);
            p.add(Shape::convex_polygon(vec![at(13.0, 2.5), at(15.0, 5.0), at(13.0, 7.5)], c, Stroke::NONE));
            line(p, at(3.0, 11.0), at(13.0, 11.0), s);
            p.add(Shape::convex_polygon(vec![at(3.0, 8.5), at(3.0, 13.5), at(1.0, 11.0)], c, Stroke::NONE));
        }
    }
}

fn tool_icon(p: &Painter, t: Tool, at: &dyn Fn(f32, f32) -> Pos2, c: Color32, s: Stroke, thin: Stroke, u: f32) {
    match t {
        Tool::Select => {
            p.add(Shape::convex_polygon(vec![at(4.0, 2.0), at(4.0, 13.5), at(7.0, 10.5), at(9.5, 14.5), at(11.0, 13.8), at(8.6, 9.8), at(12.5, 9.8)], c, Stroke::NONE));
        }
        Tool::Subselect => {
            p.add(Shape::closed_line(vec![at(4.0, 2.0), at(4.0, 13.5), at(7.0, 10.5), at(9.5, 14.5), at(11.0, 13.8), at(8.6, 9.8), at(12.5, 9.8)], thin));
        }
        Tool::FreeTransform => {
            p.rect_stroke(Rect::from_min_max(at(3.0, 3.0), at(13.0, 13.0)), 0.0, thin, egui::StrokeKind::Middle);
            for (x, y) in [(3.0, 3.0), (13.0, 3.0), (3.0, 13.0), (13.0, 13.0), (8.0, 3.0), (8.0, 13.0)] {
                p.rect_filled(Rect::from_center_size(at(x, y), egui::vec2(3.0 * u, 3.0 * u)), 0.0, c);
            }
        }
        Tool::GradientTransform => {
            p.rect_stroke(Rect::from_min_max(at(2.0, 3.0), at(14.0, 13.0)), 0.0, thin, egui::StrokeKind::Middle);
            for i in 0..6 {
                let x = 2.5 + i as f32 * 2.0;
                p.rect_filled(Rect::from_min_max(at(x, 3.5), at(x + 2.0, 12.5)), 0.0, c.gamma_multiply(i as f32 / 6.0));
            }
        }
        Tool::Lasso => {
            let pts: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let a = i as f32 / 20.0 * std::f32::consts::TAU;
                    at(8.5 + 5.5 * a.cos(), 6.5 + 4.0 * a.sin())
                })
                .collect();
            p.add(Shape::line(pts, thin));
            p.add(Shape::line(vec![at(4.5, 9.0), at(4.0, 12.0), at(6.0, 14.5)], thin));
        }
        Tool::Pen => {
            p.add(Shape::convex_polygon(vec![at(8.0, 2.0), at(12.5, 9.0), at(9.5, 13.0), at(6.5, 13.0), at(3.5, 9.0)], c.gamma_multiply(0.25), thin));
            line(p, at(8.0, 2.0), at(8.0, 8.5), thin);
            p.circle_filled(at(8.0, 9.0), 1.2 * u, c);
            line(p, at(6.5, 14.5), at(9.5, 14.5), s);
        }
        Tool::Pencil => {
            p.add(Shape::convex_polygon(vec![at(3.0, 13.0), at(4.0, 10.0), at(12.0, 2.0), at(14.0, 4.0), at(6.0, 12.0)], c.gamma_multiply(0.35), thin));
            line(p, at(3.0, 13.0), at(4.5, 11.5), s);
        }
        Tool::Brush | Tool::ClassicBrush => {
            p.add(Shape::convex_polygon(vec![at(9.0, 7.0), at(14.0, 2.0), at(15.0, 3.0), at(10.0, 8.0)], c, Stroke::NONE));
            let blob = vec![at(9.0, 7.0), at(10.0, 8.0), at(8.5, 11.0), at(5.0, 14.0), at(2.0, 14.5), at(3.5, 11.0), at(6.5, 8.0)];
            p.add(Shape::convex_polygon(blob, if t == Tool::Brush { c } else { c.gamma_multiply(0.5) }, thin));
        }
        Tool::Eraser => {
            p.add(Shape::convex_polygon(vec![at(2.0, 10.0), at(9.0, 3.0), at(14.0, 8.0), at(7.0, 15.0)], c.gamma_multiply(0.3), thin));
            line(p, at(5.5, 6.5), at(10.5, 11.5), thin);
        }
        Tool::Line => line(p, at(3.0, 13.0), at(13.0, 3.0), s),
        Tool::Rectangle => {
            p.rect(Rect::from_min_max(at(2.5, 3.5), at(13.5, 12.5)), 0.0, c.gamma_multiply(0.35), thin, egui::StrokeKind::Middle);
        }
        Tool::Oval => {
            p.circle(at(8.0, 8.0), 5.5 * u, c.gamma_multiply(0.35), thin);
        }
        Tool::PolyStar => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 10.0;
                    let r = if i % 2 == 0 { 6.5 } else { 3.0 };
                    at(8.0 + r * a.cos(), 8.5 + r * a.sin())
                })
                .collect();
            p.add(Shape::closed_line(pts, thin));
        }
        Tool::PaintBucket => {
            p.add(Shape::convex_polygon(vec![at(2.0, 8.0), at(7.0, 3.0), at(12.0, 8.0), at(7.0, 13.0)], c.gamma_multiply(0.3), thin));
            p.add(Shape::convex_polygon(vec![at(12.0, 8.0), at(14.5, 11.0), at(13.0, 13.5), at(11.5, 11.0)], c, Stroke::NONE));
        }
        Tool::InkBottle => {
            p.rect_stroke(Rect::from_min_max(at(4.0, 7.0), at(12.0, 14.0)), 1.0, thin, egui::StrokeKind::Middle);
            p.rect_filled(Rect::from_min_max(at(6.0, 3.0), at(10.0, 7.0)), 0.0, c);
        }
        Tool::Eyedropper => {
            line(p, at(3.0, 13.0), at(10.0, 6.0), s);
            p.add(Shape::convex_polygon(vec![at(9.0, 5.0), at(12.0, 2.0), at(14.0, 4.0), at(11.0, 7.0)], c, Stroke::NONE));
        }
        Tool::Text => {
            p.text(at(8.0, 8.0), egui::Align2::CENTER_CENTER, "T", egui::FontId::proportional(14.0 * u), c);
        }
        Tool::Bone => {
            p.circle_stroke(at(4.0, 12.0), 2.0 * u, thin);
            p.add(Shape::convex_polygon(vec![at(4.0, 12.0), at(9.0, 6.0), at(13.0, 3.0), at(7.0, 9.0)], c.gamma_multiply(0.5), thin));
        }
        Tool::AssetWarp => {
            for i in 0..3 {
                let y = 4.0 + i as f32 * 4.0;
                let pts: Vec<Pos2> = (0..=8).map(|k| at(2.0 + k as f32 * 1.5, y + (k as f32 * 0.8).sin() * 1.2)).collect();
                p.add(Shape::line(pts, thin));
            }
            p.circle_filled(at(8.0, 8.0), 1.8 * u, c);
        }
        Tool::Camera => paint(p, Rect::from_center_size(at(8.0, 8.0), egui::vec2(16.0 * u, 16.0 * u)), Icon::Camera, c),
        Tool::Hand => {
            for (i, h) in [8.0f32, 10.0, 10.5, 9.5].iter().enumerate() {
                let x = 4.5 + i as f32 * 2.3;
                line(p, at(x, 9.0), at(x, 13.0 - h), Stroke::new(1.8 * u, c));
            }
            p.rect_filled(Rect::from_min_max(at(3.8, 8.5), at(12.4, 14.0)), 2.0, c);
            line(p, at(3.8, 10.0), at(2.0, 7.5), Stroke::new(1.8 * u, c));
        }
        Tool::Zoom => {
            p.circle_stroke(at(7.0, 7.0), 4.5 * u, s);
            line(p, at(10.2, 10.2), at(14.0, 14.0), Stroke::new(2.0 * u, c));
        }
    }
}

/// An icon button of `size` points; returns the response.
pub fn button(ui: &mut egui::Ui, icon: Icon, selected: bool, tooltip: &str, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let visuals = ui.style().interact_selectable(&resp, selected);
    if selected || resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, visuals.weak_bg_fill);
    }
    let col = if selected { ui.visuals().selection.stroke.color } else { visuals.fg_stroke.color };
    paint(ui.painter(), rect.shrink(size * 0.15), icon, col);
    resp.on_hover_text(tooltip)
}
