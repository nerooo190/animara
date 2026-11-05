//! CPU renderer for Animara display lists built on tiny-skia.
//!
//! The same renderer drives the stage, thumbnails and every raster exporter,
//! so on-screen and exported pixels match.

pub mod bounds;
mod filters;

use animara_animation::{DisplayItem, DisplayList, EvalOptions, evaluate_document_frame};
use animara_document::{
    BlendMode, Color, ColorTransform, Document, Fill, Id, LineCap, LineJoin, Shape, SpreadMode, Stroke, StrokeStyle,
};
use animara_geometry::{Affine, FillRule, Path, PathCmd};
use std::collections::HashMap;
use std::sync::Arc;
pub use tiny_skia;
use tiny_skia::{Pixmap, PixmapPaint, Transform};

/// Caches decoded bitmaps between frames.
#[derive(Default)]
pub struct RenderCache {
    bitmaps: HashMap<(Id, usize), Arc<Pixmap>>,
}

impl RenderCache {
    pub fn new() -> RenderCache {
        RenderCache::default()
    }

    /// Decode (or fetch) a bitmap asset as a premultiplied pixmap.
    pub fn bitmap(&mut self, doc: &Document, id: Id) -> Option<Arc<Pixmap>> {
        let asset = doc.library.bitmap(id)?;
        let key = (id, Arc::as_ptr(&asset.data) as usize);
        if let Some(p) = self.bitmaps.get(&key) {
            return Some(p.clone());
        }
        let img = image::load_from_memory(&asset.data).ok()?.to_rgba8();
        let mut pm = Pixmap::new(img.width().max(1), img.height().max(1))?;
        for (dst, src) in pm.pixels_mut().iter_mut().zip(img.pixels()) {
            let [r, g, b, a] = src.0;
            *dst = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
        }
        let pm = Arc::new(pm);
        self.bitmaps.insert(key, pm.clone());
        Some(pm)
    }
}

pub fn to_ts(m: &Affine) -> Transform {
    Transform::from_row(m.a as f32, m.b as f32, m.c as f32, m.d as f32, m.tx as f32, m.ty as f32)
}

pub fn ts_color(c: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba(c.r.clamp(0.0, 1.0), c.g.clamp(0.0, 1.0), c.b.clamp(0.0, 1.0), c.a.clamp(0.0, 1.0))
        .unwrap_or(tiny_skia::Color::BLACK)
}

/// Convert a geometry path to a tiny-skia path.
pub fn to_ts_path(p: &Path) -> Option<tiny_skia::Path> {
    let mut b = tiny_skia::PathBuilder::new();
    let mut open = false;
    for c in &p.cmds {
        match *c {
            PathCmd::MoveTo(v) => {
                b.move_to(v.x as f32, v.y as f32);
                open = true;
            }
            PathCmd::LineTo(v) => {
                if !open {
                    b.move_to(v.x as f32, v.y as f32);
                    open = true;
                } else {
                    b.line_to(v.x as f32, v.y as f32);
                }
            }
            PathCmd::QuadTo(c1, v) => b.quad_to(c1.x as f32, c1.y as f32, v.x as f32, v.y as f32),
            PathCmd::CubicTo(c1, c2, v) => b.cubic_to(c1.x as f32, c1.y as f32, c2.x as f32, c2.y as f32, v.x as f32, v.y as f32),
            PathCmd::Close => b.close(),
        }
    }
    b.finish()
}

fn spread(s: SpreadMode) -> tiny_skia::SpreadMode {
    match s {
        SpreadMode::Pad => tiny_skia::SpreadMode::Pad,
        SpreadMode::Reflect => tiny_skia::SpreadMode::Reflect,
        SpreadMode::Repeat => tiny_skia::SpreadMode::Repeat,
    }
}

fn make_paint<'a>(
    fill: &Fill,
    ct: &ColorTransform,
    m: &Affine,
    doc: &Document,
    cache: &mut RenderCache,
    hold: &'a mut Option<Arc<Pixmap>>,
) -> Option<tiny_skia::Paint<'a>> {
    let mut paint = tiny_skia::Paint { anti_alias: true, ..Default::default() };
    match fill {
        Fill::Solid { color } => paint.set_color(ts_color(ct.apply(*color))),
        Fill::Linear { stops, matrix, spread: sp } | Fill::Radial { stops, matrix, spread: sp, .. } => {
            let ts_stops: Vec<tiny_skia::GradientStop> =
                stops.iter().map(|s| tiny_skia::GradientStop::new(s.offset.clamp(0.0, 1.0), ts_color(ct.apply(s.color)))).collect();
            if ts_stops.is_empty() {
                return None;
            }
            if ts_stops.len() == 1 {
                paint.set_color(ts_color(ct.apply(stops[0].color)));
                return Some(paint);
            }
            let t = to_ts(&(*m * *matrix));
            let shader = match fill {
                Fill::Linear { .. } => tiny_skia::LinearGradient::new(
                    tiny_skia::Point::from_xy(-1.0, 0.0),
                    tiny_skia::Point::from_xy(1.0, 0.0),
                    ts_stops,
                    spread(*sp),
                    t,
                ),
                Fill::Radial { focal, .. } => tiny_skia::RadialGradient::new(
                    tiny_skia::Point::from_xy(*focal as f32, 0.0),
                    0.0,
                    tiny_skia::Point::from_xy(0.0, 0.0),
                    1.0,
                    ts_stops,
                    spread(*sp),
                    t,
                ),
                _ => unreachable!(),
            };
            match shader {
                Some(s) => paint.shader = s,
                None => paint.set_color(ts_color(ct.apply(stops[0].color))),
            }
        }
        Fill::Bitmap { asset, matrix, smooth, repeat } => {
            let pm = cache.bitmap(doc, *asset)?;
            let pm = if ct.is_identity() { pm } else { Arc::new(recolor_pixmap(&pm, ct)) };
            *hold = Some(pm);
            let pm_ref = hold.as_ref().unwrap().as_ref().as_ref();
            paint.shader = tiny_skia::Pattern::new(
                pm_ref,
                if *repeat { tiny_skia::SpreadMode::Repeat } else { tiny_skia::SpreadMode::Pad },
                if *smooth { tiny_skia::FilterQuality::Bilinear } else { tiny_skia::FilterQuality::Nearest },
                1.0,
                to_ts(&(*m * *matrix)),
            );
        }
    }
    Some(paint)
}

fn recolor_pixmap(src: &Pixmap, ct: &ColorTransform) -> Pixmap {
    let mut out = src.clone();
    for px in out.pixels_mut() {
        let c = px.demultiply();
        let col = ct.apply(Color::from_rgba8(c.red(), c.green(), c.blue(), c.alpha()));
        let [r, g, b, a] = col.to_rgba8();
        *px = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    out
}

fn ts_stroke(s: &Stroke, m: &Affine) -> tiny_skia::Stroke {
    let scale = m.mean_scale().max(1e-6);
    let width = match s.style {
        StrokeStyle::Hairline => 1.0 / scale,
        _ if !s.scale_with_object => s.width / scale,
        _ => s.width,
    };
    let dash = match s.style {
        StrokeStyle::Dashed { dash, gap } => tiny_skia::StrokeDash::new(vec![dash.max(0.1) as f32, gap.max(0.1) as f32], 0.0),
        StrokeStyle::Dotted { spacing } => tiny_skia::StrokeDash::new(vec![0.01, (spacing.max(0.5) + width) as f32], 0.0),
        _ => None,
    };
    tiny_skia::Stroke {
        width: width.max(0.0) as f32,
        miter_limit: s.miter_limit as f32,
        line_cap: match (s.cap, &s.style) {
            (_, StrokeStyle::Dotted { .. }) => tiny_skia::LineCap::Round,
            (LineCap::Butt, _) => tiny_skia::LineCap::Butt,
            (LineCap::Round, _) => tiny_skia::LineCap::Round,
            (LineCap::Square, _) => tiny_skia::LineCap::Square,
        },
        line_join: match s.join {
            LineJoin::Miter => tiny_skia::LineJoin::Miter,
            LineJoin::Round => tiny_skia::LineJoin::Round,
            LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        },
        dash,
    }
}

/// Draw a shape with a world transform onto a pixmap.
pub fn draw_shape(
    pm: &mut Pixmap,
    shape: &Shape,
    m: &Affine,
    ct: &ColorTransform,
    outline: Option<Color>,
    doc: &Document,
    cache: &mut RenderCache,
    mask: Option<&tiny_skia::Mask>,
) {
    let t = to_ts(m);
    for sp in &shape.paths {
        let Some(path) = to_ts_path(&sp.path) else { continue };
        if let Some(oc) = outline {
            let mut paint = tiny_skia::Paint { anti_alias: true, ..Default::default() };
            paint.set_color(ts_color(oc));
            let stroke = tiny_skia::Stroke { width: 1.0 / m.mean_scale().max(1e-6) as f32, ..Default::default() };
            pm.stroke_path(&path, &paint, &stroke, t, mask);
            continue;
        }
        if let Some(f) = &sp.fill {
            let mut hold = None;
            if let Some(paint) = make_paint(f, ct, &Affine::IDENTITY, doc, cache, &mut hold) {
                let rule = match sp.fill_rule {
                    FillRule::NonZero => tiny_skia::FillRule::Winding,
                    FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
                };
                pm.fill_path(&path, &paint, rule, t, mask);
            }
        }
        if let Some(s) = &sp.stroke {
            let mut hold = None;
            if let Some(paint) = make_paint(&s.fill, ct, &Affine::IDENTITY, doc, cache, &mut hold) {
                pm.stroke_path(&path, &paint, &ts_stroke(s, m), t, mask);
            }
        }
    }
}

fn ts_blend(b: BlendMode) -> tiny_skia::BlendMode {
    match b {
        BlendMode::Normal | BlendMode::Layer => tiny_skia::BlendMode::SourceOver,
        BlendMode::Multiply => tiny_skia::BlendMode::Multiply,
        BlendMode::Screen => tiny_skia::BlendMode::Screen,
        BlendMode::Overlay => tiny_skia::BlendMode::Overlay,
        BlendMode::HardLight => tiny_skia::BlendMode::HardLight,
        BlendMode::Lighten => tiny_skia::BlendMode::Lighten,
        BlendMode::Darken => tiny_skia::BlendMode::Darken,
        BlendMode::Difference => tiny_skia::BlendMode::Difference,
        BlendMode::Add => tiny_skia::BlendMode::Plus,
        BlendMode::Subtract => tiny_skia::BlendMode::Exclusion,
        BlendMode::Invert => tiny_skia::BlendMode::Difference,
        BlendMode::Alpha => tiny_skia::BlendMode::DestinationIn,
        BlendMode::Erase => tiny_skia::BlendMode::DestinationOut,
    }
}

/// Render context passed through recursive drawing.
pub struct Renderer<'a> {
    pub doc: &'a Document,
    pub cache: &'a mut RenderCache,
}

impl Renderer<'_> {
    /// Draw display items (already in document space) using `view` (document → pixels).
    pub fn draw_items(&mut self, pm: &mut Pixmap, items: &[DisplayItem], view: &Affine) {
        for it in items {
            self.draw_item(pm, it, view);
        }
    }

    fn draw_item(&mut self, pm: &mut Pixmap, it: &DisplayItem, view: &Affine) {
        match it {
            DisplayItem::Shape { shape, transform, color, outline, .. } => {
                draw_shape(pm, shape, &(*view * *transform), color, *outline, self.doc, self.cache, None);
            }
            DisplayItem::Text { text, transform, color, .. } => {
                let layout = animara_text::fonts().layout(text);
                let m = *view * *transform;
                let t = to_ts(&m);
                for (path, c) in layout.to_paths() {
                    if let Some(p) = to_ts_path(&path) {
                        let mut paint = tiny_skia::Paint { anti_alias: true, ..Default::default() };
                        paint.set_color(ts_color(color.apply(c)));
                        pm.fill_path(&p, &paint, tiny_skia::FillRule::Winding, t, None);
                    }
                }
            }
            DisplayItem::Bitmap { bitmap, transform, color, .. } => {
                let Some(src) = self.cache.bitmap(self.doc, bitmap.asset) else { return };
                let src = if color.is_identity() { src } else { Arc::new(recolor_pixmap(&src, color)) };
                let smooth = self.doc.library.bitmap(bitmap.asset).is_none_or(|b| b.smoothing);
                let paint = PixmapPaint {
                    quality: if smooth { tiny_skia::FilterQuality::Bicubic } else { tiny_skia::FilterQuality::Nearest },
                    ..Default::default()
                };
                pm.draw_pixmap(0, 0, src.as_ref().as_ref(), &paint, to_ts(&(*view * *transform)), None);
            }
            DisplayItem::Group { items, filters, blend, scale, .. } => {
                if filters.is_empty() && matches!(blend, BlendMode::Normal) {
                    self.draw_items(pm, items, view);
                    return;
                }
                let Some(mut layer) = Pixmap::new(pm.width(), pm.height()) else { return };
                self.draw_items(&mut layer, items, view);
                let px_scale = scale * view.mean_scale();
                for f in filters {
                    filters::apply(&mut layer, f, px_scale);
                }
                match blend {
                    BlendMode::Invert => filters::composite_invert(pm, &layer),
                    BlendMode::Subtract => filters::composite_subtract(pm, &layer),
                    _ => {
                        let paint = PixmapPaint { blend_mode: ts_blend(*blend), ..Default::default() };
                        pm.draw_pixmap(0, 0, layer.as_ref(), &paint, Transform::identity(), None);
                    }
                }
            }
            DisplayItem::Masked { mask, content } => {
                let (Some(mut content_pm), Some(mut mask_pm)) = (Pixmap::new(pm.width(), pm.height()), Pixmap::new(pm.width(), pm.height()))
                else {
                    return;
                };
                self.draw_items(&mut content_pm, content, view);
                // Masks use shape coverage, not color: draw mask items opaque.
                let opaque: Vec<DisplayItem> = mask.iter().cloned().map(force_opaque).collect();
                self.draw_items(&mut mask_pm, &opaque, view);
                let m = tiny_skia::Mask::from_pixmap(mask_pm.as_ref(), tiny_skia::MaskType::Alpha);
                pm.draw_pixmap(0, 0, content_pm.as_ref(), &PixmapPaint::default(), Transform::identity(), Some(&m));
            }
        }
    }
}

fn force_opaque(i: DisplayItem) -> DisplayItem {
    let solid = ColorTransform { mul: [1.0, 1.0, 1.0, 0.0], add: [0.0, 0.0, 0.0, 1.0] };
    match i {
        DisplayItem::Shape { shape, transform, outline, source, .. } => {
            // Fills only: strokes do not mask in classic mask layers.
            let mut s = shape;
            for p in &mut s.paths {
                if p.fill.is_some() {
                    p.stroke = None;
                }
            }
            DisplayItem::Shape { shape: s, transform, color: solid, outline, source }
        }
        DisplayItem::Text { text, transform, source, .. } => DisplayItem::Text { text, transform, color: solid, source },
        DisplayItem::Bitmap { bitmap, transform, source, .. } => DisplayItem::Bitmap { bitmap, transform, color: solid, source },
        DisplayItem::Group { items, scale, source, .. } => DisplayItem::Group {
            items: items.into_iter().map(force_opaque).collect(),
            filters: Vec::new(),
            blend: BlendMode::Normal,
            scale,
            source,
        },
        DisplayItem::Masked { mask, content } => DisplayItem::Masked { mask, content: content.into_iter().map(force_opaque).collect() },
    }
}

/// Render a display list at `scale` (output pixels per document pixel).
/// When `transparent` is false the document background is painted.
pub fn render_display_list(dl: &DisplayList, doc: &Document, cache: &mut RenderCache, scale: f64, transparent: bool) -> Option<Pixmap> {
    let w = (dl.width * scale).round().max(1.0) as u32;
    let h = (dl.height * scale).round().max(1.0) as u32;
    let mut pm = Pixmap::new(w, h)?;
    if !transparent {
        pm.fill(ts_color(dl.background));
    }
    let view = Affine::scale(scale, scale) * dl.view;
    Renderer { doc, cache }.draw_items(&mut pm, &dl.items, &view);
    Some(pm)
}

/// Evaluate and render one scene frame for export.
pub fn render_frame(doc: &Document, scene: usize, frame: u32, scale: f64, transparent: bool, cache: &mut RenderCache) -> Option<Pixmap> {
    let dl = evaluate_document_frame(doc, scene, frame, &EvalOptions::export());
    render_display_list(&dl, doc, cache, scale, transparent)
}

/// Straight (un-premultiplied) RGBA8 bytes of a pixmap.
pub fn pixmap_to_rgba(pm: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.data().len());
    for p in pm.pixels() {
        let c = p.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

/// Encode a pixmap as PNG bytes.
pub fn pixmap_to_png(pm: &Pixmap) -> Vec<u8> {
    pm.encode_png().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use animara_document::{Element, ElementKind, Filter, GradientStop, ShapePath, Symbol, SymbolInstance, SymbolKind};
    use animara_geometry::Rect;

    fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let c = pm.pixel(x, y).unwrap().demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    fn doc_with(el: Element) -> Document {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        doc.scenes[0].timeline.layers[0].frames[0].elements.push(el);
        doc
    }

    #[test]
    fn solid_fill_and_background() {
        let doc = doc_with(Element::shape(vec![ShapePath::filled(
            Path::rect(Rect::new(10.0, 10.0, 50.0, 50.0)),
            Fill::solid(Color::rgb(1.0, 0.0, 0.0)),
        )]));
        let pm = render_frame(&doc, 0, 0, 1.0, false, &mut RenderCache::new()).unwrap();
        assert_eq!(px(&pm, 30, 30), [255, 0, 0, 255]);
        assert_eq!(px(&pm, 80, 80), [255, 255, 255, 255]);
        let pm2 = render_frame(&doc, 0, 0, 2.0, true, &mut RenderCache::new()).unwrap();
        assert_eq!(pm2.width(), 200);
        assert_eq!(px(&pm2, 160, 160)[3], 0);
    }

    #[test]
    fn linear_gradient_varies() {
        let fill = Fill::Linear {
            stops: vec![GradientStop { offset: 0.0, color: Color::BLACK }, GradientStop { offset: 1.0, color: Color::WHITE }],
            matrix: Affine::translate(50.0, 50.0) * Affine::scale(50.0, 50.0),
            spread: SpreadMode::Pad,
        };
        let doc = doc_with(Element::shape(vec![ShapePath::filled(Path::rect(Rect::new(0.0, 0.0, 100.0, 100.0)), fill)]));
        let pm = render_frame(&doc, 0, 0, 1.0, false, &mut RenderCache::new()).unwrap();
        let l = px(&pm, 5, 50)[0];
        let r = px(&pm, 95, 50)[0];
        assert!(l < 30 && r > 225, "{l} {r}");
    }

    #[test]
    fn instance_tint_and_blur() {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        let mut sym = Symbol::new("sq", SymbolKind::MovieClip);
        sym.timeline.layers[0].frames[0].elements.push(Element::shape(vec![ShapePath::filled(
            Path::rect(Rect::new(0.0, 0.0, 40.0, 40.0)),
            Fill::solid(Color::WHITE),
        )]));
        let sid = doc.library.add_symbol(sym);
        let mut inst = SymbolInstance::new(sid);
        inst.color_effect = animara_document::ColorEffect::Tint { color: Color::rgb(0.0, 0.0, 1.0), amount: 1.0 };
        inst.filters.push(Filter::Blur { blur_x: 10.0, blur_y: 10.0, quality: 2 });
        doc.background = Color::BLACK;
        doc.scenes[0].timeline.layers[0].frames[0].elements.push(Element::new(ElementKind::Instance(inst)).with_transform(Affine::translate(30.0, 30.0)));
        let pm = render_frame(&doc, 0, 0, 1.0, false, &mut RenderCache::new()).unwrap();
        let center = px(&pm, 50, 50);
        assert!(center[2] > 200 && center[0] < 30, "{center:?}");
        // Blur spreads color past the original edge.
        let edge = px(&pm, 27, 50);
        assert!(edge[2] > 10, "{edge:?}");
    }

    #[test]
    fn mask_clips_content() {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        let tl = &mut doc.scenes[0].timeline;
        tl.layers[0].frames[0].elements.push(Element::shape(vec![ShapePath::filled(
            Path::rect(Rect::new(0.0, 0.0, 100.0, 100.0)),
            Fill::solid(Color::rgb(0.0, 1.0, 0.0)),
        )]));
        let mid = tl.add_layer("mask", 0);
        tl.layers[0].kind = animara_document::LayerKind::Mask;
        tl.layers[0].frames[0].elements.push(Element::shape(vec![ShapePath::filled(
            Path::rect(Rect::new(0.0, 0.0, 50.0, 100.0)),
            Fill::solid(Color::rgba(1.0, 0.0, 0.0, 0.2)),
        )]));
        tl.layers[1].mask_layer = Some(mid);
        let pm = render_frame(&doc, 0, 0, 1.0, false, &mut RenderCache::new()).unwrap();
        assert_eq!(px(&pm, 25, 50), [0, 255, 0, 255]);
        assert_eq!(px(&pm, 75, 50), [255, 255, 255, 255]);
    }
}
