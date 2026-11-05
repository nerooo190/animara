use animara_document::{
    CameraState, ColorEffect, ColorTransform, Document, Element, ElementKind, Fill, Keyframe, Layer, LayerKind, Pose, RotateMode, Shape,
    ShapePath, Stroke, Timeline, Tween, TweenKind,
};
use animara_geometry::{Affine, Morph, Path, TransformParts, Vec2, lerp};
use std::f64::consts::{PI, TAU};

/// Elements of layer `idx` at `frame` with tweens, motion guides and armature
/// poses applied, plus the frame offset inside the current keyframe span
/// (used to synchronise graphic symbols).
pub fn layer_elements_at(doc: &Document, tl: &Timeline, idx: usize, frame: u32) -> (Vec<Element>, u32) {
    let layer = &tl.layers[idx];
    let Some(i) = layer.span_index(frame) else { return (Vec::new(), 0) };
    let kf = &layer.frames[i];
    let offset = frame - kf.start;
    let next = layer.frames.get(i + 1);
    let mut elements = match (&kf.tween, next) {
        (Some(tw), Some(nk)) if offset > 0 && !nk.elements.is_empty() && !kf.elements.is_empty() => {
            let t_lin = offset as f64 / kf.duration as f64;
            match tw.kind {
                TweenKind::Shape => shape_tween(kf, nk, tw.ease.eval(t_lin)),
                TweenKind::Classic | TweenKind::Motion => {
                    let guide = guide_path(doc, tl, layer, kf.start);
                    classic_tween(kf, nk, tw, t_lin, guide.as_ref())
                }
            }
        }
        (Some(tw), None) if offset > 0 && tw.kind == TweenKind::Motion && tw.motion_path.is_some() => {
            // A motion tween without an end keyframe follows its path over the span.
            let t_lin = offset as f64 / (kf.duration.max(2) - 1) as f64;
            let path = tw.motion_path.as_ref().unwrap();
            let t = tw.ease_for("position").eval(t_lin);
            kf.elements
                .iter()
                .map(|e| {
                    let mut e = e.clone();
                    if let (Some((p, ang)), Some((p0, ang0))) = (path.sample(t), path.sample(0.0)) {
                        let pw = e.pivot_world() + (p - p0);
                        let mut m = Affine::translate(pw.x, pw.y);
                        if tw.orient_to_path {
                            m = m * Affine::rotate(ang - ang0);
                        }
                        let lin = Affine { tx: 0.0, ty: 0.0, ..e.transform };
                        e.transform = m * lin * Affine::translate(-e.pivot.x, -e.pivot.y);
                    }
                    e
                })
                .collect()
        }
        _ => kf.elements.clone(),
    };
    if layer.kind == LayerKind::Armature
        && let Some(arm) = &layer.armature
    {
        let pose = pose_at(layer, frame).unwrap_or_else(|| animara_rigging::rest_pose(arm));
        let rest = animara_rigging::forward(arm, &animara_rigging::rest_pose(arm));
        let posed = animara_rigging::forward(arm, &pose);
        for (bi, bone) in arm.bones.iter().enumerate() {
            if let Some(eid) = bone.bound_element
                && let Some(e) = elements.iter_mut().find(|e| e.id == eid)
            {
                e.transform = animara_rigging::binding_transform(&rest, &posed, bi) * e.transform;
            }
        }
    }
    (elements, offset)
}

/// Interpolated armature pose at `frame` (pose keyframes always interpolate).
pub fn pose_at(layer: &Layer, frame: u32) -> Option<Pose> {
    let i = layer.span_index(frame)?;
    let kf = &layer.frames[i];
    let a = kf.pose.clone()?;
    match layer.frames.get(i + 1).and_then(|n| n.pose.as_ref()) {
        Some(b) => {
            let t = (frame - kf.start) as f64 / kf.duration as f64;
            let ease = kf.tween.as_ref().map(|tw| tw.ease.eval(t)).unwrap_or(t);
            Some(animara_rigging::lerp_pose(&a, b, ease))
        }
        None => Some(a),
    }
}

/// Camera state of a camera layer at `frame`.
pub fn camera_at(layer: &Layer, frame: u32) -> Option<CameraState> {
    let i = layer.span_index(frame).or_else(|| layer.frames.len().checked_sub(1))?;
    let kf = &layer.frames[i];
    let a = kf.camera?;
    if let (Some(tw), Some(b)) = (&kf.tween, layer.frames.get(i + 1).and_then(|n| n.camera)) {
        let t = (frame.saturating_sub(kf.start)) as f64 / kf.duration as f64;
        return Some(a.lerp(&b, tw.ease.eval(t)));
    }
    Some(a)
}

fn guide_path(doc: &Document, tl: &Timeline, layer: &Layer, frame: u32) -> Option<Path> {
    let gid = layer.guide_layer?;
    let gi = tl.layer_index(gid)?;
    let (els, _) = layer_elements_at(doc, tl, gi, frame);
    els.iter().find_map(|e| match &e.kind {
        ElementKind::Shape(s) => s.paths.iter().find(|p| !p.path.is_empty()).map(|p| p.path.transform(&e.transform)),
        _ => None,
    })
}

fn matching<'a>(a: &Element, idx: usize, b: &'a [Element]) -> Option<&'a Element> {
    if let Some(e) = b.iter().find(|e| e.id == a.id) {
        return Some(e);
    }
    let cand = b.get(idx)?;
    let same = match (&a.kind, &cand.kind) {
        (ElementKind::Instance(x), ElementKind::Instance(y)) => x.symbol == y.symbol,
        (ElementKind::Shape(_), ElementKind::Shape(_)) => true,
        (ElementKind::Text(_), ElementKind::Text(_)) => true,
        (ElementKind::Bitmap(x), ElementKind::Bitmap(y)) => x.asset == y.asset,
        (ElementKind::Group { .. }, ElementKind::Group { .. }) => true,
        _ => false,
    };
    same.then_some(cand)
}

fn norm_angle(mut a: f64) -> f64 {
    while a > PI {
        a -= TAU;
    }
    while a <= -PI {
        a += TAU;
    }
    a
}

/// Linear part (rotation/scale/skew) of an element transform, as decomposed parts.
fn linear_parts(m: &Affine) -> TransformParts {
    let mut p = m.decompose();
    p.x = 0.0;
    p.y = 0.0;
    p
}

pub(crate) fn interpolate_transform(a: &Element, b: &Element, t: f64, tw: &Tween) -> (Affine, Vec2) {
    let pa = linear_parts(&a.transform);
    let pb = linear_parts(&b.transform);
    let t_rot = tw.ease_for("rotation").eval(t);
    let t_scale = tw.ease_for("scale").eval(t);
    let d = norm_angle(pb.skew_x - pa.skew_x);
    let delta = match tw.rotate {
        RotateMode::Auto => d,
        RotateMode::None => 0.0,
        RotateMode::Clockwise => {
            let d = if d < 0.0 { d + TAU } else { d };
            d + TAU * tw.rotate_times as f64
        }
        RotateMode::CounterClockwise => {
            let d = if d > 0.0 { d - TAU } else { d };
            d - TAU * tw.rotate_times as f64
        }
    };
    let skew_a = norm_angle(pa.skew_y - pa.skew_x);
    let skew_b = norm_angle(pb.skew_y - pb.skew_x);
    let rot = pa.skew_x + delta * t_rot;
    let (sx, sy) =
        if tw.scale { (lerp(pa.scale_x, pb.scale_x, t_scale), lerp(pa.scale_y, pb.scale_y, t_scale)) } else { (pa.scale_x, pa.scale_y) };
    let parts = TransformParts { x: 0.0, y: 0.0, scale_x: sx, scale_y: sy, skew_x: rot, skew_y: rot + lerp(skew_a, skew_b, t_rot) };
    let pivot = a.pivot.lerp(b.pivot, t);
    let lin = Affine::compose(&parts);
    let t_pos = tw.ease_for("position").eval(t);
    let pw = a.pivot_world().lerp(b.pivot_world(), t_pos);
    (Affine::translate(pw.x, pw.y) * lin * Affine::translate(-pivot.x, -pivot.y), pivot)
}

fn effect_lerp(a: &ColorEffect, b: &ColorEffect, t: f64) -> ColorEffect {
    if *a == ColorEffect::None && *b == ColorEffect::None {
        return ColorEffect::None;
    }
    if a == b {
        return *a;
    }
    let ct: ColorTransform = a.to_transform().lerp(&b.to_transform(), t);
    ColorEffect::Advanced { transform: ct }
}

fn classic_tween(a: &Keyframe, b: &Keyframe, tw: &Tween, t_lin: f64, guide: Option<&Path>) -> Vec<Element> {
    let t = tw.ease.eval(t_lin);
    a.elements
        .iter()
        .enumerate()
        .map(|(idx, ea)| {
            let Some(eb) = matching(ea, idx, &b.elements) else { return ea.clone() };
            let mut e = ea.clone();
            let (mut m, pivot) = interpolate_transform(ea, eb, t_lin, tw);
            e.pivot = pivot;
            if let Some(path) = guide
                && let Some((pt, ang)) = path.sample(tw.ease_for("position").eval(t_lin))
            {
                let lin = Affine { tx: 0.0, ty: 0.0, ..m };
                let rot = if tw.orient_to_path { Affine::rotate(ang) } else { Affine::IDENTITY };
                m = Affine::translate(pt.x, pt.y) * rot * lin * Affine::translate(-pivot.x, -pivot.y);
            }
            e.transform = m;
            if let (ElementKind::Instance(ia), ElementKind::Instance(ib)) = (&mut e.kind, &eb.kind) {
                let tc = tw.ease_for("color").eval(t_lin);
                ia.color_effect = effect_lerp(&ia.color_effect, &ib.color_effect, tc);
                if ia.filters.len() == ib.filters.len() {
                    let tf = tw.ease_for("filters").eval(t_lin);
                    ia.filters = ia.filters.iter().zip(&ib.filters).map(|(fa, fb)| fa.lerp(fb, tf)).collect();
                }
            }
            if let (ElementKind::Text(ta), ElementKind::Text(tb)) = (&mut e.kind, &eb.kind) {
                let ca = ta.style().color;
                let cb = tb.style().color;
                let c = ca.lerp(cb, t);
                ta.set_style_all(|s| s.color = c);
            }
            e
        })
        .collect()
}

fn collapsed(p: &ShapePath) -> ShapePath {
    let c = p.path.bounds().center();
    let c = if c.is_finite() { c } else { Vec2::ZERO };
    let mut q = p.clone();
    q.path = Path::polyline(&[c, c, c], true);
    q
}

fn world_paths(els: &[Element]) -> Vec<ShapePath> {
    let mut out = Vec::new();
    for e in els {
        if let ElementKind::Shape(s) = &e.kind {
            out.extend(s.transformed(&e.transform).paths);
        }
    }
    out
}

fn stroke_lerp(a: &Option<Stroke>, b: &Option<Stroke>, t: f64) -> Option<Stroke> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.lerp(y, t)),
        (Some(x), None) => {
            let mut s = x.clone();
            s.width = lerp(x.width, 0.0, t);
            (s.width > 0.01).then_some(s)
        }
        (None, Some(y)) => {
            let mut s = y.clone();
            s.width = lerp(0.0, y.width, t);
            (s.width > 0.01).then_some(s)
        }
        (None, None) => None,
    }
}

fn fill_lerp(a: &Option<Fill>, b: &Option<Fill>, t: f64) -> Option<Fill> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.lerp(y, t)),
        (Some(x), None) => Some(x.lerp(&Fill::solid(x.primary_color().with_alpha(0.0)), t)),
        (None, Some(y)) => Some(Fill::solid(y.primary_color().with_alpha(0.0)).lerp(y, t)),
        (None, None) => None,
    }
}

fn shape_tween(a: &Keyframe, b: &Keyframe, t: f64) -> Vec<Element> {
    let pa = world_paths(&a.elements);
    let pb = world_paths(&b.elements);
    let n = pa.len().max(pb.len());
    let mut paths = Vec::with_capacity(n);
    for i in 0..n {
        let sa = pa.get(i).cloned().unwrap_or_else(|| collapsed(&pb[i]));
        let sb = pb.get(i).cloned().unwrap_or_else(|| collapsed(&pa[i]));
        let path = if t <= 0.0 {
            sa.path.clone()
        } else if t >= 1.0 {
            sb.path.clone()
        } else {
            Morph::new(&sa.path, &sb.path).at(t)
        };
        paths.push(ShapePath {
            path,
            fill: fill_lerp(&sa.fill, &sb.fill, t),
            stroke: stroke_lerp(&sa.stroke, &sb.stroke, t),
            fill_rule: sa.fill_rule,
            primitive: None,
        });
    }
    let mut out: Vec<Element> = a.elements.iter().filter(|e| !matches!(e.kind, ElementKind::Shape(_))).cloned().collect();
    let id = a.elements.iter().find(|e| matches!(e.kind, ElementKind::Shape(_))).map(|e| e.id);
    let mut shape = Element::new(ElementKind::Shape(Shape { paths }));
    if let Some(id) = id {
        shape.id = id;
    }
    out.insert(0, shape);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use animara_document::{Color, LayerKind, Symbol, SymbolInstance, SymbolKind};
    use animara_geometry::Rect;

    fn doc_with_tween(kind: TweenKind) -> Document {
        let mut doc = Document::new(400.0, 300.0, 24.0);
        let sym = Symbol::new("s", SymbolKind::Graphic);
        let sid = doc.library.add_symbol(sym);
        let layer = &mut doc.scenes[0].timeline.layers[0];
        let el = match kind {
            TweenKind::Shape => {
                Element::shape(vec![ShapePath::filled(Path::rect(Rect::new(0.0, 0.0, 10.0, 10.0)), Fill::solid(Color::BLACK))])
            }
            _ => Element::new(ElementKind::Instance(SymbolInstance::new(sid))),
        };
        layer.frames[0].elements.push(el);
        layer.insert_keyframe(10, None);
        match kind {
            TweenKind::Shape => {
                layer.frames[1].elements[0] =
                    Element::shape(vec![ShapePath::filled(Path::ellipse(Rect::new(100.0, 0.0, 140.0, 40.0)), Fill::solid(Color::WHITE))]);
            }
            _ => {
                layer.frames[1].elements[0].transform = Affine::translate(100.0, 50.0) * Affine::rotate(PI / 2.0);
            }
        }
        layer.set_tween(0, Some(Tween::new(kind)));
        doc
    }

    #[test]
    fn classic_tween_midpoint() {
        let doc = doc_with_tween(TweenKind::Classic);
        let tl = &doc.scenes[0].timeline;
        let (els, off) = layer_elements_at(&doc, tl, 0, 5);
        assert_eq!(off, 5);
        let m = els[0].transform;
        assert!((m.tx - 50.0).abs() < 1e-9 && (m.ty - 25.0).abs() < 1e-9, "{m:?}");
        let rot = m.decompose().skew_x;
        assert!((rot - PI / 4.0).abs() < 1e-9);
        let (end, _) = layer_elements_at(&doc, tl, 0, 10);
        assert!((end[0].transform.tx - 100.0).abs() < 1e-9);
    }

    #[test]
    fn clockwise_rotation_adds_turns() {
        let mut doc = doc_with_tween(TweenKind::Classic);
        let layer = &mut doc.scenes[0].timeline.layers[0];
        layer.frames[1].elements[0].transform = Affine::translate(0.0, 0.0);
        let mut tw = Tween::new(TweenKind::Classic);
        tw.rotate = RotateMode::Clockwise;
        tw.rotate_times = 1;
        layer.frames[0].tween = Some(tw);
        let (els, _) = layer_elements_at(&doc, &doc.scenes[0].timeline, 0, 5);
        // Halfway through one full clockwise turn = 180°.
        let r = els[0].transform.decompose().skew_x.abs();
        assert!((r - PI).abs() < 1e-6, "{r}");
    }

    #[test]
    fn shape_tween_midpoint_between_shapes() {
        let doc = doc_with_tween(TweenKind::Shape);
        let (els, _) = layer_elements_at(&doc, &doc.scenes[0].timeline, 0, 5);
        let ElementKind::Shape(s) = &els[0].kind else { panic!() };
        let b = s.bounds();
        assert!(b.min.x > 10.0 && b.max.x < 140.0, "{b:?}");
        let c = s.paths[0].fill.as_ref().unwrap().primary_color();
        assert!((c.r - 0.5).abs() < 0.01);
    }

    #[test]
    fn motion_guide_moves_along_path() {
        let mut doc = doc_with_tween(TweenKind::Classic);
        let tl = &mut doc.scenes[0].timeline;
        let gid = tl.add_layer("Guide", 0);
        tl.layers[0].kind = LayerKind::Guide;
        tl.layers[0].frames[0].elements.push(Element::shape(vec![ShapePath::stroked(
            Path::line(Vec2::new(0.0, 200.0), Vec2::new(200.0, 200.0)),
            Stroke::solid(Color::BLACK, 1.0),
        )]));
        tl.layers[1].guide_layer = Some(gid);
        let (els, _) = layer_elements_at(&doc, &doc.scenes[0].timeline, 1, 5);
        let p = els[0].pivot_world();
        assert!((p.y - 200.0).abs() < 1e-6 && (p.x - 100.0).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn camera_interpolates() {
        let mut layer = Layer::new("Camera").with_kind(LayerKind::Camera);
        layer.frames[0].camera = Some(CameraState::centered(100.0, 100.0));
        layer.insert_keyframe(10, None);
        layer.frames[1].camera = Some(CameraState { x: 150.0, ..CameraState::centered(100.0, 100.0) });
        layer.frames[0].tween = Some(Tween::new(TweenKind::Classic));
        let c = camera_at(&layer, 5).unwrap();
        assert!((c.x - 100.0).abs() < 1e-9);
    }
}
