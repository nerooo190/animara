//! Lottie (Bodymovin 5.x JSON) export.
//!
//! Frames are baked: each run of identical frames becomes a set of shape layers
//! with in/out points, so every Animara feature that resolves to vector paths
//! (classic/shape/motion tweens, nested symbols, rigs, cameras, text outlines)
//! plays back exactly. Bitmaps become image layers; masks become layer masks;
//! blur filters become Gaussian Blur effects. Other filters are reported.

use crate::{ExportError, ExportOptions};
use animara_animation::{DisplayItem, EvalOptions, evaluate_document_frame, scene_at_global_frame};
use animara_document::{BlendMode, ColorTransform, Document, Fill, Filter, LineCap, LineJoin, Shape};
use animara_geometry::{Affine, FillRule, Path, Segment, Vec2};
use base64::Engine as _;
use serde_json::{Value, json};

#[derive(Clone, PartialEq)]
struct LayerSpec {
    shapes: Vec<Value>,
    masks: Vec<Value>,
    effects: Vec<Value>,
    bm: u8,
    image: Option<(String, Affine, f64)>,
}

impl LayerSpec {
    fn empty() -> LayerSpec {
        LayerSpec { shapes: Vec::new(), masks: Vec::new(), effects: Vec::new(), bm: 0, image: None }
    }
    fn is_empty(&self) -> bool {
        self.shapes.is_empty() && self.image.is_none()
    }
}

fn st(v: Value) -> Value {
    json!({"a": 0, "k": v})
}

fn r(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn path_value(p: &Path) -> Vec<Value> {
    p.subpaths()
        .into_iter()
        .map(|sp| {
            let cubics: Vec<[Vec2; 4]> = sp.segments.iter().map(Segment::to_cubic).collect();
            let n = cubics.len();
            let mut v = Vec::new();
            let mut i = Vec::new();
            let mut o = Vec::new();
            for (k, c) in cubics.iter().enumerate() {
                v.push(json!([r(c[0].x), r(c[0].y)]));
                o.push(json!([r(c[1].x - c[0].x), r(c[1].y - c[0].y)]));
                let prev_in = if k == 0 { if sp.closed { cubics[n - 1][2] - c[0] } else { Vec2::ZERO } } else { cubics[k - 1][2] - c[0] };
                i.push(json!([r(prev_in.x), r(prev_in.y)]));
            }
            if !sp.closed
                && let Some(last) = cubics.last()
            {
                v.push(json!([r(last[3].x), r(last[3].y)]));
                o.push(json!([0, 0]));
                i.push(json!([r(last[2].x - last[3].x), r(last[2].y - last[3].y)]));
            }
            json!({"i": i, "o": o, "v": v, "c": sp.closed})
        })
        .collect()
}

fn color_arr(c: animara_document::Color) -> Value {
    json!([r(c.r as f64), r(c.g as f64), r(c.b as f64), 1])
}

fn fill_item(f: &Fill, m: &Affine, ct: &ColorTransform, rule: FillRule, warnings: &mut Vec<String>) -> Value {
    let rr = if rule == FillRule::EvenOdd { 2 } else { 1 };
    match f {
        Fill::Solid { color } => {
            let c = ct.apply(*color);
            json!({"ty": "fl", "c": st(color_arr(c)), "o": st(json!(r(c.a as f64 * 100.0))), "r": rr, "nm": "Fill"})
        }
        Fill::Linear { stops, matrix, .. } | Fill::Radial { stops, matrix, .. } => {
            let gm = *m * *matrix;
            let (s, e, t) = match f {
                Fill::Linear { .. } => (gm.apply(Vec2::new(-1.0, 0.0)), gm.apply(Vec2::new(1.0, 0.0)), 1),
                _ => (gm.apply(Vec2::ZERO), gm.apply(Vec2::new(1.0, 0.0)), 2),
            };
            let mut k = Vec::new();
            for s in stops {
                let c = ct.apply(s.color);
                k.extend([json!(r(s.offset as f64)), json!(r(c.r as f64)), json!(r(c.g as f64)), json!(r(c.b as f64))]);
            }
            for s in stops {
                k.extend([json!(r(s.offset as f64)), json!(r(ct.apply(s.color).a as f64))]);
            }
            json!({"ty": "gf", "o": st(json!(100)), "r": rr, "s": st(json!([r(s.x), r(s.y)])), "e": st(json!([r(e.x), r(e.y)])), "t": t,
                   "g": {"p": stops.len(), "k": st(Value::Array(k))}, "nm": "Gradient Fill"})
        }
        Fill::Bitmap { .. } => {
            warnings.push("bitmap fills are exported to Lottie as solid gray".into());
            json!({"ty": "fl", "c": st(json!([0.5, 0.5, 0.5, 1])), "o": st(json!(100)), "r": rr})
        }
    }
}

fn shape_group(s: &Shape, m: &Affine, ct: &ColorTransform, warnings: &mut Vec<String>) -> Vec<Value> {
    let mut out = Vec::new();
    let scale = m.mean_scale();
    for sp in s.paths.iter().rev() {
        let world = sp.path.transform(m);
        let mut it: Vec<Value> = path_value(&world).into_iter().map(|k| json!({"ty": "sh", "ks": st(k), "nm": "Path"})).collect();
        if it.is_empty() {
            continue;
        }
        if let Some(stroke) = &sp.stroke {
            let c = ct.apply(stroke.fill.primary_color());
            it.push(json!({"ty": "st", "c": st(color_arr(c)), "o": st(json!(r(c.a as f64 * 100.0))),
                "w": st(json!(r(stroke.width * if stroke.scale_with_object { scale } else { 1.0 }))),
                "lc": match stroke.cap { LineCap::Butt => 1, LineCap::Round => 2, LineCap::Square => 3 },
                "lj": match stroke.join { LineJoin::Miter => 1, LineJoin::Round => 2, LineJoin::Bevel => 3 },
                "ml": stroke.miter_limit, "nm": "Stroke"}));
        }
        if let Some(f) = &sp.fill {
            it.push(fill_item(f, m, ct, sp.fill_rule, warnings));
        }
        it.push(json!({"ty": "tr", "p": st(json!([0, 0])), "a": st(json!([0, 0])), "s": st(json!([100, 100])), "r": st(json!(0)), "o": st(json!(100)), "sk": st(json!(0)), "sa": st(json!(0)), "nm": "Transform"}));
        out.push(json!({"ty": "gr", "it": it, "nm": "Group"}));
    }
    out
}

fn blend_code(b: BlendMode) -> u8 {
    match b {
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::HardLight => 8,
        BlendMode::Difference | BlendMode::Invert => 10,
        BlendMode::Subtract => 11,
        BlendMode::Add => 16,
        _ => 0,
    }
}

struct Ctx<'a> {
    doc: &'a Document,
    assets: Vec<(animara_document::Id, Value)>,
    warnings: Vec<String>,
}

impl Ctx<'_> {
    /// Flatten items into shape values (no layer split).
    fn shapes_of(&mut self, items: &[DisplayItem], out: &mut Vec<Value>) {
        for it in items {
            match it {
                DisplayItem::Shape { shape, transform, color, .. } => {
                    let mut v = shape_group(shape, transform, color, &mut self.warnings);
                    v.reverse();
                    out.extend(v);
                }
                DisplayItem::Text { text, transform, color, .. } => {
                    let shape = animara_text::text_to_shape(text);
                    let mut v = shape_group(&shape, transform, color, &mut self.warnings);
                    v.reverse();
                    out.extend(v);
                }
                DisplayItem::Group { items, filters, .. } => {
                    if !filters.is_empty() {
                        self.warnings.push("nested filters inside composited groups are not exported to Lottie".into());
                    }
                    self.shapes_of(items, out);
                }
                DisplayItem::Masked { content, .. } => {
                    self.warnings.push("nested masks are flattened without masking in Lottie".into());
                    self.shapes_of(content, out);
                }
                DisplayItem::Bitmap { .. } => self.warnings.push("bitmaps nested in groups/masks are skipped in Lottie".into()),
            }
        }
    }

    fn asset_for(&mut self, id: animara_document::Id) -> Option<String> {
        if let Some(i) = self.assets.iter().position(|(a, _)| *a == id) {
            return Some(format!("img_{i}"));
        }
        let b = self.doc.library.bitmap(id)?;
        let mime = if b.format == "jpg" { "jpeg" } else { b.format.as_str() };
        let data = base64::engine::general_purpose::STANDARD.encode(b.data.as_slice());
        let name = format!("img_{}", self.assets.len());
        self.assets
            .push((id, json!({"id": name, "w": b.width, "h": b.height, "u": "", "p": format!("data:image/{mime};base64,{data}"), "e": 1})));
        Some(name)
    }

    /// Split top-level items into Lottie layers (bottom to top).
    fn specs(&mut self, items: &[DisplayItem], view: &Affine) -> Vec<LayerSpec> {
        let mut out = Vec::new();
        let mut cur = LayerSpec::empty();
        for it in items {
            let it = animara_animation::transform_item(it.clone(), view);
            match &it {
                DisplayItem::Group { items, filters, blend, scale, .. } if !filters.is_empty() || *blend != BlendMode::Normal => {
                    if !cur.is_empty() {
                        out.push(std::mem::replace(&mut cur, LayerSpec::empty()));
                    }
                    let mut g = LayerSpec::empty();
                    self.shapes_of(items, &mut g.shapes);
                    g.bm = blend_code(*blend);
                    for f in filters {
                        match f {
                            Filter::Blur { blur_x, blur_y, .. } => {
                                g.effects.push(json!({"ty": 29, "nm": "Gaussian Blur", "np": 5, "ix": 1, "en": 1, "ef": [
                                {"ty": 0, "nm": "Blurriness", "ix": 1, "v": st(json!(r((blur_x + blur_y) / 2.0 * scale)))},
                                {"ty": 7, "nm": "Blur Dimensions", "ix": 2, "v": st(json!(1))},
                                {"ty": 7, "nm": "Repeat Edge Pixels", "ix": 3, "v": st(json!(0))}]}))
                            }
                            other => self.warnings.push(format!("{} filter is not exported to Lottie", other.name())),
                        }
                    }
                    out.push(g);
                }
                DisplayItem::Masked { mask, content } => {
                    if !cur.is_empty() {
                        out.push(std::mem::replace(&mut cur, LayerSpec::empty()));
                    }
                    let mut g = LayerSpec::empty();
                    self.shapes_of(content, &mut g.shapes);
                    let mut mshapes = Vec::new();
                    self.shapes_of(mask, &mut mshapes);
                    for m in mask {
                        if let DisplayItem::Shape { shape, transform, .. } = m {
                            for sp in &shape.paths {
                                for pv in path_value(&sp.path.transform(transform)) {
                                    g.masks.push(json!({"inv": false, "mode": "a", "pt": st(pv), "o": st(json!(100)), "x": st(json!(0)), "nm": "Mask"}));
                                }
                            }
                        }
                    }
                    if g.masks.is_empty() {
                        self.warnings.push("only shape masks are exported to Lottie".into());
                    }
                    out.push(g);
                }
                DisplayItem::Bitmap { bitmap, transform, color, .. } => {
                    if !cur.is_empty() {
                        out.push(std::mem::replace(&mut cur, LayerSpec::empty()));
                    }
                    if let Some(name) = self.asset_for(bitmap.asset) {
                        let mut g = LayerSpec::empty();
                        g.image = Some((name, *transform, color.mul[3] as f64));
                        out.push(g);
                    }
                }
                other => self.shapes_of(std::slice::from_ref(other), &mut cur.shapes),
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    }
}

fn identity_ks(opacity: f64) -> Value {
    json!({"o": st(json!(r(opacity * 100.0))), "r": st(json!(0)), "p": st(json!([0, 0, 0])), "a": st(json!([0, 0, 0])), "s": st(json!([100, 100, 100]))})
}

/// Build the Lottie JSON value; returns `(json, warnings)`.
pub fn lottie_json(doc: &Document, opts: &ExportOptions) -> (Value, Vec<String>) {
    let (a, b) = opts.frames(doc);
    let mut ctx = Ctx { doc, assets: Vec::new(), warnings: Vec::new() };
    let mut frames: Vec<Vec<LayerSpec>> = Vec::new();
    for g in a..=b {
        let (s, f) = scene_at_global_frame(doc, g);
        let dl = evaluate_document_frame(doc, s, f, &EvalOptions::export());
        frames.push(ctx.specs(&dl.items, &dl.view));
    }
    // Merge runs of identical frames.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for i in 0..frames.len() {
        match runs.last_mut() {
            Some((_, end)) if frames[*end - 1] == frames[i] => *end = i + 1,
            _ => runs.push((i, i + 1)),
        }
    }
    let mut layers = Vec::new();
    let mut ind = 1;
    for &(s, e) in &runs {
        for spec in frames[s].iter().rev() {
            let mut l = match &spec.image {
                Some((refid, m, opacity)) => {
                    let p = m.decompose();
                    json!({"ddd": 0, "ind": ind, "ty": 2, "nm": "Bitmap", "refId": refid, "sr": 1,
                        "ks": {"o": st(json!(r(opacity * 100.0))), "r": st(json!(r(p.skew_x.to_degrees()))), "p": st(json!([r(p.x), r(p.y), 0])),
                               "a": st(json!([0, 0, 0])), "s": st(json!([r(p.scale_x * 100.0), r(p.scale_y * 100.0), 100]))}})
                }
                None => {
                    json!({"ddd": 0, "ind": ind, "ty": 4, "nm": format!("Frames {}-{}", s + 1, e), "sr": 1, "ks": identity_ks(1.0), "shapes": spec.shapes})
                }
            };
            let obj = l.as_object_mut().unwrap();
            obj.insert("ao".into(), json!(0));
            obj.insert("ip".into(), json!(s));
            obj.insert("op".into(), json!(e));
            obj.insert("st".into(), json!(0));
            obj.insert("bm".into(), json!(spec.bm));
            if !spec.masks.is_empty() {
                obj.insert("hasMask".into(), json!(true));
                obj.insert("masksProperties".into(), Value::Array(spec.masks.clone()));
            }
            if !spec.effects.is_empty() {
                obj.insert("ef".into(), Value::Array(spec.effects.clone()));
            }
            layers.push(l);
            ind += 1;
        }
    }
    let mut warnings = ctx.warnings;
    warnings.sort();
    warnings.dedup();
    let mut root = json!({
        "v": "5.7.4", "fr": doc.fps, "ip": 0, "op": frames.len(), "w": doc.width.round(), "h": doc.height.round(),
        "nm": doc.name, "ddd": 0, "assets": ctx.assets.into_iter().map(|(_, v)| v).collect::<Vec<_>>(), "layers": layers,
        "meta": {"g": "Animara", "a": doc.metadata.author, "d": doc.metadata.description}
    });
    if !opts.transparent {
        // Background as the bottom solid-shape layer.
        let bg = json!({"ddd": 0, "ind": 0, "ty": 4, "nm": "Background", "sr": 1, "ks": identity_ks(1.0), "ao": 0, "ip": 0, "op": frames.len(), "st": 0, "bm": 0,
            "shapes": shape_group(&Shape { paths: vec![animara_document::ShapePath::filled(Path::rect(animara_geometry::Rect::new(0.0, 0.0, doc.width, doc.height)), Fill::solid(doc.background))] }, &Affine::IDENTITY, &ColorTransform::IDENTITY, &mut Vec::new())});
        root["layers"].as_array_mut().unwrap().push(bg);
    }
    (root, warnings)
}

pub fn export_lottie(doc: &Document, path: &std::path::Path, opts: &ExportOptions) -> Result<Vec<String>, ExportError> {
    let (v, w) = lottie_json(doc, opts);
    std::fs::write(path, serde_json::to_string(&v).map_err(|e| ExportError::Encode(e.to_string()))?)?;
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lottie_structure() {
        let doc = crate::test_doc::sample();
        let (v, _) = lottie_json(&doc, &ExportOptions::default());
        assert_eq!(v["op"], 12);
        assert_eq!(v["fr"], 12.0);
        let layers = v["layers"].as_array().unwrap();
        assert!(layers.len() >= 12, "tweened frames become separate runs");
        for l in layers {
            let (ip, op) = (l["ip"].as_u64().unwrap(), l["op"].as_u64().unwrap());
            assert!(ip < op);
            if l["ty"] == 4 {
                for g in l["shapes"].as_array().unwrap() {
                    assert_eq!(g["ty"], "gr");
                    let it = g["it"].as_array().unwrap();
                    assert_eq!(it.last().unwrap()["ty"], "tr");
                    let sh = &it[0]["ks"]["k"];
                    assert_eq!(sh["v"].as_array().unwrap().len(), sh["i"].as_array().unwrap().len());
                }
            }
        }
        // Gradient fill present.
        assert!(serde_json::to_string(&v).unwrap().contains("\"gf\""));
    }
}
