//! Document editing operations used by menus, panels and tools.

use crate::state::{DocState, EditLevel, FrameSel};
use animara_document::{
    ColorEffect, Element, ElementKind, FrameClipboard, Keyframe, Layer, LayerKind, Scene, Shape, ShapePath, Symbol,
    SymbolInstance, SymbolKind, TextAlign, Tween, TweenKind,
};
use animara_geometry::{Affine, BoolOp, FillRule, Rect, Vec2, boolean_op};
use animara_renderer::bounds::element_bounds;

/// Application clipboard (elements copied from the stage, frames copied from the timeline).
#[derive(Default, Clone)]
pub struct Clipboard {
    pub elements: Vec<Element>,
    pub frames: Vec<FrameClipboard>,
}

// ---------------------------------------------------------------- elements

/// Add an element to the current layer's keyframe and select it.
pub fn add_element(st: &mut DocState, label: &str, e: Element, auto_key: bool) -> bool {
    if !st.layer_editable(st.layer) {
        return false;
    }
    st.checkpoint(label);
    let id = e.id;
    match st.editable_keyframe(auto_key) {
        Some(k) => {
            k.elements.push(e);
            st.selection = vec![id];
            true
        }
        None => false,
    }
}

pub fn delete_selection(st: &mut DocState) {
    if st.selection.is_empty() {
        return;
    }
    st.checkpoint("Delete");
    let sel = st.selection.clone();
    for li in 0..st.timeline().layers.len() {
        if !st.layer_editable(li) {
            continue;
        }
        let f = st.frame;
        let has = st.layer_elements(li).iter().any(|e| sel.contains(&e.id));
        if !has {
            continue;
        }
        let saved = st.layer;
        st.layer = li;
        if let Some(k) = st.editable_keyframe(true) {
            k.elements.retain(|e| !sel.contains(&e.id));
        }
        st.layer = saved;
        let _ = f;
    }
    st.selection.clear();
}

pub fn select_all(st: &mut DocState) {
    st.selection = (0..st.timeline().layers.len())
        .filter(|&li| st.layer_editable(li))
        .flat_map(|li| st.layer_elements(li).into_iter().filter(|e| !e.locked).map(|e| e.id))
        .collect();
}

/// Apply a mutation to every selected element (auto-keyframing tween frames).
pub fn modify_selection(st: &mut DocState, label: Option<&str>, auto_key: bool, mut f: impl FnMut(&mut Element)) {
    if st.selection.is_empty() {
        return;
    }
    if let Some(l) = label {
        st.checkpoint(l);
    } else {
        st.touch();
    }
    for id in st.selection.clone() {
        if let Some(e) = st.element_mut(id, auto_key) {
            f(e);
        }
    }
}

pub fn move_selection(st: &mut DocState, d: Vec2, label: Option<&str>, auto_key: bool) {
    modify_selection(st, label, auto_key, |e| {
        e.transform.tx += d.x;
        e.transform.ty += d.y;
    });
}

/// Transform selected elements about a world-space pivot.
pub fn transform_selection_about(st: &mut DocState, m: Affine, pivot: Vec2, label: Option<&str>, auto_key: bool) {
    let ctx = st.context_transform();
    let inv = ctx.invert().unwrap_or(Affine::IDENTITY);
    let local_pivot = inv.apply(pivot);
    let mm = Affine::around(m, local_pivot);
    modify_selection(st, label, auto_key, |e| e.transform = mm * e.transform);
}

pub fn remove_transform(st: &mut DocState, auto_key: bool) {
    modify_selection(st, Some("Remove Transform"), auto_key, |e| {
        let p = e.pivot_world();
        e.transform = Affine::translate(p.x - e.pivot.x, p.y - e.pivot.y);
    });
}

pub fn copy(st: &DocState, clip: &mut Clipboard) {
    clip.elements = st.selected().into_iter().map(|(_, e)| e).collect();
}

pub fn paste(st: &mut DocState, clip: &Clipboard, in_place: bool, auto_key: bool) {
    if clip.elements.is_empty() || !st.layer_editable(st.layer) {
        return;
    }
    let mut els: Vec<Element> = clip.elements.iter().map(Element::duplicate).collect();
    if !in_place {
        let b = els.iter().fold(Rect::EMPTY, |r, e| r.union(&element_bounds(&st.doc, e, &Affine::IDENTITY)));
        if !b.is_empty() {
            let target = Vec2::new(st.doc.width / 2.0, st.doc.height / 2.0);
            let d = target - b.center();
            for e in &mut els {
                e.transform.tx += d.x;
                e.transform.ty += d.y;
            }
        }
    }
    st.checkpoint("Paste");
    let ids: Vec<_> = els.iter().map(|e| e.id).collect();
    if let Some(k) = st.editable_keyframe(auto_key) {
        k.elements.extend(els);
        st.selection = ids;
    }
}

pub fn duplicate(st: &mut DocState, auto_key: bool) {
    let mut clip = Clipboard::default();
    copy(st, &mut clip);
    for e in &mut clip.elements {
        e.transform.tx += 10.0;
        e.transform.ty += 10.0;
    }
    paste(st, &clip, true, auto_key);
}

/// Arrange within the keyframe's stacking order.
pub fn arrange(st: &mut DocState, to_front: bool, all_the_way: bool, auto_key: bool) {
    let sel = st.selection.clone();
    if sel.is_empty() {
        return;
    }
    st.checkpoint("Arrange");
    for li in 0..st.timeline().layers.len() {
        if !st.layer_elements(li).iter().any(|e| sel.contains(&e.id)) {
            continue;
        }
        let saved = st.layer;
        st.layer = li;
        if let Some(k) = st.editable_keyframe(auto_key) {
            let n = k.elements.len();
            if all_the_way {
                let (s, rest): (Vec<Element>, Vec<Element>) = k.elements.drain(..).partition(|e| sel.contains(&e.id));
                k.elements = if to_front { rest.into_iter().chain(s).collect() } else { s.into_iter().chain(rest).collect() };
            } else if to_front {
                for i in (0..n.saturating_sub(1)).rev() {
                    if sel.contains(&k.elements[i].id) && !sel.contains(&k.elements[i + 1].id) {
                        k.elements.swap(i, i + 1);
                    }
                }
            } else {
                for i in 1..n {
                    if sel.contains(&k.elements[i].id) && !sel.contains(&k.elements[i - 1].id) {
                        k.elements.swap(i, i - 1);
                    }
                }
            }
        }
        st.layer = saved;
    }
}

pub fn group(st: &mut DocState, auto_key: bool) {
    let sel = st.selection.clone();
    if sel.is_empty() {
        return;
    }
    st.checkpoint("Group");
    let li = st.layer_of_element(sel[0]).unwrap_or(st.layer);
    st.layer = li;
    if let Some(k) = st.editable_keyframe(auto_key) {
        let pos = k.elements.iter().position(|e| sel.contains(&e.id)).unwrap_or(k.elements.len());
        let (children, rest): (Vec<Element>, Vec<Element>) = k.elements.drain(..).partition(|e| sel.contains(&e.id));
        k.elements = rest;
        let g = Element::new(ElementKind::Group { children });
        let id = g.id;
        k.elements.insert(pos.min(k.elements.len()), g);
        st.selection = vec![id];
    }
}

/// Ungroup groups, and Break Apart instances / text / bitmaps.
pub fn break_apart(st: &mut DocState, auto_key: bool) {
    let sel = st.selection.clone();
    if sel.is_empty() {
        return;
    }
    st.checkpoint("Break Apart");
    let doc_snapshot = st.doc.clone();
    let mut new_sel = Vec::new();
    for li in 0..st.timeline().layers.len() {
        if !st.layer_elements(li).iter().any(|e| sel.contains(&e.id)) {
            continue;
        }
        let saved = st.layer;
        st.layer = li;
        if let Some(k) = st.editable_keyframe(auto_key) {
            let mut out = Vec::new();
            for e in k.elements.drain(..) {
                if !sel.contains(&e.id) {
                    out.push(e);
                    continue;
                }
                match &e.kind {
                    ElementKind::Group { children } => {
                        for c in children {
                            let mut c = c.clone();
                            c.transform = e.transform * c.transform;
                            new_sel.push(c.id);
                            out.push(c);
                        }
                    }
                    ElementKind::Instance(inst) => {
                        if let Some(sym) = doc_snapshot.library.symbol(inst.symbol) {
                            let f = inst.first_frame.min(sym.timeline.length().saturating_sub(1));
                            for sl in (0..sym.timeline.layers.len()).rev() {
                                if matches!(sym.timeline.layers[sl].kind, LayerKind::Guide | LayerKind::Folder | LayerKind::Camera) {
                                    continue;
                                }
                                let (els, _) = animara_animation::layer_elements_at(&doc_snapshot, &sym.timeline, sl, f);
                                for c in els {
                                    let mut c = c.duplicate();
                                    c.transform = e.transform * c.transform;
                                    if let (ElementKind::Shape(s), ColorEffect::Tint { .. } | ColorEffect::Brightness { .. } | ColorEffect::Alpha { .. }) =
                                        (&mut c.kind, inst.color_effect)
                                    {
                                        let ct = inst.color_effect.to_transform();
                                        for p in &mut s.paths {
                                            if let Some(animara_document::Fill::Solid { color }) = &mut p.fill {
                                                *color = ct.apply(*color);
                                            }
                                        }
                                    }
                                    new_sel.push(c.id);
                                    out.push(c);
                                }
                            }
                        }
                    }
                    ElementKind::Text(t) => {
                        if t.plain_text().chars().count() > 1 {
                            // First break apart: one text element per character.
                            let layout = animara_text::fonts().layout(t);
                            for g in &layout.glyphs {
                                let ch: String = t.plain_text()[g.cluster..].chars().take(1).collect();
                                if ch.trim().is_empty() {
                                    continue;
                                }
                                let b = g.path.bounds();
                                let line = layout.lines.iter().find(|l| l.range.contains(&g.cluster) || l.range.start == g.cluster);
                                let mut nt = t.clone();
                                nt.set_plain_text(&ch);
                                nt.width = None;
                                nt.align = TextAlign::Left;
                                let top = line.map(|l| l.top).unwrap_or(0.0);
                                let mut ne = Element::new(ElementKind::Text(nt));
                                ne.transform = e.transform * Affine::translate(b.min.x, top);
                                new_sel.push(ne.id);
                                out.push(ne);
                            }
                        } else {
                            let shape = animara_text::text_to_shape(t);
                            let mut ne = Element::new(ElementKind::Shape(shape));
                            ne.transform = e.transform;
                            new_sel.push(ne.id);
                            out.push(ne);
                        }
                    }
                    ElementKind::Bitmap(b) => {
                        // Bitmap → shape filled with the bitmap.
                        if let Some(a) = doc_snapshot.library.bitmap(b.asset) {
                            let r = Rect::new(0.0, 0.0, a.width as f64, a.height as f64);
                            let mut ne = Element::shape(vec![ShapePath::filled(
                                animara_geometry::Path::rect(r),
                                animara_document::Fill::Bitmap { asset: b.asset, matrix: Affine::IDENTITY, smooth: true, repeat: false },
                            )]);
                            ne.transform = e.transform;
                            new_sel.push(ne.id);
                            out.push(ne);
                        }
                    }
                    ElementKind::Shape(_) => out.push(e),
                }
            }
            k.elements = out;
        }
        st.layer = saved;
    }
    st.selection = new_sel;
}

/// Convert the selection into a new symbol and replace it with an instance.
pub fn convert_to_symbol(st: &mut DocState, name: &str, kind: SymbolKind, registration_center: bool, auto_key: bool) -> bool {
    let sel = st.selected();
    if sel.is_empty() {
        return false;
    }
    let ctx = st.context_transform();
    let bounds = sel.iter().fold(Rect::EMPTY, |r, (_, e)| r.union(&element_bounds(&st.doc, e, &Affine::IDENTITY)));
    let _ = ctx;
    let reg = if registration_center { bounds.center() } else { bounds.min };
    st.checkpoint("Convert to Symbol");
    let mut sym = Symbol::new(&st.doc.library.unique_name(name), kind);
    sym.timeline.layers[0].frames[0].elements = sel
        .iter()
        .map(|(_, e)| {
            let mut e = e.clone();
            e.transform = Affine::translate(-reg.x, -reg.y) * e.transform;
            e
        })
        .collect();
    let sid = st.doc.library.add_symbol(sym);
    let ids: Vec<_> = sel.iter().map(|(_, e)| e.id).collect();
    let li = sel[0].0;
    st.layer = li;
    let mut inst = Element::new(ElementKind::Instance(SymbolInstance::new(sid))).with_transform(Affine::translate(reg.x, reg.y));
    if registration_center {
        inst.pivot = Vec2::ZERO;
    } else {
        inst.pivot = bounds.center() - reg;
    }
    let iid = inst.id;
    // Remove originals from every layer at this frame.
    for l in 0..st.timeline().layers.len() {
        if !st.layer_elements(l).iter().any(|e| ids.contains(&e.id)) {
            continue;
        }
        let saved = st.layer;
        st.layer = l;
        if let Some(k) = st.editable_keyframe(auto_key) {
            let pos = k.elements.iter().position(|e| ids.contains(&e.id));
            k.elements.retain(|e| !ids.contains(&e.id));
            if l == li {
                let p = pos.unwrap_or(k.elements.len()).min(k.elements.len());
                k.elements.insert(p, inst.clone());
            }
        }
        st.layer = saved;
    }
    st.selection = vec![iid];
    true
}

pub fn new_symbol(st: &mut DocState, name: &str, kind: SymbolKind) {
    st.checkpoint("New Symbol");
    let sym = Symbol::new(&st.doc.library.unique_name(name), kind);
    let id = st.doc.library.add_symbol(sym);
    enter_symbol(st, id, None);
}

/// Enter symbol editing mode (`in_place` = instance transform for edit-in-place).
pub fn enter_symbol(st: &mut DocState, symbol: animara_document::Id, in_place: Option<Affine>) {
    let parent_frame = st.frame;
    st.stack.push(EditLevel { symbol, instance_transform: in_place.unwrap_or(Affine::IDENTITY), in_place: in_place.is_some(), parent_frame });
    st.frame = 0;
    st.layer = 0;
    st.selection.clear();
    st.frame_sel = Some(FrameSel::single(0, 0));
    st.revision += 1;
}

pub fn exit_to_level(st: &mut DocState, level: usize) {
    if let Some(l) = st.stack.get(level) {
        st.frame = l.parent_frame;
    }
    st.stack.truncate(level);
    st.selection.clear();
    st.frame = st.frame.min(st.timeline().length().saturating_sub(1));
    st.clamp();
    st.revision += 1;
}

/// Align selected elements. `which`: 0 left,1 hcenter,2 right,3 top,4 vcenter,5 bottom.
pub fn align(st: &mut DocState, which: u8, to_stage: bool, auto_key: bool) {
    let sel = st.selected();
    if sel.is_empty() || (sel.len() < 2 && !to_stage) {
        return;
    }
    let ctx = st.context_transform();
    let bounds: Vec<(animara_document::Id, Rect)> = sel.iter().map(|(_, e)| (e.id, element_bounds(&st.doc, e, &ctx))).collect();
    let all = if to_stage {
        ctx.invert().map(|_| Rect::new(0.0, 0.0, st.doc.width, st.doc.height)).unwrap_or(Rect::new(0.0, 0.0, st.doc.width, st.doc.height))
    } else {
        bounds.iter().fold(Rect::EMPTY, |r, (_, b)| r.union(b))
    };
    st.checkpoint("Align");
    for (id, b) in bounds {
        let d = match which {
            0 => Vec2::new(all.min.x - b.min.x, 0.0),
            1 => Vec2::new(all.center().x - b.center().x, 0.0),
            2 => Vec2::new(all.max.x - b.max.x, 0.0),
            3 => Vec2::new(0.0, all.min.y - b.min.y),
            4 => Vec2::new(0.0, all.center().y - b.center().y),
            _ => Vec2::new(0.0, all.max.y - b.max.y),
        };
        if let Some(e) = st.element_mut(id, auto_key) {
            e.transform.tx += d.x;
            e.transform.ty += d.y;
        }
    }
}

/// Distribute centers evenly (`horizontal` or vertical) or space evenly.
pub fn distribute(st: &mut DocState, horizontal: bool, auto_key: bool) {
    let sel = st.selected();
    if sel.len() < 3 {
        return;
    }
    let ctx = st.context_transform();
    let mut items: Vec<(animara_document::Id, Rect)> = sel.iter().map(|(_, e)| (e.id, element_bounds(&st.doc, e, &ctx))).collect();
    let key = |r: &Rect| if horizontal { r.center().x } else { r.center().y };
    items.sort_by(|a, b| key(&a.1).total_cmp(&key(&b.1)));
    let first = key(&items[0].1);
    let last = key(&items[items.len() - 1].1);
    let step = (last - first) / (items.len() - 1) as f64;
    st.checkpoint("Distribute");
    for (i, (id, r)) in items.into_iter().enumerate() {
        let target = first + step * i as f64;
        let d = target - key(&r);
        if let Some(e) = st.element_mut(id, auto_key) {
            if horizontal {
                e.transform.tx += d;
            } else {
                e.transform.ty += d;
            }
        }
    }
}

pub fn match_size(st: &mut DocState, width: bool, auto_key: bool) {
    let sel = st.selected();
    if sel.len() < 2 {
        return;
    }
    let ctx = st.context_transform();
    let bounds: Vec<(animara_document::Id, Rect)> = sel.iter().map(|(_, e)| (e.id, element_bounds(&st.doc, e, &ctx))).collect();
    let target = bounds.iter().map(|(_, b)| if width { b.width() } else { b.height() }).fold(0.0, f64::max);
    st.checkpoint("Match Size");
    for (id, b) in bounds {
        let cur = if width { b.width() } else { b.height() };
        if cur < 1e-6 {
            continue;
        }
        let k = target / cur;
        let s = if width { Affine::scale(k, 1.0) } else { Affine::scale(1.0, k) };
        let m = Affine::around(s, b.min);
        if let Some(e) = st.element_mut(id, auto_key) {
            e.transform = m * e.transform;
        }
    }
}

/// Boolean combine of selected shapes (Modify ▸ Combine Objects).
pub fn combine(st: &mut DocState, op: BoolOp, auto_key: bool) {
    let sel = st.selected();
    let shapes: Vec<(usize, Element)> = sel.into_iter().filter(|(_, e)| matches!(e.kind, ElementKind::Shape(_))).collect();
    if shapes.len() < 2 {
        return;
    }
    let to_world = |e: &Element| -> (animara_geometry::Path, Option<animara_document::Fill>, Option<animara_document::Stroke>) {
        let ElementKind::Shape(s) = &e.kind else { unreachable!() };
        let ws = s.transformed(&e.transform);
        let mut p = animara_geometry::Path::new();
        for sp in &ws.paths {
            p.extend(&sp.path);
        }
        let fill = ws.paths.iter().find_map(|p| p.fill.clone());
        let stroke = ws.paths.iter().find_map(|p| p.stroke.clone());
        (p, fill, stroke)
    };
    // Stacking order: the top-most object acts as the cutter for Punch/Crop.
    let (bottom, top) = (&shapes[0].1, &shapes[shapes.len() - 1].1);
    let result = match op {
        BoolOp::Union | BoolOp::Intersect | BoolOp::Xor => {
            let mut acc = to_world(&shapes[0].1);
            for (_, e) in &shapes[1..] {
                let (p, ..) = to_world(e);
                acc.0 = boolean_op(&acc.0, &p, op, FillRule::NonZero);
            }
            acc
        }
        BoolOp::Difference => {
            let (b, fill, stroke) = to_world(bottom);
            let (t, ..) = to_world(top);
            (boolean_op(&b, &t, BoolOp::Difference, FillRule::NonZero), fill, stroke)
        }
    };
    let ids: Vec<_> = shapes.iter().map(|(_, e)| e.id).collect();
    st.checkpoint("Combine Objects");
    let li = shapes[0].0;
    let mut ne = Element::shape(vec![ShapePath {
        path: result.0,
        fill: result.1.or(Some(animara_document::Fill::solid(animara_document::Color::BLACK))),
        stroke: result.2,
        fill_rule: FillRule::NonZero,
        primitive: None,
    }]);
    ne.pivot = ne.pivot;
    let nid = ne.id;
    for l in 0..st.timeline().layers.len() {
        if !st.layer_elements(l).iter().any(|e| ids.contains(&e.id)) {
            continue;
        }
        let saved = st.layer;
        st.layer = l;
        if let Some(k) = st.editable_keyframe(auto_key) {
            let pos = k.elements.iter().position(|e| ids.contains(&e.id));
            k.elements.retain(|e| !ids.contains(&e.id));
            if l == li {
                let p = pos.unwrap_or(0).min(k.elements.len());
                k.elements.insert(p, ne.clone());
            }
        }
        st.layer = saved;
    }
    st.selection = vec![nid];
}

/// Crop: keep the intersection of the bottom shape with the top shape.
pub fn crop(st: &mut DocState, auto_key: bool) {
    combine(st, BoolOp::Intersect, auto_key);
}

/// Shape optimisations (Modify ▸ Shape): smooth, straighten, optimize, lines→fills.
pub fn reshape(st: &mut DocState, mode: u8, auto_key: bool) {
    let label = ["Smooth", "Straighten", "Optimize Curves", "Convert Lines to Fills"][mode as usize % 4];
    modify_selection(st, Some(label), auto_key, |e| {
        let ElementKind::Shape(s) = &mut e.kind else { return };
        let mut out = Vec::new();
        for sp in s.paths.drain(..) {
            let closed = sp.path.subpaths().iter().all(|p| p.closed);
            match mode {
                0 | 1 | 2 => {
                    let mut np = animara_geometry::Path::new();
                    for (pts, cl) in sp.path.flatten(0.5) {
                        let tol = match mode {
                            0 => 1.5,
                            1 => 3.0,
                            _ => 0.75,
                        };
                        let path = if mode == 1 {
                            animara_geometry::Path::polyline(&animara_geometry::simplify_polyline(&pts, tol), cl)
                        } else {
                            let mut p = animara_geometry::fit_smooth_path(&pts, tol);
                            if cl && !matches!(p.cmds.last(), Some(animara_geometry::PathCmd::Close)) {
                                p.close();
                            }
                            p
                        };
                        np.extend(&path);
                    }
                    out.push(ShapePath { path: np, primitive: None, ..sp });
                }
                _ => {
                    if let Some(stroke) = &sp.stroke {
                        if sp.fill.is_some() && closed {
                            out.push(ShapePath { stroke: None, primitive: None, ..sp.clone() });
                        }
                        let mut outline = animara_geometry::Path::new();
                        for (pts, cl) in sp.path.flatten(0.5) {
                            let mut samples: Vec<animara_geometry::StrokeSample> =
                                pts.iter().map(|p| animara_geometry::StrokeSample { pos: *p, width: stroke.width }).collect();
                            if cl && let Some(f) = samples.first().copied() {
                                samples.push(f);
                            }
                            outline.extend(&animara_geometry::variable_width_outline(&samples));
                        }
                        out.push(ShapePath::filled(outline, stroke.fill.clone()));
                    } else {
                        out.push(sp);
                    }
                }
            }
        }
        s.paths = out;
    });
}

// ---------------------------------------------------------------- timeline

fn sel_layers_and_frames(st: &DocState) -> (Vec<usize>, u32, u32) {
    match st.frame_sel {
        Some(fs) => (fs.layers().collect(), fs.frames().0, fs.frames().1),
        None => (vec![st.layer], st.frame, st.frame),
    }
}

pub fn insert_frames(st: &mut DocState) {
    let (layers, f0, f1) = sel_layers_and_frames(st);
    st.checkpoint("Insert Frame");
    let n = f1 - f0 + 1;
    for li in layers {
        if li < st.timeline().layers.len() && !st.timeline().layers[li].locked && st.timeline().layers[li].kind != LayerKind::Folder {
            let l = &mut st.timeline_mut().layers[li];
            if f0 >= l.end() { l.extend_to(f1) } else { l.insert_frames(f0, n) }
        }
    }
}

pub fn remove_frames(st: &mut DocState) {
    let (layers, f0, f1) = sel_layers_and_frames(st);
    st.checkpoint("Remove Frames");
    for li in layers {
        if li < st.timeline().layers.len() && !st.timeline().layers[li].locked {
            st.timeline_mut().layers[li].remove_frames(f0, f1 - f0 + 1);
        }
    }
    st.selection.clear();
}

pub fn insert_keyframe(st: &mut DocState, blank: bool) {
    let (layers, f0, _) = sel_layers_and_frames(st);
    st.checkpoint(if blank { "Insert Blank Keyframe" } else { "Insert Keyframe" });
    for li in layers {
        if li >= st.timeline().layers.len() || st.timeline().layers[li].locked || st.timeline().layers[li].kind == LayerKind::Folder {
            continue;
        }
        if blank {
            let l = &mut st.timeline_mut().layers[li];
            if !l.insert_blank_keyframe(f0) {
                // Already a non-empty keyframe: insert at the next frame like the classic tool.
                l.insert_blank_keyframe(f0 + 1);
            }
        } else {
            // Copy the interpolated state when inside a tween.
            let content = {
                let tl = st.timeline();
                let l = &tl.layers[li];
                l.keyframe_at(f0).filter(|k| k.tween.is_some() && k.start != f0).map(|_| animara_animation::layer_elements_at(&st.doc, tl, li, f0).0)
            };
            let l = &mut st.timeline_mut().layers[li];
            l.insert_keyframe(f0, content);
            if let Some(i) = l.span_index(f0) {
                l.frames[i].script.clear();
                l.frames[i].label = None;
            }
        }
    }
    st.selection.clear();
}

pub fn clear_keyframe(st: &mut DocState) {
    let (layers, f0, f1) = sel_layers_and_frames(st);
    st.checkpoint("Clear Keyframe");
    for li in layers {
        if li < st.timeline().layers.len() {
            for f in f0..=f1 {
                st.timeline_mut().layers[li].clear_keyframe(f);
            }
        }
    }
}

pub fn create_tween(st: &mut DocState, kind: Option<TweenKind>) {
    let (layers, f0, f1) = sel_layers_and_frames(st);
    st.checkpoint(match kind {
        Some(TweenKind::Classic) => "Create Classic Tween",
        Some(TweenKind::Motion) => "Create Motion Tween",
        Some(TweenKind::Shape) => "Create Shape Tween",
        None => "Remove Tween",
    });
    for li in layers {
        if li >= st.timeline().layers.len() {
            continue;
        }
        if kind == Some(TweenKind::Motion) {
            // Motion tweens need a single symbol instance: convert loose shapes automatically.
            let needs_symbol = st
                .timeline()
                .layers[li]
                .keyframe_at(f0)
                .is_some_and(|k| k.elements.iter().any(|e| !matches!(e.kind, ElementKind::Instance(_))));
            if needs_symbol {
                let f = st.frame;
                st.frame = st.timeline().layers[li].keyframe_at(f0).map(|k| k.start).unwrap_or(f0);
                st.layer = li;
                st.selection = st.layer_elements(li).iter().map(|e| e.id).collect();
                let name = format!("Tween {}", st.doc.library.symbols.len() + 1);
                convert_to_symbol(st, &name, SymbolKind::Graphic, true, false);
                st.frame = f;
            }
            let l = &mut st.timeline_mut().layers[li];
            if l.end() <= f1 {
                l.extend_to(f1.max(f0 + 23));
            }
        }
        let starts: Vec<u32> = {
            let l = &st.timeline().layers[li];
            let mut s: Vec<u32> = (f0..=f1).filter_map(|f| l.keyframe_at(f).map(|k| k.start)).collect();
            s.dedup();
            s
        };
        for s in starts {
            st.timeline_mut().layers[li].set_tween(s, kind.map(Tween::new));
        }
    }
}

pub fn copy_frames(st: &DocState, clip: &mut Clipboard) {
    let (layers, f0, f1) = sel_layers_and_frames(st);
    clip.frames = layers.into_iter().filter(|&li| li < st.timeline().layers.len()).map(|li| st.timeline().layers[li].copy_frames(f0, f1)).collect();
}

pub fn paste_frames(st: &mut DocState, clip: &Clipboard) {
    if clip.frames.is_empty() {
        return;
    }
    let (layers, f0, _) = sel_layers_and_frames(st);
    let first = layers.first().copied().unwrap_or(st.layer);
    st.checkpoint("Paste Frames");
    for (i, fc) in clip.frames.iter().enumerate() {
        let li = first + i;
        if li >= st.timeline().layers.len() {
            let name = st.timeline().unique_layer_name("Layer");
            let n = st.timeline().layers.len();
            st.timeline_mut().add_layer(&name, n);
        }
        st.timeline_mut().layers[li].paste_frames(f0, fc);
    }
}

pub fn reverse_frames(st: &mut DocState) {
    let (layers, f0, f1) = sel_layers_and_frames(st);
    st.checkpoint("Reverse Frames");
    for li in layers {
        if li < st.timeline().layers.len() {
            st.timeline_mut().layers[li].reverse_frames(f0, f1);
        }
    }
}

pub fn convert_to_keyframes(st: &mut DocState) {
    let (layers, f0, f1) = sel_layers_and_frames(st);
    st.checkpoint("Convert to Keyframes");
    for li in layers {
        if li >= st.timeline().layers.len() {
            continue;
        }
        // Bake tweened frames so each new keyframe keeps its interpolated pose.
        let baked: Vec<(u32, Vec<Element>)> =
            (f0..=f1).map(|f| (f, animara_animation::layer_elements_at(&st.doc, st.timeline(), li, f).0)).collect();
        let l = &mut st.timeline_mut().layers[li];
        l.convert_to_keyframes(f0, f1);
        for (f, els) in baked {
            if let Some(k) = l.keyframe_at_mut(f) {
                k.elements = els;
                k.tween = None;
            }
        }
    }
}

pub fn add_layer(st: &mut DocState, kind: LayerKind) {
    st.checkpoint("New Layer");
    let base = match kind {
        LayerKind::Folder => "Folder",
        LayerKind::Camera => "Camera",
        LayerKind::Guide => "Guide",
        _ => "Layer",
    };
    if kind == LayerKind::Camera && st.timeline().layers.iter().any(|l| l.kind == LayerKind::Camera) {
        return;
    }
    let name = st.timeline().unique_layer_name(base);
    let at = if kind == LayerKind::Camera { 0 } else { st.layer.min(st.timeline().layers.len()) };
    let folder = st.current_layer().and_then(|l| if l.kind == LayerKind::Folder { Some(l.id) } else { l.folder });
    let (w, h) = (st.doc.width, st.doc.height);
    let tl = st.timeline_mut();
    let id = tl.add_layer(&name, at);
    let l = tl.layer_mut(id).unwrap();
    l.kind = kind;
    if kind == LayerKind::Folder {
        l.frames.clear();
    } else if kind != LayerKind::Camera {
        l.folder = folder;
    }
    if kind == LayerKind::Camera {
        let len = l.end().max(1);
        l.frames = vec![Keyframe::new(0, len)];
        l.frames[0].camera = Some(animara_document::CameraState::centered(w, h));
    }
    st.layer = at;
    st.frame_sel = Some(FrameSel::single(at, st.frame));
}

pub fn delete_layer(st: &mut DocState) {
    let (layers, ..) = sel_layers_and_frames(st);
    if st.timeline().layers.len() <= layers.len() {
        return;
    }
    st.checkpoint("Delete Layer");
    let ids: Vec<_> = layers.iter().filter_map(|&li| st.timeline().layers.get(li).map(|l| l.id)).collect();
    for id in ids {
        if st.timeline().layers.len() > 1 {
            st.timeline_mut().remove_layer(id);
        }
    }
    st.clamp();
    st.selection.clear();
}

pub fn duplicate_layer(st: &mut DocState) {
    let Some(l) = st.current_layer().cloned() else { return };
    st.checkpoint("Duplicate Layer");
    let mut nl: Layer = l;
    nl.id = animara_document::fresh_id();
    nl.name = format!("{} copy", nl.name);
    for k in &mut nl.frames {
        for e in &mut k.elements {
            e.reassign_ids();
        }
    }
    let at = st.layer;
    st.timeline_mut().layers.insert(at, nl);
}

pub fn move_layer(st: &mut DocState, from: usize, to: usize) {
    let n = st.timeline().layers.len();
    if from >= n || to > n || from == to {
        return;
    }
    st.checkpoint("Move Layer");
    let tl = st.timeline_mut();
    let l = tl.layers.remove(from);
    let to = if to > from { to - 1 } else { to };
    let to = to.min(tl.layers.len());
    // Dropping onto a folder's children adopts that folder.
    let folder = if to < tl.layers.len() {
        if tl.layers[to].kind == LayerKind::Folder && to > 0 { tl.layers[to - 1].folder } else { tl.layers[to].folder }
    } else {
        None
    };
    let mut l = l;
    if l.kind != LayerKind::Folder && l.folder != folder {
        l.folder = folder;
    }
    tl.layers.insert(to, l);
    st.layer = to;
}

/// Distribute selected elements to their own new layers.
pub fn distribute_to_layers(st: &mut DocState) {
    let sel = st.selected();
    if sel.len() < 2 {
        return;
    }
    st.checkpoint("Distribute to Layers");
    let ids: Vec<_> = sel.iter().map(|(_, e)| e.id).collect();
    let src_layer = sel[0].0;
    let f = st.frame;
    if let Some(k) = st.timeline_mut().layers[src_layer].keyframe_at_mut(f) {
        k.elements.retain(|e| !ids.contains(&e.id));
    }
    let len = st.timeline().length();
    for (i, (_, e)) in sel.into_iter().enumerate() {
        let name = match &e.kind {
            ElementKind::Instance(inst) => st.doc.library.symbol(inst.symbol).map(|s| s.name.clone()).unwrap_or_default(),
            _ => st.timeline().unique_layer_name("Layer"),
        };
        let mut l = Layer::new(&name);
        l.frames = vec![Keyframe::new(0, len)];
        if f > 0 {
            l.frames[0].duration = f;
            let mut k = Keyframe::new(f, len - f);
            k.elements.push(e);
            l.frames.push(k);
        } else {
            l.frames[0].elements.push(e);
        }
        st.timeline_mut().layers.insert(src_layer + i, l);
    }
}

pub fn add_scene(st: &mut DocState) {
    st.checkpoint("Insert Scene");
    let name = format!("Scene {}", st.doc.scenes.len() + 1);
    st.doc.scenes.push(Scene::new(&name));
    st.scene = st.doc.scenes.len() - 1;
    st.stack.clear();
    st.frame = 0;
    st.layer = 0;
    st.selection.clear();
}

pub fn swap_symbol(st: &mut DocState, new_symbol: animara_document::Id, auto_key: bool) {
    modify_selection(st, Some("Swap Symbol"), auto_key, |e| {
        if let Some(i) = e.as_instance_mut() {
            i.symbol = new_symbol;
        }
    });
}

pub fn duplicate_symbol(st: &mut DocState, id: animara_document::Id) -> Option<animara_document::Id> {
    let s = st.doc.library.symbol(id)?.clone();
    st.checkpoint("Duplicate Symbol");
    let mut ns = s;
    ns.id = animara_document::fresh_id();
    ns.name = st.doc.library.unique_name(&format!("{} copy", ns.name));
    for l in &mut ns.timeline.layers {
        l.id = animara_document::fresh_id();
    }
    Some(st.doc.library.add_symbol(ns))
}

pub fn delete_library_item(st: &mut DocState, id: animara_document::Id) {
    st.checkpoint("Delete Library Item");
    st.doc.library.symbols.retain(|s| s.id != id);
    st.doc.library.bitmaps.retain(|b| b.id != id);
    st.doc.library.sounds.retain(|s| s.id != id);
    // Remove instances/references to the deleted item everywhere.
    fn purge(els: &mut Vec<Element>, id: animara_document::Id) {
        els.retain(|e| match &e.kind {
            ElementKind::Instance(i) => i.symbol != id,
            ElementKind::Bitmap(b) => b.asset != id,
            _ => true,
        });
        for e in els.iter_mut() {
            if let ElementKind::Group { children } = &mut e.kind {
                purge(children, id);
            }
        }
    }
    let mut tls: Vec<&mut animara_document::Timeline> = st.doc.scenes.iter_mut().map(|s| &mut s.timeline).collect();
    tls.extend(st.doc.library.symbols.iter_mut().map(|s| &mut s.timeline));
    for tl in tls {
        for l in &mut tl.layers {
            for k in &mut l.frames {
                purge(&mut k.elements, id);
                if k.sound.as_ref().is_some_and(|s| s.asset == id) {
                    k.sound = None;
                }
            }
        }
    }
    st.clamp();
}

/// Convert a shape element's stroke-only closed paths into filled paths (paint bucket).
pub fn shape_from(e: &Element) -> Option<&Shape> {
    match &e.kind {
        ElementKind::Shape(s) => Some(s),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use animara_document::{Color, Document, Fill};
    use animara_geometry::Path;

    fn st_with_squares() -> DocState {
        let mut st = DocState::new(Document::new(200.0, 200.0, 24.0), None);
        for x in [0.0, 50.0, 120.0] {
            let e = Element::shape(vec![ShapePath::filled(Path::rect(Rect::new(x, 0.0, x + 20.0, 20.0)), Fill::solid(Color::BLACK))]);
            add_element(&mut st, "Draw", e, true);
        }
        st
    }

    #[test]
    fn align_and_distribute() {
        let mut st = st_with_squares();
        select_all(&mut st);
        align(&mut st, 3, false, true);
        distribute(&mut st, true, true);
        let xs: Vec<f64> = st.selected().iter().map(|(_, e)| element_bounds(&st.doc, e, &Affine::IDENTITY).center().x).collect();
        assert!((xs[1] - (xs[0] + xs[2]) / 2.0).abs() < 1e-6);
        st.undo();
        st.undo();
        assert_eq!(st.selected().len(), 3);
    }

    #[test]
    fn group_ungroup_and_symbol() {
        let mut st = st_with_squares();
        select_all(&mut st);
        group(&mut st, true);
        assert_eq!(st.layer_elements(0).len(), 1);
        break_apart(&mut st, true);
        assert_eq!(st.layer_elements(0).len(), 3);
        select_all(&mut st);
        assert!(convert_to_symbol(&mut st, "Squares", SymbolKind::MovieClip, true, true));
        assert_eq!(st.layer_elements(0).len(), 1);
        assert_eq!(st.doc.library.symbols.len(), 1);
        break_apart(&mut st, true);
        assert_eq!(st.layer_elements(0).len(), 3);
    }

    #[test]
    fn combine_union() {
        let mut st = DocState::new(Document::new(200.0, 200.0, 24.0), None);
        for x in [0.0, 10.0] {
            let e = Element::shape(vec![ShapePath::filled(Path::rect(Rect::new(x, 0.0, x + 20.0, 20.0)), Fill::solid(Color::BLACK))]);
            add_element(&mut st, "Draw", e, true);
        }
        select_all(&mut st);
        combine(&mut st, BoolOp::Union, true);
        assert_eq!(st.layer_elements(0).len(), 1);
    }

    #[test]
    fn timeline_ops_with_auto_keyframe() {
        let mut st = st_with_squares();
        st.frame_sel = Some(FrameSel::single(0, 9));
        insert_keyframe(&mut st, false);
        st.frame_sel = Some(FrameSel::single(0, 0));
        create_tween(&mut st, Some(TweenKind::Classic));
        assert!(st.timeline().layers[0].frames[0].tween.is_some());
        // Moving at a tweened frame auto-inserts a keyframe.
        st.frame = 5;
        select_all(&mut st);
        move_selection(&mut st, Vec2::new(5.0, 0.0), Some("Move"), true);
        assert!(st.timeline().layers[0].is_keyframe(5));
        st.timeline().validate().unwrap();
    }
}
