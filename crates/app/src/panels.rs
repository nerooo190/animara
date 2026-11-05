//! Docked panels: Properties, Library, Color, Swatches, Align, Transform,
//! History, Actions, Output, Motion Editor, Scene, Info, Components, Assets.

use crate::commands::Command as C;
use crate::core::{AppCore, LogLevel};
use crate::icons::{self, Icon};
use crate::ops;
use crate::stage::c32;
use crate::tools::{EraserMode, FillKind, PencilMode, Tool};
use animara_document::{
    BlendMode, Color, ColorEffect, ColorTransform, CurvePoint, Ease, EasePreset, Element, ElementKind, Fill, Filter, FrameLabel,
    GradientStop, LabelKind, LoopMode, Primitive, RotateMode, SoundRef, SoundSync, SymbolKind, TextAlign, TextKind, TweenKind,
};
use animara_geometry::{Affine, Vec2};
use egui::{Color32, RichText};

pub fn to_color(c: Color32) -> Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color::from_rgba8(r, g, b, a)
}

fn color_button(ui: &mut egui::Ui, c: &mut Color) -> bool {
    let mut c32_ = c32(*c);
    let changed = egui::color_picker::color_edit_button_srgba(ui, &mut c32_, egui::color_picker::Alpha::OnlyBlend).changed();
    if changed {
        *c = to_color(c32_);
    }
    changed
}

fn section(ui: &mut egui::Ui, title: &str, open: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong()).default_open(open).show(ui, add);
}

// ------------------------------------------------------------------ Properties

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropTab {
    #[default]
    Tool,
    Object,
    Frame,
    Doc,
}

pub fn properties(core: &mut AppCore, ui: &mut egui::Ui, tab: &mut PropTab) {
    ui.horizontal(|ui| {
        for (t, n) in [(PropTab::Tool, "Tool"), (PropTab::Object, "Object"), (PropTab::Frame, "Frame"), (PropTab::Doc, "Doc")] {
            if ui.selectable_label(*tab == t, n).clicked() {
                *tab = t;
            }
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match tab {
        PropTab::Tool => tool_props(core, ui),
        PropTab::Object => object_props(core, ui),
        PropTab::Frame => frame_props(core, ui),
        PropTab::Doc => doc_props(core, ui),
    });
}

fn tool_props(core: &mut AppCore, ui: &mut egui::Ui) {
    let tool = core.tool;
    ui.horizontal(|ui| {
        icons::paint(
            ui.painter(),
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(18.0, 18.0)),
            Icon::Tool(tool),
            ui.visuals().text_color(),
        );
        ui.add_space(22.0);
        ui.label(RichText::new(tool.short()).strong());
    });
    let t = &mut core.settings.tools;
    section(ui, "Fill and Stroke", true, |ui| {
        ui.horizontal(|ui| {
            ui.label("Stroke");
            color_button(ui, &mut t.stroke_color);
            ui.checkbox(&mut t.no_stroke, "None");
        });
        ui.horizontal(|ui| {
            ui.label("Fill");
            color_button(ui, &mut t.fill_color);
            ui.checkbox(&mut t.no_fill, "None");
            egui::ComboBox::from_id_salt("fillkind").selected_text(format!("{:?}", t.fill_kind)).show_ui(ui, |ui| {
                ui.selectable_value(&mut t.fill_kind, FillKind::Solid, "Solid");
                ui.selectable_value(&mut t.fill_kind, FillKind::Linear, "Linear gradient");
                ui.selectable_value(&mut t.fill_kind, FillKind::Radial, "Radial gradient");
            });
        });
        ui.add(egui::Slider::new(&mut t.stroke_width, 0.1..=100.0).logarithmic(true).text("Stroke size"));
        ui.horizontal(|ui| {
            ui.label("Style");
            let name = match t.stroke_style {
                animara_document::StrokeStyle::Solid => "Solid",
                animara_document::StrokeStyle::Dashed { .. } => "Dashed",
                animara_document::StrokeStyle::Dotted { .. } => "Dotted",
                animara_document::StrokeStyle::Hairline => "Hairline",
            };
            egui::ComboBox::from_id_salt("sstyle").selected_text(name).show_ui(ui, |ui| {
                if ui.selectable_label(false, "Solid").clicked() {
                    t.stroke_style = animara_document::StrokeStyle::Solid;
                }
                if ui.selectable_label(false, "Dashed").clicked() {
                    t.stroke_style = animara_document::StrokeStyle::Dashed { dash: 6.0, gap: 4.0 };
                }
                if ui.selectable_label(false, "Dotted").clicked() {
                    t.stroke_style = animara_document::StrokeStyle::Dotted { spacing: 4.0 };
                }
                if ui.selectable_label(false, "Hairline").clicked() {
                    t.stroke_style = animara_document::StrokeStyle::Hairline;
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Cap");
            for (c, n) in [
                (animara_document::LineCap::Butt, "Butt"),
                (animara_document::LineCap::Round, "Round"),
                (animara_document::LineCap::Square, "Square"),
            ] {
                ui.selectable_value(&mut t.cap, c, n);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Join");
            for (c, n) in [
                (animara_document::LineJoin::Miter, "Miter"),
                (animara_document::LineJoin::Round, "Round"),
                (animara_document::LineJoin::Bevel, "Bevel"),
            ] {
                ui.selectable_value(&mut t.join, c, n);
            }
        });
    });
    match tool {
        Tool::Brush | Tool::ClassicBrush | Tool::Pencil => section(ui, "Brush", true, |ui| {
            ui.add(egui::Slider::new(&mut t.brush_size, 0.5..=200.0).logarithmic(true).text("Size"));
            ui.add(egui::Slider::new(&mut t.brush_smoothing, 0.0..=10.0).text("Smoothing"));
            if tool == Tool::Brush {
                ui.checkbox(&mut t.pressure, "Pressure (tablet)");
                ui.checkbox(&mut t.velocity, "Velocity / taper (fluid brush)");
            }
            if tool == Tool::Pencil {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut t.pencil_mode, PencilMode::Straighten, "Straighten");
                    ui.selectable_value(&mut t.pencil_mode, PencilMode::Smooth, "Smooth");
                    ui.selectable_value(&mut t.pencil_mode, PencilMode::Ink, "Ink");
                });
            }
        }),
        Tool::Eraser => section(ui, "Eraser", true, |ui| {
            ui.add(egui::Slider::new(&mut t.eraser_size, 1.0..=200.0).logarithmic(true).text("Size"));
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut t.eraser_mode, EraserMode::Normal, "Normal");
                ui.selectable_value(&mut t.eraser_mode, EraserMode::FillsOnly, "Fills");
                ui.selectable_value(&mut t.eraser_mode, EraserMode::LinesOnly, "Lines");
                ui.selectable_value(&mut t.eraser_mode, EraserMode::Faucet, "Faucet");
            });
        }),
        Tool::Rectangle => section(ui, "Rectangle Options", true, |ui| {
            ui.add(egui::Slider::new(&mut t.rect_radius, 0.0..=200.0).text("Corner radius"));
        }),
        Tool::PolyStar => section(ui, "PolyStar Options", true, |ui| {
            ui.checkbox(&mut t.poly_star, "Star");
            ui.add(egui::Slider::new(&mut t.poly_sides, 3..=32).text("Sides"));
            ui.add(egui::Slider::new(&mut t.star_ratio, 0.05..=1.0).text("Star point size"));
        }),
        Tool::Text => {
            let fonts = core.font_families.clone();
            let t = &mut core.settings.tools;
            section(ui, "Character", true, |ui| {
                text_style_ui(ui, &mut t.text_style, &fonts);
            });
        }
        _ => {}
    }
    let t = &mut core.settings.tools;
    section(ui, "Options", false, |ui| {
        ui.checkbox(&mut t.snap_objects, "Snap to objects");
        ui.checkbox(&mut t.auto_keyframe, "Auto keyframe");
    });
}

fn text_style_ui(ui: &mut egui::Ui, s: &mut animara_document::TextStyle, fonts: &[String]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Family");
        egui::ComboBox::from_id_salt("fontfam").selected_text(&s.font_family).height(300.0).show_ui(ui, |ui| {
            for f in fonts {
                if ui.selectable_label(&s.font_family == f, f).clicked() {
                    s.font_family = f.clone();
                    changed = true;
                }
            }
        });
    });
    ui.horizontal(|ui| {
        changed |= ui.add(egui::DragValue::new(&mut s.size).range(1.0..=999.0).suffix(" pt")).changed();
        changed |= color_button(ui, &mut s.color);
        changed |= ui.toggle_value(&mut s.bold, RichText::new("B").strong()).changed();
        changed |= ui.toggle_value(&mut s.italic, RichText::new("I").italics()).changed();
    });
    ui.horizontal(|ui| {
        ui.label("Letter spacing");
        changed |= ui.add(egui::DragValue::new(&mut s.letter_spacing).speed(0.1)).changed();
        changed |= ui.checkbox(&mut s.kerning, "Auto kern").changed();
    });
    changed
}

fn object_props(core: &mut AppCore, ui: &mut egui::Ui) {
    let auto_key = core.settings.tools.auto_keyframe;
    let fonts = core.font_families.clone();
    let Some(st) = core.doc_mut() else { return };
    let sel = st.selected();
    if sel.is_empty() {
        ui.weak("No selection. Select an object on the stage to edit its properties.");
        return;
    }
    if sel.len() > 1 {
        ui.label(format!("{} objects selected", sel.len()));
    }
    let (_, e) = sel[0].clone();
    let id = e.id;
    let ctx = st.context_transform();
    let b = animara_renderer::bounds::element_bounds(&st.doc, &e, &ctx);
    ui.label(RichText::new(e.kind_name()).strong());
    section(ui, "Position and Size", true, |ui| {
        let (mut x, mut y, mut w, mut h) = (b.min.x, b.min.y, b.width(), b.height());
        let mut changed = None;
        egui::Grid::new("pos").num_columns(4).show(ui, |ui| {
            ui.label("X");
            if ui.add(egui::DragValue::new(&mut x).speed(1.0).max_decimals(2)).changed() {
                changed = Some(0);
            }
            ui.label("Y");
            if ui.add(egui::DragValue::new(&mut y).speed(1.0).max_decimals(2)).changed() {
                changed = Some(1);
            }
            ui.end_row();
            ui.label("W");
            if ui.add(egui::DragValue::new(&mut w).speed(1.0).range(0.1..=1e6).max_decimals(2)).changed() {
                changed = Some(2);
            }
            ui.label("H");
            if ui.add(egui::DragValue::new(&mut h).speed(1.0).range(0.1..=1e6).max_decimals(2)).changed() {
                changed = Some(3);
            }
            ui.end_row();
        });
        let parts = e.transform.decompose();
        let mut rot = parts.skew_x.to_degrees();
        let mut skew = (parts.skew_y - parts.skew_x).to_degrees();
        ui.horizontal(|ui| {
            ui.label("Rotate");
            if ui.add(egui::DragValue::new(&mut rot).speed(1.0).suffix("°")).changed() {
                changed = Some(4);
            }
            ui.label("Skew");
            if ui.add(egui::DragValue::new(&mut skew).speed(1.0).suffix("°")).changed() {
                changed = Some(5);
            }
        });
        if let Some(c) = changed {
            if st.history.undo_labels().last() != Some("Properties") {
                st.checkpoint("Properties");
            } else {
                st.touch();
            }
            let ids = st.selection.clone();
            for sid in ids {
                if let Some(el) = st.element_mut(sid, auto_key) {
                    match c {
                        0 => el.transform.tx += x - b.min.x,
                        1 => el.transform.ty += y - b.min.y,
                        2 | 3 => {
                            let (sx, sy) = if c == 2 { (w / b.width().max(1e-6), 1.0) } else { (1.0, h / b.height().max(1e-6)) };
                            el.transform = Affine::around(Affine::scale(sx, sy), b.min) * el.transform;
                        }
                        4 | 5 => {
                            let mut p = el.transform.decompose();
                            if c == 4 {
                                let d = rot.to_radians() - p.skew_x;
                                p.skew_x += d;
                                p.skew_y += d;
                            } else {
                                p.skew_y = p.skew_x + skew.to_radians();
                            }
                            let pivot = el.pivot_world();
                            let mut m = Affine::compose(&p);
                            let np = m.apply(el.pivot);
                            m.tx += pivot.x - np.x;
                            m.ty += pivot.y - np.y;
                            el.transform = m;
                        }
                        _ => {}
                    }
                }
            }
        }
    });
    match e.kind.clone() {
        ElementKind::Instance(inst) => {
            let sym = st.doc.library.symbol(inst.symbol).cloned();
            let mut ni = inst.clone();
            section(ui, "Instance", true, |ui| {
                if let Some(s) = &sym {
                    ui.label(format!("Symbol: {} ({})", s.name, s.kind.name()));
                    let mut kind = s.kind;
                    ui.horizontal(|ui| {
                        ui.label("Behavior");
                        for k in [SymbolKind::MovieClip, SymbolKind::Graphic, SymbolKind::Button] {
                            ui.selectable_value(&mut kind, k, k.name());
                        }
                    });
                    if kind != s.kind {
                        st.checkpoint("Symbol Behavior");
                        if let Some(sm) = st.doc.library.symbol_mut(s.id) {
                            sm.kind = kind;
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("Instance name");
                    ui.text_edit_singleline(&mut ni.instance_name);
                });
                ui.checkbox(&mut ni.visible, "Visible");
                ui.checkbox(&mut ni.cache_as_bitmap, "Cache as bitmap");
            });
            if sym.as_ref().is_some_and(|s| s.kind == SymbolKind::Graphic) {
                section(ui, "Looping", true, |ui| {
                    egui::ComboBox::from_id_salt("loopmode").selected_text(format!("{:?}", ni.loop_mode)).show_ui(ui, |ui| {
                        for m in
                            [LoopMode::Loop, LoopMode::PlayOnce, LoopMode::SingleFrame, LoopMode::LoopReverse, LoopMode::PlayOnceReverse]
                        {
                            ui.selectable_value(&mut ni.loop_mode, m, format!("{m:?}"));
                        }
                    });
                    let len = sym.as_ref().map(|s| s.timeline.length()).unwrap_or(1);
                    let mut ff = ni.first_frame + 1;
                    if ui.add(egui::DragValue::new(&mut ff).range(1..=len).prefix("First frame: ")).changed() {
                        ni.first_frame = ff - 1;
                    }
                });
            }
            section(ui, "Color Effect", true, |ui| color_effect_ui(ui, &mut ni.color_effect));
            section(ui, "Display", true, |ui| {
                egui::ComboBox::from_id_salt("blend").selected_text(format!("{:?}", ni.blend)).show_ui(ui, |ui| {
                    for m in BlendMode::ALL {
                        ui.selectable_value(&mut ni.blend, m, format!("{m:?}"));
                    }
                });
            });
            section(ui, "Filters", true, |ui| filters_ui(ui, &mut ni.filters));
            if ni != inst {
                if st.history.undo_labels().last() != Some("Instance Properties") {
                    st.checkpoint("Instance Properties");
                } else {
                    st.touch();
                }
                if let Some(el) = st.element_mut(id, auto_key) {
                    el.kind = ElementKind::Instance(ni);
                }
            }
        }
        ElementKind::Shape(s) => {
            let mut ns = s.clone();
            section(ui, "Shape", true, |ui| {
                ui.label(format!("{} path(s)", ns.paths.len()));
                if let Some(sp) = ns.paths.first_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Fill");
                        match &mut sp.fill {
                            Some(Fill::Solid { color }) => {
                                color_button(ui, color);
                            }
                            Some(f) => {
                                ui.label(if matches!(f, Fill::Linear { .. }) { "Linear gradient" } else { "Gradient / bitmap" });
                            }
                            None => {
                                ui.weak("none");
                            }
                        }
                        if sp.fill.is_some() && ui.small_button("✕").on_hover_text("Remove fill").clicked() {
                            sp.fill = None;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Stroke");
                        match &mut sp.stroke {
                            Some(stroke) => {
                                let mut c = stroke.fill.primary_color();
                                if color_button(ui, &mut c) {
                                    stroke.fill = Fill::solid(c);
                                }
                                ui.add(egui::DragValue::new(&mut stroke.width).range(0.0..=200.0).speed(0.1));
                                if ui.small_button("✕").clicked() {
                                    sp.stroke = None;
                                }
                            }
                            None => {
                                if ui.small_button("Add stroke").clicked() {
                                    sp.stroke = Some(animara_document::Stroke::solid(Color::BLACK, 1.0));
                                }
                            }
                        }
                    });
                    if let Some(prim) = &mut sp.primitive {
                        ui.separator();
                        ui.label("Primitive");
                        let mut changed = false;
                        match prim {
                            Primitive::Rect { corner_radius, .. } => {
                                changed |= ui.add(egui::Slider::new(corner_radius, 0.0..=300.0).text("Corner radius")).changed();
                            }
                            Primitive::Oval { start_angle, end_angle, inner_radius, .. } => {
                                changed |= ui.add(egui::Slider::new(start_angle, 0.0..=360.0).text("Start angle")).changed();
                                changed |= ui.add(egui::Slider::new(end_angle, 0.0..=360.0).text("End angle")).changed();
                                changed |= ui.add(egui::Slider::new(inner_radius, 0.0..=0.99).text("Inner radius")).changed();
                            }
                            Primitive::Polygon { sides, star, inner_ratio, .. } => {
                                changed |= ui.add(egui::Slider::new(sides, 3..=32).text("Sides")).changed();
                                changed |= ui.checkbox(star, "Star").changed();
                                changed |= ui.add(egui::Slider::new(inner_ratio, 0.05..=1.0).text("Point size")).changed();
                            }
                        }
                        if changed {
                            sp.path = prim.to_path();
                        }
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Smooth").clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("reshape"), 0u8));
                    }
                    if ui.button("Straighten").clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("reshape"), 1u8));
                    }
                    if ui.button("Lines → Fills").clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("reshape"), 3u8));
                    }
                });
            });
            if ns != s {
                if st.history.undo_labels().last() != Some("Shape Properties") {
                    st.checkpoint("Shape Properties");
                } else {
                    st.touch();
                }
                if let Some(el) = st.element_mut(id, auto_key) {
                    el.kind = ElementKind::Shape(ns);
                }
            }
            if let Some(m) = ui.ctx().data_mut(|d| d.remove_temp::<u8>(egui::Id::unique("reshape"))) {
                ops::reshape(st, m, auto_key);
            }
        }
        ElementKind::Text(t) => {
            let mut nt = t.clone();
            section(ui, "Text", true, |ui| {
                ui.horizontal(|ui| {
                    for (k, n) in [(TextKind::Static, "Static"), (TextKind::Dynamic, "Dynamic"), (TextKind::Input, "Input")] {
                        ui.selectable_value(&mut nt.kind, k, n);
                    }
                });
                if nt.kind != TextKind::Static {
                    ui.horizontal(|ui| {
                        ui.label("Instance name");
                        ui.text_edit_singleline(&mut nt.instance_name);
                    });
                }
                let mut text = nt.plain_text();
                if ui.add(egui::TextEdit::multiline(&mut text).desired_rows(3)).changed() {
                    nt.set_plain_text(&text);
                }
                let mut style = nt.style();
                if text_style_ui(ui, &mut style, &fonts) {
                    nt.set_style_all(|s| *s = style.clone());
                }
                ui.horizontal(|ui| {
                    for (a, n) in [(TextAlign::Left, "⬅"), (TextAlign::Center, "↔"), (TextAlign::Right, "➡"), (TextAlign::Justify, "☰")]
                    {
                        ui.selectable_value(&mut nt.align, a, n);
                    }
                    ui.checkbox(&mut nt.rtl, "Right-to-left");
                });
                ui.horizontal(|ui| {
                    ui.label("Line spacing");
                    ui.add(egui::DragValue::new(&mut nt.line_spacing).speed(0.1));
                });
                let mut fixed = nt.width.is_some();
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut fixed, "Fixed width").changed() {
                        nt.width = if fixed { Some(200.0) } else { None };
                    }
                    if let Some(w) = &mut nt.width {
                        ui.add(egui::DragValue::new(w).range(5.0..=10000.0));
                    }
                });
                if ui.button("Convert to outlines").clicked() {
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("text_outlines"), true));
                }
            });
            if nt != t {
                if st.history.undo_labels().last() != Some("Text Properties") {
                    st.checkpoint("Text Properties");
                } else {
                    st.touch();
                }
                if let Some(el) = st.element_mut(id, auto_key) {
                    el.kind = ElementKind::Text(nt);
                }
            }
            if ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::unique("text_outlines"))).is_some() {
                st.selection = vec![id];
                ops::break_apart(st, auto_key);
                ops::break_apart(st, auto_key);
            }
        }
        ElementKind::Bitmap(bm) => {
            if let Some(a) = st.doc.library.bitmap(bm.asset).cloned() {
                section(ui, "Bitmap", true, |ui| {
                    ui.label(format!("{} — {}×{} {}", a.name, a.width, a.height, a.format.to_uppercase()));
                    let mut smooth = a.smoothing;
                    if ui.checkbox(&mut smooth, "Allow smoothing").changed() {
                        st.checkpoint("Bitmap Properties");
                        if let Some(b) = st.doc.library.bitmaps.iter_mut().find(|b| b.id == a.id) {
                            b.smoothing = smooth;
                        }
                    }
                });
            }
        }
        ElementKind::Group { children } => {
            ui.label(format!("Group of {} objects", children.len()));
        }
    }
}

fn color_effect_ui(ui: &mut egui::Ui, ce: &mut ColorEffect) {
    let name = match ce {
        ColorEffect::None => "None",
        ColorEffect::Brightness { .. } => "Brightness",
        ColorEffect::Tint { .. } => "Tint",
        ColorEffect::Alpha { .. } => "Alpha",
        ColorEffect::Advanced { .. } => "Advanced",
    };
    egui::ComboBox::from_id_salt("ce").selected_text(name).show_ui(ui, |ui| {
        if ui.selectable_label(false, "None").clicked() {
            *ce = ColorEffect::None;
        }
        if ui.selectable_label(false, "Brightness").clicked() {
            *ce = ColorEffect::Brightness { amount: 0.0 };
        }
        if ui.selectable_label(false, "Tint").clicked() {
            *ce = ColorEffect::Tint { color: Color::WHITE, amount: 0.5 };
        }
        if ui.selectable_label(false, "Alpha").clicked() {
            *ce = ColorEffect::Alpha { alpha: 1.0 };
        }
        if ui.selectable_label(false, "Advanced").clicked() {
            *ce = ColorEffect::Advanced { transform: ColorTransform::IDENTITY };
        }
    });
    match ce {
        ColorEffect::None => {}
        ColorEffect::Brightness { amount } => {
            ui.add(egui::Slider::new(amount, -1.0..=1.0).text("Brightness"));
        }
        ColorEffect::Tint { color, amount } => {
            ui.horizontal(|ui| {
                color_button(ui, color);
                ui.add(egui::Slider::new(amount, 0.0..=1.0).text("Tint"));
            });
        }
        ColorEffect::Alpha { alpha } => {
            ui.add(egui::Slider::new(alpha, 0.0..=1.0).text("Alpha"));
        }
        ColorEffect::Advanced { transform } => {
            for (i, n) in ["Red", "Green", "Blue", "Alpha"].iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(*n);
                    ui.add(egui::DragValue::new(&mut transform.mul[i]).range(-1.0..=1.0).speed(0.01).prefix("× "));
                    ui.add(egui::DragValue::new(&mut transform.add[i]).range(-1.0..=1.0).speed(0.01).prefix("+ "));
                });
            }
        }
    }
}

fn filters_ui(ui: &mut egui::Ui, filters: &mut Vec<Filter>) {
    let mut remove = None;
    for (i, f) in filters.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(f.name()).strong());
                if ui.small_button("✕").clicked() {
                    remove = Some(i);
                }
            });
            match f {
                Filter::Blur { blur_x, blur_y, quality } => {
                    ui.add(egui::Slider::new(blur_x, 0.0..=100.0).text("Blur X"));
                    ui.add(egui::Slider::new(blur_y, 0.0..=100.0).text("Blur Y"));
                    ui.add(egui::Slider::new(quality, 1..=3).text("Quality"));
                }
                Filter::DropShadow { blur_x, blur_y, strength, quality, angle, distance, color, knockout, inner, hide_object } => {
                    ui.add(egui::Slider::new(blur_x, 0.0..=100.0).text("Blur X"));
                    ui.add(egui::Slider::new(blur_y, 0.0..=100.0).text("Blur Y"));
                    ui.add(egui::Slider::new(strength, 0.0..=10.0).text("Strength"));
                    ui.add(egui::Slider::new(angle, 0.0..=360.0).text("Angle"));
                    ui.add(egui::Slider::new(distance, -100.0..=100.0).text("Distance"));
                    ui.add(egui::Slider::new(quality, 1..=3).text("Quality"));
                    ui.horizontal(|ui| {
                        color_button(ui, color);
                        ui.checkbox(knockout, "Knockout");
                        ui.checkbox(inner, "Inner");
                        ui.checkbox(hide_object, "Hide object");
                    });
                }
                Filter::Glow { blur_x, blur_y, strength, quality, color, knockout, inner } => {
                    ui.add(egui::Slider::new(blur_x, 0.0..=100.0).text("Blur X"));
                    ui.add(egui::Slider::new(blur_y, 0.0..=100.0).text("Blur Y"));
                    ui.add(egui::Slider::new(strength, 0.0..=10.0).text("Strength"));
                    ui.add(egui::Slider::new(quality, 1..=3).text("Quality"));
                    ui.horizontal(|ui| {
                        color_button(ui, color);
                        ui.checkbox(knockout, "Knockout");
                        ui.checkbox(inner, "Inner");
                    });
                }
                Filter::AdjustColor { brightness, contrast, saturation, hue } => {
                    ui.add(egui::Slider::new(brightness, -100.0..=100.0).text("Brightness"));
                    ui.add(egui::Slider::new(contrast, -100.0..=100.0).text("Contrast"));
                    ui.add(egui::Slider::new(saturation, -100.0..=100.0).text("Saturation"));
                    ui.add(egui::Slider::new(hue, -180.0..=180.0).text("Hue"));
                }
            }
            ui.separator();
        });
    }
    if let Some(i) = remove {
        filters.remove(i);
    }
    ui.menu_button("+ Add filter", |ui| {
        if ui.button("Blur").clicked() {
            filters.push(Filter::default_blur());
            ui.close();
        }
        if ui.button("Drop Shadow").clicked() {
            filters.push(Filter::default_drop_shadow());
            ui.close();
        }
        if ui.button("Glow").clicked() {
            filters.push(Filter::default_glow());
            ui.close();
        }
        if ui.button("Adjust Color").clicked() {
            filters.push(Filter::AdjustColor { brightness: 0.0, contrast: 0.0, saturation: 0.0, hue: 0.0 });
            ui.close();
        }
    });
}

fn ease_ui(ui: &mut egui::Ui, ease: &mut Ease, id: &str) {
    let name = match ease {
        Ease::Linear => "No Ease".to_string(),
        Ease::Classic { strength } => format!("Classic {strength}"),
        Ease::Preset { preset } => preset.name().to_string(),
        Ease::Custom { .. } => "Custom".to_string(),
    };
    egui::ComboBox::from_id_salt(id).selected_text(name).height(320.0).show_ui(ui, |ui| {
        if ui.selectable_label(matches!(ease, Ease::Linear), "No Ease").clicked() {
            *ease = Ease::Linear;
        }
        if ui.selectable_label(matches!(ease, Ease::Classic { .. }), "Classic Ease").clicked() {
            *ease = Ease::Classic { strength: 0 };
        }
        if ui.selectable_label(matches!(ease, Ease::Custom { .. }), "Custom…").clicked() {
            *ease = Ease::Custom { points: vec![] };
        }
        ui.separator();
        for p in EasePreset::ALL {
            if ui.selectable_label(matches!(ease, Ease::Preset { preset } if *preset == p), p.name()).clicked() {
                *ease = Ease::Preset { preset: p };
            }
        }
    });
    if let Ease::Classic { strength } = ease {
        ui.add(egui::Slider::new(strength, -100..=100).text("Ease (− in / + out)"));
    }
}

fn frame_props(core: &mut AppCore, ui: &mut egui::Ui) {
    let Some(st) = core.doc_mut() else { return };
    let li = st.layer;
    let f = st.frame;
    let Some(layer) = st.timeline().layers.get(li).cloned() else { return };
    let Some(k) = layer.keyframe_at(f).cloned() else {
        ui.weak("No frame at the playhead on this layer.");
        return;
    };
    ui.label(RichText::new(format!("Layer “{}” — keyframe at {}", layer.name, k.start + 1)).strong());
    let mut nk = k.clone();
    section(ui, "Label", true, |ui| {
        let mut name = nk.label.as_ref().map(|l| l.name.clone()).unwrap_or_default();
        let mut kind = nk.label.as_ref().map(|l| l.kind).unwrap_or(LabelKind::Name);
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut name);
        });
        ui.horizontal(|ui| {
            for (kk, n) in [(LabelKind::Name, "Name"), (LabelKind::Comment, "Comment"), (LabelKind::Anchor, "Anchor")] {
                ui.selectable_value(&mut kind, kk, n);
            }
        });
        nk.label = (!name.is_empty()).then_some(FrameLabel { name, kind });
    });
    section(ui, "Tweening", true, |ui| {
        let mut kind = nk.tween.as_ref().map(|t| t.kind);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut kind, None, "None");
            ui.selectable_value(&mut kind, Some(TweenKind::Classic), "Classic");
            ui.selectable_value(&mut kind, Some(TweenKind::Motion), "Motion");
            ui.selectable_value(&mut kind, Some(TweenKind::Shape), "Shape");
        });
        match (kind, &mut nk.tween) {
            (None, t) => *t = None,
            (Some(kd), Some(t)) => t.kind = kd,
            (Some(kd), t) => *t = Some(animara_document::Tween::new(kd)),
        }
        if let Some(t) = &mut nk.tween {
            ease_ui(ui, &mut t.ease, "frame_ease");
            if t.kind != TweenKind::Shape {
                ui.horizontal(|ui| {
                    ui.label("Rotate");
                    egui::ComboBox::from_id_salt("rot").selected_text(format!("{:?}", t.rotate)).show_ui(ui, |ui| {
                        for r in [RotateMode::Auto, RotateMode::None, RotateMode::Clockwise, RotateMode::CounterClockwise] {
                            ui.selectable_value(&mut t.rotate, r, format!("{r:?}"));
                        }
                    });
                    if matches!(t.rotate, RotateMode::Clockwise | RotateMode::CounterClockwise) {
                        ui.add(egui::DragValue::new(&mut t.rotate_times).range(0..=100).suffix("×"));
                    }
                });
                ui.checkbox(&mut t.orient_to_path, "Orient to path");
                ui.checkbox(&mut t.scale, "Scale");
                ui.checkbox(&mut t.snap, "Snap");
            }
            if t.kind == TweenKind::Motion {
                ui.horizontal(|ui| {
                    if ui.button("Use guide/selected path as motion path").clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("motion_path_from_guide"), true));
                    }
                    if t.motion_path.is_some() && ui.button("Clear path").clicked() {
                        t.motion_path = None;
                    }
                });
            }
        }
    });
    section(ui, "Sound", true, |ui| {
        let sounds: Vec<(animara_document::Id, String)> = st.doc.library.sounds.iter().map(|s| (s.id, s.name.clone())).collect();
        let cur = nk.sound.as_ref().map(|s| s.asset);
        let cur_name = cur.and_then(|id| sounds.iter().find(|s| s.0 == id).map(|s| s.1.clone())).unwrap_or_else(|| "None".into());
        egui::ComboBox::from_id_salt("snd").selected_text(cur_name).show_ui(ui, |ui| {
            if ui.selectable_label(cur.is_none(), "None").clicked() {
                nk.sound = None;
            }
            for (id, n) in &sounds {
                if ui.selectable_label(cur == Some(*id), n).clicked() {
                    nk.sound = Some(SoundRef::new(*id));
                }
            }
        });
        if let Some(s) = &mut nk.sound {
            ui.horizontal(|ui| {
                ui.label("Sync");
                for (k, n) in
                    [(SoundSync::Event, "Event"), (SoundSync::Start, "Start"), (SoundSync::Stop, "Stop"), (SoundSync::Stream, "Stream")]
                {
                    ui.selectable_value(&mut s.sync, k, n);
                }
            });
            ui.add(egui::Slider::new(&mut s.volume, 0.0..=2.0).text("Volume"));
            ui.add(egui::DragValue::new(&mut s.loops).range(0..=999).prefix("Repeat ×"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut s.trim_start).range(0.0..=3600.0).speed(0.01).prefix("Trim start ").suffix(" s"));
                ui.add(egui::DragValue::new(&mut s.trim_end).range(0.0..=3600.0).speed(0.01).prefix("end ").suffix(" s"));
            });
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut s.fade_in).range(0.0..=60.0).speed(0.01).prefix("Fade in ").suffix(" s"));
                ui.add(egui::DragValue::new(&mut s.fade_out).range(0.0..=60.0).speed(0.01).prefix("out ").suffix(" s"));
            });
        }
    });
    if layer.kind == animara_document::LayerKind::Camera {
        section(ui, "Camera", true, |ui| {
            let mut cam = nk.camera.unwrap_or(animara_document::CameraState::centered(st.doc.width, st.doc.height));
            ui.add(egui::DragValue::new(&mut cam.x).prefix("X ").speed(1.0));
            ui.add(egui::DragValue::new(&mut cam.y).prefix("Y ").speed(1.0));
            ui.add(egui::Slider::new(&mut cam.zoom, 0.05..=20.0).logarithmic(true).text("Zoom"));
            ui.add(egui::Slider::new(&mut cam.rotation, -360.0..=360.0).text("Rotation"));
            let mut tint = cam.tint.is_some();
            ui.horizontal(|ui| {
                ui.checkbox(&mut tint, "Tint");
                if tint {
                    let (mut c, mut a) = cam.tint.unwrap_or((Color::BLACK, 0.3));
                    color_button(ui, &mut c);
                    ui.add(egui::Slider::new(&mut a, 0.0..=1.0));
                    cam.tint = Some((c, a));
                } else {
                    cam.tint = None;
                }
            });
            nk.camera = Some(cam);
        });
    }
    section(ui, "Actions", false, |ui| {
        ui.add(egui::TextEdit::multiline(&mut nk.script).code_editor().desired_rows(4).hint_text("stop();"));
    });
    let use_guide = ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::unique("motion_path_from_guide"))).is_some();
    if use_guide {
        // Take the first path from the guide layer, or from the selected shape.
        let path = layer
            .guide_layer
            .and_then(|g| st.timeline().layer(g).cloned())
            .and_then(|g| g.keyframe_at(f).cloned())
            .and_then(|gk| {
                gk.elements.iter().find_map(|e| ops::shape_from(e).and_then(|s| s.paths.first()).map(|p| p.path.transform(&e.transform)))
            })
            .or_else(|| {
                st.selected()
                    .iter()
                    .find_map(|(_, e)| ops::shape_from(e).and_then(|s| s.paths.first()).map(|p| p.path.transform(&e.transform)))
            });
        if let (Some(p), Some(t)) = (path, &mut nk.tween) {
            t.motion_path = Some(p);
        }
    }
    if nk != k {
        if st.history.undo_labels().last() != Some("Frame Properties") {
            st.checkpoint("Frame Properties");
        } else {
            st.touch();
        }
        if let Some(target) = st.timeline_mut().layers[li].keyframe_at_mut(f) {
            *target = nk;
        }
    }
}

fn doc_props(core: &mut AppCore, ui: &mut egui::Ui) {
    let Some(st) = core.doc_mut() else { return };
    let mut doc_changed = false;
    let mut d = st.doc.clone();
    ui.label(RichText::new(&d.name).strong());
    section(ui, "Document Settings", true, |ui| {
        egui::Grid::new("docset").num_columns(2).show(ui, |ui| {
            ui.label("W");
            doc_changed |= ui.add(egui::DragValue::new(&mut d.width).range(1.0..=8192.0)).changed();
            ui.end_row();
            ui.label("H");
            doc_changed |= ui.add(egui::DragValue::new(&mut d.height).range(1.0..=8192.0)).changed();
            ui.end_row();
            ui.label("Stage");
            doc_changed |= color_button(ui, &mut d.background);
            ui.end_row();
            ui.label("FPS");
            doc_changed |= ui.add(egui::DragValue::new(&mut d.fps).range(0.01..=120.0).speed(0.1)).changed();
            ui.end_row();
        });
        if ui.button("Match contents").clicked() {
            ui.ctx().data_mut(|dd| dd.insert_temp(egui::Id::unique("match_contents"), true));
        }
    });
    section(ui, "Publish Settings", true, |ui| {
        ui.horizontal(|ui| {
            ui.label("Output name");
            doc_changed |= ui.text_edit_singleline(&mut d.publish.output_name).changed();
        });
        doc_changed |= ui.checkbox(&mut d.publish.loop_playback, "Loop playback").changed();
        doc_changed |= ui.checkbox(&mut d.publish.transparent, "Transparent background").changed();
        doc_changed |= ui.checkbox(&mut d.publish.include_audio, "Include audio").changed();
        doc_changed |= ui.add(egui::Slider::new(&mut d.publish.scale, 0.1..=4.0).text("Scale")).changed();
        doc_changed |= ui.add(egui::Slider::new(&mut d.publish.quality, 1..=100).text("Quality")).changed();
    });
    section(ui, "Metadata", false, |ui| {
        doc_changed |= ui.add(egui::TextEdit::singleline(&mut d.metadata.title).hint_text("Title")).changed();
        doc_changed |= ui.add(egui::TextEdit::singleline(&mut d.metadata.author).hint_text("Author")).changed();
        doc_changed |= ui.add(egui::TextEdit::multiline(&mut d.metadata.description).hint_text("Description").desired_rows(2)).changed();
    });
    section(ui, "Grid & Guides", false, |ui| {
        doc_changed |= ui.checkbox(&mut d.grid.visible, "Show grid").changed();
        doc_changed |= ui.checkbox(&mut d.grid.snap, "Snap to grid").changed();
        doc_changed |= ui.add(egui::DragValue::new(&mut d.grid.spacing).range(2.0..=500.0).prefix("Spacing ")).changed();
        doc_changed |= color_button(ui, &mut d.grid.color);
        doc_changed |= ui.checkbox(&mut d.guides.visible, "Show guides").changed();
        doc_changed |= ui.checkbox(&mut d.guides.snap, "Snap to guides").changed();
    });
    if doc_changed {
        if st.history.undo_labels().last() != Some("Document Settings") {
            st.checkpoint("Document Settings");
        } else {
            st.touch();
        }
        st.doc = d;
    }
    if ui.ctx().data_mut(|dd| dd.remove_temp::<bool>(egui::Id::unique("match_contents"))).is_some() {
        let dl = animara_animation::evaluate_document_frame(&st.doc, st.scene, st.frame, &animara_animation::EvalOptions::export());
        let b = animara_renderer::bounds::display_bounds(&st.doc, &dl.items);
        if !b.is_empty() {
            st.checkpoint("Match Contents");
            st.doc.width = b.max.x.ceil().max(1.0);
            st.doc.height = b.max.y.ceil().max(1.0);
        }
    }
}

// ------------------------------------------------------------------ Library

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LibDrag(pub animara_document::Id);

fn symbol_thumbnail(core: &mut AppCore, ctx: &egui::Context, id: animara_document::Id, size: u32) -> Option<egui::TextureHandle> {
    let st = core.doc_mut()?;
    let key = (st.uid, id, st.revision);
    if let Some(t) = core.thumbnails.get(&key) {
        return Some(t.clone());
    }
    let st = core.docs.get_mut(core.active)?;
    let pm = crate::thumbs::render_library_item(&st.doc, id, size, &mut st.cache)?;
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    let tex = ctx.load_texture(format!("thumb-{id}"), img, egui::TextureOptions::LINEAR);
    // Drop stale thumbnails for this item.
    let uid = st.uid;
    core.thumbnails.retain(|k, _| !(k.0 == uid && k.1 == id));
    core.thumbnails.insert(key, tex.clone());
    Some(tex)
}

pub fn library(core: &mut AppCore, ui: &mut egui::Ui) {
    if core.doc().is_none() {
        ui.weak("No document");
        return;
    }
    let ctx = ui.ctx().clone();
    // Preview of the selected item.
    if let Some(sel) = core.library_selected
        && let Some(tex) = symbol_thumbnail(core, &ctx, sel, 192)
    {
        let size = tex.size_vec2();
        let avail = ui.available_width().min(220.0);
        let s = (avail / size.x).min(120.0 / size.y);
        ui.vertical_centered(|ui| {
            ui.add(egui::Image::new(&tex).fit_to_exact_size(size * s));
        });
    }
    let mut filter = core.library_filter.clone();
    let mut action: Option<(&'static str, animara_document::Id)> = None;
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut filter).hint_text("🔍 Search library").desired_width(ui.available_width() - 60.0));
        if ui.button("+◆").on_hover_text("New Symbol").clicked() {
            core.queue.push(C::NewSymbol);
        }
        if ui.button("+📁").on_hover_text("New Folder").clicked() {
            action = Some(("new_folder", animara_document::Id(0)));
        }
    });
    core.library_filter = filter.clone();
    let st = core.doc().unwrap();
    let items: Vec<(animara_document::Id, String, String, &'static str, Icon)> = st
        .doc
        .library
        .symbols
        .iter()
        .map(|s| (s.id, s.name.clone(), s.folder.clone(), s.kind.name(), Icon::Symbol))
        .chain(st.doc.library.bitmaps.iter().map(|b| (b.id, b.name.clone(), b.folder.clone(), "Bitmap", Icon::Bitmap)))
        .chain(st.doc.library.sounds.iter().map(|s| (s.id, s.name.clone(), s.folder.clone(), "Sound", Icon::Sound)))
        .filter(|(_, n, ..)| filter.is_empty() || n.to_lowercase().contains(&filter.to_lowercase()))
        .collect();
    let folders = st.doc.library.all_folders();
    let scenes: Vec<&animara_document::Timeline> = st.doc.scenes.iter().map(|s| &s.timeline).collect();
    let counts: std::collections::HashMap<animara_document::Id, usize> =
        items.iter().map(|(id, ..)| (*id, st.doc.library.use_count(*id, &scenes))).collect();
    ui.label(RichText::new(format!("{} items", items.len())).weak().small());
    let selected = core.library_selected;
    let mut new_selected = selected;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let mut show_item =
            |ui: &mut egui::Ui, (id, name, _folder, kind, icon): &(animara_document::Id, String, String, &'static str, Icon)| {
                let resp = ui
                    .horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                        icons::paint(ui.painter(), r, *icon, ui.visuals().text_color());
                        let lbl = ui.selectable_label(selected == Some(*id), name);
                        ui.weak(format!("{kind} · {}", counts.get(id).copied().unwrap_or(0)));
                        lbl
                    })
                    .inner;
                let resp = ui.interact(resp.rect, resp.id.with("drag"), egui::Sense::click_and_drag());
                if resp.clicked() {
                    new_selected = Some(*id);
                }
                if resp.double_clicked() {
                    action = Some(("edit", *id));
                }
                if resp.drag_started() {
                    resp.dnd_set_drag_payload(LibDrag(*id));
                }
                if resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                resp.context_menu(|ui| {
                    for (a, n) in [
                        ("edit", "Edit"),
                        ("place", "Place on Stage"),
                        ("rename", "Rename…"),
                        ("duplicate", "Duplicate"),
                        ("delete", "Delete"),
                        ("kind_mc", "Behavior: Movie Clip"),
                        ("kind_gr", "Behavior: Graphic"),
                        ("kind_bt", "Behavior: Button"),
                        ("play", "Play sound"),
                    ] {
                        if ui.button(n).clicked() {
                            action = Some((a, *id));
                            ui.close();
                        }
                    }
                    ui.menu_button("Move to folder", |ui| {
                        if ui.button("(root)").clicked() {
                            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("lib_move"), (*id, String::new())));
                            ui.close();
                        }
                        for f in &folders {
                            if ui.button(f).clicked() {
                                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("lib_move"), (*id, f.clone())));
                                ui.close();
                            }
                        }
                    });
                });
            };
        for it in items.iter().filter(|i| i.2.is_empty()) {
            show_item(ui, it);
        }
        for f in &folders {
            egui::CollapsingHeader::new(format!("📁 {f}")).default_open(true).show(ui, |ui| {
                for it in items.iter().filter(|i| &i.2 == f) {
                    show_item(ui, it);
                }
            });
        }
    });
    core.library_selected = new_selected;
    if let Some((id, folder)) = ui.ctx().data_mut(|d| d.remove_temp::<(animara_document::Id, String)>(egui::Id::unique("lib_move"))) {
        let st = core.doc_mut().unwrap();
        st.checkpoint("Move to Folder");
        if let Some(s) = st.doc.library.symbol_mut(id) {
            s.folder = folder.clone();
        }
        if let Some(b) = st.doc.library.bitmaps.iter_mut().find(|b| b.id == id) {
            b.folder = folder.clone();
        }
        if let Some(s) = st.doc.library.sounds.iter_mut().find(|s| s.id == id) {
            s.folder = folder;
        }
    }
    // Rename dialog state.
    let rename_id = egui::Id::unique("lib_rename");
    if let Some((id, mut name)) = ui.ctx().data(|d| d.get_temp::<(animara_document::Id, String)>(rename_id)) {
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Rename").collapsible(false).open(&mut open).show(ui.ctx(), |ui| {
            let r = ui.text_edit_singleline(&mut name);
            r.request_focus();
            if ui.button("OK").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                apply = true;
            }
        });
        if apply {
            let st = core.doc_mut().unwrap();
            st.checkpoint("Rename");
            if let Some(s) = st.doc.library.symbol_mut(id) {
                s.name = name.clone();
            }
            if let Some(b) = st.doc.library.bitmaps.iter_mut().find(|b| b.id == id) {
                b.name = name.clone();
            }
            if let Some(s) = st.doc.library.sounds.iter_mut().find(|s| s.id == id) {
                s.name = name.clone();
            }
            ui.ctx().data_mut(|d| d.remove_temp::<(animara_document::Id, String)>(rename_id));
        } else if open {
            ui.ctx().data_mut(|d| d.insert_temp(rename_id, (id, name)));
        } else {
            ui.ctx().data_mut(|d| d.remove_temp::<(animara_document::Id, String)>(rename_id));
        }
    }
    if let Some((a, id)) = action {
        let auto_key = core.settings.tools.auto_keyframe;
        let st = core.doc_mut().unwrap();
        match a {
            "edit" => {
                if st.doc.library.symbol(id).is_some() {
                    st.stack.clear();
                    ops::enter_symbol(st, id, None);
                } else if st.doc.library.sound(id).is_some() {
                    let doc = st.doc.clone();
                    core.audio.preview_sound(&doc, id);
                }
            }
            "play" => {
                let doc = st.doc.clone();
                core.audio.preview_sound(&doc, id);
            }
            "place" => place_library_item(core, id, None, auto_key),
            "rename" => {
                let name = st
                    .doc
                    .library
                    .symbol(id)
                    .map(|s| s.name.clone())
                    .or_else(|| st.doc.library.bitmap(id).map(|b| b.name.clone()))
                    .or_else(|| st.doc.library.sound(id).map(|s| s.name.clone()))
                    .unwrap_or_default();
                ui.ctx().data_mut(|d| d.insert_temp(rename_id, (id, name)));
            }
            "duplicate" => {
                ops::duplicate_symbol(st, id);
            }
            "delete" => {
                ops::delete_library_item(st, id);
                core.library_selected = None;
            }
            "kind_mc" | "kind_gr" | "kind_bt" => {
                st.checkpoint("Symbol Behavior");
                if let Some(s) = st.doc.library.symbol_mut(id) {
                    s.kind = match a {
                        "kind_mc" => SymbolKind::MovieClip,
                        "kind_gr" => SymbolKind::Graphic,
                        _ => SymbolKind::Button,
                    };
                }
            }
            "new_folder" => {
                st.checkpoint("New Folder");
                let n = (1..).map(|i| format!("Folder {i}")).find(|n| !st.doc.library.all_folders().contains(n)).unwrap();
                st.doc.library.folders.push(n);
            }
            _ => {}
        }
    }
}

/// Place a library item on the stage (symbol instance, bitmap or sound on the current keyframe).
pub fn place_library_item(core: &mut AppCore, id: animara_document::Id, at: Option<Vec2>, auto_key: bool) {
    let Some(st) = core.doc_mut() else { return };
    let ctx_inv = st.context_transform().invert().unwrap_or(Affine::IDENTITY);
    let center = Vec2::new(st.doc.width / 2.0, st.doc.height / 2.0);
    let p = ctx_inv.apply(at.unwrap_or(center));
    if st.current_symbol() == Some(id) || st.stack.iter().any(|l| st.doc.library.symbol_contains(id, l.symbol)) {
        core.warn("Cannot place a symbol inside itself.");
        return;
    }
    let st = core.doc_mut().unwrap();
    if st.doc.library.symbol(id).is_some() {
        let e = st.doc.instantiate(id, p.x, p.y);
        ops::add_element(st, "Place Instance", e, auto_key);
    } else if let Some(b) = st.doc.library.bitmap(id) {
        let e = Element::new(ElementKind::Bitmap(animara_document::BitmapInstance { asset: id }))
            .with_transform(Affine::translate(p.x - b.width as f64 / 2.0, p.y - b.height as f64 / 2.0));
        ops::add_element(st, "Place Bitmap", e, auto_key);
    } else if st.doc.library.sound(id).is_some() {
        st.checkpoint("Add Sound");
        if let Some(k) = st.editable_keyframe(auto_key) {
            k.sound = Some(SoundRef::new(id));
        }
        // Extend the layer to cover the sound's duration.
        let dur = st.doc.library.sound(id).map(|s| s.duration).unwrap_or(0.0);
        let frames = (dur * st.doc.fps).ceil() as u32;
        let li = st.layer;
        let f = st.frame;
        let layer = &mut st.timeline_mut().layers[li];
        if f + frames > layer.end() {
            layer.extend_to(f + frames.max(1) - 1);
        }
    }
}

// ------------------------------------------------------------------ Color & swatches

pub fn color_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    let auto_key = core.settings.tools.auto_keyframe;
    let t = &mut core.settings.tools;
    let mut target_fill = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::unique("color_target_fill"))).unwrap_or(true);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut target_fill, false, "✏ Stroke");
        ui.selectable_value(&mut target_fill, true, "⬛ Fill");
        if ui.button("⇄").on_hover_text("Swap stroke and fill colors").clicked() {
            std::mem::swap(&mut t.stroke_color, &mut t.fill_color);
        }
        if ui.button("◐").on_hover_text("Black and white").clicked() {
            t.stroke_color = Color::BLACK;
            t.fill_color = Color::WHITE;
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("color_target_fill"), target_fill));
    if target_fill {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut t.fill_kind, FillKind::Solid, "Solid");
            ui.selectable_value(&mut t.fill_kind, FillKind::Linear, "Linear");
            ui.selectable_value(&mut t.fill_kind, FillKind::Radial, "Radial");
            if ui.selectable_label(t.no_fill, "None").clicked() {
                t.no_fill = !t.no_fill;
            }
        });
    } else if ui.selectable_label(t.no_stroke, "No stroke").clicked() {
        t.no_stroke = !t.no_stroke;
    }
    let editing_gradient = target_fill && t.fill_kind != FillKind::Solid;
    let sel_stop_id = egui::Id::unique("grad_stop");
    let mut sel_stop = ui.ctx().data(|d| d.get_temp::<usize>(sel_stop_id)).unwrap_or(0).min(t.gradient.len().saturating_sub(1));
    let mut color = if editing_gradient {
        t.gradient.get(sel_stop).map(|s| s.color).unwrap_or(Color::WHITE)
    } else if target_fill {
        t.fill_color
    } else {
        t.stroke_color
    };
    let mut c = c32(color);
    let changed = egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::OnlyBlend);
    let mut hex = color.to_hex();
    let mut hex_changed = false;
    ui.horizontal(|ui| {
        ui.label("Hex");
        hex_changed = ui.add(egui::TextEdit::singleline(&mut hex).desired_width(90.0)).lost_focus();
        let (h, s, v) = color.to_hsv();
        ui.weak(format!("H {:.0}° S {:.0}% B {:.0}%", h, s * 100.0, v * 100.0));
    });
    if changed {
        color = to_color(c);
    }
    if hex_changed && let Some(hc) = Color::from_hex(&hex) {
        color = hc;
    }
    let any_change = changed || hex_changed;
    if any_change {
        if editing_gradient {
            if let Some(s) = t.gradient.get_mut(sel_stop) {
                s.color = color;
            }
        } else if target_fill {
            t.fill_color = color;
        } else {
            t.stroke_color = color;
        }
    }
    if editing_gradient {
        // Gradient bar with draggable stops.
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(260.0), 22.0), egui::Sense::click_and_drag());
        let n = 64;
        for i in 0..n {
            let a = i as f32 / n as f32;
            let col = sample_gradient(&t.gradient, a);
            let r = egui::Rect::from_min_max(
                egui::pos2(rect.min.x + rect.width() * a, rect.min.y),
                egui::pos2(rect.min.x + rect.width() * (a + 1.0 / n as f32) + 0.5, rect.max.y),
            );
            ui.painter().rect_filled(r, 0.0, c32(col));
        }
        for (i, s) in t.gradient.iter().enumerate() {
            let x = rect.min.x + rect.width() * s.offset;
            let tri = vec![egui::pos2(x, rect.max.y), egui::pos2(x - 5.0, rect.max.y + 8.0), egui::pos2(x + 5.0, rect.max.y + 8.0)];
            ui.painter().add(egui::Shape::convex_polygon(
                tri,
                c32(s.color),
                egui::Stroke::new(if i == sel_stop { 2.0 } else { 1.0 }, Color32::WHITE),
            ));
        }
        ui.add_space(10.0);
        if let Some(p) = resp.interact_pointer_pos() {
            let off = ((p.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
            if resp.drag_started() || resp.clicked() {
                let near = t.gradient.iter().position(|s| ((s.offset - off) * rect.width()).abs() < 6.0);
                match near {
                    Some(i) => sel_stop = i,
                    None if resp.clicked() => {
                        t.gradient.push(GradientStop { offset: off, color: sample_gradient(&t.gradient, off) });
                        sel_stop = t.gradient.len() - 1;
                    }
                    None => {}
                }
            }
            if resp.dragged()
                && let Some(s) = t.gradient.get_mut(sel_stop)
            {
                s.offset = off;
            }
        }
        if resp.double_clicked() && t.gradient.len() > 2 {
            t.gradient.remove(sel_stop);
            sel_stop = 0;
        }
        let order: Vec<GradientStop> = {
            let mut g = t.gradient.clone();
            g.sort_by(|a, b| a.offset.total_cmp(&b.offset));
            g
        };
        if !resp.dragged() && order != t.gradient {
            let cur = t.gradient[sel_stop];
            t.gradient = order;
            sel_stop = t.gradient.iter().position(|s| *s == cur).unwrap_or(0);
        }
        ui.weak("Click bar to add a stop, drag to move, double-click to remove.");
    }
    ui.ctx().data_mut(|d| d.insert_temp(sel_stop_id, sel_stop));
    // Apply to selection.
    let tools = core.settings.tools.clone();
    if let Some(st) = core.doc_mut()
        && !st.selection.is_empty()
        && ui.button("Apply to selection").clicked()
    {
        ops::modify_selection(st, Some("Change Color"), auto_key, |e| match &mut e.kind {
            ElementKind::Shape(s) => {
                for p in &mut s.paths {
                    if target_fill && p.fill.is_some() {
                        let b = p.path.bounds();
                        p.fill = tools.fill_for(b).or(p.fill.take());
                    } else if !target_fill && let Some(sk) = &mut p.stroke {
                        sk.fill = Fill::solid(tools.stroke_color);
                    }
                }
            }
            ElementKind::Text(t) => t.set_style_all(|s| s.color = tools.fill_color),
            _ => {}
        });
    }
}

fn sample_gradient(stops: &[GradientStop], t: f32) -> Color {
    let mut s = stops.to_vec();
    s.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    if s.is_empty() {
        return Color::BLACK;
    }
    if t <= s[0].offset {
        return s[0].color;
    }
    for w in s.windows(2) {
        if t >= w[0].offset && t <= w[1].offset {
            let u = (t - w[0].offset) / (w[1].offset - w[0].offset).max(1e-6);
            return w[0].color.lerp(w[1].color, u as f64);
        }
    }
    s.last().unwrap().color
}

pub fn swatches(core: &mut AppCore, ui: &mut egui::Ui) {
    let Some(st) = core.doc() else { return };
    let sw = st.doc.swatches.clone();
    let mut remove = None;
    let mut pick = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
        for (i, c) in sw.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
            ui.painter().rect_filled(r, 1.0, c32(*c));
            ui.painter().rect_stroke(r, 1.0, egui::Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Middle);
            let resp = resp.on_hover_text(c.to_hex());
            if resp.clicked() {
                pick = Some((*c, ui.input(|i| i.modifiers.shift)));
            }
            if resp.secondary_clicked() {
                remove = Some(i);
            }
        }
    });
    ui.separator();
    ui.weak("Click: fill · Shift+click: stroke · Right-click: remove");
    let add = ui.button("+ Add current fill color").clicked();
    if let Some((c, stroke)) = pick {
        if stroke {
            core.settings.tools.stroke_color = c;
        } else {
            core.settings.tools.fill_color = c;
            core.settings.tools.fill_kind = FillKind::Solid;
        }
    }
    let fill = core.settings.tools.fill_color;
    let st = core.doc_mut().unwrap();
    if let Some(i) = remove {
        st.checkpoint("Remove Swatch");
        st.doc.swatches.remove(i);
    }
    if add {
        st.checkpoint("Add Swatch");
        st.doc.swatches.push(fill);
    }
}

// ------------------------------------------------------------------ Align / Transform

pub fn align_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    ui.checkbox(&mut core.settings.tools.align_to_stage, "Align to stage");
    ui.label(RichText::new("Align").strong());
    let mut cmd = None;
    ui.horizontal_wrapped(|ui| {
        for (c, n) in [
            (C::AlignLeft, "⇤ Left"),
            (C::AlignHCenter, "⇹ Center"),
            (C::AlignRight, "⇥ Right"),
            (C::AlignTop, "⤒ Top"),
            (C::AlignVCenter, "⇳ Middle"),
            (C::AlignBottom, "⤓ Bottom"),
        ] {
            if ui.button(n).on_hover_text(c.label()).clicked() {
                cmd = Some(c);
            }
        }
    });
    ui.label(RichText::new("Distribute").strong());
    ui.horizontal_wrapped(|ui| {
        if ui.button("↔ Horizontal centers").clicked() {
            cmd = Some(C::DistributeH);
        }
        if ui.button("↕ Vertical centers").clicked() {
            cmd = Some(C::DistributeV);
        }
    });
    ui.label(RichText::new("Match size").strong());
    ui.horizontal_wrapped(|ui| {
        if ui.button("Width").clicked() {
            cmd = Some(C::MatchWidth);
        }
        if ui.button("Height").clicked() {
            cmd = Some(C::MatchHeight);
        }
    });
    if let Some(c) = cmd {
        core.run(c);
    }
}

pub fn transform_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    let auto_key = core.settings.tools.auto_keyframe;
    let id = egui::Id::unique("transform_panel");
    let (mut sx, mut sy, mut rot, mut skx, mut sky, mut lock) =
        ui.ctx().data(|d| d.get_temp::<(f64, f64, f64, f64, f64, bool)>(id)).unwrap_or((100.0, 100.0, 0.0, 0.0, 0.0, true));
    ui.horizontal(|ui| {
        ui.label("Scale");
        ui.add(egui::DragValue::new(&mut sx).suffix("%").speed(1.0));
        if lock {
            sy = sx;
        }
        ui.add_enabled(!lock, egui::DragValue::new(&mut sy).suffix("%").speed(1.0));
        ui.checkbox(&mut lock, "🔗");
    });
    ui.horizontal(|ui| {
        ui.label("Rotate");
        ui.add(egui::DragValue::new(&mut rot).suffix("°"));
    });
    ui.horizontal(|ui| {
        ui.label("Skew");
        ui.add(egui::DragValue::new(&mut skx).suffix("°"));
        ui.add(egui::DragValue::new(&mut sky).suffix("°"));
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, (sx, sy, rot, skx, sky, lock)));
    let mut apply = None;
    ui.horizontal(|ui| {
        if ui.button("Apply").clicked() {
            apply = Some(false);
        }
        if ui.button("Copy & apply").clicked() {
            apply = Some(true);
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Flip H").clicked() {
            core.run(C::FlipHorizontal);
        }
        if ui.button("Flip V").clicked() {
            core.run(C::FlipVertical);
        }
        if ui.button("Reset").clicked() {
            core.run(C::RemoveTransform);
        }
    });
    if let Some(copy) = apply
        && let Some(st) = core.doc_mut()
        && !st.selection.is_empty()
    {
        if copy {
            ops::duplicate(st, auto_key);
            ops::move_selection(st, Vec2::new(-10.0, -10.0), None, auto_key);
        }
        let pivot = st.selection_bounds().center();
        let m = Affine::rotate(rot.to_radians()) * Affine::skew(skx.to_radians(), sky.to_radians()) * Affine::scale(sx / 100.0, sy / 100.0);
        ops::transform_selection_about(st, m, pivot, Some("Transform"), auto_key);
    }
}

// ------------------------------------------------------------------ History / Output / Info

pub fn history_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    let Some(st) = core.doc_mut() else { return };
    let undo: Vec<String> = st.history.undo_labels().map(String::from).collect();
    let redo: Vec<String> = st.history.redo_labels().map(String::from).collect();
    let mut goto = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        if ui.selectable_label(undo.is_empty(), "◇ Open document").clicked() {
            goto = Some(0);
        }
        for (i, l) in undo.iter().enumerate() {
            if ui.selectable_label(i + 1 == undo.len(), format!("• {l}")).clicked() {
                goto = Some(i + 1);
            }
        }
        for (i, l) in redo.iter().rev().enumerate() {
            if ui.add(egui::Button::selectable(false, RichText::new(format!("◦ {l}")).weak())).clicked() {
                goto = Some(undo.len() + i + 1);
            }
        }
    });
    if let Some(n) = goto {
        st.history.goto(n, &mut st.doc);
        st.revision += 1;
        st.clamp();
    }
}

pub fn output_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.button("Clear").clicked() {
            core.log.clear();
        }
        if ui.button("Copy").clicked() {
            let text: String = core.log.iter().map(|(_, m)| format!("{m}\n")).collect();
            ui.ctx().copy_text(text);
        }
    });
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        for (lvl, m) in &core.log {
            let col = match lvl {
                LogLevel::Info => ui.visuals().text_color(),
                LogLevel::Warn => Color32::from_rgb(0xe0, 0xa0, 0x30),
                LogLevel::Error => Color32::from_rgb(0xe0, 0x50, 0x50),
            };
            ui.label(RichText::new(m).monospace().color(col));
        }
    });
}

pub fn info_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    let p = core.tstate.last_pointer_doc;
    let Some(st) = core.doc() else { return };
    ui.label(format!("Cursor  X: {:.1}  Y: {:.1}", p.x, p.y));
    let b = st.selection_bounds();
    if !b.is_empty() {
        ui.label(format!("Selection  X: {:.1}  Y: {:.1}  W: {:.1}  H: {:.1}", b.min.x, b.min.y, b.width(), b.height()));
    }
    ui.separator();
    ui.label(format!("Stage {}×{} px @ {:.2} fps", st.doc.width, st.doc.height, st.doc.fps));
    ui.label(format!(
        "Scenes: {} · Frames: {} · Library items: {}",
        st.doc.scenes.len(),
        st.doc.total_frames(),
        st.doc.library.symbols.len() + st.doc.library.bitmaps.len() + st.doc.library.sounds.len()
    ));
    ui.label(format!("Undo steps: {}", st.history.undo_labels().count()));
}

// ------------------------------------------------------------------ Actions (script editor)

fn highlight(ui: &egui::Ui, code: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let font = egui::FontId::monospace(13.0);
    let dark = ui.visuals().dark_mode;
    let kw = if dark { Color32::from_rgb(0x56, 0x9c, 0xd6) } else { Color32::from_rgb(0x00, 0x33, 0xb3) };
    let st = if dark { Color32::from_rgb(0xce, 0x91, 0x78) } else { Color32::from_rgb(0xa3, 0x15, 0x15) };
    let cm = if dark { Color32::from_rgb(0x6a, 0x99, 0x55) } else { Color32::from_rgb(0x00, 0x80, 0x00) };
    let num = if dark { Color32::from_rgb(0xb5, 0xce, 0xa8) } else { Color32::from_rgb(0x09, 0x86, 0x58) };
    let api = if dark { Color32::from_rgb(0xdc, 0xdc, 0xaa) } else { Color32::from_rgb(0x79, 0x5e, 0x26) };
    let normal = ui.visuals().text_color();
    let keywords = [
        "let", "const", "fn", "if", "else", "while", "loop", "for", "in", "return", "break", "continue", "true", "false", "this", "switch",
        "do", "until", "import", "export", "as", "private",
    ];
    let apis = animara_scripting::API_FUNCTIONS;
    let bytes: Vec<char> = code.chars().collect();
    let mut i = 0;
    let push = |job: &mut egui::text::LayoutJob, s: String, c: Color32| job.append(&s, 0.0, egui::TextFormat::simple(font.clone(), c));
    while i < bytes.len() {
        let c = bytes[i];
        if c == '/' && bytes.get(i + 1) == Some(&'/') {
            let s: String = bytes[i..].iter().take_while(|&&x| x != '\n').collect();
            i += s.chars().count();
            push(&mut job, s, cm);
        } else if c == '"' || c == '\'' {
            let mut s = String::from(c);
            i += 1;
            while i < bytes.len() && bytes[i] != c {
                s.push(bytes[i]);
                i += 1;
            }
            if i < bytes.len() {
                s.push(bytes[i]);
                i += 1;
            }
            push(&mut job, s, st);
        } else if c.is_ascii_digit() {
            let s: String = bytes[i..].iter().take_while(|x| x.is_ascii_digit() || **x == '.').collect();
            i += s.len();
            push(&mut job, s, num);
        } else if c.is_alphabetic() || c == '_' {
            let s: String = bytes[i..].iter().take_while(|x| x.is_alphanumeric() || **x == '_').collect();
            i += s.chars().count();
            let col = if keywords.contains(&s.as_str()) {
                kw
            } else if apis.contains(&s.as_str()) {
                api
            } else {
                normal
            };
            push(&mut job, s, col);
        } else {
            push(&mut job, c.to_string(), normal);
            i += 1;
        }
    }
    job
}

pub fn actions_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    let Some(st) = core.doc_mut() else { return };
    let li = st.layer;
    let f = st.frame;
    let Some(layer) = st.timeline().layers.get(li).cloned() else { return };
    let Some(k) = layer.keyframe_at(f).cloned() else {
        ui.weak("Select a frame to attach a script.");
        return;
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}: frame {}", layer.name, k.start + 1)).strong());
        ui.menu_button("Code Snippets", |ui| {
            for (name, code) in animara_scripting::SNIPPETS {
                if ui.button(*name).clicked() {
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("snippet"), code.to_string()));
                    ui.close();
                }
            }
        });
    });
    let mut script = k.script.clone();
    if let Some(sn) = ui.ctx().data_mut(|d| d.remove_temp::<String>(egui::Id::unique("snippet"))) {
        if !script.is_empty() && !script.ends_with('\n') {
            script.push('\n');
        }
        script.push_str(&sn);
    }
    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
        let mut job = highlight(ui, text.as_str());
        job.wrap.max_width = wrap;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let check = ui.button("✔ Check syntax").clicked();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add(egui::TextEdit::multiline(&mut script).code_editor().desired_rows(16).desired_width(f32::INFINITY).layouter(&mut layouter));
    });
    if script != k.script {
        if st.history.undo_labels().last() != Some("Edit Script") {
            st.checkpoint("Edit Script");
        } else {
            st.touch();
        }
        if let Some(t) = st.timeline_mut().layers[li].keyframe_at_mut(f) {
            t.script = script.clone();
        }
    }
    if check {
        match animara_scripting::check_syntax(&script) {
            Ok(()) => core.info(format!("Script on frame {} compiles without errors.", k.start + 1)),
            Err(e) => core.error(format!("Script error (frame {}): {e}", k.start + 1)),
        }
    }
}

// ------------------------------------------------------------------ Motion editor

pub fn motion_editor(core: &mut AppCore, ui: &mut egui::Ui) {
    let Some(st) = core.doc_mut() else { return };
    let li = st.layer;
    let f = st.frame;
    let Some(k) = st.timeline().layers.get(li).and_then(|l| l.keyframe_at(f)).cloned() else { return };
    let Some(mut tween) = k.tween.clone() else {
        ui.weak("Select a tweened frame to edit its easing curves.");
        return;
    };
    let prop_id = egui::Id::unique("me_prop");
    let mut prop = ui.ctx().data(|d| d.get_temp::<String>(prop_id)).unwrap_or_else(|| "all".into());
    ui.horizontal(|ui| {
        for p in ["all", "position", "rotation", "scale", "color", "filters"] {
            ui.selectable_value(&mut prop, p.to_string(), p);
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(prop_id, prop.clone()));
    let ease: &mut Ease = if prop == "all" {
        &mut tween.ease
    } else {
        if !tween.property_eases.iter().any(|(p, _)| *p == prop) {
            let e = tween.ease.clone();
            tween.property_eases.push((prop.clone(), e));
        }
        &mut tween.property_eases.iter_mut().find(|(p, _)| *p == prop).unwrap().1
    };
    ease_ui(ui, ease, "me_ease");
    // Curve graph.
    let size = egui::vec2(ui.available_width().min(420.0), 200.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect.expand(4.0));
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    let inner = rect.shrink(12.0);
    let to_screen = |x: f64, y: f64| {
        egui::pos2(inner.min.x + inner.width() * x as f32, inner.max.y - inner.height() * 0.7 * y as f32 - inner.height() * 0.15)
    };
    let from_screen = |p: egui::Pos2| {
        ((p.x - inner.min.x) as f64 / inner.width() as f64, ((inner.max.y - inner.height() * 0.15 - p.y) / (inner.height() * 0.7)) as f64)
    };
    painter.line_segment([to_screen(0.0, 0.0), to_screen(1.0, 0.0)], egui::Stroke::new(1.0, Color32::from_gray(90)));
    painter.line_segment([to_screen(0.0, 1.0), to_screen(1.0, 1.0)], egui::Stroke::new(1.0, Color32::from_gray(90)));
    let pts: Vec<egui::Pos2> = (0..=100)
        .map(|i| {
            let x = i as f64 / 100.0;
            to_screen(x, ease.eval(x))
        })
        .collect();
    painter.add(egui::Shape::line(pts, egui::Stroke::new(2.0, Color32::from_rgb(0x2b, 0x9c, 0xff))));
    // Playhead within the tween.
    let tpos = (f - k.start) as f64 / k.duration.max(1) as f64;
    painter.line_segment([to_screen(tpos, -0.2), to_screen(tpos, 1.2)], egui::Stroke::new(1.0, Color32::from_rgb(0xd0, 0x40, 0x40)));
    if let Ease::Custom { points } = ease {
        let mut drag_idx = ui.ctx().data(|d| d.get_temp::<usize>(egui::Id::unique("me_drag")));
        for (i, p) in points.iter().enumerate() {
            painter.circle(to_screen(p.x, p.y), 5.0, Color32::WHITE, egui::Stroke::new(1.0, Color32::BLACK));
            let h1 = to_screen(p.x + p.in_handle.0, p.y + p.in_handle.1);
            let h2 = to_screen(p.x + p.out_handle.0, p.y + p.out_handle.1);
            painter.line_segment([h1, h2], egui::Stroke::new(1.0, Color32::GRAY));
            let _ = i;
        }
        if let Some(pp) = resp.interact_pointer_pos() {
            let (x, y) = from_screen(pp);
            if resp.drag_started() {
                drag_idx = points.iter().position(|p| to_screen(p.x, p.y).distance(pp) < 8.0);
            }
            if resp.dragged()
                && let Some(i) = drag_idx
                && let Some(p) = points.get_mut(i)
            {
                p.x = x.clamp(0.01, 0.99);
                p.y = y.clamp(-1.0, 2.0);
            }
            if resp.double_clicked() {
                if let Some(i) = points.iter().position(|p| to_screen(p.x, p.y).distance(pp) < 8.0) {
                    points.remove(i);
                } else {
                    points.push(CurvePoint { x: x.clamp(0.01, 0.99), y, in_handle: (-0.1, 0.0), out_handle: (0.1, 0.0) });
                    points.sort_by(|a, b| a.x.total_cmp(&b.x));
                }
            }
        }
        if resp.drag_stopped() {
            drag_idx = None;
        }
        match drag_idx {
            Some(i) => {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("me_drag"), i));
            }
            None => {
                ui.ctx().data_mut(|d| d.remove_temp::<usize>(egui::Id::unique("me_drag")));
            }
        }
        ui.weak("Double-click to add/remove points; drag to shape the curve.");
    }
    if Some(&tween) != k.tween.as_ref() {
        if st.history.undo_labels().last() != Some("Edit Ease") {
            st.checkpoint("Edit Ease");
        } else {
            st.touch();
        }
        if let Some(t) = st.timeline_mut().layers[li].keyframe_at_mut(f) {
            t.tween = Some(tween);
        }
    }
}

// ------------------------------------------------------------------ Scenes

pub fn scenes_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    let Some(st) = core.doc_mut() else { return };
    let mut act: Option<(&str, usize)> = None;
    for (i, s) in st.doc.scenes.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.selectable_label(i == st.scene, &s.name).clicked() {
                act = Some(("go", i));
            }
            ui.weak(format!("{} frames", s.timeline.length()));
            if ui.small_button("⬆").clicked() && i > 0 {
                act = Some(("up", i));
            }
            if ui.small_button("⧉").on_hover_text("Duplicate").clicked() {
                act = Some(("dup", i));
            }
            if ui.small_button("✕").on_hover_text("Delete").clicked() {
                act = Some(("del", i));
            }
        });
    }
    if ui.button("+ Add scene").clicked() {
        ops::add_scene(st);
    }
    let mut name = st.doc.scenes.get(st.scene).map(|s| s.name.clone()).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Rename");
        if ui.text_edit_singleline(&mut name).lost_focus() && !name.is_empty() && st.doc.scenes[st.scene].name != name {
            st.checkpoint("Rename Scene");
            let i = st.scene;
            st.doc.scenes[i].name = name.clone();
        }
    });
    if let Some((a, i)) = act {
        match a {
            "go" => {
                st.scene = i;
                st.stack.clear();
                st.frame = 0;
                st.layer = 0;
                st.selection.clear();
                st.revision += 1;
            }
            "up" => {
                st.checkpoint("Reorder Scenes");
                st.doc.scenes.swap(i, i - 1);
                st.scene = i - 1;
            }
            "dup" => {
                st.checkpoint("Duplicate Scene");
                let mut s = st.doc.scenes[i].clone();
                s.id = animara_document::fresh_id();
                s.name = format!("{} copy", s.name);
                for l in &mut s.timeline.layers {
                    l.id = animara_document::fresh_id();
                    for k in &mut l.frames {
                        k.elements.iter_mut().for_each(Element::reassign_ids);
                    }
                }
                st.doc.scenes.insert(i + 1, s);
            }
            "del" if st.doc.scenes.len() > 1 => {
                st.checkpoint("Delete Scene");
                st.doc.scenes.remove(i);
                st.stack.clear();
                st.clamp();
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------------ Components & Assets

pub fn components_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    ui.weak("Pre-built interactive components. Each is created as an editable symbol in the library.");
    for (name, kind) in crate::thumbs::COMPONENTS {
        if ui.button(format!("◆ {name}")).clicked() {
            let auto_key = core.settings.tools.auto_keyframe;
            if let Some(st) = core.doc_mut() {
                st.checkpoint(&format!("Add {name}"));
                let id = crate::thumbs::build_component(&mut st.doc, kind);
                let e = st.doc.instantiate(id, st.doc.width / 2.0, st.doc.height / 2.0);
                if let Some(k) = st.editable_keyframe(auto_key) {
                    let eid = e.id;
                    k.elements.push(e);
                    st.selection = vec![eid];
                }
            }
        }
    }
}

pub fn assets_panel(core: &mut AppCore, ui: &mut egui::Ui) {
    let tab_id = egui::Id::unique("assets_tab");
    let mut custom = ui.ctx().data(|d| d.get_temp::<bool>(tab_id)).unwrap_or(false);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut custom, false, "Default");
        ui.selectable_value(&mut custom, true, "Custom");
    });
    ui.ctx().data_mut(|d| d.insert_temp(tab_id, custom));
    ui.separator();
    if !custom {
        ui.label(RichText::new("Animated").strong());
        for name in crate::thumbs::DEFAULT_ASSETS {
            if ui.button(format!("▶ {name}")).clicked()
                && let Some(st) = core.doc_mut()
            {
                st.checkpoint(&format!("Add asset {name}"));
                let id = crate::thumbs::build_default_asset(&mut st.doc, name);
                let e = st.doc.instantiate(id, st.doc.width / 2.0, st.doc.height / 2.0);
                let eid = e.id;
                if let Some(k) = st.editable_keyframe(true) {
                    k.elements.push(e);
                    st.selection = vec![eid];
                }
            }
        }
    } else {
        let dir = crate::core::Settings::config_dir().join("assets");
        let saved: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "anim")).collect())
            .unwrap_or_default();
        if saved.is_empty() {
            ui.weak("No custom assets yet. Select a symbol in the Library and click “Save selected symbol as asset”.");
        }
        for p in saved {
            let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            ui.horizontal(|ui| {
                if ui.button(format!("◆ {name}")).clicked() {
                    match animara_document::load_document(&p) {
                        Ok(src) => {
                            if let Some(st) = core.docs.get_mut(core.active) {
                                st.checkpoint("Insert Asset");
                                let ids = crate::thumbs::merge_library(&mut st.doc, &src);
                                if let Some(first) = ids.first() {
                                    let e = st.doc.instantiate(*first, st.doc.width / 2.0, st.doc.height / 2.0);
                                    if let Some(k) = st.editable_keyframe(true) {
                                        k.elements.push(e);
                                    }
                                }
                            }
                        }
                        Err(e) => core.log.push((LogLevel::Error, format!("Asset load failed: {e}"))),
                    }
                }
                if ui.small_button("✕").clicked() {
                    let _ = std::fs::remove_file(&p);
                }
            });
        }
        if let Some(sel) = core.library_selected
            && ui.button("Save selected symbol as asset").clicked()
            && let Some(st) = core.doc()
            && let Some(s) = st.doc.library.symbol(sel)
        {
            let mut d = animara_document::Document::new(st.doc.width, st.doc.height, st.doc.fps);
            d.name = s.name.clone();
            crate::thumbs::copy_symbol_with_deps(&st.doc, sel, &mut d);
            let _ = std::fs::create_dir_all(&dir);
            match animara_document::save_document(&d, &dir.join(format!("{}.anim", sanitize(&s.name)))) {
                Ok(()) => core.info(format!("Saved asset “{}”.", s.name)),
                Err(e) => core.error(format!("Saving asset failed: {e}")),
            }
        }
    }
}

pub fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }).collect()
}
