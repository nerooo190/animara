use crate::tween::{camera_at, layer_elements_at};
use animara_document::{
    BitmapInstance, BlendMode, Color, ColorTransform, Document, Element, ElementKind, Filter, Id, LayerKind, LoopMode,
    Shape, SymbolKind, TextElement, Timeline,
};
use animara_geometry::{Affine, Vec2};
use std::collections::HashMap;

/// Flattened, renderer-ready drawing command.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayItem {
    Shape {
        shape: Shape,
        transform: Affine,
        color: ColorTransform,
        /// Outline-mode color (layer outlines in authoring).
        outline: Option<Color>,
        /// Element id in the timeline being evaluated (top-level only).
        source: Option<Id>,
    },
    Text { text: TextElement, transform: Affine, color: ColorTransform, source: Option<Id> },
    Bitmap { bitmap: BitmapInstance, transform: Affine, color: ColorTransform, source: Option<Id> },
    /// Composited group (symbol instance with filters, blend mode or bitmap caching).
    Group { items: Vec<DisplayItem>, filters: Vec<Filter>, blend: BlendMode, scale: f64, source: Option<Id> },
    /// Content clipped by the alpha of the mask items.
    Masked { mask: Vec<DisplayItem>, content: Vec<DisplayItem> },
}

/// A complete frame ready to render.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayList {
    pub items: Vec<DisplayItem>,
    pub background: Color,
    pub width: f64,
    pub height: f64,
    /// Camera view transform (stage → output), identity without a camera layer.
    pub view: Affine,
}

/// Evaluation options.
#[derive(Debug, Clone, Default)]
pub struct EvalOptions {
    /// Authoring mode: draw guide layers, outline layers, apply masks only when locked.
    pub authoring: bool,
    /// Skip layers whose eye is off (authoring) — exports always skip hidden layers too.
    pub respect_visibility: bool,
    /// Advance movie clips with the parent timeline (export / test movie).
    pub play_movieclips: bool,
    /// Apply camera layers.
    pub apply_camera: bool,
    /// Per-instance playhead overrides from the player runtime (element id → frame).
    pub instance_frames: HashMap<Id, u32>,
    /// Hide instances flagged invisible (`visible = false`).
    pub honor_instance_visibility: bool,
}

impl EvalOptions {
    pub fn authoring() -> EvalOptions {
        EvalOptions { authoring: true, respect_visibility: true, apply_camera: false, ..Default::default() }
    }
    pub fn export() -> EvalOptions {
        EvalOptions {
            authoring: false,
            respect_visibility: true,
            play_movieclips: true,
            apply_camera: true,
            honor_instance_visibility: true,
            ..Default::default()
        }
    }
}

const MAX_DEPTH: usize = 32;

/// Map a global frame (scenes back to back) to `(scene index, local frame)`.
pub fn scene_at_global_frame(doc: &Document, global: u32) -> (usize, u32) {
    let mut g = global;
    for (i, s) in doc.scenes.iter().enumerate() {
        let len = s.timeline.length();
        if g < len {
            return (i, g);
        }
        g -= len;
    }
    let last = doc.scenes.len().saturating_sub(1);
    (last, doc.scenes.get(last).map(|s| s.timeline.length().saturating_sub(1)).unwrap_or(0))
}

/// Evaluate a scene frame into a display list.
pub fn evaluate_document_frame(doc: &Document, scene: usize, frame: u32, opts: &EvalOptions) -> DisplayList {
    let mut view = Affine::IDENTITY;
    let Some(sc) = doc.scenes.get(scene) else {
        return DisplayList { items: Vec::new(), background: doc.background, width: doc.width, height: doc.height, view };
    };
    let tl = &sc.timeline;
    let mut camera = None;
    if opts.apply_camera
        && let Some(cl) = tl.layers.iter().find(|l| l.kind == LayerKind::Camera && l.visible)
        && let Some(c) = camera_at(cl, frame)
    {
        view = Affine::translate(doc.width / 2.0, doc.height / 2.0)
            * Affine::rotate(-c.rotation.to_radians())
            * Affine::scale(c.zoom, c.zoom)
            * Affine::translate(-c.x, -c.y);
        camera = Some(c);
    }
    let mut items = evaluate_timeline(doc, tl, frame, Affine::IDENTITY, ColorTransform::IDENTITY, opts, 0, true);
    if let Some(c) = camera {
        // Layer depth parallax: deeper layers move less with the camera.
        let center = Vec2::new(doc.width / 2.0, doc.height / 2.0);
        let shift = Vec2::new(c.x, c.y) - center;
        let mut idx = 0;
        let mut out = Vec::with_capacity(items.len());
        for layer_items in split_by_layer_depth(doc, tl, frame, opts, &mut items, &mut idx) {
            let (depth, its) = layer_items;
            if depth.abs() < 1e-9 {
                out.extend(its);
            } else {
                let k = 1.0 / (1.0 + depth / 100.0);
                let m = Affine::translate(shift.x * (1.0 - k), shift.y * (1.0 - k));
                out.extend(its.into_iter().map(|i| transform_item(i, &m)));
            }
        }
        items = out;
        if let Some((tint, amount)) = c.tint {
            let ct = animara_document::ColorEffect::Tint { color: tint, amount }.to_transform();
            items = items.into_iter().map(|i| recolor_item(i, &ct)).collect();
        }
    }
    DisplayList { items, background: doc.background, width: doc.width, height: doc.height, view }
}

/// Group already-evaluated top-level items by the depth of the layer they came from.
fn split_by_layer_depth(
    doc: &Document,
    tl: &Timeline,
    frame: u32,
    opts: &EvalOptions,
    items: &mut Vec<DisplayItem>,
    _idx: &mut usize,
) -> Vec<(f64, Vec<DisplayItem>)> {
    if tl.layers.iter().all(|l| l.depth.abs() < 1e-9) {
        return vec![(0.0, std::mem::take(items))];
    }
    // Re-evaluate layer by layer to know each layer's items (bottom to top).
    let mut out = Vec::new();
    for li in (0..tl.layers.len()).rev() {
        let l = &tl.layers[li];
        if !layer_draws_directly(tl, li, opts) {
            continue;
        }
        let its = layer_items(doc, tl, li, frame, Affine::IDENTITY, ColorTransform::IDENTITY, opts, 0, true);
        out.push((l.depth, its));
    }
    items.clear();
    out
}

fn layer_draws_directly(tl: &Timeline, li: usize, opts: &EvalOptions) -> bool {
    let l = &tl.layers[li];
    if matches!(l.kind, LayerKind::Folder | LayerKind::Camera) {
        return false;
    }
    if l.kind == LayerKind::Guide && !opts.authoring {
        return false;
    }
    if opts.respect_visibility && !tl.effectively_visible(li) {
        return false;
    }
    true
}

fn mask_active(tl: &Timeline, mask_id: Id, opts: &EvalOptions) -> bool {
    tl.layer(mask_id).is_some_and(|m| m.kind == LayerKind::Mask && (!opts.authoring || m.locked))
}

/// Evaluate a timeline (bottom-to-top display items) with a parent transform and color.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_timeline(
    doc: &Document,
    tl: &Timeline,
    frame: u32,
    parent: Affine,
    color: ColorTransform,
    opts: &EvalOptions,
    depth: usize,
    top_level: bool,
) -> Vec<DisplayItem> {
    let mut out = Vec::new();
    if depth > MAX_DEPTH {
        return out;
    }
    for li in (0..tl.layers.len()).rev() {
        let l = &tl.layers[li];
        // Masked layers are drawn as part of their mask layer.
        if let Some(m) = l.mask_layer
            && mask_active(tl, m, opts)
        {
            continue;
        }
        if !layer_draws_directly(tl, li, opts) {
            continue;
        }
        if l.kind == LayerKind::Mask && mask_active(tl, l.id, opts) {
            let mask = layer_items(doc, tl, li, frame, parent, ColorTransform::IDENTITY, opts, depth, top_level);
            let mut content = Vec::new();
            for ci in (0..tl.layers.len()).rev() {
                if tl.layers[ci].mask_layer == Some(l.id) && layer_draws_directly(tl, ci, opts) {
                    content.extend(layer_items(doc, tl, ci, frame, parent, color, opts, depth, top_level));
                }
            }
            if !content.is_empty() {
                out.push(DisplayItem::Masked { mask, content });
            }
            continue;
        }
        out.extend(layer_items(doc, tl, li, frame, parent, color, opts, depth, top_level));
    }
    out
}

/// Accumulated transform of a layer's parent chain (layer parenting).
fn parent_chain_transform(doc: &Document, tl: &Timeline, li: usize, frame: u32) -> Affine {
    let mut m = Affine::IDENTITY;
    let mut cur = tl.layers[li].parent_layer;
    let mut guard = 0;
    while let Some(pid) = cur {
        let Some(pi) = tl.layer_index(pid) else { break };
        let (els, _) = layer_elements_at(doc, tl, pi, frame);
        if let Some(e) = els.first() {
            m = e.transform * m;
        }
        cur = tl.layers[pi].parent_layer;
        guard += 1;
        if guard > 16 {
            break;
        }
    }
    m
}

#[allow(clippy::too_many_arguments)]
fn layer_items(
    doc: &Document,
    tl: &Timeline,
    li: usize,
    frame: u32,
    parent: Affine,
    color: ColorTransform,
    opts: &EvalOptions,
    depth: usize,
    top_level: bool,
) -> Vec<DisplayItem> {
    let l = &tl.layers[li];
    let (elements, offset) = layer_elements_at(doc, tl, li, frame);
    let outline = (opts.authoring && l.outline).then_some(l.color);
    let base = if l.parent_layer.is_some() { parent * parent_chain_transform(doc, tl, li, frame) } else { parent };
    let mut out = Vec::new();
    for e in &elements {
        element_items(doc, e, offset, base, color, outline, opts, depth, top_level.then_some(e.id), &mut out);
    }
    out
}

/// Local frame of a symbol instance's timeline.
fn instance_frame(doc: &Document, e: &Element, offset: u32, opts: &EvalOptions) -> u32 {
    let Some(inst) = e.as_instance() else { return 0 };
    let Some(sym) = doc.library.symbol(inst.symbol) else { return 0 };
    let len = sym.timeline.length().max(1);
    if let Some(&f) = opts.instance_frames.get(&e.id) {
        return f.min(len - 1);
    }
    match sym.kind {
        SymbolKind::Graphic => {
            let first = inst.first_frame.min(len - 1);
            match inst.loop_mode {
                LoopMode::Loop => (first + offset) % len,
                LoopMode::PlayOnce => (first + offset).min(len - 1),
                LoopMode::SingleFrame => first,
                LoopMode::LoopReverse => {
                    let k = (offset % len) as i64;
                    (first as i64 - k).rem_euclid(len as i64) as u32
                }
                LoopMode::PlayOnceReverse => first.saturating_sub(offset),
            }
        }
        SymbolKind::MovieClip => {
            if opts.play_movieclips {
                offset % len
            } else {
                0
            }
        }
        SymbolKind::Button => 0,
    }
}

#[allow(clippy::too_many_arguments)]
fn element_items(
    doc: &Document,
    e: &Element,
    offset: u32,
    parent: Affine,
    color: ColorTransform,
    outline: Option<Color>,
    opts: &EvalOptions,
    depth: usize,
    source: Option<Id>,
    out: &mut Vec<DisplayItem>,
) {
    let m = parent * e.transform;
    match &e.kind {
        ElementKind::Shape(s) => out.push(DisplayItem::Shape { shape: s.clone(), transform: m, color, outline, source }),
        ElementKind::Text(t) => out.push(DisplayItem::Text { text: t.clone(), transform: m, color, source }),
        ElementKind::Bitmap(b) => out.push(DisplayItem::Bitmap { bitmap: b.clone(), transform: m, color, source }),
        ElementKind::Group { children } => {
            for c in children {
                element_items(doc, c, offset, m, color, outline, opts, depth, source, out);
            }
        }
        ElementKind::Instance(inst) => {
            if opts.honor_instance_visibility && !inst.visible {
                return;
            }
            let Some(sym) = doc.library.symbol(inst.symbol) else { return };
            let f = instance_frame(doc, e, offset, opts);
            let ct = color.concat(&inst.color_effect.to_transform());
            let mut children = evaluate_timeline(doc, &sym.timeline, f, m, ct, opts, depth + 1, false);
            if let Some(oc) = outline {
                children = children.into_iter().map(|i| outline_item(i, oc)).collect();
            }
            if !inst.filters.is_empty() || !matches!(inst.blend, BlendMode::Normal) || inst.cache_as_bitmap {
                out.push(DisplayItem::Group { items: children, filters: inst.filters.clone(), blend: inst.blend, scale: m.mean_scale(), source });
            } else if source.is_some() {
                // Keep top-level instance identity for picking by wrapping in a plain group.
                out.push(DisplayItem::Group { items: children, filters: Vec::new(), blend: BlendMode::Normal, scale: m.mean_scale(), source });
            } else {
                out.extend(children);
            }
        }
    }
}

fn outline_item(i: DisplayItem, c: Color) -> DisplayItem {
    match i {
        DisplayItem::Shape { shape, transform, color, source, .. } => DisplayItem::Shape { shape, transform, color, outline: Some(c), source },
        DisplayItem::Group { items, filters, blend, scale, source } => {
            DisplayItem::Group { items: items.into_iter().map(|x| outline_item(x, c)).collect(), filters, blend, scale, source }
        }
        DisplayItem::Masked { mask, content } => {
            DisplayItem::Masked { mask, content: content.into_iter().map(|x| outline_item(x, c)).collect() }
        }
        other => other,
    }
}

/// Pre-multiply an item's transform.
pub fn transform_item(i: DisplayItem, m: &Affine) -> DisplayItem {
    match i {
        DisplayItem::Shape { shape, transform, color, outline, source } => {
            DisplayItem::Shape { shape, transform: *m * transform, color, outline, source }
        }
        DisplayItem::Text { text, transform, color, source } => DisplayItem::Text { text, transform: *m * transform, color, source },
        DisplayItem::Bitmap { bitmap, transform, color, source } => DisplayItem::Bitmap { bitmap, transform: *m * transform, color, source },
        DisplayItem::Group { items, filters, blend, scale, source } => DisplayItem::Group {
            items: items.into_iter().map(|x| transform_item(x, m)).collect(),
            filters,
            blend,
            scale: scale * m.mean_scale(),
            source,
        },
        DisplayItem::Masked { mask, content } => DisplayItem::Masked {
            mask: mask.into_iter().map(|x| transform_item(x, m)).collect(),
            content: content.into_iter().map(|x| transform_item(x, m)).collect(),
        },
    }
}

/// Apply an extra color transform on top of an item (onion skins, camera tint).
pub fn recolor_item(i: DisplayItem, ct: &ColorTransform) -> DisplayItem {
    match i {
        DisplayItem::Shape { shape, transform, color, outline, source } => {
            DisplayItem::Shape { shape, transform, color: ct.concat(&color), outline, source }
        }
        DisplayItem::Text { text, transform, color, source } => DisplayItem::Text { text, transform, color: ct.concat(&color), source },
        DisplayItem::Bitmap { bitmap, transform, color, source } => DisplayItem::Bitmap { bitmap, transform, color: ct.concat(&color), source },
        DisplayItem::Group { items, filters, blend, scale, source } => {
            DisplayItem::Group { items: items.into_iter().map(|x| recolor_item(x, ct)).collect(), filters, blend, scale, source }
        }
        DisplayItem::Masked { mask, content } => {
            DisplayItem::Masked { mask, content: content.into_iter().map(|x| recolor_item(x, ct)).collect() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use animara_document::{Fill, Layer, ShapePath, Symbol, SymbolInstance};
    use animara_geometry::{Path, Rect};

    fn square() -> Element {
        Element::shape(vec![ShapePath::filled(Path::rect(Rect::new(0.0, 0.0, 10.0, 10.0)), Fill::solid(Color::BLACK))])
    }

    #[test]
    fn graphic_symbol_sync_and_loop() {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        let mut sym = Symbol::new("anim", SymbolKind::Graphic);
        sym.timeline.layers[0].frames[0].elements.push(square());
        sym.timeline.layers[0].insert_blank_keyframe(1);
        sym.timeline.layers[0].insert_keyframe(2, Some(vec![square().with_transform(Affine::translate(50.0, 0.0))]));
        let sid = doc.library.add_symbol(sym);
        let layer = &mut doc.scenes[0].timeline.layers[0];
        layer.frames[0].elements.push(Element::new(ElementKind::Instance(SymbolInstance::new(sid))));
        layer.extend_to(9);
        let opts = EvalOptions::export();
        let count = |f: u32| {
            let dl = evaluate_document_frame(&doc, 0, f, &opts);
            fn n(items: &[DisplayItem]) -> usize {
                items
                    .iter()
                    .map(|i| match i {
                        DisplayItem::Group { items, .. } => n(items),
                        _ => 1,
                    })
                    .sum()
            }
            n(&dl.items)
        };
        assert_eq!(count(0), 1);
        assert_eq!(count(1), 0); // blank keyframe inside the symbol
        assert_eq!(count(3), 1); // loops: frame 3 -> symbol frame 0
        assert_eq!(count(4), 0);
    }

    #[test]
    fn masks_and_guides() {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        let tl = &mut doc.scenes[0].timeline;
        tl.layers[0].frames[0].elements.push(square());
        let mask_id = tl.add_layer("Mask", 0);
        tl.layers[0].kind = animara_document::LayerKind::Mask;
        tl.layers[0].frames[0].elements.push(square());
        tl.layers[1].mask_layer = Some(mask_id);
        let mut guide = Layer::new("Guide").with_kind(animara_document::LayerKind::Guide);
        guide.frames[0].elements.push(square());
        tl.layers.push(guide);
        let dl = evaluate_document_frame(&doc, 0, 0, &EvalOptions::export());
        assert_eq!(dl.items.len(), 1, "guide hidden, masked content grouped");
        assert!(matches!(dl.items[0], DisplayItem::Masked { .. }));
    }

    #[test]
    fn camera_view_and_global_frames() {
        let mut doc = Document::new(200.0, 100.0, 24.0);
        let tl = &mut doc.scenes[0].timeline;
        tl.layers[0].extend_to(9);
        let mut cam = Layer::new("Camera").with_kind(animara_document::LayerKind::Camera);
        cam.frames = vec![animara_document::Keyframe::new(0, 10)];
        cam.frames[0].camera = Some(animara_document::CameraState { x: 100.0, y: 50.0, zoom: 2.0, rotation: 0.0, tint: None });
        tl.layers.insert(0, cam);
        let dl = evaluate_document_frame(&doc, 0, 0, &EvalOptions::export());
        let p = dl.view.apply(Vec2::new(110.0, 50.0));
        assert!((p.x - 120.0).abs() < 1e-9 && (p.y - 50.0).abs() < 1e-9);
        doc.scenes.push(animara_document::Scene::new("Scene 2"));
        assert_eq!(scene_at_global_frame(&doc, 10), (1, 0));
        assert_eq!(scene_at_global_frame(&doc, 3), (0, 3));
    }
}
