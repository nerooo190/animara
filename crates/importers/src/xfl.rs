//! XFL (uncompressed Animate document folder) and ZIP-based FLA (CS5+) import.
//!
//! Supported: document settings, scenes, layers (normal/guide/mask/folder/camera),
//! layer parenting via `parentLayerIndex` (folder, mask, motion guide), frames,
//! labels, frame scripts, classic & shape tweens with easing and rotation,
//! shapes (fill/stroke styles + edge reconstruction), rectangle/oval primitives,
//! symbol instances (matrix, transformation point, color effects, blend, filters,
//! loop/first frame), groups, static/dynamic/input text, bitmap instances,
//! library symbols/folders, bitmaps and sounds whose source files are present.
//! Everything else produces a warning.

use crate::xml::{XmlNode, parse};
use crate::{ImportError, import_image_bytes};
use animara_document::{
    BitmapInstance, BlendMode, Color, ColorEffect, ColorTransform, Document, Element, ElementKind, Fill, Filter, FrameLabel, GradientStop,
    Id, Keyframe, LabelKind, Layer, LayerKind, LineCap, LineJoin, LoopMode, Primitive, RotateMode, Scene, Shape, ShapePath, SoundRef,
    SoundSync, SpreadMode, Stroke, StrokeStyle, Symbol, SymbolInstance, SymbolKind, TextAlign, TextElement, TextKind, TextRun, TextStyle,
    Timeline, Tween, TweenKind, fresh_id,
};
use animara_geometry::{Affine, FillRule, Path, Rect, Vec2};
use std::collections::HashMap;
use std::io::Read;

pub struct XflImport {
    pub document: Document,
    pub warnings: Vec<String>,
}

type Reader<'a> = dyn Fn(&str) -> Option<Vec<u8>> + 'a;

struct Ctx {
    symbols: HashMap<String, Id>,
    bitmaps: HashMap<String, Id>,
    sounds: HashMap<String, Id>,
    warnings: Vec<String>,
}

impl Ctx {
    fn warn(&mut self, w: String) {
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }
}

/// Parse an XFL/SWF-style coordinate: decimal, or `#hex[.hexfrac]` (signed 32-bit) — in twips.
pub fn parse_twips(tok: &str) -> Option<f64> {
    let t = tok.trim();
    if let Some(h) = t.strip_prefix('#') {
        let (int, frac) = match h.split_once('.') {
            Some((a, b)) => (a, Some(b)),
            None => (h, None),
        };
        let v = u32::from_str_radix(int, 16).ok()? as i32 as f64;
        let f =
            frac.and_then(|f| u32::from_str_radix(f, 16).ok()).map(|x| x as f64 / 16f64.powi(frac.unwrap().len() as i32)).unwrap_or(0.0);
        Some(v + f)
    } else {
        t.parse().ok()
    }
}

#[derive(Debug, Clone, Copy)]
struct Seg {
    a: Vec2,
    c: Option<Vec2>,
    b: Vec2,
}

/// Decode an `edges` attribute into segments (pixels).
pub(crate) fn parse_edges(s: &str) -> Vec<(Vec2, Option<Vec2>, Vec2)> {
    let mut out = Vec::new();
    let mut pos = Vec2::ZERO;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let nums = |i: &mut usize, n: usize| -> Option<Vec<f64>> {
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            while *i < chars.len() && chars[*i].is_whitespace() {
                *i += 1;
            }
            let st = *i;
            while *i < chars.len() && !chars[*i].is_whitespace() && !"!|/[]S".contains(chars[*i]) {
                *i += 1;
            }
            if st == *i {
                return None;
            }
            let tok: String = chars[st..*i].iter().collect();
            v.push(parse_twips(&tok)? / 20.0);
        }
        Some(v)
    };
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            '!' => {
                if let Some(v) = nums(&mut i, 2) {
                    pos = Vec2::new(v[0], v[1]);
                }
            }
            '|' | '/' => {
                if let Some(v) = nums(&mut i, 2) {
                    let b = Vec2::new(v[0], v[1]);
                    out.push((pos, None, b));
                    pos = b;
                }
            }
            '[' | ']' => {
                if let Some(v) = nums(&mut i, 4) {
                    let ctrl = Vec2::new(v[0], v[1]);
                    let b = Vec2::new(v[2], v[3]);
                    out.push((pos, Some(ctrl), b));
                    pos = b;
                }
            }
            'S' => {
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            _ => {}
        }
    }
    out
}

fn key(p: Vec2) -> (i64, i64) {
    ((p.x * 20.0).round() as i64, (p.y * 20.0).round() as i64)
}

/// Chain directed segments into contours.
fn chain(segs: &[Seg], closed_only: bool) -> Path {
    let mut by_start: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        by_start.entry(key(s.a)).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut path = Path::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        let first = segs[start].a;
        path.move_to(first);
        let mut cur = start;
        loop {
            used[cur] = true;
            let s = segs[cur];
            match s.c {
                Some(c) => path.quad_to(c, s.b),
                None => path.line_to(s.b),
            };
            if key(s.b) == key(first) {
                path.close();
                break;
            }
            let next = by_start.get(&key(s.b)).and_then(|v| v.iter().copied().find(|&j| !used[j]));
            match next {
                Some(n) => cur = n,
                None => {
                    if closed_only {
                        path.close();
                    }
                    break;
                }
            }
        }
    }
    path
}

fn color_attr(n: &XmlNode, k: &str, alpha_key: &str) -> Color {
    let c = n.attr(k).and_then(Color::from_hex).unwrap_or(Color::BLACK);
    c.with_alpha(n.f_or(alpha_key, 1.0) as f32)
}

fn matrix(n: Option<&XmlNode>) -> Affine {
    match n.and_then(|m| m.child("Matrix")) {
        Some(m) => {
            Affine::new(m.f_or("a", 1.0), m.f_or("b", 0.0), m.f_or("c", 0.0), m.f_or("d", 1.0), m.f_or("tx", 0.0), m.f_or("ty", 0.0))
        }
        None => Affine::IDENTITY,
    }
}

/// Gradient matrices in XFL map the ±16384-twip (±819.2 px) gradient square.
const GRADIENT_HALF: f64 = 819.2;

fn fill_style(ctx: &mut Ctx, n: &XmlNode) -> Option<Fill> {
    let k = n.children.first()?;
    match k.name.as_str() {
        "SolidColor" => Some(Fill::solid(color_attr(k, "color", "alpha"))),
        "LinearGradient" | "RadialGradient" => {
            let stops: Vec<GradientStop> = k
                .kids("GradientEntry")
                .map(|e| GradientStop { offset: e.f_or("ratio", 0.0) as f32, color: color_attr(e, "color", "alpha") })
                .collect();
            let m = matrix(k.child("matrix")) * Affine::scale(GRADIENT_HALF, GRADIENT_HALF);
            let spread = match k.attr("spreadMethod") {
                Some("reflect") => SpreadMode::Reflect,
                Some("repeat") => SpreadMode::Repeat,
                _ => SpreadMode::Pad,
            };
            if k.name == "LinearGradient" {
                Some(Fill::Linear { stops, matrix: m, spread })
            } else {
                Some(Fill::Radial { stops, matrix: m, spread, focal: k.f_or("focalPointRatio", 0.0) })
            }
        }
        "BitmapFill" => {
            let path = k.attr("bitmapPath").unwrap_or_default().to_string();
            match ctx.bitmaps.get(&path) {
                Some(id) => Some(Fill::Bitmap {
                    asset: *id,
                    matrix: matrix(k.child("matrix")) * Affine::scale(1.0 / 20.0, 1.0 / 20.0),
                    smooth: k.attr("bitmapIsSmoothed") != Some("false"),
                    repeat: k.attr("bitmapIsClipped") != Some("true"),
                }),
                None => {
                    ctx.warn(format!("bitmap fill references missing bitmap '{path}'"));
                    Some(Fill::solid(Color::rgb(0.5, 0.5, 0.5)))
                }
            }
        }
        other => {
            ctx.warn(format!("unsupported fill type <{other}>"));
            None
        }
    }
}

fn stroke_style(ctx: &mut Ctx, n: &XmlNode) -> Option<Stroke> {
    let k = n.children.first()?;
    let fill = k.child("fill").and_then(|f| fill_style(ctx, f)).unwrap_or(Fill::solid(Color::BLACK));
    let style = match k.name.as_str() {
        "SolidStroke" => {
            if k.attr("solidStyle") == Some("hairline") {
                StrokeStyle::Hairline
            } else {
                StrokeStyle::Solid
            }
        }
        "DashedStroke" => StrokeStyle::Dashed { dash: k.f_or("dash1", 6.0), gap: k.f_or("dash2", 6.0) },
        "DottedStroke" => StrokeStyle::Dotted { spacing: k.f_or("dotSpace", 3.0) },
        other => {
            ctx.warn(format!("stroke style <{other}> imported as a solid stroke"));
            StrokeStyle::Solid
        }
    };
    Some(Stroke {
        fill,
        width: k.f_or("weight", 1.0),
        cap: match k.attr("caps") {
            Some("none") => LineCap::Butt,
            Some("square") => LineCap::Square,
            _ => LineCap::Round,
        },
        join: match k.attr("joints") {
            Some("miter") => LineJoin::Miter,
            Some("bevel") => LineJoin::Bevel,
            _ => LineJoin::Round,
        },
        miter_limit: k.f_or("miterLimit", 3.0),
        style,
        scale_with_object: k.attr("scaleMode") != Some("none"),
    })
}

fn shape(ctx: &mut Ctx, n: &XmlNode) -> Shape {
    let mut fills: HashMap<u32, Fill> = HashMap::new();
    for fs in n.list("fills", "FillStyle") {
        if let (Some(i), Some(f)) = (fs.f("index"), fill_style(ctx, fs)) {
            fills.insert(i as u32, f);
        }
    }
    let mut strokes: HashMap<u32, Stroke> = HashMap::new();
    for ss in n.list("strokes", "StrokeStyle") {
        if let (Some(i), Some(s)) = (ss.f("index"), stroke_style(ctx, ss)) {
            strokes.insert(i as u32, s);
        }
    }
    let mut fill_segs: HashMap<u32, Vec<Seg>> = HashMap::new();
    let mut stroke_segs: HashMap<u32, Vec<Seg>> = HashMap::new();
    for e in n.list("edges", "Edge") {
        let Some(data) = e.attr("edges") else { continue };
        let f0 = e.f("fillStyle0").map(|v| v as u32);
        let f1 = e.f("fillStyle1").map(|v| v as u32);
        let st = e.f("strokeStyle").map(|v| v as u32);
        for (a, c, b) in parse_edges(data) {
            let seg = Seg { a, c, b };
            if let Some(f) = f1 {
                fill_segs.entry(f).or_default().push(seg);
            }
            if let Some(f) = f0 {
                fill_segs.entry(f).or_default().push(Seg { a: b, c, b: a });
            }
            if let Some(s) = st {
                stroke_segs.entry(s).or_default().push(seg);
            }
        }
    }
    let mut paths = Vec::new();
    let mut fkeys: Vec<_> = fill_segs.keys().copied().collect();
    fkeys.sort();
    for k in fkeys {
        let Some(fill) = fills.get(&k).cloned() else { continue };
        let p = chain(&fill_segs[&k], true);
        if !p.is_empty() {
            paths.push(ShapePath { path: p, fill: Some(fill), stroke: None, fill_rule: FillRule::NonZero, primitive: None });
        }
    }
    let mut skeys: Vec<_> = stroke_segs.keys().copied().collect();
    skeys.sort();
    for k in skeys {
        let Some(stroke) = strokes.get(&k).cloned() else { continue };
        // Undirected chaining: add reversed copies so polylines can continue either way.
        let p = chain(&stroke_segs[&k], false);
        if !p.is_empty() {
            paths.push(ShapePath::stroked(p, stroke));
        }
    }
    Shape { paths }
}

fn primitive(ctx: &mut Ctx, n: &XmlNode) -> Element {
    let r = Rect::from_xywh(n.f_or("x", 0.0), n.f_or("y", 0.0), n.f_or("objectWidth", 0.0), n.f_or("objectHeight", 0.0));
    let prim = if n.name == "DOMRectangleObject" {
        Primitive::Rect { rect: r, corner_radius: n.f_or("topLeftRadius", 0.0) }
    } else {
        Primitive::Oval {
            rect: r,
            start_angle: n.f_or("startAngle", 0.0),
            end_angle: n.f_or("endAngle", 0.0),
            inner_radius: n.f_or("innerRadius", 0.0) / 100.0,
        }
    };
    let fill = n.child("fill").and_then(|f| fill_style(ctx, f));
    let stroke = n.child("stroke").and_then(|s| stroke_style(ctx, s));
    let sp = ShapePath { path: prim.to_path(), fill, stroke, fill_rule: FillRule::NonZero, primitive: Some(prim) };
    Element::shape(vec![sp]).with_transform(matrix(n.child("matrix")))
}

fn color_effect(n: Option<&XmlNode>) -> ColorEffect {
    let Some(c) = n.and_then(|c| c.child("Color")) else { return ColorEffect::None };
    if let Some(b) = c.f("brightness") {
        return ColorEffect::Brightness { amount: b as f32 };
    }
    if let Some(t) = c.f("tintMultiplier") {
        return ColorEffect::Tint { color: c.attr("tintColor").and_then(Color::from_hex).unwrap_or(Color::BLACK), amount: t as f32 };
    }
    let mul = [c.f_or("redMultiplier", 1.0), c.f_or("greenMultiplier", 1.0), c.f_or("blueMultiplier", 1.0), c.f_or("alphaMultiplier", 1.0)];
    let add = [c.f_or("redOffset", 0.0), c.f_or("greenOffset", 0.0), c.f_or("blueOffset", 0.0), c.f_or("alphaOffset", 0.0)];
    if mul[..3] == [1.0, 1.0, 1.0] && add == [0.0; 4] {
        if mul[3] == 1.0 {
            return ColorEffect::None;
        }
        return ColorEffect::Alpha { alpha: mul[3] as f32 };
    }
    ColorEffect::Advanced { transform: ColorTransform { mul: mul.map(|v| v as f32), add: add.map(|v| (v / 255.0) as f32) } }
}

fn filters(ctx: &mut Ctx, n: Option<&XmlNode>) -> Vec<Filter> {
    let Some(n) = n else { return Vec::new() };
    let mut out = Vec::new();
    for f in &n.children {
        let q = f.f_or("quality", 1.0) as u32;
        match f.name.as_str() {
            "BlurFilter" => out.push(Filter::Blur { blur_x: f.f_or("blurX", 5.0), blur_y: f.f_or("blurY", 5.0), quality: q }),
            "DropShadowFilter" => out.push(Filter::DropShadow {
                blur_x: f.f_or("blurX", 5.0),
                blur_y: f.f_or("blurY", 5.0),
                strength: f.f_or("strength", 1.0) as f32,
                quality: q,
                angle: f.f_or("angle", 45.0),
                distance: f.f_or("distance", 5.0),
                color: color_attr(f, "color", "alpha"),
                knockout: f.attr("knockout") == Some("true"),
                inner: f.attr("inner") == Some("true"),
                hide_object: f.attr("hideObject") == Some("true"),
            }),
            "GlowFilter" => out.push(Filter::Glow {
                blur_x: f.f_or("blurX", 5.0),
                blur_y: f.f_or("blurY", 5.0),
                strength: f.f_or("strength", 1.0) as f32,
                quality: q,
                color: f
                    .attr("color")
                    .and_then(Color::from_hex)
                    .unwrap_or(Color::rgb(1.0, 0.0, 0.0))
                    .with_alpha(f.f_or("alpha", 1.0) as f32),
                knockout: f.attr("knockout") == Some("true"),
                inner: f.attr("inner") == Some("true"),
            }),
            "AdjustColorFilter" => out.push(Filter::AdjustColor {
                brightness: f.f_or("brightness", 0.0) as f32,
                contrast: f.f_or("contrast", 0.0) as f32,
                saturation: f.f_or("saturation", 0.0) as f32,
                hue: f.f_or("hue", 0.0) as f32,
            }),
            other => ctx.warn(format!("filter <{other}> is not supported and was dropped")),
        }
    }
    out
}

fn blend(s: Option<&str>) -> BlendMode {
    match s.unwrap_or("normal") {
        "layer" => BlendMode::Layer,
        "multiply" => BlendMode::Multiply,
        "screen" => BlendMode::Screen,
        "overlay" => BlendMode::Overlay,
        "hardlight" => BlendMode::HardLight,
        "lighten" => BlendMode::Lighten,
        "darken" => BlendMode::Darken,
        "difference" => BlendMode::Difference,
        "add" => BlendMode::Add,
        "subtract" => BlendMode::Subtract,
        "invert" => BlendMode::Invert,
        "alpha" => BlendMode::Alpha,
        "erase" => BlendMode::Erase,
        _ => BlendMode::Normal,
    }
}

fn text(ctx: &mut Ctx, n: &XmlNode) -> Element {
    let runs: Vec<TextRun> = n
        .list("textRuns", "DOMTextRun")
        .into_iter()
        .map(|r| {
            let chars = r.child("characters").map(|c| c.all_text()).unwrap_or_default().replace('\r', "\n");
            let a = r.child("textAttrs").and_then(|t| t.child("DOMTextAttrs"));
            let face = a.and_then(|a| a.attr("face")).unwrap_or("sans-serif").to_string();
            let bold = face.contains("Bold");
            let italic = face.contains("Italic") || face.contains("Oblique");
            let family = face.split('-').next().unwrap_or(&face).trim_end_matches("MT").replace("PS", "").to_string();
            let style = TextStyle {
                font_family: if family.is_empty() { "sans-serif".into() } else { family },
                size: a.map(|a| a.f_or("size", 12.0)).unwrap_or(12.0),
                color: a.map(|a| color_attr(a, "fillColor", "alpha")).unwrap_or(Color::BLACK),
                bold,
                italic,
                letter_spacing: a.map(|a| a.f_or("letterSpacing", 0.0)).unwrap_or(0.0),
                kerning: a.is_none_or(|a| a.attr("autoKern") != Some("false")),
                url: a.and_then(|a| a.attr("url")).map(String::from),
            };
            TextRun { text: chars, style }
        })
        .collect();
    let attrs = n.list("textRuns", "DOMTextRun").first().and_then(|r| r.child("textAttrs")).and_then(|t| t.child("DOMTextAttrs")).cloned();
    let mut t = TextElement::new("", TextStyle::default());
    t.runs = if runs.is_empty() { vec![TextRun { text: String::new(), style: TextStyle::default() }] } else { runs };
    t.kind = match n.name.as_str() {
        "DOMDynamicText" => TextKind::Dynamic,
        "DOMInputText" => TextKind::Input,
        _ => TextKind::Static,
    };
    t.align = match attrs.as_ref().and_then(|a| a.attr("alignment")) {
        Some("center") => TextAlign::Center,
        Some("right") => TextAlign::Right,
        Some("justify") => TextAlign::Justify,
        _ => TextAlign::Left,
    };
    t.line_spacing = attrs.as_ref().map(|a| a.f_or("lineSpacing", 2.0)).unwrap_or(2.0);
    t.width = n.f("width").filter(|_| n.attr("autoExpand") != Some("true"));
    t.instance_name = n.attr("name").unwrap_or_default().to_string();
    t.selectable = n.attr("isSelectable") != Some("false");
    if n.name == "DOMTLFText" {
        ctx.warn("TLF text imported as classic text".into());
    }
    let m = matrix(n.child("matrix")) * Affine::translate(n.f_or("left", 0.0), n.f_or("top", 0.0));
    Element::new(ElementKind::Text(t)).with_transform(m)
}

fn elements(ctx: &mut Ctx, list: &[XmlNode]) -> Vec<Element> {
    let mut out = Vec::new();
    for n in list {
        match n.name.as_str() {
            "DOMShape" => {
                let s = shape(ctx, n);
                if !s.paths.is_empty() {
                    out.push(Element::new(ElementKind::Shape(s)).with_transform(matrix(n.child("matrix"))));
                }
            }
            "DOMRectangleObject" | "DOMOvalObject" => out.push(primitive(ctx, n)),
            "DOMSymbolInstance" => {
                let name = n.attr("libraryItemName").unwrap_or_default();
                let Some(&sid) = ctx.symbols.get(name) else {
                    ctx.warn(format!("instance references missing symbol '{name}'"));
                    continue;
                };
                let mut inst = SymbolInstance::new(sid);
                inst.instance_name = n.attr("name").unwrap_or_default().to_string();
                inst.first_frame = n.f_or("firstFrame", 0.0) as u32;
                inst.loop_mode = match n.attr("loop") {
                    Some("play once") => LoopMode::PlayOnce,
                    Some("single frame") => LoopMode::SingleFrame,
                    Some("loop reverse") => LoopMode::LoopReverse,
                    Some("play once reverse") => LoopMode::PlayOnceReverse,
                    _ => LoopMode::Loop,
                };
                inst.color_effect = color_effect(n.child("color"));
                inst.filters = filters(ctx, n.child("filters"));
                inst.blend = blend(n.attr("blendMode"));
                inst.visible = n.attr("isVisible") != Some("false");
                inst.cache_as_bitmap = n.attr("cacheAsBitmap") == Some("true");
                let mut e = Element::new(ElementKind::Instance(inst)).with_transform(matrix(n.child("matrix")));
                if let Some(p) = n.child("transformationPoint").and_then(|t| t.child("Point")) {
                    e.pivot = Vec2::new(p.f_or("x", 0.0), p.f_or("y", 0.0));
                }
                if n.child("Actionscript").is_some() {
                    ctx.warn("instance-level ActionScript (on(...) handlers) is not imported".into());
                }
                out.push(e);
            }
            "DOMBitmapInstance" => {
                let name = n.attr("libraryItemName").unwrap_or_default();
                match ctx.bitmaps.get(name) {
                    Some(&id) => {
                        out.push(Element::new(ElementKind::Bitmap(BitmapInstance { asset: id })).with_transform(matrix(n.child("matrix"))))
                    }
                    None => ctx.warn(format!("bitmap instance references missing bitmap '{name}'")),
                }
            }
            "DOMGroup" => {
                let members: Vec<XmlNode> = n.child("members").map(|m| m.children.clone()).unwrap_or_default();
                let children = elements(ctx, &members);
                out.push(Element::new(ElementKind::Group { children }));
            }
            "DOMStaticText" | "DOMDynamicText" | "DOMInputText" | "DOMTLFText" => out.push(text(ctx, n)),
            other => ctx.warn(format!("element <{other}> is not supported and was skipped")),
        }
    }
    out
}

fn timeline(ctx: &mut Ctx, n: &XmlNode) -> Timeline {
    let layer_nodes = n.list("layers", "DOMLayer");
    let mut layers: Vec<Layer> = Vec::new();
    let mut parents: Vec<Option<usize>> = Vec::new();
    for ln in &layer_nodes {
        let mut l = Layer::new(ln.attr("name").unwrap_or("Layer"));
        l.kind = match ln.attr("layerType") {
            Some("guide") => LayerKind::Guide,
            Some("mask") => LayerKind::Mask,
            Some("folder") => LayerKind::Folder,
            Some("camera") => LayerKind::Camera,
            _ => LayerKind::Normal,
        };
        if let Some(c) = ln.attr("color").and_then(Color::from_hex) {
            l.color = c;
        }
        l.visible = ln.attr("visible") != Some("false");
        l.locked = ln.attr("locked") == Some("true");
        l.outline = ln.attr("outline") == Some("true");
        l.expanded = ln.attr("open") != Some("false");
        l.depth = ln.f_or("layerDepth", 0.0);
        l.frames.clear();
        let mut frames: Vec<&XmlNode> = ln.list("frames", "DOMFrame");
        frames.sort_by_key(|f| f.f_or("index", 0.0) as u32);
        let mut cursor = 0u32;
        for f in frames {
            let index = f.f_or("index", 0.0) as u32;
            let duration = f.f_or("duration", 1.0).max(1.0) as u32;
            if index > cursor {
                // Fill gaps with blank keyframes so spans stay contiguous.
                l.frames.push(Keyframe::new(cursor, index - cursor));
            }
            if index < cursor {
                ctx.warn(format!("layer '{}': overlapping frame at {}", l.name, index + 1));
                continue;
            }
            let mut k = Keyframe::new(index, duration);
            let els: Vec<XmlNode> = f.child("elements").map(|e| e.children.clone()).unwrap_or_default();
            k.elements = elements(ctx, &els);
            if let Some(name) = f.attr("name") {
                k.label = Some(FrameLabel {
                    name: name.to_string(),
                    kind: match f.attr("labelType") {
                        Some("comment") => LabelKind::Comment,
                        Some("anchor") => LabelKind::Anchor,
                        _ => LabelKind::Name,
                    },
                });
            }
            if let Some(s) = f.child("Actionscript").and_then(|a| a.child("script")) {
                k.script = s.all_text();
                ctx.warn("ActionScript frame scripts were imported as text; they run through the Rhai-based runtime and only the timeline API subset is compatible".into());
            }
            match f.attr("tweenType") {
                Some("motion") | Some("motion object") => {
                    let mut t =
                        Tween::new(if f.attr("tweenType") == Some("motion object") { TweenKind::Motion } else { TweenKind::Classic });
                    if let Some(a) = f.f("acceleration") {
                        t.ease = animara_document::Ease::Classic { strength: a as i32 };
                    }
                    t.rotate = match f.attr("motionTweenRotate") {
                        Some("none") => RotateMode::None,
                        Some("clockwise") => RotateMode::Clockwise,
                        Some("counter-clockwise") => RotateMode::CounterClockwise,
                        _ => RotateMode::Auto,
                    };
                    t.rotate_times = f.f_or("motionTweenRotateTimes", 0.0) as u32;
                    t.orient_to_path = f.attr("motionTweenOrientToPath") == Some("true");
                    t.scale = f.attr("motionTweenScale") != Some("false");
                    if f.child("tweens").is_some() {
                        ctx.warn("custom classic-tween ease curves (<tweens>) are approximated by the frame's acceleration".into());
                    }
                    if f.attr("tweenType") == Some("motion object") {
                        ctx.warn("motion-object tweens are approximated as tweens between keyframes; property keyframes inside the span are not imported".into());
                    }
                    k.tween = Some(t);
                }
                Some("shape") => {
                    let mut t = Tween::new(TweenKind::Shape);
                    if let Some(a) = f.f("acceleration") {
                        t.ease = animara_document::Ease::Classic { strength: a as i32 };
                    }
                    k.tween = Some(t);
                }
                _ => {}
            }
            if let Some(sn) = f.attr("soundName") {
                match ctx.sounds.get(sn) {
                    Some(&sid) => {
                        let mut s = SoundRef::new(sid);
                        s.sync = match f.attr("soundSync") {
                            Some("start") => SoundSync::Start,
                            Some("stop") => SoundSync::Stop,
                            Some("stream") => SoundSync::Stream,
                            _ => SoundSync::Event,
                        };
                        s.loops = f.f_or("soundLoop", 1.0).max(1.0) as u32 - 1;
                        k.sound = Some(s);
                    }
                    None => ctx.warn(format!("frame sound '{sn}' is not available")),
                }
            }
            l.frames.push(k);
            cursor = index + duration;
        }
        if l.frames.is_empty() && l.kind != LayerKind::Folder {
            l.frames.push(Keyframe::new(0, 1));
        }
        parents.push(ln.f("parentLayerIndex").map(|v| v as usize));
        layers.push(l);
    }
    // Resolve parentLayerIndex relationships.
    let ids: Vec<Id> = layers.iter().map(|l| l.id).collect();
    let kinds: Vec<LayerKind> = layers.iter().map(|l| l.kind).collect();
    for (i, p) in parents.iter().enumerate() {
        let Some(p) = *p else { continue };
        if p >= layers.len() {
            continue;
        }
        match kinds[p] {
            LayerKind::Folder => layers[i].folder = Some(ids[p]),
            LayerKind::Mask => layers[i].mask_layer = Some(ids[p]),
            LayerKind::Guide => layers[i].guide_layer = Some(ids[p]),
            _ => layers[i].parent_layer = Some(ids[p]),
        }
    }
    // Masked/guided layers may also sit in folders: inherit the parent's folder.
    for i in 0..layers.len() {
        if layers[i].folder.is_none()
            && let Some(m) = layers[i].mask_layer.or(layers[i].guide_layer)
            && let Some(pl) = layers.iter().find(|l| l.id == m)
        {
            layers[i].folder = pl.folder;
        }
    }
    Timeline { layers }
}

fn bitmap_from_dat(data: &[u8]) -> Option<Vec<u8>> {
    // JPEG / PNG payloads are stored verbatim.
    if data.starts_with(&[0xFF, 0xD8]) || data.starts_with(b"\x89PNG") || data.starts_with(b"GIF8") {
        return Some(data.to_vec());
    }
    // Lossless bitmap: 0x0503 header, row bytes, width, height, bounds, flags, then
    // zlib-compressed ARGB in u16-length-prefixed chunks (community-documented layout).
    if data.len() > 30 && data[0] == 0x03 && data[1] == 0x05 {
        let w = u16::from_le_bytes([data[4], data[5]]) as u32;
        let h = u16::from_le_bytes([data[6], data[7]]) as u32;
        let compressed = data[25] != 0;
        let mut payload = Vec::new();
        let mut i = 26usize;
        if compressed {
            while i + 2 <= data.len() {
                let len = u16::from_le_bytes([data[i], data[i + 1]]) as usize;
                i += 2;
                if len == 0 || i + len > data.len() {
                    break;
                }
                payload.extend_from_slice(&data[i..i + len]);
                i += len;
            }
            payload = miniz_oxide::inflate::decompress_to_vec_zlib(&payload).ok()?;
        } else {
            payload = data[26..].to_vec();
        }
        if payload.len() < (w * h * 4) as usize {
            return None;
        }
        let mut img = image::RgbaImage::new(w, h);
        for (px, c) in img.pixels_mut().zip(payload.as_chunks::<4>().0.iter()) {
            // ARGB, premultiplied.
            let a = c[0];
            let un = |v: u8| if a == 0 { 0 } else { ((v as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
            *px = image::Rgba([un(c[1]), un(c[2]), un(c[3]), a]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).ok()?;
        return Some(out.into_inner());
    }
    None
}

fn import_with(read: &Reader) -> Result<XflImport, ImportError> {
    let dom = read("DOMDocument.xml").ok_or_else(|| ImportError::Unsupported("DOMDocument.xml not found".into()))?;
    let root = parse(&String::from_utf8_lossy(&dom)).map_err(ImportError::Xml)?;
    if root.name != "DOMDocument" {
        return Err(ImportError::Unsupported(format!("unexpected root element <{}>", root.name)));
    }
    let mut doc = Document::new(root.f_or("width", 550.0), root.f_or("height", 400.0), root.f_or("frameRate", 24.0));
    doc.background = root.attr("backgroundColor").and_then(Color::from_hex).unwrap_or(Color::WHITE);
    doc.scenes.clear();
    let mut ctx = Ctx { symbols: HashMap::new(), bitmaps: HashMap::new(), sounds: HashMap::new(), warnings: Vec::new() };
    for f in root.list("folders", "DOMFolderItem") {
        if let Some(n) = f.attr("name") {
            doc.library.folders.push(n.to_string());
        }
    }
    let split = |name: &str| -> (String, String) {
        match name.rsplit_once('/') {
            Some((folder, n)) => (folder.to_string(), n.to_string()),
            None => (String::new(), name.to_string()),
        }
    };
    // Media.
    if let Some(media) = root.child("media") {
        for m in &media.children {
            let name = m.attr("name").unwrap_or_default().to_string();
            let href = m.attr("href").unwrap_or(&name).to_string();
            let (folder, short) = split(&name);
            match m.name.as_str() {
                "DOMBitmapItem" => {
                    let bytes = read(&format!("LIBRARY/{href}"))
                        .or_else(|| read(&format!("LIBRARY/{name}")))
                        .or_else(|| m.attr("bitmapDataHRef").and_then(|d| read(&format!("bin/{d}"))).and_then(|d| bitmap_from_dat(&d)));
                    match bytes.and_then(|b| import_image_bytes(&short, b).ok()) {
                        Some(mut a) => {
                            a.folder = folder;
                            a.smoothing = m.attr("allowSmoothing") != Some("false");
                            ctx.bitmaps.insert(name.clone(), a.id);
                            doc.library.bitmaps.push(a);
                        }
                        None => ctx.warn(format!("bitmap '{name}' could not be decoded (source file missing and bin data not readable)")),
                    }
                }
                "DOMSoundItem" => {
                    let bytes = read(&format!("LIBRARY/{href}")).or_else(|| read(&format!("LIBRARY/{name}")));
                    match bytes {
                        Some(b) => {
                            let ext = std::path::Path::new(&href)
                                .extension()
                                .map(|e| e.to_string_lossy().to_lowercase())
                                .unwrap_or_else(|| "wav".into());
                            let id = fresh_id();
                            ctx.sounds.insert(name.clone(), id);
                            doc.library.sounds.push(animara_document::SoundAsset {
                                id,
                                name: short,
                                format: ext,
                                sample_rate: m.f_or("sampleRate", 44100.0) as u32,
                                channels: if m.attr("format").is_some_and(|f| f.contains("mono")) { 1 } else { 2 },
                                duration: m.f_or("sampleCount", 0.0) / m.f_or("sampleRate", 44100.0).max(1.0),
                                folder,
                                data: std::sync::Arc::new(b),
                            });
                        }
                        None => ctx.warn(format!("sound '{name}' source file is not available; sound skipped")),
                    }
                }
                other => ctx.warn(format!("library item <{other}> '{name}' is not supported")),
            }
        }
    }
    // Symbols: first allocate ids, then parse timelines.
    let includes: Vec<String> = root.list("symbols", "Include").iter().filter_map(|i| i.attr("href").map(String::from)).collect();
    let mut sym_nodes = Vec::new();
    for href in &includes {
        match read(&format!("LIBRARY/{href}")).map(|b| parse(&String::from_utf8_lossy(&b))) {
            Some(Ok(n)) => {
                let name = n.attr("name").unwrap_or(href.trim_end_matches(".xml")).to_string();
                ctx.symbols.insert(name.clone(), fresh_id());
                sym_nodes.push((name, n));
            }
            Some(Err(e)) => ctx.warn(format!("symbol file '{href}' is malformed: {e}")),
            None => ctx.warn(format!("symbol file '{href}' is missing")),
        }
    }
    for (name, n) in &sym_nodes {
        let kind = match n.attr("symbolType") {
            Some("graphic") => SymbolKind::Graphic,
            Some("button") => SymbolKind::Button,
            _ => SymbolKind::MovieClip,
        };
        let (folder, short) = split(name);
        let tl = n.child("timeline").and_then(|t| t.child("DOMTimeline")).map(|t| timeline(&mut ctx, t)).unwrap_or_default();
        doc.library.symbols.push(Symbol {
            id: ctx.symbols[name],
            name: short,
            kind,
            timeline: if tl.layers.is_empty() { Timeline::with_layer("Layer 1") } else { tl },
            folder,
            linkage: n.attr("linkageClassName").unwrap_or_default().to_string(),
        });
    }
    // Library names in instances use full paths; symbols store short names + folder.
    for t in root.list("timelines", "DOMTimeline") {
        let tl = timeline(&mut ctx, t);
        doc.scenes.push(Scene { id: fresh_id(), name: t.attr("name").unwrap_or("Scene").to_string(), timeline: tl });
    }
    if doc.scenes.is_empty() {
        doc.scenes.push(Scene::new("Scene 1"));
        ctx.warn("document had no timelines; created an empty scene".into());
    }
    if root.child("persistentData").is_some() {
        ctx.warn("document persistent data was ignored".into());
    }
    let problems = doc.validate();
    for p in problems {
        ctx.warn(format!("validation: {p}"));
    }
    animara_document::bump_ids_above(doc.max_id());
    Ok(XflImport { document: doc, warnings: ctx.warnings })
}

/// Import an XFL document folder.
pub fn import_xfl_dir(dir: &std::path::Path) -> Result<XflImport, ImportError> {
    let read = |p: &str| std::fs::read(dir.join(p)).ok();
    let mut r = import_with(&read)?;
    if let Some(n) = dir.file_name() {
        r.document.name = n.to_string_lossy().into_owned();
    }
    Ok(r)
}

/// Import from a `.xfl` file (the document folder is its parent) or the folder itself.
pub fn import_xfl(path: &std::path::Path) -> Result<XflImport, ImportError> {
    let dir = if path.is_dir() { path.to_path_buf() } else { path.parent().map(|p| p.to_path_buf()).unwrap_or_default() };
    let mut r = import_xfl_dir(&dir)?;
    if let Some(s) = path.file_stem() {
        r.document.name = s.to_string_lossy().into_owned();
    }
    Ok(r)
}

/// Import a `.fla` file. CS5+ FLA files are ZIP archives of XFL; legacy binary
/// (OLE compound document) FLA files from Flash CS4 and earlier are rejected.
pub fn import_fla(path: &std::path::Path) -> Result<XflImport, ImportError> {
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return Err(ImportError::Unsupported(
            "this is a legacy binary FLA (Flash CS4 or earlier). Re-save it as CS5+ FLA or XFL in the original application to import it."
                .into(),
        ));
    }
    let zip = std::cell::RefCell::new(zip::ZipArchive::new(std::io::Cursor::new(bytes))?);
    let read = |p: &str| -> Option<Vec<u8>> {
        let mut z = zip.borrow_mut();
        let mut f = z.by_name(p).ok()?;
        let mut v = Vec::new();
        f.read_to_end(&mut v).ok()?;
        Some(v)
    };
    let mut r = import_with(&read)?;
    if let Some(s) = path.file_stem() {
        r.document.name = s.to_string_lossy().into_owned();
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twips_parsing() {
        assert_eq!(parse_twips("200"), Some(200.0));
        assert_eq!(parse_twips("#A0"), Some(160.0));
        assert_eq!(parse_twips("#FFFFFF60"), Some(-160.0));
        assert_eq!(parse_twips("#10.8"), Some(16.5));
    }

    #[test]
    fn edges_build_closed_fill() {
        let segs = parse_edges("!0 0|2000 0|2000 2000|0 2000|0 0");
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[1].2, Vec2::new(100.0, 100.0));
        let q = parse_edges("!0 0[1000 1000 2000 0");
        assert_eq!(q[0].1, Some(Vec2::new(50.0, 50.0)));
    }

    pub(crate) const SAMPLE: &str = r##"<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/" width="640" height="360" frameRate="12" backgroundColor="#336699">
  <symbols><Include href="Ball.xml"/></symbols>
  <timelines>
    <DOMTimeline name="Scene 1">
      <layers>
        <DOMLayer name="Guide" layerType="guide" color="#FF0000">
          <frames><DOMFrame index="0" duration="10"><elements>
            <DOMShape><strokes><StrokeStyle index="1"><SolidStroke weight="1"><fill><SolidColor color="#000000"/></fill></SolidStroke></StrokeStyle></strokes>
            <edges><Edge strokeStyle="1" edges="!0 4000|8000 4000"/></edges></DOMShape>
          </elements></DOMFrame></frames>
        </DOMLayer>
        <DOMLayer name="Ball" parentLayerIndex="0" color="#00FF00">
          <frames>
            <DOMFrame index="0" duration="9" tweenType="motion" acceleration="50" name="start"><Actionscript><script><![CDATA[trace("hi");]]></script></Actionscript><elements>
              <DOMSymbolInstance libraryItemName="Ball" name="ball_mc"><matrix><Matrix tx="10" ty="20"/></matrix><color><Color alphaMultiplier="0.5"/></color><filters><BlurFilter blurX="4" blurY="4"/></filters></DOMSymbolInstance>
            </elements></DOMFrame>
            <DOMFrame index="9"><elements>
              <DOMSymbolInstance libraryItemName="Ball"><matrix><Matrix tx="300" ty="20"/></matrix></DOMSymbolInstance>
            </elements></DOMFrame>
          </frames>
        </DOMLayer>
        <DOMLayer name="Box">
          <frames><DOMFrame index="0" duration="10"><elements>
            <DOMShape><fills><FillStyle index="1"><SolidColor color="#FF8800"/></FillStyle></fills>
              <edges><Edge fillStyle1="1" edges="!0 0|2000 0|2000 2000|0 2000|0 0"/></edges></DOMShape>
            <DOMStaticText left="2" width="100"><matrix><Matrix tx="5" ty="5"/></matrix><textRuns><DOMTextRun><characters>Hello&#xD;World</characters><textAttrs><DOMTextAttrs face="Arial-BoldMT" size="20" fillColor="#112233" alignment="center"/></textAttrs></DOMTextRun></textRuns></DOMStaticText>
            <DOMFancyThing/>
          </elements></DOMFrame></frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timelines>
</DOMDocument>"##;

    pub(crate) const BALL: &str = r##"<DOMSymbolItem name="Ball" symbolType="graphic"><timeline><DOMTimeline name="Ball"><layers><DOMLayer name="L"><frames><DOMFrame index="0"><elements>
      <DOMOvalObject x="-10" y="-10" objectWidth="20" objectHeight="20"><fill><SolidColor color="#CC0000"/></fill></DOMOvalObject>
    </elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>"##;

    #[test]
    fn imports_sample_document() {
        let read = |p: &str| match p {
            "DOMDocument.xml" => Some(SAMPLE.as_bytes().to_vec()),
            "LIBRARY/Ball.xml" => Some(BALL.as_bytes().to_vec()),
            _ => None,
        };
        let r = import_with(&read).unwrap();
        let d = &r.document;
        assert_eq!((d.width, d.height, d.fps), (640.0, 360.0, 12.0));
        assert_eq!(d.background, Color::from_hex("#336699").unwrap());
        let tl = &d.scenes[0].timeline;
        assert_eq!(tl.layers.len(), 3);
        assert_eq!(tl.layers[0].kind, LayerKind::Guide);
        assert_eq!(tl.layers[1].guide_layer, Some(tl.layers[0].id));
        let k0 = &tl.layers[1].frames[0];
        assert_eq!(k0.label.as_ref().unwrap().name, "start");
        assert_eq!(k0.script, "trace(\"hi\");");
        assert!(matches!(k0.tween.as_ref().unwrap().ease, animara_document::Ease::Classic { strength: 50 }));
        let ElementKind::Instance(i) = &k0.elements[0].kind else { panic!() };
        assert_eq!(i.instance_name, "ball_mc");
        assert_eq!(i.color_effect, ColorEffect::Alpha { alpha: 0.5 });
        assert_eq!(i.filters.len(), 1);
        assert_eq!(d.library.symbols[0].kind, SymbolKind::Graphic);
        // Box shape reconstructed as a closed 100×100 fill.
        let ElementKind::Shape(s) = &tl.layers[2].frames[0].elements[0].kind else { panic!() };
        let b = s.bounds();
        assert!((b.width() - 100.0).abs() < 1e-9 && (b.height() - 100.0).abs() < 1e-9);
        assert!(s.paths[0].path.contains(Vec2::new(50.0, 50.0), FillRule::NonZero));
        let ElementKind::Text(t) = &tl.layers[2].frames[0].elements[1].kind else { panic!() };
        assert_eq!(t.plain_text(), "Hello\nWorld");
        assert!(t.style().bold);
        assert_eq!(t.align, TextAlign::Center);
        assert!(r.warnings.iter().any(|w| w.contains("DOMFancyThing")));
        assert!(d.validate().is_empty());
    }
}
