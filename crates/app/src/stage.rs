//! The stage: rendering, overlays (selection, handles, guides, rulers, bones,
//! motion paths, onion skins) and all pointer/keyboard tool interactions.

use crate::core::AppCore;
use crate::ops;
use crate::state::DocState;
use crate::tools::{Drag, EraserMode, Handle, PencilMode, Tool};
use animara_animation::{DisplayItem, EvalOptions, evaluate_timeline, layer_elements_at, recolor_item};
use animara_document::{
    Armature, Color, ColorTransform, Element, ElementKind, Fill, LayerKind, Pose, Primitive, Shape, ShapePath, TextElement,
};
use animara_geometry::{Affine, BoolOp, FillRule, Path, PathCmd, Rect, StrokeSample, Vec2, boolean_op};
use animara_renderer::bounds::{element_bounds, element_hit};
use animara_renderer::tiny_skia;
use egui::{Color32, Pos2, Sense, Stroke as EStroke};
use std::hash::{Hash, Hasher};

const SEL_COLOR: Color32 = Color32::from_rgb(0x2b, 0x9c, 0xff);
const HANDLE: f32 = 7.0;

fn to_pos(v: Vec2) -> Pos2 {
    Pos2::new(v.x as f32, v.y as f32)
}
fn to_vec(p: Pos2) -> Vec2 {
    Vec2::new(p.x as f64, p.y as f64)
}
pub fn c32(c: Color) -> Color32 {
    let [r, g, b, a] = c.to_rgba8();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Document → screen-point transform.
pub fn view_matrix(st: &DocState, canvas: egui::Rect) -> Affine {
    Affine::translate(canvas.min.x as f64 + st.view.pan.x, canvas.min.y as f64 + st.view.pan.y) * Affine::scale(st.view.zoom, st.view.zoom)
}

pub fn fit(st: &mut DocState, canvas: egui::Rect) {
    let (w, h) = (st.doc.width, st.doc.height);
    let z = ((canvas.width() as f64 - 60.0) / w).min((canvas.height() as f64 - 60.0) / h).clamp(0.02, 64.0);
    st.view.zoom = z;
    st.view.pan = Vec2::new((canvas.width() as f64 - w * z) / 2.0, (canvas.height() as f64 - h * z) / 2.0);
    st.view.needs_fit = false;
}

pub fn zoom_about(st: &mut DocState, factor: f64, screen_local: Vec2) {
    let old = st.view.zoom;
    let new = (old * factor).clamp(0.02, 64.0);
    // Keep the document point under the cursor fixed.
    let doc_pt = (screen_local - st.view.pan) / old;
    st.view.zoom = new;
    st.view.pan = screen_local - doc_pt * new;
}

/// Authoring display list for the current editing context (including dimmed parents).
fn authoring_items(st: &DocState, outlines: bool) -> (Vec<DisplayItem>, Vec<DisplayItem>) {
    let opts = EvalOptions { authoring: true, respect_visibility: true, ..Default::default() };
    let ctx = st.context_transform();
    let mut parent_items = Vec::new();
    if let Some(top) = st.stack.last()
        && top.in_place
    {
        // Parent levels, dimmed, so edit-in-place shows surrounding content.
        let parent_sym = if st.stack.len() >= 2 { Some(st.stack[st.stack.len() - 2].symbol) } else { None };
        if let Some(ptl) = st.doc.timeline(st.scene, parent_sym) {
            let mut pctx = Affine::IDENTITY;
            for lvl in &st.stack[..st.stack.len() - 1] {
                pctx = if lvl.in_place { pctx * lvl.instance_transform } else { Affine::IDENTITY };
            }
            let dim = ColorTransform { mul: [1.0, 1.0, 1.0, 0.35], add: [0.0; 4] };
            parent_items = evaluate_timeline(&st.doc, ptl, top.parent_frame, pctx, ColorTransform::IDENTITY, &opts, 0, false)
                .into_iter()
                .map(|i| recolor_item(i, &dim))
                .collect();
        }
    }
    let mut items = Vec::new();
    let tl = st.timeline();
    if st.onion.enabled && !st.playing {
        for d in (1..=st.onion.before).rev() {
            if let Some(f) = st.frame.checked_sub(d) {
                let a = 0.45 * (1.0 - (d as f32 - 1.0) / st.onion.before.max(1) as f32);
                let tint = ColorTransform { mul: [0.3, 0.3, 0.3, a], add: [0.1, 0.25, 0.7 * a, 0.0] };
                for it in evaluate_timeline(&st.doc, tl, f, ctx, ColorTransform::IDENTITY, &opts, 0, false) {
                    items.push(onion(recolor_item(it, &tint), st.onion.outline, Color::rgba(0.2, 0.4, 1.0, a)));
                }
            }
        }
        for d in 1..=st.onion.after {
            let f = st.frame + d;
            if f < tl.length() {
                let a = 0.45 * (1.0 - (d as f32 - 1.0) / st.onion.after.max(1) as f32);
                let tint = ColorTransform { mul: [0.3, 0.3, 0.3, a], add: [0.1, 0.6 * a, 0.2, 0.0] };
                for it in evaluate_timeline(&st.doc, tl, f, ctx, ColorTransform::IDENTITY, &opts, 0, false) {
                    items.push(onion(recolor_item(it, &tint), st.onion.outline, Color::rgba(0.2, 0.8, 0.3, a)));
                }
            }
        }
    }
    let mut cur = evaluate_timeline(&st.doc, tl, st.frame, ctx, ColorTransform::IDENTITY, &opts, 0, true);
    if outlines {
        cur = cur.into_iter().map(|i| onion(i, true, Color::BLACK)).collect();
    }
    items.extend(cur);
    // Hide text being edited (drawn by the overlay with a caret).
    (parent_items, items)
}

fn onion(i: DisplayItem, outline: bool, c: Color) -> DisplayItem {
    if !outline {
        return i;
    }
    match i {
        DisplayItem::Shape { shape, transform, color, source, .. } => DisplayItem::Shape { shape, transform, color, outline: Some(c), source },
        DisplayItem::Group { items, filters, blend, scale, source } => {
            DisplayItem::Group { items: items.into_iter().map(|x| onion(x, true, c)).collect(), filters, blend, scale, source }
        }
        DisplayItem::Masked { mask, content } => DisplayItem::Masked { mask, content: content.into_iter().map(|x| onion(x, true, c)).collect() },
        o => o,
    }
}

/// Render the stage into the document's texture (cached by a content key).
fn update_texture(core: &mut AppCore, ui: &egui::Ui, canvas: egui::Rect) {
    let ppp = ui.ctx().pixels_per_point() as f64;
    let dark = core.settings.dark;
    let outlines = core.settings.preview_outlines;
    let pasteboard = core.settings.pasteboard_content;
    let Some(st) = core.doc_mut() else { return };
    let pw = (canvas.width() as f64 * ppp).round().max(1.0) as u32;
    let ph = (canvas.height() as f64 * ppp).round().max(1.0) as u32;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (st.revision, st.frame, st.scene, st.stack.len(), pw, ph, dark, outlines, pasteboard).hash(&mut h);
    (st.view.zoom.to_bits(), st.view.pan.x.to_bits(), st.view.pan.y.to_bits()).hash(&mut h);
    (st.onion.enabled, st.onion.before, st.onion.after, st.onion.outline, st.playing).hash(&mut h);
    st.text_edit.as_ref().map(|t| t.element).hash(&mut h);
    let key = h.finish();
    if key == st.tex_key && st.texture.is_some() {
        return;
    }
    st.tex_key = key;
    let Some(mut pm) = tiny_skia::Pixmap::new(pw, ph) else { return };
    let paste = if dark { tiny_skia::Color::from_rgba8(0x3a, 0x3a, 0x3a, 255) } else { tiny_skia::Color::from_rgba8(0xd8, 0xd8, 0xd8, 255) };
    pm.fill(paste);
    let view_px = Affine::scale(ppp, ppp) * Affine::translate(st.view.pan.x, st.view.pan.y) * Affine::scale(st.view.zoom, st.view.zoom);
    let (parent_items, mut items) = authoring_items(st, outlines);
    if let Some(te) = &st.text_edit {
        let id = te.element;
        items.retain(|i| !matches!(i, DisplayItem::Text { source: Some(s), .. } if *s == id));
    }
    let stage = Rect::new(0.0, 0.0, st.doc.width, st.doc.height);
    let ts = animara_renderer::to_ts(&view_px);
    if let Some(r) = tiny_skia::Rect::from_xywh(0.0, 0.0, stage.width() as f32, stage.height() as f32) {
        let mut shadow = tiny_skia::Paint::default();
        shadow.set_color(tiny_skia::Color::from_rgba8(0, 0, 0, 60));
        let sh = Affine::scale(ppp, ppp) * Affine::translate(st.view.pan.x + 3.0, st.view.pan.y + 3.0) * Affine::scale(st.view.zoom, st.view.zoom);
        pm.fill_rect(r, &shadow, animara_renderer::to_ts(&sh), None);
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(animara_renderer::ts_color(st.doc.background));
        pm.fill_rect(r, &paint, ts, None);
    }
    {
        let doc = &st.doc;
        let mut r = animara_renderer::Renderer { doc, cache: &mut st.cache };
        r.draw_items(&mut pm, &parent_items, &view_px);
        r.draw_items(&mut pm, &items, &view_px);
    }
    if !pasteboard {
        // Hide content outside the stage.
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(paste);
        let s0 = view_px.apply(Vec2::ZERO);
        let s1 = view_px.apply(Vec2::new(stage.width(), stage.height()));
        for (x, y, w, h) in [
            (0.0, 0.0, pw as f64, s0.y),
            (0.0, s1.y, pw as f64, ph as f64 - s1.y),
            (0.0, s0.y, s0.x, s1.y - s0.y),
            (s1.x, s0.y, pw as f64 - s1.x, s1.y - s0.y),
        ] {
            if let Some(r) = tiny_skia::Rect::from_xywh(x as f32, y as f32, w.max(0.0) as f32, h.max(0.0) as f32) {
                pm.fill_rect(r, &paint, tiny_skia::Transform::identity(), None);
            }
        }
    }
    let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pm.data());
    match &mut st.texture {
        Some(t) => t.set(img, egui::TextureOptions::NEAREST),
        None => st.texture = Some(ui.ctx().load_texture(format!("stage-{}", st.uid), img, egui::TextureOptions::NEAREST)),
    }
}

/// Topmost element under a document point on editable layers.
pub fn pick(st: &DocState, p: Vec2, tol: f64, include_locked_layers: bool) -> Option<(usize, Element)> {
    let ctx = st.context_transform();
    let tl = st.timeline();
    for li in 0..tl.layers.len() {
        if !tl.effectively_visible(li) || (!include_locked_layers && tl.effectively_locked(li)) {
            continue;
        }
        if matches!(tl.layers[li].kind, LayerKind::Folder | LayerKind::Camera) {
            continue;
        }
        let els = st.layer_elements(li);
        for e in els.iter().rev() {
            if !e.locked && element_hit(&st.doc, e, &ctx, p, tol) {
                return Some((li, e.clone()));
            }
        }
    }
    None
}

fn snap(core: &AppCore, st: &DocState, p: Vec2, view: &Affine, ignore: &[animara_document::Id]) -> Vec2 {
    let tol = 8.0 / view.mean_scale();
    let mut out = p;
    let mut best = (tol, tol);
    if st.doc.grid.snap {
        let s = st.doc.grid.spacing.max(1.0);
        let g = Vec2::new((p.x / s).round() * s, (p.y / s).round() * s);
        if (g.x - p.x).abs() < best.0 {
            best.0 = (g.x - p.x).abs();
            out.x = g.x;
        }
        if (g.y - p.y).abs() < best.1 {
            best.1 = (g.y - p.y).abs();
            out.y = g.y;
        }
    }
    if st.doc.guides.snap && st.doc.guides.visible {
        for &x in &st.doc.guides.vertical {
            if (x - p.x).abs() < best.0 {
                best.0 = (x - p.x).abs();
                out.x = x;
            }
        }
        for &y in &st.doc.guides.horizontal {
            if (y - p.y).abs() < best.1 {
                best.1 = (y - p.y).abs();
                out.y = y;
            }
        }
    }
    if core.settings.tools.snap_objects {
        let ctx = st.context_transform();
        let mut pts = vec![Vec2::ZERO, Vec2::new(st.doc.width, st.doc.height), Vec2::new(st.doc.width / 2.0, st.doc.height / 2.0)];
        for li in 0..st.timeline().layers.len() {
            for e in st.layer_elements(li) {
                if ignore.contains(&e.id) {
                    continue;
                }
                let b = element_bounds(&st.doc, &e, &ctx);
                if !b.is_empty() {
                    pts.extend(b.corners());
                    pts.push(b.center());
                }
            }
        }
        for q in pts {
            if (q - p).length() < tol {
                return q;
            }
        }
    }
    out
}

/// Oriented box of the selection: (frame → world, local rect).
fn selection_box(st: &DocState) -> Option<(Affine, Rect)> {
    let sel = st.selected();
    if sel.is_empty() {
        return None;
    }
    let ctx = st.context_transform();
    if sel.len() == 1 {
        let e = &sel[0].1;
        let mut ident = e.clone();
        ident.transform = Affine::IDENTITY;
        let local = element_bounds(&st.doc, &ident, &Affine::IDENTITY);
        let m = ctx * e.transform;
        if !local.is_empty() && m.determinant().abs() > 1e-9 {
            return Some((m, local));
        }
    }
    let b = st.selection_bounds();
    (!b.is_empty()).then_some((Affine::IDENTITY, b))
}

fn selection_pivot(st: &DocState) -> Option<Vec2> {
    let sel = st.selected();
    if sel.len() == 1 {
        return Some(st.context_transform().apply(sel[0].1.pivot_world()));
    }
    let b = st.selection_bounds();
    (!b.is_empty()).then(|| b.center())
}

fn handle_at(st: &DocState, view: &Affine, sp: Vec2) -> Option<Handle> {
    let (frame, local) = selection_box(st)?;
    let to_screen = |u: f64, v: f64| view.apply(frame.apply(Vec2::new(local.min.x + local.width() * u, local.min.y + local.height() * v)));
    if let Some(pv) = selection_pivot(st)
        && view.apply(pv).distance(sp) < 7.0
    {
        return Some(Handle::Pivot);
    }
    for (u, v) in [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 0.5), (1.0, 1.0), (0.5, 1.0), (0.0, 1.0), (0.0, 0.5)] {
        if to_screen(u, v).distance(sp) < HANDLE as f64 {
            return Some(Handle::Scale(u, v));
        }
    }
    for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
        let d = to_screen(u, v).distance(sp);
        if d < 22.0 {
            return Some(Handle::Rotate);
        }
    }
    // Edges (between handles) skew.
    let edges = [((0.0, 0.0), (1.0, 0.0), true, 0.0), ((0.0, 1.0), (1.0, 1.0), true, 1.0), ((0.0, 0.0), (0.0, 1.0), false, 0.0), ((1.0, 0.0), (1.0, 1.0), false, 1.0)];
    for ((u0, v0), (u1, v1), horiz, side) in edges {
        let a = to_screen(u0, v0);
        let b = to_screen(u1, v1);
        let ab = b - a;
        let t = ((sp - a).dot(ab) / ab.length_sq().max(1e-9)).clamp(0.0, 1.0);
        if (a + ab * t).distance(sp) < 4.0 && t > 0.15 && t < 0.85 && !(t > 0.4 && t < 0.6) {
            return Some(Handle::Skew(horiz, side));
        }
    }
    None
}

fn new_shape_element(core: &AppCore, path: Path, closed: bool, primitive: Option<Primitive>) -> Element {
    let t = &core.settings.tools;
    let b = path.bounds();
    let fill = if closed { t.fill_for(b) } else { None };
    let stroke = t.stroke();
    let stroke = if fill.is_none() && stroke.is_none() { Some(animara_document::Stroke::solid(t.stroke_color, 1.0)) } else { stroke };
    // Store geometry relative to its center so transforms pivot naturally.
    let c = b.center();
    let c = if c.is_finite() { c } else { Vec2::ZERO };
    let local = path.translate(-c);
    let mut fill_local = fill;
    if let Some(f) = &mut fill_local {
        f.set_matrix(Affine::translate(-c.x, -c.y) * f.matrix().unwrap_or(Affine::IDENTITY));
    }
    let primitive = primitive.map(|p| match p {
        Primitive::Rect { rect, corner_radius } => Primitive::Rect {
            rect: Rect::new(rect.min.x - c.x, rect.min.y - c.y, rect.max.x - c.x, rect.max.y - c.y),
            corner_radius,
        },
        Primitive::Oval { rect, start_angle, end_angle, inner_radius } => Primitive::Oval {
            rect: Rect::new(rect.min.x - c.x, rect.min.y - c.y, rect.max.x - c.x, rect.max.y - c.y),
            start_angle,
            end_angle,
            inner_radius,
        },
        Primitive::Polygon { center, radius, sides, star, inner_ratio, rotation } => {
            Primitive::Polygon { center: center - c, radius, sides, star, inner_ratio, rotation }
        }
    });
    let sp = ShapePath { path: local, fill: fill_local, stroke, fill_rule: FillRule::NonZero, primitive };
    Element::shape(vec![sp]).with_transform(Affine::translate(c.x, c.y))
}

fn filled_element(path: Path, fill: Fill) -> Element {
    let c = path.bounds().center();
    let c = if c.is_finite() { c } else { Vec2::ZERO };
    let mut f = fill;
    f.set_matrix(Affine::translate(-c.x, -c.y) * f.matrix().unwrap_or(Affine::IDENTITY));
    Element::shape(vec![ShapePath::filled(path.translate(-c), f)]).with_transform(Affine::translate(c.x, c.y))
}

fn rect_from_drag(a: Vec2, b: Vec2, shift: bool, alt: bool) -> Rect {
    let mut d = b - a;
    if shift {
        let m = d.x.abs().max(d.y.abs());
        d = Vec2::new(m * d.x.signum(), m * d.y.signum());
    }
    if alt { Rect::from_points(a - d, a + d) } else { Rect::from_points(a, a + d) }
}

fn constrain_angle(a: Vec2, b: Vec2) -> Vec2 {
    let d = b - a;
    let ang = (d.angle() / (std::f64::consts::PI / 4.0)).round() * (std::f64::consts::PI / 4.0);
    a + Vec2::from_angle(ang) * d.length()
}

/// Erase a brush outline (world coords) from shapes on editable layers.
fn erase(core: &mut AppCore, outline: &Path, faucet_point: Option<Vec2>) {
    let mode = core.settings.tools.eraser_mode;
    let auto_key = core.settings.tools.auto_keyframe;
    let Some(st) = core.doc_mut() else { return };
    let ctx = st.context_transform();
    let ob = outline.bounds();
    let mut changed = false;
    let mut first = true;
    for li in 0..st.timeline().layers.len() {
        if !st.layer_editable(li) {
            continue;
        }
        let els = st.layer_elements(li);
        let targets: Vec<Element> = els
            .into_iter()
            .filter(|e| matches!(e.kind, ElementKind::Shape(_)) && element_bounds(&st.doc, e, &ctx).intersects(&ob.inflate(1.0)))
            .collect();
        for e in targets {
            let m = ctx * e.transform;
            let Some(inv) = m.invert() else { continue };
            let local_outline = outline.transform(&inv);
            let ElementKind::Shape(shape) = &e.kind else { continue };
            let mut new_paths = Vec::new();
            for sp in &shape.paths {
                if let Some(fp) = faucet_point {
                    let lp = inv.apply(fp);
                    if sp.hit(lp, 3.0 / m.mean_scale().max(1e-6)) {
                        changed = true;
                        continue;
                    }
                    new_paths.push(sp.clone());
                    continue;
                }
                if sp.fill.is_some() && mode != EraserMode::LinesOnly {
                    let cut = boolean_op(&sp.path, &local_outline, BoolOp::Difference, sp.fill_rule);
                    let lb = local_outline.bounds();
                    if sp.path.bounds().intersects(&lb) {
                        changed = true;
                        if !cut.is_empty() {
                            new_paths.push(ShapePath { path: cut, primitive: None, ..sp.clone() });
                        }
                        continue;
                    }
                }
                if sp.fill.is_none() && sp.stroke.is_some() && mode != EraserMode::FillsOnly {
                    // Split stroked polylines where the eraser covers them.
                    let mut pieces = Path::new();
                    let mut touched = false;
                    for (pts, closed) in sp.path.flatten(0.5) {
                        let mut pts = pts;
                        if closed && let Some(f) = pts.first().copied() {
                            pts.push(f);
                        }
                        let mut run: Vec<Vec2> = Vec::new();
                        for q in pts {
                            if local_outline.contains(q, FillRule::NonZero) {
                                touched = true;
                                if run.len() > 1 {
                                    pieces.extend(&Path::polyline(&run, false));
                                }
                                run.clear();
                            } else {
                                run.push(q);
                            }
                        }
                        if run.len() > 1 {
                            pieces.extend(&Path::polyline(&run, false));
                        }
                    }
                    if touched {
                        changed = true;
                        if !pieces.is_empty() {
                            new_paths.push(ShapePath { path: pieces, primitive: None, ..sp.clone() });
                        }
                        continue;
                    }
                }
                new_paths.push(sp.clone());
            }
            if changed {
                if first {
                    st.checkpoint("Erase");
                    first = false;
                }
                st.layer = li;
                if let Some(target) = st.element_mut(e.id, auto_key) {
                    if new_paths.is_empty() {
                        target.kind = ElementKind::Shape(Shape::default());
                    } else {
                        target.kind = ElementKind::Shape(Shape { paths: new_paths.clone() });
                    }
                }
                // Drop emptied shapes.
                let f = st.frame;
                if let Some(k) = st.timeline_mut().layers[li].keyframe_at_mut(f) {
                    k.elements.retain(|x| !matches!(&x.kind, ElementKind::Shape(s) if s.paths.is_empty()));
                }
                changed = false;
            }
        }
    }
}

fn armature_layer_index(st: &DocState) -> Option<usize> {
    let tl = st.timeline();
    if tl.layers.get(st.layer).is_some_and(|l| l.kind == LayerKind::Armature) {
        return Some(st.layer);
    }
    tl.layers.iter().position(|l| l.kind == LayerKind::Armature)
}

fn current_pose(st: &DocState, li: usize) -> Option<(Armature, Pose)> {
    let l = &st.timeline().layers[li];
    let arm = l.armature.clone()?;
    let pose = animara_animation::pose_at(l, st.frame).unwrap_or_else(|| animara_rigging::rest_pose(&arm));
    Some((arm, pose))
}

/// Main stage widget.
pub fn stage_ui(core: &mut AppCore, ui: &mut egui::Ui) {
    if core.docs.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new("No document open — File ▸ New (Ctrl+N) or File ▸ Open (Ctrl+O)").size(16.0).weak());
        });
        return;
    }
    edit_bar(core, ui);
    let rulers = core.settings.rulers;
    let full = ui.available_rect_before_wrap();
    let canvas = if rulers { egui::Rect::from_min_max(full.min + egui::vec2(18.0, 18.0), full.max) } else { full };
    let resp = ui.allocate_rect(full, Sense::click_and_drag());
    {
        let st = core.docs.get_mut(core.active).unwrap();
        if st.view.needs_fit {
            fit(st, canvas);
        }
    }
    update_texture(core, ui, canvas);
    let painter = ui.painter_at(full);
    if let Some(tex) = &core.doc().unwrap().texture {
        painter.image(tex.id(), canvas, egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
    let view = view_matrix(core.doc().unwrap(), canvas);
    let inv_view = view.invert().unwrap_or(Affine::IDENTITY);
    draw_overlays(core, &painter, &view, canvas);
    if rulers {
        draw_rulers(core, &painter, &view, full, canvas);
    }
    handle_input(core, ui, &resp, &view, &inv_view, canvas, full);
}

fn edit_bar(core: &mut AppCore, ui: &mut egui::Ui) {
    let mut exit_to: Option<usize> = None;
    let mut zoom_to: Option<f64> = None;
    let mut fit_now = false;
    let mut scene_to: Option<usize> = None;
    let mut edit_symbol: Option<animara_document::Id> = None;
    {
        let st = core.doc().unwrap();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if !st.stack.is_empty() && ui.button("⬅").on_hover_text("Back (exit symbol editing)").clicked() {
                exit_to = Some(st.stack.len() - 1);
            }
            let scene_name = st.doc.scenes.get(st.scene).map(|s| s.name.clone()).unwrap_or_default();
            if ui.selectable_label(st.stack.is_empty(), format!("🎬 {scene_name}")).clicked() {
                exit_to = Some(0);
            }
            for (i, lvl) in st.stack.iter().enumerate() {
                ui.label("›");
                let name = st.doc.library.symbol(lvl.symbol).map(|s| s.name.clone()).unwrap_or_default();
                if ui.selectable_label(i + 1 == st.stack.len(), format!("◆ {name}")).clicked() {
                    exit_to = Some(i + 1);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut z = st.view.zoom * 100.0;
                egui::ComboBox::from_id_salt("zoom")
                    .selected_text(format!("{:.0}%", z))
                    .width(70.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(false, "Fit in Window").clicked() {
                            fit_now = true;
                        }
                        for p in [25.0, 50.0, 64.0, 100.0, 200.0, 400.0, 800.0] {
                            if ui.selectable_value(&mut z, p, format!("{p:.0}%")).clicked() {
                                zoom_to = Some(p / 100.0);
                            }
                        }
                    });
                ui.menu_button("◆", |ui| {
                    ui.label("Edit Symbol");
                    for s in &st.doc.library.symbols {
                        if ui.button(&s.name).clicked() {
                            edit_symbol = Some(s.id);
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("Edit symbols");
                ui.menu_button("🎬", |ui| {
                    for (i, s) in st.doc.scenes.iter().enumerate() {
                        if ui.selectable_label(i == st.scene, &s.name).clicked() {
                            scene_to = Some(i);
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("Edit scene");
            });
        });
    }
    let st = core.docs.get_mut(core.active).unwrap();
    if let Some(l) = exit_to {
        ops::exit_to_level(st, l);
    }
    if let Some(z) = zoom_to {
        st.view.zoom = z;
        st.view.needs_fit = false;
    }
    if fit_now {
        st.view.needs_fit = true;
    }
    if let Some(s) = scene_to {
        st.scene = s;
        st.stack.clear();
        st.frame = 0;
        st.layer = 0;
        st.selection.clear();
        st.revision += 1;
    }
    if let Some(id) = edit_symbol {
        st.stack.clear();
        ops::enter_symbol(st, id, None);
    }
}

fn draw_rulers(core: &AppCore, painter: &egui::Painter, view: &Affine, full: egui::Rect, canvas: egui::Rect) {
    let bg = if core.settings.dark { Color32::from_gray(45) } else { Color32::from_gray(225) };
    let fg = if core.settings.dark { Color32::from_gray(170) } else { Color32::from_gray(60) };
    let top = egui::Rect::from_min_max(Pos2::new(canvas.min.x, full.min.y), Pos2::new(full.max.x, canvas.min.y));
    let left = egui::Rect::from_min_max(Pos2::new(full.min.x, canvas.min.y), Pos2::new(canvas.min.x, full.max.y));
    painter.rect_filled(top, 0.0, bg);
    painter.rect_filled(left, 0.0, bg);
    painter.rect_filled(egui::Rect::from_min_max(full.min, canvas.min), 0.0, bg);
    let z = view.mean_scale();
    let step = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0].into_iter().find(|s| s * z >= 50.0).unwrap_or(1000.0);
    let inv = view.invert().unwrap_or(Affine::IDENTITY);
    let x0 = (inv.apply(Vec2::new(canvas.min.x as f64, 0.0)).x / step).floor() * step;
    let x1 = inv.apply(Vec2::new(canvas.max.x as f64, 0.0)).x;
    let mut x = x0;
    while x <= x1 {
        let sx = view.apply(Vec2::new(x, 0.0)).x as f32;
        painter.line_segment([Pos2::new(sx, top.max.y - 8.0), Pos2::new(sx, top.max.y)], EStroke::new(1.0, fg));
        painter.text(Pos2::new(sx + 2.0, top.min.y + 1.0), egui::Align2::LEFT_TOP, format!("{x:.0}"), egui::FontId::proportional(9.0), fg);
        x += step;
    }
    let y0 = (inv.apply(Vec2::new(0.0, canvas.min.y as f64)).y / step).floor() * step;
    let y1 = inv.apply(Vec2::new(0.0, canvas.max.y as f64)).y;
    let mut y = y0;
    while y <= y1 {
        let sy = view.apply(Vec2::new(0.0, y)).y as f32;
        painter.line_segment([Pos2::new(left.max.x - 8.0, sy), Pos2::new(left.max.x, sy)], EStroke::new(1.0, fg));
        painter.text(Pos2::new(left.min.x + 1.0, sy + 2.0), egui::Align2::LEFT_TOP, format!("{y:.0}"), egui::FontId::proportional(9.0), fg);
        y += step;
    }
}

fn draw_overlays(core: &AppCore, painter: &egui::Painter, view: &Affine, canvas: egui::Rect) {
    let st = core.doc().unwrap();
    let doc = &st.doc;
    // Grid.
    if doc.grid.visible {
        let s = doc.grid.spacing.max(2.0);
        let col = c32(doc.grid.color);
        let mut x = 0.0;
        while x <= doc.width {
            painter.line_segment([to_pos(view.apply(Vec2::new(x, 0.0))), to_pos(view.apply(Vec2::new(x, doc.height)))], EStroke::new(1.0, col));
            x += s;
        }
        let mut y = 0.0;
        while y <= doc.height {
            painter.line_segment([to_pos(view.apply(Vec2::new(0.0, y))), to_pos(view.apply(Vec2::new(doc.width, y)))], EStroke::new(1.0, col));
            y += s;
        }
    }
    // Guides.
    if doc.guides.visible {
        let col = Color32::from_rgb(0x2b, 0xd6, 0xd6);
        for &x in &doc.guides.vertical {
            let sx = view.apply(Vec2::new(x, 0.0)).x as f32;
            painter.line_segment([Pos2::new(sx, canvas.min.y), Pos2::new(sx, canvas.max.y)], EStroke::new(1.0, col));
        }
        for &y in &doc.guides.horizontal {
            let sy = view.apply(Vec2::new(0.0, y)).y as f32;
            painter.line_segment([Pos2::new(canvas.min.x, sy), Pos2::new(canvas.max.x, sy)], EStroke::new(1.0, col));
        }
    }
    let ctx = st.context_transform();
    // Motion paths of selected tweened elements.
    for (li, e) in st.selected() {
        let layer = &st.timeline().layers[li];
        if let Some(k) = layer.keyframe_at(st.frame)
            && k.tween.is_some()
        {
            let pts: Vec<Pos2> = (k.start..=k.end().min(st.timeline().length().saturating_sub(1)))
                .filter_map(|f| {
                    let (els, _) = layer_elements_at(doc, st.timeline(), li, f);
                    els.iter().find(|x| x.id == e.id).map(|x| to_pos(view.apply(ctx.apply(x.pivot_world()))))
                })
                .collect();
            if pts.len() > 1 {
                painter.add(egui::Shape::line(pts.clone(), EStroke::new(1.0, Color32::from_rgb(0x9b, 0x59, 0xf6))));
                for p in pts {
                    painter.circle_filled(p, 2.0, Color32::from_rgb(0x9b, 0x59, 0xf6));
                }
            }
        }
    }
    // Bones.
    for li in 0..st.timeline().layers.len() {
        let l = &st.timeline().layers[li];
        if l.kind != LayerKind::Armature || !l.visible {
            continue;
        }
        if let Some((arm, pose)) = current_pose(st, li) {
            for w in animara_rigging::forward(&arm, &pose) {
                let a = to_pos(view.apply(ctx.apply(w.head)));
                let b = to_pos(view.apply(ctx.apply(w.tail)));
                let n = (b - a).normalized().rot90() * 5.0;
                let mid = a + (b - a) * 0.15;
                let col = Color32::from_rgb(0xff, 0xb3, 0x00);
                painter.add(egui::Shape::convex_polygon(vec![a, mid + n, b, mid - n], col.gamma_multiply(0.5), EStroke::new(1.0, col)));
                painter.circle_stroke(a, 4.0, EStroke::new(1.5, col));
            }
        }
    }
    // Selection boxes and handles.
    let sel = st.selected();
    for (_, e) in &sel {
        let b = element_bounds(doc, e, &ctx);
        if b.is_empty() {
            continue;
        }
        let r = egui::Rect::from_min_max(to_pos(view.apply(b.min)), to_pos(view.apply(b.max)));
        painter.rect_stroke(r, 0.0, EStroke::new(1.0, SEL_COLOR), egui::StrokeKind::Middle);
        if let ElementKind::Instance(_) = e.kind {
            let o = to_pos(view.apply(ctx.apply(e.transform.apply(Vec2::ZERO))));
            painter.line_segment([o - egui::vec2(4.0, 0.0), o + egui::vec2(4.0, 0.0)], EStroke::new(1.0, Color32::BLACK));
            painter.line_segment([o - egui::vec2(0.0, 4.0), o + egui::vec2(0.0, 4.0)], EStroke::new(1.0, Color32::BLACK));
        }
    }
    let show_handles = matches!(core.tool, Tool::FreeTransform) || (matches!(core.tool, Tool::Select) && sel.len() == 1);
    if show_handles && let Some((frame, local)) = selection_box(st) {
        let corner = |u: f64, v: f64| to_pos(view.apply(frame.apply(Vec2::new(local.min.x + local.width() * u, local.min.y + local.height() * v))));
        let pts = [corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0)];
        painter.add(egui::Shape::closed_line(pts.to_vec(), EStroke::new(1.0, SEL_COLOR)));
        if core.tool == Tool::FreeTransform {
            for (u, v) in [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 0.5), (1.0, 1.0), (0.5, 1.0), (0.0, 1.0), (0.0, 0.5)] {
                let p = corner(u, v);
                painter.rect(egui::Rect::from_center_size(p, egui::vec2(HANDLE, HANDLE)), 0.0, Color32::WHITE, EStroke::new(1.0, Color32::BLACK), egui::StrokeKind::Middle);
            }
            if let Some(pv) = selection_pivot(st) {
                let p = to_pos(view.apply(pv));
                painter.circle(p, 4.5, Color32::WHITE, EStroke::new(1.2, Color32::BLACK));
            }
        }
    }
    // Subselection anchors.
    if core.tool == Tool::Subselect {
        for (_, e) in &sel {
            if let ElementKind::Shape(s) = &e.kind {
                let m = *view * ctx * e.transform;
                for (pi, sp) in s.paths.iter().enumerate() {
                    let mut prev = Vec2::ZERO;
                    for (ci, c) in sp.path.cmds.iter().enumerate() {
                        let selected = st.anchor_sel == Some((e.id, pi, ci));
                        if let PathCmd::CubicTo(c1, c2, p) = c {
                            if selected || st.anchor_sel.is_some_and(|a| a.0 == e.id && a.1 == pi && a.2 + 1 == ci) {
                                for (h, anchor) in [(*c1, prev), (*c2, *p)] {
                                    painter.line_segment([to_pos(m.apply(anchor)), to_pos(m.apply(h))], EStroke::new(1.0, SEL_COLOR));
                                    painter.circle_filled(to_pos(m.apply(h)), 3.0, SEL_COLOR);
                                }
                            }
                        }
                        if let Some(p) = c.end() {
                            let sp_ = to_pos(m.apply(p));
                            let r = egui::Rect::from_center_size(sp_, egui::vec2(6.0, 6.0));
                            painter.rect(r, 0.0, if selected { SEL_COLOR } else { Color32::WHITE }, EStroke::new(1.0, SEL_COLOR), egui::StrokeKind::Middle);
                            prev = p;
                        }
                    }
                }
            }
        }
    }
    // Gradient transform handles.
    if core.tool == Tool::GradientTransform
        && let Some((_, e)) = sel.first()
        && let ElementKind::Shape(s) = &e.kind
        && let Some(f) = s.paths.iter().find_map(|p| p.fill.as_ref().filter(|f| f.matrix().is_some()))
    {
        let m = *view * ctx * e.transform * f.matrix().unwrap();
        let c = to_pos(m.apply(Vec2::ZERO));
        let rx = to_pos(m.apply(Vec2::new(1.0, 0.0)));
        let ry = to_pos(m.apply(Vec2::new(0.0, 1.0)));
        painter.circle_stroke(c, 5.0, EStroke::new(1.5, SEL_COLOR));
        painter.line_segment([c, rx], EStroke::new(1.0, SEL_COLOR));
        painter.rect(egui::Rect::from_center_size(rx, egui::vec2(8.0, 8.0)), 0.0, Color32::WHITE, EStroke::new(1.0, SEL_COLOR), egui::StrokeKind::Middle);
        painter.circle(ry, 5.0, Color32::WHITE, EStroke::new(1.0, SEL_COLOR));
        if matches!(f, Fill::Radial { .. }) {
            let pts: Vec<Pos2> = (0..=48).map(|i| to_pos(m.apply(Vec2::from_angle(i as f64 / 48.0 * std::f64::consts::TAU)))).collect();
            painter.add(egui::Shape::line(pts, EStroke::new(1.0, SEL_COLOR.gamma_multiply(0.6))));
        } else {
            for x in [-1.0, 1.0] {
                painter.line_segment([to_pos(m.apply(Vec2::new(x, -1.0))), to_pos(m.apply(Vec2::new(x, 1.0)))], EStroke::new(1.0, SEL_COLOR.gamma_multiply(0.6)));
            }
        }
    }
    // Asset warp pins.
    if let Some((id, pins)) = &core.tstate.warp_pins
        && let Some((_, e)) = sel.iter().find(|(_, e)| e.id == *id)
    {
        let m = *view * ctx * e.transform;
        for p in pins {
            painter.circle(to_pos(m.apply(*p)), 5.0, Color32::from_rgb(255, 200, 0), EStroke::new(1.0, Color32::BLACK));
        }
    }
    // Text editing caret.
    if let Some(te) = &st.text_edit
        && let Some(e) = (0..st.timeline().layers.len()).flat_map(|li| st.layer_elements(li)).find(|e| e.id == te.element)
        && let ElementKind::Text(t) = &e.kind
    {
        draw_text_live(painter, view, &(ctx * e.transform), t, te.cursor);
    }
    // In-progress drags.
    let p_now = core.tstate.last_pointer_doc;
    match &core.tstate.drag {
        Drag::Marquee { start, .. } => {
            let r = egui::Rect::from_two_pos(to_pos(view.apply(*start)), to_pos(view.apply(p_now)));
            painter.rect(r, 0.0, SEL_COLOR.gamma_multiply(0.1), EStroke::new(1.0, SEL_COLOR), egui::StrokeKind::Middle);
        }
        Drag::Lasso { points } => {
            let pts: Vec<Pos2> = points.iter().map(|p| to_pos(view.apply(*p))).collect();
            painter.add(egui::Shape::closed_line(pts, EStroke::new(1.0, SEL_COLOR)));
        }
        Drag::Points { points } => {
            let t = &core.settings.tools;
            let pts: Vec<Pos2> = points.iter().map(|p| to_pos(view.apply(p.0))).collect();
            let (w, col) = match core.tool {
                Tool::Eraser => (t.eraser_size * view.mean_scale(), Color32::from_rgba_unmultiplied(255, 255, 255, 120)),
                Tool::Brush | Tool::ClassicBrush => (t.brush_size * view.mean_scale(), c32(t.fill_color)),
                _ => (t.stroke_width * view.mean_scale(), c32(t.stroke_color)),
            };
            painter.add(egui::Shape::line(pts, EStroke::new(w.max(1.0) as f32, col)));
        }
        Drag::Rect { start } => {
            let shift = false;
            let r = rect_from_drag(*start, p_now, shift, false);
            let col = c32(core.settings.tools.stroke_color);
            let path = match core.tool {
                Tool::Line => Path::line(*start, p_now),
                Tool::Oval => Path::ellipse(r),
                Tool::PolyStar => {
                    let t = &core.settings.tools;
                    let rad = start.distance(p_now);
                    let rot = (p_now - *start).angle();
                    if t.poly_star { Path::star(*start, rad, t.poly_sides, t.star_ratio, rot) } else { Path::polygon(*start, rad, t.poly_sides, rot) }
                }
                _ => Path::rounded_rect(r, core.settings.tools.rect_radius),
            };
            for (pts, closed) in path.transform(view).flatten(0.5) {
                let pts: Vec<Pos2> = pts.into_iter().map(to_pos).collect();
                painter.add(if closed { egui::Shape::closed_line(pts, EStroke::new(1.0, col)) } else { egui::Shape::line(pts, EStroke::new(1.0, col)) });
            }
        }
        Drag::TextBox { start } => {
            let r = egui::Rect::from_two_pos(to_pos(view.apply(*start)), to_pos(view.apply(p_now)));
            painter.rect_stroke(r, 0.0, EStroke::new(1.0, SEL_COLOR), egui::StrokeKind::Middle);
        }
        Drag::Bone { start, .. } => {
            painter.line_segment([to_pos(view.apply(*start)), to_pos(view.apply(p_now))], EStroke::new(3.0, Color32::from_rgb(0xff, 0xb3, 0x00)));
        }
        _ => {}
    }
    // Pen preview.
    if core.tool == Tool::Pen && !core.tstate.pen.points.is_empty() {
        let pts = &core.tstate.pen.points;
        let mut path = Path::new();
        path.move_to(pts[0].0);
        for w in pts.windows(2) {
            let (a, ah) = w[0];
            let (b, bh) = w[1];
            path.cubic_to(ah, b - (bh - b), b);
            let _ = a;
        }
        let (last, lh) = *pts.last().unwrap();
        path.cubic_to(lh, p_now, p_now);
        for (poly, _) in path.transform(view).flatten(0.5) {
            painter.add(egui::Shape::line(poly.into_iter().map(to_pos).collect(), EStroke::new(1.0, SEL_COLOR)));
        }
        for (a, h) in pts {
            painter.rect(egui::Rect::from_center_size(to_pos(view.apply(*a)), egui::vec2(6.0, 6.0)), 0.0, Color32::WHITE, EStroke::new(1.0, SEL_COLOR), egui::StrokeKind::Middle);
            if h.distance(*a) > 0.5 {
                painter.line_segment([to_pos(view.apply(*a)), to_pos(view.apply(*h))], EStroke::new(1.0, SEL_COLOR));
                painter.circle_filled(to_pos(view.apply(*h)), 3.0, SEL_COLOR);
            }
        }
        let _ = last;
    }
    // Brush / eraser cursors.
    let sp = to_pos(view.apply(p_now));
    if canvas.contains(sp) {
        let t = &core.settings.tools;
        match core.tool {
            Tool::Brush | Tool::ClassicBrush => {
                painter.circle_stroke(sp, (t.brush_size * view.mean_scale() / 2.0).max(1.0) as f32, EStroke::new(1.0, Color32::GRAY));
            }
            Tool::Eraser => {
                painter.circle_stroke(sp, (t.eraser_size * view.mean_scale() / 2.0).max(1.0) as f32, EStroke::new(1.0, Color32::GRAY));
            }
            _ => {}
        }
    }
}

fn draw_text_live(painter: &egui::Painter, view: &Affine, m: &Affine, t: &TextElement, cursor: usize) {
    let layout = animara_text::fonts().layout(t);
    let full = *view * *m;
    for (path, c) in layout.to_paths() {
        // Live text is drawn with egui tessellation while editing (fast, no stage re-render).
        for (pts, _) in path.transform(&full).flatten(0.3) {
            if pts.len() > 2 {
                painter.add(egui::Shape::closed_line(pts.into_iter().map(to_pos).collect(), EStroke::new(1.0, c32(c))));
            }
        }
    }
    let b = layout.bounds.inflate(2.0);
    let corners: Vec<Pos2> = b.corners().iter().map(|p| to_pos(full.apply(*p))).collect();
    painter.add(egui::Shape::closed_line(corners, EStroke::new(1.0, SEL_COLOR)));
    let (x, top, h) = layout.caret(cursor);
    painter.line_segment([to_pos(full.apply(Vec2::new(x, top))), to_pos(full.apply(Vec2::new(x, top + h)))], EStroke::new(1.5, Color32::BLACK));
}

#[allow(clippy::too_many_arguments)]
fn handle_input(core: &mut AppCore, ui: &mut egui::Ui, resp: &egui::Response, view: &Affine, inv_view: &Affine, canvas: egui::Rect, full: egui::Rect) {
    let (pointer, modifiers, scroll, zoom_delta, events) =
        ui.input(|i| (i.pointer.interact_pos().or(i.pointer.hover_pos()), i.modifiers, i.smooth_scroll_delta, i.zoom_delta(), i.events.clone()));
    let Some(pointer) = pointer else { return };
    let p_raw = inv_view.apply(to_vec(pointer));
    // Pen pressure from touch/stylus events where the platform provides it.
    let pressure = events
        .iter()
        .rev()
        .find_map(|e| match e {
            egui::Event::Touch { force: Some(f), .. } => Some(*f as f64),
            _ => None,
        })
        .unwrap_or(1.0);
    let ignore: Vec<_> = if matches!(core.tstate.drag, Drag::Move { .. } | Drag::Transform { .. }) { core.doc().unwrap().selection.clone() } else { Vec::new() };
    let p = snap(core, core.doc().unwrap(), p_raw, view, &ignore);
    core.tstate.last_pointer_doc = p;
    let hovered = resp.hovered() || resp.dragged();

    // Zoom & pan with wheel / pinch.
    if hovered {
        let st = core.docs.get_mut(core.active).unwrap();
        if zoom_delta != 1.0 {
            zoom_about(st, zoom_delta as f64, to_vec(pointer) - to_vec(canvas.min));
        } else if modifiers.command && scroll.y != 0.0 {
            zoom_about(st, (1.0 + scroll.y as f64 * 0.002).clamp(0.5, 2.0), to_vec(pointer) - to_vec(canvas.min));
        } else if scroll != egui::Vec2::ZERO {
            st.view.pan += Vec2::new(scroll.x as f64, scroll.y as f64);
        }
    }
    // Ruler drags create guides.
    if core.settings.rulers && resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pointer);
        if !canvas.contains(origin) && full.contains(origin) {
            let st = core.docs.get_mut(core.active).unwrap();
            st.checkpoint("Add Guide");
            if origin.y < canvas.min.y {
                st.doc.guides.horizontal.push(p.y);
            } else {
                st.doc.guides.vertical.push(p.x);
            }
            return;
        }
    }
    // Middle mouse / space-drag pans with any tool.
    let space = ui.input(|i| i.key_down(egui::Key::Space)) && core.doc().unwrap().text_edit.is_none();
    if resp.dragged_by(egui::PointerButton::Middle) || (space && resp.dragged()) || (core.tool == Tool::Hand && resp.dragged()) {
        let d = resp.drag_delta();
        core.doc_mut().unwrap().view.pan += Vec2::new(d.x as f64, d.y as f64);
        return;
    }
    if !canvas.contains(pointer) && !matches!(core.tstate.drag, Drag::None) && !resp.dragged() && !resp.drag_stopped() {
        return;
    }
    if core.doc().unwrap().playing {
        return;
    }
    let tool = core.tool;
    let auto_key = core.settings.tools.auto_keyframe;
    let shift = modifiers.shift;
    let alt = modifiers.alt;

    // Double-click actions.
    if resp.double_clicked() && matches!(tool, Tool::Select | Tool::FreeTransform | Tool::Subselect) {
        let st = core.docs.get_mut(core.active).unwrap();
        let tol = 3.0 / view.mean_scale();
        match pick(st, p, tol, false) {
            Some((_, e)) => match &e.kind {
                ElementKind::Instance(inst) => {
                    let ctx_m = e.transform;
                    let sym = inst.symbol;
                    ops::enter_symbol(st, sym, Some(ctx_m));
                }
                ElementKind::Text(t) => {
                    let local = (st.context_transform() * e.transform).invert().map(|m| m.apply(p)).unwrap_or(Vec2::ZERO);
                    let cursor = animara_text::fonts().layout(t).hit(local);
                    st.selection = vec![e.id];
                    st.text_edit = Some(crate::state::TextEdit { element: e.id, cursor });
                }
                ElementKind::Shape(s) if tool == Tool::Subselect => {
                    // Insert an anchor on the nearest segment.
                    let m = st.context_transform() * e.transform;
                    if let Some(inv) = m.invert() {
                        let lp = inv.apply(p);
                        let best = s.paths.iter().enumerate().filter_map(|(pi, sp)| sp.path.nearest_segment(lp).map(|n| (pi, n))).min_by(|a, b| a.1.2.total_cmp(&b.1.2));
                        if let Some((pi, (ci, t, _))) = best {
                            st.checkpoint("Add Anchor Point");
                            let id = e.id;
                            if let Some(el) = st.element_mut(id, auto_key)
                                && let ElementKind::Shape(s) = &mut el.kind
                            {
                                s.paths[pi].path.insert_anchor(ci, t);
                                s.paths[pi].primitive = None;
                            }
                        }
                    }
                }
                _ => {}
            },
            None => {
                if !st.stack.is_empty() {
                    let l = st.stack.len() - 1;
                    ops::exit_to_level(st, l);
                }
            }
        }
        return;
    }
    if resp.double_clicked() && tool == Tool::Pen {
        finish_pen(core, false);
        return;
    }

    // ---- Press
    if resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked_by(egui::PointerButton::Primary) {
        let origin = ui.input(|i| i.pointer.press_origin()).map(|o| inv_view.apply(to_vec(o))).unwrap_or(p);
        let origin = snap(core, core.doc().unwrap(), origin, view, &[]);
        let is_click = resp.clicked();
        press(core, tool, origin, p, view, shift, alt, is_click, auto_key, pressure);
    }
    // ---- Drag
    if resp.dragged_by(egui::PointerButton::Primary) {
        drag(core, tool, p, view, shift, alt, auto_key, pressure);
    }
    // ---- Release
    if resp.drag_stopped_by(egui::PointerButton::Primary) {
        release(core, tool, p, view, shift, alt, auto_key);
    }
    // Context menu on the stage.
    resp.context_menu(|ui| stage_context_menu(core, ui));
    // Cursor feedback.
    if hovered {
        let icon = match tool {
            Tool::Hand => egui::CursorIcon::Grab,
            Tool::Zoom => egui::CursorIcon::ZoomIn,
            Tool::Text => egui::CursorIcon::Text,
            Tool::Select | Tool::Subselect => egui::CursorIcon::Default,
            Tool::FreeTransform => match handle_at(core.doc().unwrap(), view, to_vec(pointer)) {
                Some(Handle::Rotate) => egui::CursorIcon::Alias,
                Some(Handle::Scale(..)) => egui::CursorIcon::ResizeNwSe,
                Some(Handle::Skew(h, _)) => {
                    if h {
                        egui::CursorIcon::ResizeHorizontal
                    } else {
                        egui::CursorIcon::ResizeVertical
                    }
                }
                Some(Handle::Pivot) => egui::CursorIcon::Move,
                None => egui::CursorIcon::Default,
            },
            _ => egui::CursorIcon::Crosshair,
        };
        ui.ctx().set_cursor_icon(icon);
    }
}

#[allow(clippy::too_many_arguments)]
fn press(core: &mut AppCore, tool: Tool, origin: Vec2, p: Vec2, view: &Affine, shift: bool, alt: bool, is_click: bool, auto_key: bool, pressure: f64) {
    let tol = 3.0 / view.mean_scale();
    match tool {
        Tool::Select => {
            let st = core.docs.get_mut(core.active).unwrap();
            st.text_edit = None;
            // Bone IK drag on armature layers.
            if let Some(ai) = armature_layer_index(st)
                && let Some((arm, pose)) = current_pose(st, ai)
            {
                let world = animara_rigging::forward(&arm, &pose);
                let ctx = st.context_transform();
                let lp = ctx.invert().map(|m| m.apply(origin)).unwrap_or(origin);
                if let Some(b) = animara_rigging::pick_bone(&world, lp, 6.0 / view.mean_scale()) {
                    st.checkpoint("Pose Armature");
                    core.tstate.drag = Drag::Ik { bone: b };
                    return;
                }
            }
            match pick(st, origin, tol, false) {
                Some((li, e)) => {
                    st.layer = li;
                    if shift {
                        if let Some(i) = st.selection.iter().position(|x| *x == e.id) {
                            st.selection.remove(i);
                        } else {
                            st.selection.push(e.id);
                        }
                    } else if !st.selection.contains(&e.id) {
                        st.selection = vec![e.id];
                    }
                    st.frame_sel = Some(crate::state::FrameSel::single(li, st.frame));
                    if !is_click {
                        if alt {
                            ops::duplicate(st, auto_key);
                            // duplicate offsets by 10px; undo that offset so the copy follows the cursor.
                            ops::move_selection(st, Vec2::new(-10.0, -10.0), None, auto_key);
                        } else {
                            st.checkpoint("Move");
                        }
                        core.tstate.drag = Drag::Move { last: origin, started: true };
                    }
                }
                None => {
                    if !shift {
                        st.selection.clear();
                    }
                    if !is_click {
                        core.tstate.drag = Drag::Marquee { start: origin, add: shift };
                    }
                }
            }
        }
        Tool::FreeTransform => {
            let st = core.docs.get_mut(core.active).unwrap();
            let sp = view.apply(origin);
            if let Some(h) = handle_at(st, view, sp)
                && let Some((frame, local)) = selection_box(st)
            {
                let pivot = selection_pivot(st).unwrap_or(local.center());
                let originals = st.selected().iter().map(|(_, e)| (e.id, e.transform, e.pivot)).collect();
                st.checkpoint("Transform");
                core.tstate.drag = Drag::Transform { handle: h, start: origin, frame, local, pivot, originals };
                return;
            }
            match pick(st, origin, tol, false) {
                Some((li, e)) => {
                    st.layer = li;
                    if shift {
                        st.selection.push(e.id);
                    } else if !st.selection.contains(&e.id) {
                        st.selection = vec![e.id];
                    }
                    if !is_click {
                        st.checkpoint("Move");
                        core.tstate.drag = Drag::Move { last: origin, started: true };
                    }
                }
                None => {
                    st.selection.clear();
                    if !is_click {
                        core.tstate.drag = Drag::Marquee { start: origin, add: false };
                    }
                }
            }
        }
        Tool::Subselect => {
            let st = core.docs.get_mut(core.active).unwrap();
            let ctx = st.context_transform();
            // Anchor / handle hit on selected shapes first.
            for (_, e) in st.selected() {
                if let ElementKind::Shape(s) = &e.kind {
                    let m = ctx * e.transform;
                    for (pi, sp) in s.paths.iter().enumerate() {
                        for (ci, c) in sp.path.cmds.iter().enumerate() {
                            let mut hits: Vec<(Option<usize>, Vec2)> = Vec::new();
                            if let Some(a) = c.end() {
                                hits.push((None, a));
                            }
                            if let PathCmd::CubicTo(c1, c2, _) = c
                                && st.anchor_sel.is_some_and(|x| x.0 == e.id && x.1 == pi && (x.2 == ci || x.2 + 1 == ci))
                            {
                                hits.push((Some(0), *c1));
                                hits.push((Some(1), *c2));
                            }
                            for (ctl, q) in hits {
                                if view.apply(m.apply(q)).distance(view.apply(origin)) < 6.0 {
                                    st.anchor_sel = Some((e.id, pi, ci));
                                    st.checkpoint("Edit Anchor");
                                    if alt && ctl.is_none() {
                                        if let Some(el) = st.element_mut(e.id, auto_key)
                                            && let ElementKind::Shape(s) = &mut el.kind
                                        {
                                            s.paths[pi].path.convert_to_curve(ci);
                                        }
                                    }
                                    core.tstate.drag = Drag::Anchor { element: e.id, path: pi, cmd: ci, control: ctl, last: origin };
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            match pick(st, origin, tol, false) {
                Some((li, e)) => {
                    st.layer = li;
                    st.selection = vec![e.id];
                    st.anchor_sel = None;
                    if !is_click {
                        st.checkpoint("Move");
                        core.tstate.drag = Drag::Move { last: origin, started: true };
                    }
                }
                None => {
                    st.selection.clear();
                    st.anchor_sel = None;
                    if !is_click {
                        core.tstate.drag = Drag::Marquee { start: origin, add: false };
                    }
                }
            }
        }
        Tool::Lasso => {
            if !is_click {
                core.tstate.drag = Drag::Lasso { points: vec![origin] };
            }
        }
        Tool::GradientTransform => {
            let st = core.docs.get_mut(core.active).unwrap();
            let ctx = st.context_transform();
            if let Some((_, e)) = st.selected().first().cloned()
                && let ElementKind::Shape(s) = &e.kind
                && let Some((pi, f)) = s.paths.iter().enumerate().find_map(|(i, p)| p.fill.as_ref().filter(|f| f.matrix().is_some()).map(|f| (i, f)))
            {
                let gm = f.matrix().unwrap();
                let m = *view * ctx * e.transform * gm;
                let sp = view.apply(origin);
                let kind = if m.apply(Vec2::new(1.0, 0.0)).distance(sp) < 8.0 {
                    1
                } else if m.apply(Vec2::new(0.0, 1.0)).distance(sp) < 8.0 {
                    2
                } else {
                    0
                };
                st.checkpoint("Gradient Transform");
                core.tstate.drag = Drag::Gradient { element: e.id, path: pi, kind, start: origin, original: gm };
                return;
            }
            if let Some((li, e)) = pick(st, origin, tol, false) {
                st.layer = li;
                st.selection = vec![e.id];
            }
        }
        Tool::Pen => {
            let pen = &mut core.tstate.pen;
            if let Some((first, _)) = pen.points.first().copied()
                && pen.points.len() > 2
                && view.apply(first).distance(view.apply(origin)) < 8.0
            {
                finish_pen(core, true);
                return;
            }
            pen.points.push((origin, origin));
            pen.dragging = true;
        }
        Tool::Pencil | Tool::Brush | Tool::ClassicBrush | Tool::Eraser => {
            if tool == Tool::Eraser && core.settings.tools.eraser_mode == EraserMode::Faucet {
                erase(core, &Path::new(), Some(origin));
                return;
            }
            let pts = vec![(origin, pressure, 0.0)];
            if is_click && tool != Tool::Pencil {
                // Single click dab.
                core.tstate.drag = Drag::Points { points: pts };
                release(core, tool, p, view, shift, alt, auto_key);
            } else {
                core.tstate.drag = Drag::Points { points: pts };
            }
        }
        Tool::Line | Tool::Rectangle | Tool::Oval | Tool::PolyStar => {
            if !is_click {
                core.tstate.drag = Drag::Rect { start: origin };
            }
        }
        Tool::PaintBucket | Tool::InkBottle | Tool::Eyedropper => {
            bucket_like(core, tool, origin, tol, auto_key);
        }
        Tool::Text => {
            let st = core.docs.get_mut(core.active).unwrap();
            if let Some((li, e)) = pick(st, origin, tol, false)
                && let ElementKind::Text(t) = &e.kind
            {
                st.layer = li;
                let local = (st.context_transform() * e.transform).invert().map(|m| m.apply(origin)).unwrap_or(Vec2::ZERO);
                let cursor = animara_text::fonts().layout(t).hit(local);
                st.selection = vec![e.id];
                st.text_edit = Some(crate::state::TextEdit { element: e.id, cursor });
                return;
            }
            if st.text_edit.is_some() {
                st.text_edit = None;
                return;
            }
            if is_click {
                create_text(core, origin, None);
            } else {
                core.tstate.drag = Drag::TextBox { start: origin };
            }
        }
        Tool::Bone => {
            let st = core.docs.get_mut(core.active).unwrap();
            let parent = armature_layer_index(st).and_then(|ai| {
                let (arm, pose) = current_pose(st, ai)?;
                let world = animara_rigging::forward(&arm, &pose);
                world.iter().position(|w| view.apply(w.tail).distance(view.apply(origin)) < 8.0)
            });
            let start = match (parent, armature_layer_index(st).and_then(|ai| current_pose(st, ai))) {
                (Some(pi), Some((arm, pose))) => animara_rigging::forward(&arm, &pose)[pi].tail,
                _ => origin,
            };
            if !is_click {
                core.tstate.drag = Drag::Bone { start, parent };
            }
        }
        Tool::AssetWarp => {
            let st = core.docs.get_mut(core.active).unwrap();
            let ctx = st.context_transform();
            if let Some((id, pins)) = core.tstate.warp_pins.clone()
                && let Some((_, e)) = st.selected().into_iter().find(|(_, e)| e.id == id)
            {
                let m = ctx * e.transform;
                if let Some(pi) = pins.iter().position(|q| view.apply(m.apply(*q)).distance(view.apply(origin)) < 8.0) {
                    st.checkpoint("Warp");
                    core.tstate.drag = Drag::Warp { element: id, pin: pi, last: origin };
                    return;
                }
                if element_hit(&st.doc, &e, &ctx, origin, tol) {
                    if let Some(inv) = m.invert() {
                        core.tstate.warp_pins.as_mut().unwrap().1.push(inv.apply(origin));
                    }
                    return;
                }
            }
            if let Some((li, e)) = pick(st, origin, tol, false)
                && matches!(e.kind, ElementKind::Shape(_))
            {
                st.layer = li;
                st.selection = vec![e.id];
                let m = ctx * e.transform;
                let pin = m.invert().map(|inv| inv.apply(origin)).unwrap_or(Vec2::ZERO);
                core.tstate.warp_pins = Some((e.id, vec![pin]));
            }
        }
        Tool::Camera => {
            let st = core.docs.get_mut(core.active).unwrap();
            if let Some(ci) = st.timeline().layers.iter().position(|l| l.kind == LayerKind::Camera) {
                st.checkpoint("Move Camera");
                st.layer = ci;
                core.tstate.drag = Drag::Camera { last: origin };
            } else {
                core.warn("Add a camera layer first (Insert ▸ Camera Layer).");
            }
        }
        Tool::Zoom => {
            let st = core.docs.get_mut(core.active).unwrap();
            let sp = view.apply(origin);
            let local = sp - Vec2::new(view.tx, view.ty) + st.view.pan;
            zoom_about(st, if alt { 0.5 } else { 2.0 }, local);
        }
        Tool::Hand => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn drag(core: &mut AppCore, tool: Tool, p: Vec2, view: &Affine, shift: bool, alt: bool, auto_key: bool, pressure: f64) {
    let velocity_enabled = core.settings.tools.velocity;
    match &mut core.tstate.drag {
        Drag::Move { last, .. } => {
            let mut d = p - *last;
            *last = p;
            if shift {
                // Constrain to the dominant axis.
                if d.x.abs() > d.y.abs() {
                    d.y = 0.0;
                } else {
                    d.x = 0.0;
                }
            }
            let st = core.docs.get_mut(core.active).unwrap();
            ops::move_selection(st, d, None, auto_key);
        }
        Drag::Transform { handle, start, frame, local, pivot, originals } => {
            let (handle, start, frame, local, pivot, originals) = (*handle, *start, *frame, *local, *pivot, originals.clone());
            let st = core.docs.get_mut(core.active).unwrap();
            let ctx = st.context_transform();
            let ctx_inv = ctx.invert().unwrap_or(Affine::IDENTITY);
            let finv = frame.invert().unwrap_or(Affine::IDENTITY);
            let world_delta: Affine = match handle {
                Handle::Pivot => {
                    for (id, t, _) in &originals {
                        if let Some(e) = st.element_mut(*id, auto_key) {
                            let lp = (ctx * *t).invert().map(|m| m.apply(p)).unwrap_or(e.pivot);
                            e.pivot = lp;
                        }
                    }
                    st.touch();
                    return;
                }
                Handle::Rotate => {
                    let mut ang = (p - pivot).angle() - (start - pivot).angle();
                    if shift {
                        let step = std::f64::consts::PI / 12.0;
                        ang = (ang / step).round() * step;
                    }
                    Affine::around(Affine::rotate(ang), pivot)
                }
                Handle::Scale(u, v) => {
                    let q = finv.apply(p);
                    let q0 = finv.apply(start);
                    let anchor_local = if alt {
                        finv.apply(pivot)
                    } else {
                        Vec2::new(local.min.x + local.width() * (1.0 - u), local.min.y + local.height() * (1.0 - v))
                    };
                    let mut sx = if (u - 0.5).abs() > 0.1 { (q.x - anchor_local.x) / (q0.x - anchor_local.x).abs().max(1e-6).copysign(q0.x - anchor_local.x) } else { 1.0 };
                    let mut sy = if (v - 0.5).abs() > 0.1 { (q.y - anchor_local.y) / (q0.y - anchor_local.y).abs().max(1e-6).copysign(q0.y - anchor_local.y) } else { 1.0 };
                    if shift && (u - 0.5).abs() > 0.1 && (v - 0.5).abs() > 0.1 {
                        let s = sx.abs().max(sy.abs());
                        sx = s * sx.signum();
                        sy = s * sy.signum();
                    }
                    let sl = Affine::around(Affine::scale(sx, sy), anchor_local);
                    frame * sl * finv
                }
                Handle::Skew(horizontal, side) => {
                    let q = finv.apply(p);
                    let q0 = finv.apply(start);
                    let anchor = if horizontal {
                        Vec2::new(0.0, if side == 0.0 { local.max.y } else { local.min.y })
                    } else {
                        Vec2::new(if side == 0.0 { local.max.x } else { local.min.x }, 0.0)
                    };
                    let sk = if horizontal {
                        let h = (if side == 0.0 { local.min.y } else { local.max.y }) - anchor.y;
                        Affine::new(1.0, 0.0, (q.x - q0.x) / if h.abs() < 1e-6 { 1.0 } else { h }, 1.0, 0.0, 0.0)
                    } else {
                        let w = (if side == 0.0 { local.min.x } else { local.max.x }) - anchor.x;
                        Affine::new(1.0, (q.y - q0.y) / if w.abs() < 1e-6 { 1.0 } else { w }, 0.0, 1.0, 0.0, 0.0)
                    };
                    frame * Affine::around(sk, anchor) * finv
                }
            };
            for (id, t, _) in &originals {
                if let Some(e) = st.element_mut(*id, auto_key) {
                    e.transform = ctx_inv * world_delta * ctx * *t;
                }
            }
            st.touch();
        }
        Drag::Points { points } => {
            let last = points.last().map(|x| x.0).unwrap_or(p);
            let speed = last.distance(p);
            if speed > 0.3 {
                points.push((p, pressure, speed));
            }
            let _ = velocity_enabled;
        }
        Drag::Lasso { points } => {
            if points.last().is_none_or(|q| q.distance(p) > 1.0) {
                points.push(p);
            }
        }
        Drag::Anchor { element, path, cmd, control, last } => {
            let (element, path, cmd, control) = (*element, *path, *cmd, *control);
            let d = p - *last;
            *last = p;
            let st = core.docs.get_mut(core.active).unwrap();
            let ctx = st.context_transform();
            if let Some(e) = st.element_mut(element, auto_key) {
                let inv = (ctx * e.transform).invert().unwrap_or(Affine::IDENTITY);
                let ld = inv.apply_vector(d);
                if let ElementKind::Shape(s) = &mut e.kind
                    && let Some(sp) = s.paths.get_mut(path)
                {
                    sp.primitive = None;
                    match control {
                        None => sp.path.move_anchor(cmd, ld),
                        Some(w) => sp.path.set_control(cmd, w, inv.apply(p)),
                    }
                }
            }
            st.touch();
        }
        Drag::Gradient { element, path, kind, start, original } => {
            let (element, path, kind, start, original) = (*element, *path, *kind, *start, *original);
            let st = core.docs.get_mut(core.active).unwrap();
            let ctx = st.context_transform();
            if let Some(e) = st.element_mut(element, auto_key) {
                let inv = (ctx * e.transform).invert().unwrap_or(Affine::IDENTITY);
                let lp = inv.apply(p);
                let ls = inv.apply(start);
                let center = original.apply(Vec2::ZERO);
                let new = match kind {
                    0 => Affine::translate(lp.x - ls.x, lp.y - ls.y) * original,
                    1 => {
                        // Rotate + scale width around the center.
                        let a0 = (ls - center).angle();
                        let a1 = (lp - center).angle();
                        let s = (lp - center).length() / (ls - center).length().max(1e-6);
                        Affine::around(Affine::rotate(a1 - a0), center) * Affine::around(Affine::scale(1.0, 1.0), center) * original * Affine::scale(s, 1.0)
                    }
                    _ => {
                        let s = (lp - center).length() / (ls - center).length().max(1e-6);
                        original * Affine::scale(1.0, s)
                    }
                };
                if let ElementKind::Shape(s) = &mut e.kind
                    && let Some(sp) = s.paths.get_mut(path)
                    && let Some(f) = &mut sp.fill
                {
                    f.set_matrix(new);
                }
            }
            st.touch();
        }
        Drag::Ik { bone } => {
            let bone = *bone;
            let st = core.docs.get_mut(core.active).unwrap();
            if let Some(ai) = armature_layer_index(st)
                && let Some((arm, pose)) = current_pose(st, ai)
            {
                let lp = st.context_transform().invert().map(|m| m.apply(p)).unwrap_or(p);
                let solved = animara_rigging::solve_ik(&arm, &pose, bone, lp, 20);
                let f = st.frame;
                let layer = &mut st.timeline_mut().layers[ai];
                if !layer.is_keyframe(f) {
                    layer.insert_keyframe(f, None);
                }
                if let Some(k) = layer.keyframe_at_mut(f) {
                    k.pose = Some(solved);
                }
                st.touch();
            }
        }
        Drag::Camera { last } => {
            let d = p - *last;
            *last = p;
            let st = core.docs.get_mut(core.active).unwrap();
            if let Some(ci) = st.timeline().layers.iter().position(|l| l.kind == LayerKind::Camera) {
                let f = st.frame;
                let (w, h) = (st.doc.width, st.doc.height);
                let layer = &mut st.timeline_mut().layers[ci];
                let cur = animara_animation::camera_at(layer, f).unwrap_or(animara_document::CameraState::centered(w, h));
                if !layer.is_keyframe(f) && f < layer.end() {
                    layer.insert_keyframe(f, None);
                } else if f >= layer.end() {
                    layer.insert_keyframe(f, None);
                }
                if let Some(k) = layer.keyframe_at_mut(f) {
                    k.camera = Some(animara_document::CameraState { x: cur.x - d.x / cur.zoom, y: cur.y - d.y / cur.zoom, ..cur });
                }
                st.touch();
            }
        }
        Drag::Warp { element, pin, last } => {
            let (element, pin) = (*element, *pin);
            let d = p - *last;
            *last = p;
            let pins = core.tstate.warp_pins.as_ref().map(|w| w.1.clone()).unwrap_or_default();
            let st = core.docs.get_mut(core.active).unwrap();
            let ctx = st.context_transform();
            let mut moved_pin = None;
            if let Some(e) = st.element_mut(element, auto_key) {
                let inv = (ctx * e.transform).invert().unwrap_or(Affine::IDENTITY);
                let ld = inv.apply_vector(d);
                let src = pins[pin];
                // Inverse-distance weighting: points follow the dragged pin, other pins stay fixed.
                let weight = |q: Vec2| -> f64 {
                    let wd = 1.0 / (q.distance(src).powi(2) + 1e-3);
                    let wo: f64 = pins.iter().enumerate().filter(|(i, _)| *i != pin).map(|(_, o)| 1.0 / (q.distance(*o).powi(2) + 1e-3)).sum();
                    if pins.len() == 1 { 1.0 } else { wd / (wd + wo) }
                };
                if let ElementKind::Shape(s) = &mut e.kind {
                    for sp in &mut s.paths {
                        sp.primitive = None;
                        sp.path.cmds = sp
                            .path
                            .cmds
                            .iter()
                            .map(|c| {
                                let f = |q: Vec2| q + ld * weight(q);
                                match *c {
                                    PathCmd::MoveTo(a) => PathCmd::MoveTo(f(a)),
                                    PathCmd::LineTo(a) => PathCmd::LineTo(f(a)),
                                    PathCmd::QuadTo(a, b) => PathCmd::QuadTo(f(a), f(b)),
                                    PathCmd::CubicTo(a, b, c2) => PathCmd::CubicTo(f(a), f(b), f(c2)),
                                    PathCmd::Close => PathCmd::Close,
                                }
                            })
                            .collect();
                    }
                }
                moved_pin = Some(src + ld);
            }
            if let (Some(np), Some((_, pins))) = (moved_pin, core.tstate.warp_pins.as_mut()) {
                pins[pin] = np;
            }
            core.docs.get_mut(core.active).unwrap().touch();
        }
        _ => {}
    }
    if tool == Tool::Pen
        && core.tstate.pen.dragging
        && let Some(last) = core.tstate.pen.points.last_mut()
    {
        last.1 = p;
    }
    let _ = view;
}

#[allow(clippy::too_many_arguments)]
fn release(core: &mut AppCore, tool: Tool, p: Vec2, view: &Affine, shift: bool, alt: bool, auto_key: bool) {
    let drag = std::mem::take(&mut core.tstate.drag);
    match drag {
        Drag::Marquee { start, add } => {
            let st = core.docs.get_mut(core.active).unwrap();
            let r = Rect::from_points(start, p);
            let ctx = st.context_transform();
            let mut ids = if add { st.selection.clone() } else { Vec::new() };
            for li in 0..st.timeline().layers.len() {
                if !st.layer_editable(li) {
                    continue;
                }
                for e in st.layer_elements(li) {
                    let b = element_bounds(&st.doc, &e, &ctx);
                    if !e.locked && r.intersects(&b) && !ids.contains(&e.id) {
                        ids.push(e.id);
                    }
                }
            }
            st.selection = ids;
        }
        Drag::Lasso { points } => {
            let st = core.docs.get_mut(core.active).unwrap();
            let poly = Path::polyline(&points, true);
            let ctx = st.context_transform();
            let mut ids = Vec::new();
            for li in 0..st.timeline().layers.len() {
                if !st.layer_editable(li) {
                    continue;
                }
                for e in st.layer_elements(li) {
                    if poly.contains(element_bounds(&st.doc, &e, &ctx).center(), FillRule::EvenOdd) {
                        ids.push(e.id);
                    }
                }
            }
            st.selection = ids;
        }
        Drag::Points { points } => {
            let ctx_inv = core.doc().unwrap().context_transform().invert().unwrap_or(Affine::IDENTITY);
            let pts: Vec<(Vec2, f64, f64)> = points.into_iter().map(|(q, pr, v)| (ctx_inv.apply(q), pr, v)).collect();
            let t = core.settings.tools.clone();
            match tool {
                Tool::Pencil => {
                    let raw: Vec<Vec2> = pts.iter().map(|x| x.0).collect();
                    if raw.len() < 2 {
                        return;
                    }
                    let path = match t.pencil_mode {
                        PencilMode::Smooth => animara_geometry::fit_smooth_path(&raw, 1.0 + t.brush_smoothing),
                        PencilMode::Straighten => Path::polyline(&animara_geometry::simplify_polyline(&raw, 4.0), false),
                        PencilMode::Ink => Path::polyline(&animara_geometry::simplify_polyline(&raw, 0.3), false),
                    };
                    let mut e = new_shape_element(core, path, false, None);
                    if let ElementKind::Shape(s) = &mut e.kind {
                        s.paths[0].fill = None;
                        if s.paths[0].stroke.is_none() {
                            s.paths[0].stroke = Some(animara_document::Stroke::solid(t.stroke_color, t.stroke_width));
                        }
                    }
                    ops::add_element(core.doc_mut().unwrap(), "Pencil", e, auto_key);
                }
                Tool::Brush | Tool::ClassicBrush => {
                    let n = pts.len();
                    let smooth_pts: Vec<Vec2> = if n > 3 {
                        let raw: Vec<Vec2> = pts.iter().map(|x| x.0).collect();
                        // Light smoothing of the stroke spine.
                        (0..n).map(|i| {
                            let a = raw[i.saturating_sub(1)];
                            let b = raw[i];
                            let c = raw[(i + 1).min(n - 1)];
                            (a + b * 2.0 + c) / 4.0
                        }).collect()
                    } else {
                        pts.iter().map(|x| x.0).collect()
                    };
                    let samples: Vec<StrokeSample> = pts
                        .iter()
                        .enumerate()
                        .map(|(i, (_, pr, speed))| {
                            let mut w = t.brush_size;
                            if tool == Tool::Brush {
                                if t.pressure {
                                    w *= pr.clamp(0.1, 1.5);
                                }
                                if t.velocity {
                                    // Fluid-brush style: faster strokes get thinner, ends taper.
                                    w *= (1.2 - (speed / 30.0).min(0.7)).max(0.35);
                                    let taper = ((i as f64 + 1.0) / 4.0).min(1.0).min(((n - i) as f64) / 4.0);
                                    w *= taper.max(0.25);
                                }
                            }
                            StrokeSample { pos: smooth_pts[i], width: w.max(0.3) }
                        })
                        .collect();
                    let outline = animara_geometry::variable_width_outline(&samples);
                    if outline.is_empty() {
                        return;
                    }
                    let fill = Fill::solid(t.fill_color);
                    let e = filled_element(outline, fill);
                    ops::add_element(core.doc_mut().unwrap(), "Brush", e, auto_key);
                }
                Tool::Eraser => {
                    let ctx = core.doc().unwrap().context_transform();
                    let samples: Vec<StrokeSample> = pts.iter().map(|(q, _, _)| StrokeSample { pos: ctx.apply(*q), width: t.eraser_size }).collect();
                    let outline = animara_geometry::variable_width_outline(&samples);
                    erase(core, &outline, None);
                }
                _ => {}
            }
        }
        Drag::Rect { start } => {
            let ctx_inv = core.doc().unwrap().context_transform().invert().unwrap_or(Affine::IDENTITY);
            let (a, b) = (ctx_inv.apply(start), ctx_inv.apply(p));
            if a.distance(b) < 1.0 {
                return;
            }
            let t = core.settings.tools.clone();
            let (path, closed, prim, label) = match tool {
                Tool::Line => {
                    let end = if shift { constrain_angle(a, b) } else { b };
                    (Path::line(a, end), false, None, "Line")
                }
                Tool::Oval => {
                    let r = rect_from_drag(a, b, shift, alt);
                    (Path::ellipse(r), true, Some(Primitive::Oval { rect: r, start_angle: 0.0, end_angle: 0.0, inner_radius: 0.0 }), "Oval")
                }
                Tool::PolyStar => {
                    let rad = a.distance(b);
                    let rot = (b - a).angle();
                    let prim = Primitive::Polygon { center: a, radius: rad, sides: t.poly_sides, star: t.poly_star, inner_ratio: t.star_ratio, rotation: rot };
                    (prim.to_path(), true, Some(prim), "PolyStar")
                }
                _ => {
                    let r = rect_from_drag(a, b, shift, alt);
                    let prim = Primitive::Rect { rect: r, corner_radius: t.rect_radius };
                    (prim.to_path(), true, Some(prim), "Rectangle")
                }
            };
            let e = new_shape_element(core, path, closed, prim);
            ops::add_element(core.doc_mut().unwrap(), label, e, auto_key);
        }
        Drag::TextBox { start } => {
            let w = (p.x - start.x).abs().max(20.0);
            create_text(core, Vec2::new(start.x.min(p.x), start.y.min(p.y)), Some(w));
        }
        Drag::Bone { start, parent } => {
            if start.distance(p) < 3.0 {
                return;
            }
            let st = core.docs.get_mut(core.active).unwrap();
            let ctx_inv = st.context_transform().invert().unwrap_or(Affine::IDENTITY);
            let (a, b) = (ctx_inv.apply(start), ctx_inv.apply(p));
            st.checkpoint("Add Bone");
            // Find or create the armature layer.
            let ai = match armature_layer_index(st) {
                Some(i) => i,
                None => {
                    let li = st.layer;
                    let layer = &mut st.timeline_mut().layers[li];
                    if layer.kind == LayerKind::Normal {
                        layer.kind = LayerKind::Armature;
                        layer.name = format!("Armature {}", layer.name);
                    }
                    li
                }
            };
            let target = pick(st, start, 2.0, false).filter(|(li, _)| *li == ai).map(|(_, e)| e.id);
            let f = st.frame;
            let layer = &mut st.timeline_mut().layers[ai];
            let arm = layer.armature.get_or_insert_with(Armature::default);
            let n = arm.bones.len() + 1;
            let bi = animara_rigging::add_bone(arm, parent, a, b, &format!("Bone {n}"));
            if let Some(id) = target
                && !arm.bones.iter().any(|x| x.bound_element == Some(id))
            {
                arm.bones[bi].bound_element = Some(id);
            }
            let rest = animara_rigging::rest_pose(arm);
            // Keep pose keyframes in sync with the bone count.
            for k in &mut layer.frames {
                if let Some(pose) = &mut k.pose {
                    while pose.angles.len() < rest.angles.len() {
                        pose.angles.push(rest.angles[pose.angles.len()]);
                    }
                }
            }
            if let Some(k) = layer.keyframe_at_mut(f)
                && k.pose.is_none()
            {
                k.pose = Some(rest);
            }
        }
        Drag::Ik { .. } | Drag::Camera { .. } | Drag::Warp { .. } | Drag::Anchor { .. } | Drag::Gradient { .. } | Drag::Transform { .. } | Drag::Move { .. } => {}
        Drag::Pan { .. } | Drag::None => {}
    }
    if tool == Tool::Pen {
        core.tstate.pen.dragging = false;
        if let Some(last) = core.tstate.pen.points.last_mut()
            && last.1.distance(last.0) < 1.0 / view.mean_scale()
        {
            last.1 = last.0;
        }
    }
}

/// Complete the pen path being drawn.
pub fn finish_pen(core: &mut AppCore, close: bool) {
    let pts = std::mem::take(&mut core.tstate.pen.points);
    if pts.len() < 2 {
        return;
    }
    let ctx_inv = core.doc().unwrap().context_transform().invert().unwrap_or(Affine::IDENTITY);
    let mut path = Path::new();
    path.move_to(ctx_inv.apply(pts[0].0));
    let n = pts.len();
    let seg = |path: &mut Path, (a, ah): (Vec2, Vec2), (b, bh): (Vec2, Vec2)| {
        let c1 = ah;
        let c2 = b - (bh - b);
        if c1.distance(a) < 1e-6 && c2.distance(b) < 1e-6 {
            path.line_to(ctx_inv.apply(b));
        } else {
            path.cubic_to(ctx_inv.apply(c1), ctx_inv.apply(c2), ctx_inv.apply(b));
        }
    };
    for i in 0..n - 1 {
        seg(&mut path, pts[i], pts[i + 1]);
    }
    if close {
        seg(&mut path, pts[n - 1], pts[0]);
        path.close();
    }
    let auto_key = core.settings.tools.auto_keyframe;
    let e = new_shape_element(core, path, close, None);
    ops::add_element(core.doc_mut().unwrap(), "Pen", e, auto_key);
}

fn create_text(core: &mut AppCore, at: Vec2, width: Option<f64>) {
    let auto_key = core.settings.tools.auto_keyframe;
    let style = core.settings.tools.text_style.clone();
    let st = core.docs.get_mut(core.active).unwrap();
    let ctx_inv = st.context_transform().invert().unwrap_or(Affine::IDENTITY);
    let mut t = TextElement::new("", style);
    t.width = width;
    let e = Element::new(ElementKind::Text(t)).with_transform(Affine::translate(ctx_inv.apply(at).x, ctx_inv.apply(at).y));
    let id = e.id;
    if ops::add_element(st, "Text", e, auto_key) {
        st.text_edit = Some(crate::state::TextEdit { element: id, cursor: 0 });
    }
}

fn bucket_like(core: &mut AppCore, tool: Tool, p: Vec2, tol: f64, auto_key: bool) {
    let st_ref = core.doc().unwrap();
    let Some((li, e)) = pick(st_ref, p, tol, tool == Tool::Eyedropper) else { return };
    let ctx = st_ref.context_transform();
    let m = ctx * e.transform;
    let Some(inv) = m.invert() else { return };
    let lp = inv.apply(p);
    match (&e.kind, tool) {
        (ElementKind::Shape(s), Tool::Eyedropper) => {
            for sp in s.paths.iter().rev() {
                if sp.stroke.is_some() && sp.path.distance_to(lp) <= sp.stroke.as_ref().unwrap().width / 2.0 + tol {
                    let stroke = sp.stroke.clone().unwrap();
                    core.settings.tools.stroke_color = stroke.fill.primary_color();
                    core.settings.tools.stroke_width = stroke.width;
                    core.tool = Tool::InkBottle;
                    return;
                }
                if let Some(f) = &sp.fill
                    && sp.path.contains(lp, sp.fill_rule)
                {
                    core.settings.tools.fill_color = f.primary_color();
                    if let Some(stops) = f.stops() {
                        core.settings.tools.gradient = stops.to_vec();
                        core.settings.tools.fill_kind = if matches!(f, Fill::Radial { .. }) { crate::tools::FillKind::Radial } else { crate::tools::FillKind::Linear };
                    } else {
                        core.settings.tools.fill_kind = crate::tools::FillKind::Solid;
                    }
                    core.tool = Tool::PaintBucket;
                    return;
                }
            }
        }
        (ElementKind::Text(t), Tool::Eyedropper) => {
            core.settings.tools.fill_color = t.style().color;
            core.settings.tools.text_style = t.style();
        }
        (ElementKind::Shape(s), Tool::PaintBucket | Tool::InkBottle) => {
            let fill_tpl = core.settings.tools.clone();
            let stroke = core.settings.tools.stroke().unwrap_or(animara_document::Stroke::solid(fill_tpl.stroke_color, fill_tpl.stroke_width));
            let target = s.paths.iter().enumerate().rev().find(|(_, sp)| {
                if tool == Tool::PaintBucket {
                    sp.path.contains(lp, sp.fill_rule) && sp.path.subpaths().iter().any(|x| x.closed || x.segments.len() > 1)
                } else {
                    sp.path.distance_to(lp) <= sp.stroke.as_ref().map(|s| s.width / 2.0).unwrap_or(0.0) + tol * 2.0 || sp.path.contains(lp, sp.fill_rule)
                }
            });
            let Some((pi, sp)) = target else { return };
            let bounds = sp.path.bounds();
            let st = core.docs.get_mut(core.active).unwrap();
            st.layer = li;
            st.checkpoint(if tool == Tool::PaintBucket { "Paint Bucket" } else { "Ink Bottle" });
            if let Some(el) = st.element_mut(e.id, auto_key)
                && let ElementKind::Shape(s) = &mut el.kind
            {
                if tool == Tool::PaintBucket {
                    let mut f = fill_tpl.fill_for(bounds).unwrap_or(Fill::solid(fill_tpl.fill_color));
                    if fill_tpl.no_fill {
                        f = Fill::solid(fill_tpl.fill_color);
                    }
                    s.paths[pi].fill = Some(f);
                } else {
                    s.paths[pi].stroke = Some(stroke);
                }
            }
        }
        _ => {}
    }
}

fn stage_context_menu(core: &mut AppCore, ui: &mut egui::Ui) {
    use crate::commands::Command as C;
    let has_sel = core.doc().is_some_and(|d| !d.selection.is_empty());
    let items: &[(C, bool)] = &[
        (C::Cut, has_sel),
        (C::Copy, has_sel),
        (C::Paste, true),
        (C::PasteInPlace, true),
        (C::Clear, has_sel),
        (C::SelectAll, true),
        (C::ConvertToSymbol, has_sel),
        (C::BreakApart, has_sel),
        (C::Group, has_sel),
        (C::Ungroup, has_sel),
        (C::BringToFront, has_sel),
        (C::SendToBack, has_sel),
        (C::DistributeToLayers, has_sel),
        (C::CreateClassicTween, true),
        (C::CreateMotionTween, true),
        (C::CreateShapeTween, true),
        (C::EditInPlace, has_sel),
    ];
    for (c, enabled) in items {
        if ui.add_enabled(*enabled, egui::Button::new(c.label())).clicked() {
            core.run(*c);
            ui.close();
        }
    }
}

/// Keyboard handling for the stage (nudge, delete, text editing, pen finish, tool keys).
pub fn stage_keys(core: &mut AppCore, ctx: &egui::Context) {
    let events = ctx.input(|i| i.events.clone());
    let text_editing = core.doc().is_some_and(|d| d.text_edit.is_some());
    if text_editing {
        let auto_key = core.settings.tools.auto_keyframe;
        let st = core.docs.get_mut(core.active).unwrap();
        let te = st.text_edit.clone().unwrap();
        let Some(current) = st.selected().into_iter().find(|(_, e)| e.id == te.element).and_then(|(_, e)| match e.kind {
            ElementKind::Text(t) => Some(t),
            _ => None,
        }) else {
            st.text_edit = None;
            return;
        };
        let mut text = current.plain_text();
        let mut cursor = te.cursor.min(text.len());
        let mut changed = false;
        let mut done = false;
        for ev in &events {
            match ev {
                egui::Event::Text(s) => {
                    text.insert_str(cursor, s);
                    cursor += s.len();
                    changed = true;
                }
                egui::Event::Paste(s) => {
                    text.insert_str(cursor, s);
                    cursor += s.len();
                    changed = true;
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => match key {
                    egui::Key::Backspace if cursor > 0 => {
                        let prev = text[..cursor].char_indices().last().map(|(i, _)| i).unwrap_or(0);
                        text.replace_range(prev..cursor, "");
                        cursor = prev;
                        changed = true;
                    }
                    egui::Key::Delete if cursor < text.len() => {
                        let next = text[cursor..].char_indices().nth(1).map(|(i, _)| cursor + i).unwrap_or(text.len());
                        text.replace_range(cursor..next, "");
                        changed = true;
                    }
                    egui::Key::ArrowLeft => cursor = text[..cursor].char_indices().last().map(|(i, _)| i).unwrap_or(0),
                    egui::Key::ArrowRight => cursor = text[cursor..].char_indices().nth(1).map(|(i, _)| cursor + i).unwrap_or(text.len()),
                    egui::Key::Home => cursor = text[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0),
                    egui::Key::End => cursor = text[cursor..].find('\n').map(|i| cursor + i).unwrap_or(text.len()),
                    egui::Key::Enter if !modifiers.command => {
                        text.insert(cursor, '\n');
                        cursor += 1;
                        changed = true;
                    }
                    egui::Key::Escape => done = true,
                    _ => {}
                },
                _ => {}
            }
        }
        if changed {
            if st.history.undo_labels().last() != Some("Edit Text") {
                st.checkpoint("Edit Text");
            } else {
                st.touch();
            }
            if let Some(e) = st.element_mut(te.element, auto_key)
                && let ElementKind::Text(t) = &mut e.kind
            {
                t.set_plain_text(&text);
            }
        }
        if done {
            st.text_edit = None;
            // Remove empty text blocks.
            if text.is_empty() {
                let id = te.element;
                st.selection = vec![id];
                ops::delete_selection(st);
            }
        } else if let Some(t) = &mut st.text_edit {
            t.cursor = cursor;
        }
        return;
    }
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    for ev in &events {
        if let egui::Event::Key { key, pressed: true, modifiers, .. } = ev {
            let auto_key = core.settings.tools.auto_keyframe;
            if modifiers.command || modifiers.alt {
                continue;
            }
            let nudge = if modifiers.shift { 10.0 } else { 1.0 };
            let d = match key {
                egui::Key::ArrowLeft => Some(Vec2::new(-nudge, 0.0)),
                egui::Key::ArrowRight => Some(Vec2::new(nudge, 0.0)),
                egui::Key::ArrowUp => Some(Vec2::new(0.0, -nudge)),
                egui::Key::ArrowDown => Some(Vec2::new(0.0, nudge)),
                _ => None,
            };
            if let Some(d) = d
                && let Some(st) = core.doc_mut()
                && !st.selection.is_empty()
            {
                ops::move_selection(st, d, Some("Nudge"), auto_key);
                continue;
            }
            match key {
                egui::Key::Escape => {
                    if core.tool == Tool::Pen && !core.tstate.pen.points.is_empty() {
                        finish_pen(core, false);
                    } else if let Some(st) = core.doc_mut() {
                        st.selection.clear();
                        st.anchor_sel = None;
                        core.tstate.warp_pins = None;
                    }
                }
                egui::Key::Enter if core.tool == Tool::Pen && !core.tstate.pen.points.is_empty() => finish_pen(core, false),
                egui::Key::Delete | egui::Key::Backspace => {
                    let st = core.doc_mut().unwrap_or_else(|| unreachable!());
                    if let Some((id, pi, ci)) = st.anchor_sel {
                        st.checkpoint("Delete Anchor Point");
                        if let Some(e) = st.element_mut(id, auto_key)
                            && let ElementKind::Shape(s) = &mut e.kind
                        {
                            s.paths[pi].path.delete_anchor(ci);
                        }
                        st.anchor_sel = None;
                    } else {
                        ops::delete_selection(st);
                    }
                }
                k => {
                    if let Some(t) = crate::commands::tool_key(*k, modifiers.shift) {
                        if core.tool == Tool::Pen && t != Tool::Pen {
                            finish_pen(core, false);
                        }
                        core.tool = t;
                    }
                }
            }
        }
    }
}
