//! Bounds and hit-testing of document elements (used by the stage, align and exporters).

use animara_animation::{DisplayItem, layer_elements_at};
use animara_document::{Document, Element, ElementKind, LayerKind, SymbolKind};
use animara_geometry::{Affine, Rect, Vec2};

fn symbol_frame(doc: &Document, e: &Element) -> u32 {
    match e.as_instance() {
        Some(i) => match doc.library.symbol(i.symbol) {
            Some(s) if s.kind == SymbolKind::Graphic => i.first_frame.min(s.timeline.length().saturating_sub(1)),
            _ => 0,
        },
        None => 0,
    }
}

/// World-space bounds of an element (`parent` maps the element's parent space to world).
pub fn element_bounds(doc: &Document, e: &Element, parent: &Affine) -> Rect {
    element_bounds_depth(doc, e, parent, 0)
}

fn element_bounds_depth(doc: &Document, e: &Element, parent: &Affine, depth: usize) -> Rect {
    let m = *parent * e.transform;
    match &e.kind {
        ElementKind::Shape(s) => {
            let mut r = Rect::EMPTY;
            for p in &s.paths {
                let b = p.path.transform(&m).bounds();
                let pad = p.stroke.as_ref().map(|s| s.width * m.mean_scale() / 2.0).unwrap_or(0.0);
                r = r.union(&b.inflate(pad));
            }
            r
        }
        ElementKind::Text(t) => {
            let l = animara_text::fonts().layout(t);
            m.transform_rect(&l.bounds)
        }
        ElementKind::Bitmap(b) => match doc.library.bitmap(b.asset) {
            Some(a) => m.transform_rect(&Rect::new(0.0, 0.0, a.width as f64, a.height as f64)),
            None => Rect::EMPTY,
        },
        ElementKind::Group { children } => children.iter().fold(Rect::EMPTY, |r, c| r.union(&element_bounds_depth(doc, c, &m, depth))),
        ElementKind::Instance(i) => {
            if depth > 16 {
                return Rect::EMPTY;
            }
            let Some(sym) = doc.library.symbol(i.symbol) else { return Rect::EMPTY };
            let f = symbol_frame(doc, e);
            let tl = &sym.timeline;
            let mut r = Rect::EMPTY;
            for li in 0..tl.layers.len() {
                if matches!(tl.layers[li].kind, LayerKind::Guide | LayerKind::Folder | LayerKind::Camera) {
                    continue;
                }
                let (els, _) = layer_elements_at(doc, tl, li, f);
                for c in &els {
                    r = r.union(&element_bounds_depth(doc, c, &m, depth + 1));
                }
            }
            if r.is_empty() {
                // Empty symbols still get a small pickable box around the registration point.
                let o = m.apply(Vec2::ZERO);
                r = Rect::new(o.x - 4.0, o.y - 4.0, o.x + 4.0, o.y + 4.0);
            }
            r
        }
    }
}

/// Precise hit test of a world point against an element.
pub fn element_hit(doc: &Document, e: &Element, parent: &Affine, p: Vec2, tol: f64) -> bool {
    element_hit_depth(doc, e, parent, p, tol, 0)
}

fn element_hit_depth(doc: &Document, e: &Element, parent: &Affine, p: Vec2, tol: f64, depth: usize) -> bool {
    let m = *parent * e.transform;
    let Some(inv) = m.invert() else { return false };
    let local = inv.apply(p);
    let ltol = tol / m.mean_scale().max(1e-6);
    match &e.kind {
        ElementKind::Shape(s) => s.paths.iter().any(|sp| sp.hit(local, ltol)),
        ElementKind::Text(t) => animara_text::fonts().layout(t).bounds.inflate(ltol).contains(local),
        ElementKind::Bitmap(b) => {
            doc.library.bitmap(b.asset).is_some_and(|a| Rect::new(0.0, 0.0, a.width as f64, a.height as f64).contains(local))
        }
        ElementKind::Group { children } => children.iter().any(|c| element_hit_depth(doc, c, &m, p, tol, depth)),
        ElementKind::Instance(i) => {
            if depth > 16 {
                return false;
            }
            let Some(sym) = doc.library.symbol(i.symbol) else { return false };
            let f = symbol_frame(doc, e);
            let tl = &sym.timeline;
            // Buttons are hit by their Hit frame (4th) when present.
            let f = if sym.kind == SymbolKind::Button && tl.length() >= 4 { 3 } else { f };
            (0..tl.layers.len()).any(|li| {
                !matches!(tl.layers[li].kind, LayerKind::Guide | LayerKind::Folder | LayerKind::Camera)
                    && layer_elements_at(doc, tl, li, f).0.iter().any(|c| element_hit_depth(doc, c, &m, p, tol, depth + 1))
            }) || element_bounds_depth(doc, e, parent, depth).width() < 9.0 && element_bounds_depth(doc, e, parent, depth).contains(p)
        }
    }
}

/// Bounds of rendered display items (document space, before the camera view).
pub fn display_bounds(doc: &Document, items: &[DisplayItem]) -> Rect {
    let mut r = Rect::EMPTY;
    for it in items {
        let b = match it {
            DisplayItem::Shape { shape, transform, .. } => shape.transformed(transform).bounds(),
            DisplayItem::Text { text, transform, .. } => transform.transform_rect(&animara_text::fonts().layout(text).bounds),
            DisplayItem::Bitmap { bitmap, transform, .. } => match doc.library.bitmap(bitmap.asset) {
                Some(a) => transform.transform_rect(&Rect::new(0.0, 0.0, a.width as f64, a.height as f64)),
                None => Rect::EMPTY,
            },
            DisplayItem::Group { items, filters, scale, .. } => {
                let pad: f64 = filters.iter().map(|f| f.padding()).sum::<f64>() * scale;
                display_bounds(doc, items).inflate(pad)
            }
            DisplayItem::Masked { mask, content } => {
                let m = display_bounds(doc, mask);
                let c = display_bounds(doc, content);
                if m.intersects(&c) {
                    Rect::new(m.min.x.max(c.min.x), m.min.y.max(c.min.y), m.max.x.min(c.max.x), m.max.y.min(c.max.y))
                } else {
                    Rect::EMPTY
                }
            }
        };
        r = r.union(&b);
    }
    r
}
