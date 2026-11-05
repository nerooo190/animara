//! Library thumbnails, built-in components, default animated assets and
//! library merging (custom assets / external libraries).

use animara_animation::{EvalOptions, evaluate_timeline};
use animara_document::{
    Color, ColorTransform, Document, Element, ElementKind, Fill, Id, Keyframe, LayerKind, ShapePath, Stroke, Symbol, SymbolKind,
    TextElement, TextStyle, Tween, TweenKind, fresh_id,
};
use animara_geometry::{Affine, Path, Rect, Vec2};
use animara_renderer::{RenderCache, Renderer, tiny_skia};
use std::collections::HashMap;

/// Render a library item (symbol frame 1, bitmap, or sound waveform) into a square-ish pixmap.
pub fn render_library_item(doc: &Document, id: Id, size: u32, cache: &mut RenderCache) -> Option<tiny_skia::Pixmap> {
    if let Some(sym) = doc.library.symbol(id) {
        let opts = EvalOptions { authoring: false, respect_visibility: true, ..Default::default() };
        let items = evaluate_timeline(doc, &sym.timeline, 0, Affine::IDENTITY, ColorTransform::IDENTITY, &opts, 0, false);
        let b = animara_renderer::bounds::display_bounds(doc, &items);
        let b = if b.is_empty() || b.width() < 1e-6 { Rect::new(-10.0, -10.0, 10.0, 10.0) } else { b.inflate(2.0) };
        let s = (size as f64 / b.width()).min(size as f64 / b.height());
        let w = (b.width() * s).ceil().max(1.0) as u32;
        let h = (b.height() * s).ceil().max(1.0) as u32;
        let mut pm = tiny_skia::Pixmap::new(w, h)?;
        let view = Affine::scale(s, s) * Affine::translate(-b.min.x, -b.min.y);
        Renderer { doc, cache }.draw_items(&mut pm, &items, &view);
        return Some(pm);
    }
    if doc.library.bitmap(id).is_some() {
        let src = cache.bitmap(doc, id)?;
        let s = (size as f32 / src.width() as f32).min(size as f32 / src.height() as f32).min(1.0);
        let w = ((src.width() as f32 * s).ceil() as u32).max(1);
        let h = ((src.height() as f32 * s).ceil() as u32).max(1);
        let mut pm = tiny_skia::Pixmap::new(w, h)?;
        let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
        pm.draw_pixmap(0, 0, src.as_ref().as_ref(), &paint, tiny_skia::Transform::from_scale(s, s), None);
        return Some(pm);
    }
    if let Some(snd) = doc.library.sound(id) {
        let (w, h) = (size, size / 2);
        let mut pm = tiny_skia::Pixmap::new(w, h)?;
        if let Ok(pcm) = animara_media::decode_sound(snd) {
            let peaks = animara_media::peaks(&pcm, w as usize);
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(0x2b, 0x9c, 0xff, 255));
            for (x, p) in peaks.iter().enumerate() {
                let hh = (p * h as f32 / 2.0).max(0.5);
                if let Some(r) = tiny_skia::Rect::from_xywh(x as f32, h as f32 / 2.0 - hh, 1.0, hh * 2.0) {
                    pm.fill_rect(r, &paint, tiny_skia::Transform::identity(), None);
                }
            }
        }
        return Some(pm);
    }
    None
}

pub const COMPONENTS: &[(&str, &str)] = &[("Button", "button"), ("Checkbox", "checkbox"), ("Label", "label"), ("Progress Bar", "progress")];

fn rect_shape(r: Rect, radius: f64, fill: Color, stroke: Option<Color>) -> Element {
    let mut sp = ShapePath::filled(Path::rounded_rect(r, radius), Fill::solid(fill));
    sp.stroke = stroke.map(|c| Stroke::solid(c, 1.0));
    Element::shape(vec![sp])
}

fn label(text: &str, x: f64, y: f64, size: f64, color: Color) -> Element {
    let style = TextStyle { size, color, ..TextStyle::default() };
    Element::new(ElementKind::Text(TextElement::new(text, style))).with_transform(Affine::translate(x, y))
}

/// Build a component symbol; returns its id.
pub fn build_component(doc: &mut Document, kind: &str) -> Id {
    let blue = Color::from_rgba8(0x26, 0x80, 0xEB, 255);
    let name = doc.library.unique_name(match kind {
        "button" => "Button",
        "checkbox" => "Checkbox",
        "label" => "Label",
        _ => "Progress Bar",
    });
    let mut sym;
    match kind {
        "button" => {
            // Up / Over / Down / Hit frames.
            sym = Symbol::new(&name, SymbolKind::Button);
            let r = Rect::new(-60.0, -18.0, 60.0, 18.0);
            let layer = &mut sym.timeline.layers[0];
            layer.frames = (0..4).map(|i| Keyframe::new(i, 1)).collect();
            let fills = [blue, Color::from_rgba8(0x4a, 0x9a, 0xf5, 255), Color::from_rgba8(0x1a, 0x5f, 0xb0, 255), blue];
            for (i, f) in fills.iter().enumerate() {
                layer.frames[i].elements.push(rect_shape(r, 8.0, *f, None));
            }
            let text_layer_id = sym.timeline.add_layer("Label", 0);
            let tl = sym.timeline.layer_mut(text_layer_id).unwrap();
            tl.frames = vec![Keyframe::new(0, 3)];
            tl.frames[0].elements.push(label("Button", -30.0, -12.0, 16.0, Color::WHITE));
        }
        "checkbox" => {
            sym = Symbol::new(&name, SymbolKind::MovieClip);
            let layer = &mut sym.timeline.layers[0];
            layer.frames = vec![Keyframe::new(0, 1), Keyframe::new(1, 1)];
            for k in &mut layer.frames {
                k.elements.push(rect_shape(Rect::new(0.0, 0.0, 18.0, 18.0), 3.0, Color::WHITE, Some(Color::from_rgba8(80, 80, 80, 255))));
                k.elements.push(label("Checkbox", 26.0, -2.0, 15.0, Color::BLACK));
            }
            let mut check = Path::new();
            check.move_to(Vec2::new(4.0, 9.0)).line_to(Vec2::new(8.0, 13.0)).line_to(Vec2::new(14.0, 4.0));
            layer.frames[1].elements.push(Element::shape(vec![ShapePath::stroked(check, Stroke::solid(blue, 2.5))]));
            layer.frames[0].script = "stop();".into();
            layer.frames[1].script = "stop();".into();
        }
        "label" => {
            sym = Symbol::new(&name, SymbolKind::MovieClip);
            let mut t = TextElement::new("Label", TextStyle { size: 18.0, ..TextStyle::default() });
            t.kind = animara_document::TextKind::Dynamic;
            t.instance_name = "label_txt".into();
            sym.timeline.layers[0].frames[0].elements.push(Element::new(ElementKind::Text(t)));
        }
        _ => {
            sym = Symbol::new(&name, SymbolKind::MovieClip);
            let layer = &mut sym.timeline.layers[0];
            layer.frames[0].elements.push(rect_shape(Rect::new(0.0, 0.0, 200.0, 12.0), 6.0, Color::from_rgba8(220, 220, 220, 255), None));
            let bar_id = sym.timeline.add_layer("Bar", 0);
            let bl = sym.timeline.layer_mut(bar_id).unwrap();
            bl.frames = vec![Keyframe::new(0, 60), Keyframe::new(60, 1)];
            bl.frames[0].elements.push(rect_shape(Rect::new(0.0, 0.0, 2.0, 12.0), 6.0, blue, None));
            bl.frames[1].elements.push(rect_shape(Rect::new(0.0, 0.0, 200.0, 12.0), 6.0, blue, None));
            bl.frames[0].tween = Some(Tween::new(TweenKind::Shape));
            sym.timeline.layers[1].extend_to(60);
        }
    }
    sym.folder = "Components".into();
    doc.library.add_symbol(sym)
}

pub const DEFAULT_ASSETS: &[&str] = &["Bouncing Ball", "Spinner", "Pulsing Star", "Waving Arm (rig)"];

pub fn build_default_asset(doc: &mut Document, name: &str) -> Id {
    let nm = doc.library.unique_name(name);
    let mut sym = Symbol::new(&nm, SymbolKind::Graphic);
    let red = Color::from_rgba8(0xe7, 0x4c, 0x3c, 255);
    match name {
        "Bouncing Ball" => {
            let ball_sym = {
                let mut b = Symbol::new(&doc.library.unique_name("Ball"), SymbolKind::Graphic);
                b.timeline.layers[0].frames[0].elements.push(Element::shape(vec![ShapePath::filled(
                    Path::ellipse(Rect::new(-20.0, -20.0, 20.0, 20.0)),
                    Fill::Radial {
                        stops: vec![
                            animara_document::GradientStop { offset: 0.0, color: Color::from_rgba8(255, 160, 140, 255) },
                            animara_document::GradientStop { offset: 1.0, color: red },
                        ],
                        matrix: Affine::translate(-6.0, -6.0) * Affine::scale(26.0, 26.0),
                        spread: animara_document::SpreadMode::Pad,
                        focal: 0.0,
                    },
                )]));
                b.folder = "Assets".into();
                doc.library.add_symbol(b)
            };
            let l = &mut sym.timeline.layers[0];
            let inst = |y: f64, sy: f64| {
                let mut e = Element::new(ElementKind::Instance(animara_document::SymbolInstance::new(ball_sym)))
                    .with_transform(Affine::translate(0.0, y) * Affine::scale(1.0 / sy, sy));
                e.id = Id(1);
                e
            };
            let shared = fresh_id();
            l.frames = vec![Keyframe::new(0, 12), Keyframe::new(12, 2), Keyframe::new(14, 12), Keyframe::new(26, 1)];
            let ys = [(-120.0, 1.0), (0.0, 1.0), (0.0, 0.7), (-120.0, 1.0)];
            for (k, (y, s)) in l.frames.iter_mut().zip(ys) {
                let mut e = inst(y, s);
                e.id = shared;
                k.elements.push(e);
            }
            let mut down = Tween::new(TweenKind::Classic);
            down.ease = animara_document::Ease::Preset { preset: animara_document::EasePreset::QuadIn };
            let mut up = Tween::new(TweenKind::Classic);
            up.ease = animara_document::Ease::Preset { preset: animara_document::EasePreset::QuadOut };
            l.frames[0].tween = Some(down);
            l.frames[1].tween = Some(Tween::new(TweenKind::Classic));
            l.frames[2].tween = Some(up);
        }
        "Spinner" => {
            let l = &mut sym.timeline.layers[0];
            let mut arc = Path::new();
            for i in 0..=24 {
                let a = i as f64 / 24.0 * std::f64::consts::PI * 1.5;
                let p = Vec2::from_angle(a) * 30.0;
                if i == 0 {
                    arc.move_to(p);
                } else {
                    arc.line_to(p);
                }
            }
            let mut e = Element::shape(vec![ShapePath::stroked(arc, Stroke::solid(Color::from_rgba8(0x26, 0x80, 0xEB, 255), 6.0))]);
            let id = e.id;
            l.frames = vec![Keyframe::new(0, 24), Keyframe::new(24, 1)];
            l.frames[0].elements.push(e.clone());
            e.id = id;
            l.frames[1].elements.push(e);
            let mut tw = Tween::new(TweenKind::Classic);
            tw.rotate = animara_document::RotateMode::Clockwise;
            tw.rotate_times = 1;
            l.frames[0].tween = Some(tw);
        }
        "Pulsing Star" => {
            let l = &mut sym.timeline.layers[0];
            let star = |r: f64, c: Color| {
                Element::shape(vec![ShapePath::filled(Path::star(Vec2::ZERO, r, 5, 0.45, -std::f64::consts::FRAC_PI_2), Fill::solid(c))])
            };
            l.frames = vec![Keyframe::new(0, 15), Keyframe::new(15, 15), Keyframe::new(30, 1)];
            l.frames[0].elements.push(star(30.0, Color::from_rgba8(0xf1, 0xc4, 0x0f, 255)));
            l.frames[1].elements.push(star(45.0, Color::from_rgba8(0xe6, 0x7e, 0x22, 255)));
            l.frames[2].elements.push(star(30.0, Color::from_rgba8(0xf1, 0xc4, 0x0f, 255)));
            l.frames[0].tween = Some(Tween::new(TweenKind::Shape));
            l.frames[1].tween = Some(Tween::new(TweenKind::Shape));
        }
        _ => {
            // Two-bone arm on an armature layer with a waving pose cycle.
            let seg = |len: f64, c: Color| {
                Element::shape(vec![ShapePath::filled(Path::rounded_rect(Rect::new(0.0, -6.0, len, 6.0), 6.0), Fill::solid(c))])
            };
            let upper = seg(60.0, Color::from_rgba8(0x34, 0x98, 0xdb, 255));
            let lower = seg(50.0, Color::from_rgba8(0x5d, 0xad, 0xe2, 255)).with_transform(Affine::translate(60.0, 0.0));
            let (uid, lid) = (upper.id, lower.id);
            let l = &mut sym.timeline.layers[0];
            l.kind = LayerKind::Armature;
            l.name = "Armature".into();
            let mut arm = animara_document::Armature::default();
            let b0 = animara_rigging::add_bone(&mut arm, None, Vec2::ZERO, Vec2::new(60.0, 0.0), "Upper");
            let b1 = animara_rigging::add_bone(&mut arm, Some(b0), Vec2::new(60.0, 0.0), Vec2::new(110.0, 0.0), "Lower");
            arm.bones[b0].bound_element = Some(uid);
            arm.bones[b1].bound_element = Some(lid);
            l.frames = vec![Keyframe::new(0, 12), Keyframe::new(12, 12), Keyframe::new(24, 1)];
            for (i, k) in l.frames.iter_mut().enumerate() {
                k.elements = vec![upper.clone(), lower.clone()];
                let up = i % 2 == 1;
                k.pose = Some(animara_document::Pose {
                    angles: vec![if up { -1.2 } else { -0.4 }, if up { -0.9 } else { 0.3 }],
                    root_offset: Vec2::ZERO,
                });
            }
            l.armature = Some(arm);
        }
    }
    sym.folder = "Assets".into();
    doc.library.add_symbol(sym)
}

fn collect_deps(doc: &Document, id: Id, out: &mut Vec<Id>) {
    if out.contains(&id) {
        return;
    }
    out.push(id);
    let Some(s) = doc.library.symbol(id) else { return };
    fn walk(doc: &Document, els: &[Element], out: &mut Vec<Id>) {
        for e in els {
            match &e.kind {
                ElementKind::Instance(i) => collect_deps(doc, i.symbol, out),
                ElementKind::Bitmap(b) => {
                    if !out.contains(&b.asset) {
                        out.push(b.asset)
                    }
                }
                ElementKind::Group { children } => walk(doc, children, out),
                ElementKind::Shape(sh) => {
                    for p in &sh.paths {
                        if let Some(Fill::Bitmap { asset, .. }) = &p.fill
                            && !out.contains(asset)
                        {
                            out.push(*asset);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for l in &s.timeline.layers {
        for k in &l.frames {
            walk(doc, &k.elements, out);
            if let Some(snd) = &k.sound
                && !out.contains(&snd.asset)
            {
                out.push(snd.asset);
            }
        }
    }
}

/// Copy a symbol and everything it references into another document.
pub fn copy_symbol_with_deps(src: &Document, id: Id, dst: &mut Document) {
    let mut ids = Vec::new();
    collect_deps(src, id, &mut ids);
    for i in ids {
        if let Some(s) = src.library.symbol(i) {
            dst.library.symbols.push(s.clone());
        } else if let Some(b) = src.library.bitmap(i) {
            dst.library.bitmaps.push(b.clone());
        } else if let Some(s) = src.library.sound(i) {
            dst.library.sounds.push(s.clone());
        }
    }
}

fn remap_elements(els: &mut [Element], map: &HashMap<Id, Id>, ids: &mut HashMap<Id, Id>) {
    for e in els {
        e.id = *ids.entry(e.id).or_insert_with(fresh_id);
        match &mut e.kind {
            ElementKind::Instance(i) => {
                if let Some(n) = map.get(&i.symbol) {
                    i.symbol = *n;
                }
            }
            ElementKind::Bitmap(b) => {
                if let Some(n) = map.get(&b.asset) {
                    b.asset = *n;
                }
            }
            ElementKind::Group { children } => remap_elements(children, map, ids),
            ElementKind::Shape(s) => {
                for p in &mut s.paths {
                    if let Some(Fill::Bitmap { asset, .. }) = &mut p.fill
                        && let Some(n) = map.get(asset)
                    {
                        *asset = *n;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Merge every library item of `src` into `dst` with fresh ids; returns new symbol ids in order.
pub fn merge_library(dst: &mut Document, src: &Document) -> Vec<Id> {
    let mut map = HashMap::new();
    for s in &src.library.symbols {
        map.insert(s.id, fresh_id());
    }
    for b in &src.library.bitmaps {
        map.insert(b.id, fresh_id());
    }
    for s in &src.library.sounds {
        map.insert(s.id, fresh_id());
    }
    let mut out = Vec::new();
    for b in &src.library.bitmaps {
        let mut b = b.clone();
        b.id = map[&b.id];
        b.name = dst.library.unique_name(&b.name);
        dst.library.bitmaps.push(b);
    }
    for s in &src.library.sounds {
        let mut s = s.clone();
        s.id = map[&s.id];
        s.name = dst.library.unique_name(&s.name);
        dst.library.sounds.push(s);
    }
    for s in &src.library.symbols {
        let mut s = s.clone();
        s.id = map[&s.id];
        s.name = dst.library.unique_name(&s.name);
        // Element ids stay consistent across keyframes so tweens still match.
        let mut id_map: HashMap<Id, Id> = HashMap::new();
        for l in &mut s.timeline.layers {
            l.id = fresh_id();
            for k in &mut l.frames {
                remap_elements(&mut k.elements, &map, &mut id_map);
                if let Some(snd) = &mut k.sound
                    && let Some(n) = map.get(&snd.asset)
                {
                    snd.asset = *n;
                }
            }
            if let Some(arm) = &mut l.armature {
                for b in &mut arm.bones {
                    b.bound_element = b.bound_element.and_then(|e| id_map.get(&e).copied());
                }
            }
        }
        out.push(s.id);
        dst.library.symbols.push(s);
    }
    out
}
