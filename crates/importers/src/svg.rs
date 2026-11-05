//! SVG import via usvg: paths, fills, strokes, gradients, group transforms/opacity,
//! embedded raster images and text (converted to outlines).

use crate::{ImportError, import_image_bytes};
use animara_document::{
    BitmapAsset, BitmapInstance, Color, Element, ElementKind, Fill, GradientStop, LineCap, LineJoin, ShapePath, SpreadMode, Stroke,
};
use animara_geometry::{Affine, FillRule, Path, Vec2};
use std::sync::{Arc, OnceLock};

pub struct SvgImport {
    pub elements: Vec<Element>,
    pub bitmaps: Vec<BitmapAsset>,
    pub width: f64,
    pub height: f64,
    pub warnings: Vec<String>,
}

fn affine(t: usvg::Transform) -> Affine {
    Affine::new(t.sx as f64, t.ky as f64, t.kx as f64, t.sy as f64, t.tx as f64, t.ty as f64)
}

fn convert_path(p: &usvg::tiny_skia_path::Path) -> Path {
    use usvg::tiny_skia_path::PathSegment;
    let mut out = Path::new();
    let v = |p: usvg::tiny_skia_path::Point| Vec2::new(p.x as f64, p.y as f64);
    for seg in p.segments() {
        match seg {
            PathSegment::MoveTo(a) => {
                out.move_to(v(a));
            }
            PathSegment::LineTo(a) => {
                out.line_to(v(a));
            }
            PathSegment::QuadTo(c, a) => {
                out.quad_to(v(c), v(a));
            }
            PathSegment::CubicTo(c1, c2, a) => {
                out.cubic_to(v(c1), v(c2), v(a));
            }
            PathSegment::Close => {
                out.close();
            }
        }
    }
    out
}

fn color(c: usvg::Color, opacity: f32) -> Color {
    Color::from_rgba8(c.red, c.green, c.blue, (opacity.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn spread(s: usvg::SpreadMethod) -> SpreadMode {
    match s {
        usvg::SpreadMethod::Pad => SpreadMode::Pad,
        usvg::SpreadMethod::Reflect => SpreadMode::Reflect,
        usvg::SpreadMethod::Repeat => SpreadMode::Repeat,
    }
}

fn paint(p: &usvg::Paint, opacity: f32, warnings: &mut Vec<String>) -> Option<Fill> {
    let stops = |s: &[usvg::Stop]| -> Vec<GradientStop> {
        s.iter().map(|st| GradientStop { offset: st.offset().get(), color: color(st.color(), st.opacity().get() * opacity) }).collect()
    };
    match p {
        usvg::Paint::Color(c) => Some(Fill::solid(color(*c, opacity))),
        usvg::Paint::LinearGradient(g) => {
            // Map canonical (-1,0)..(1,0) onto (x1,y1)..(x2,y2) then apply the gradient transform.
            let a = Vec2::new(g.x1() as f64, g.y1() as f64);
            let b = Vec2::new(g.x2() as f64, g.y2() as f64);
            let mid = (a + b) * 0.5;
            let d = (b - a) * 0.5;
            let m = Affine::new(d.x, d.y, -d.y, d.x, mid.x, mid.y);
            Some(Fill::Linear { stops: stops(g.stops()), matrix: affine(g.transform()) * m, spread: spread(g.spread_method()) })
        }
        usvg::Paint::RadialGradient(g) => {
            let r = g.r().get() as f64;
            let c = Vec2::new(g.cx() as f64, g.cy() as f64);
            let focal = if r > 0.0 { (g.fx() as f64 - c.x) / r } else { 0.0 };
            let m = Affine::translate(c.x, c.y) * Affine::scale(r, r);
            Some(Fill::Radial { stops: stops(g.stops()), matrix: affine(g.transform()) * m, spread: spread(g.spread_method()), focal })
        }
        usvg::Paint::Pattern(_) => {
            warnings.push("SVG pattern paint is not supported; replaced with a gray fill".into());
            Some(Fill::solid(Color::rgba(0.5, 0.5, 0.5, opacity)))
        }
    }
}

fn walk(g: &usvg::Group, parent_opacity: f32, out: &mut SvgImport) {
    let opacity = parent_opacity * g.opacity().get();
    if g.clip_path().is_some() || g.mask().is_some() {
        out.warnings.push(format!("SVG clip-path/mask on group '{}' was ignored", g.id()));
    }
    if !g.filters().is_empty() {
        out.warnings.push(format!("SVG filters on group '{}' were ignored", g.id()));
    }
    for node in g.children() {
        match node {
            usvg::Node::Group(child) => walk(child, opacity, out),
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let path = convert_path(p.data());
                let fill = p.fill().and_then(|f| paint(f.paint(), f.opacity().get() * opacity, &mut out.warnings));
                let rule = match p.fill().map(|f| f.rule()) {
                    Some(usvg::FillRule::EvenOdd) => FillRule::EvenOdd,
                    _ => FillRule::NonZero,
                };
                let stroke = p.stroke().and_then(|s| {
                    let f = paint(s.paint(), s.opacity().get() * opacity, &mut out.warnings)?;
                    Some(Stroke {
                        fill: f,
                        width: s.width().get() as f64,
                        cap: match s.linecap() {
                            usvg::LineCap::Butt => LineCap::Butt,
                            usvg::LineCap::Round => LineCap::Round,
                            usvg::LineCap::Square => LineCap::Square,
                        },
                        join: match s.linejoin() {
                            usvg::LineJoin::Round => LineJoin::Round,
                            usvg::LineJoin::Bevel => LineJoin::Bevel,
                            _ => LineJoin::Miter,
                        },
                        miter_limit: s.miterlimit().get() as f64,
                        style: match s.dasharray() {
                            Some(d) if d.len() >= 2 => animara_document::StrokeStyle::Dashed { dash: d[0] as f64, gap: d[1] as f64 },
                            _ => animara_document::StrokeStyle::Solid,
                        },
                        scale_with_object: true,
                    })
                });
                if fill.is_none() && stroke.is_none() {
                    continue;
                }
                let sp = ShapePath { path, fill, stroke, fill_rule: rule, primitive: None };
                out.elements.push(Element::shape(vec![sp]).with_transform(affine(p.abs_transform())));
            }
            usvg::Node::Image(img) => {
                let data = match img.kind() {
                    usvg::ImageKind::PNG(d) | usvg::ImageKind::JPEG(d) | usvg::ImageKind::GIF(d) | usvg::ImageKind::WEBP(d) => {
                        Some(d.clone())
                    }
                    usvg::ImageKind::SVG(_) => {
                        out.warnings.push("nested SVG image was ignored".into());
                        None
                    }
                };
                if let Some(d) = data {
                    match import_image_bytes(&format!("svg image {}", out.bitmaps.len() + 1), d.as_ref().clone()) {
                        Ok(asset) => {
                            let size = img.size();
                            let sx = size.width() as f64 / asset.width.max(1) as f64;
                            let sy = size.height() as f64 / asset.height.max(1) as f64;
                            let e = Element::new(ElementKind::Bitmap(BitmapInstance { asset: asset.id }))
                                .with_transform(affine(img.abs_transform()) * Affine::scale(sx, sy));
                            out.bitmaps.push(asset);
                            out.elements.push(e);
                        }
                        Err(e) => out.warnings.push(format!("embedded image could not be decoded: {e}")),
                    }
                }
            }
            usvg::Node::Text(t) => walk(t.flattened(), opacity, out),
        }
    }
}

fn options() -> &'static usvg::Options<'static> {
    static OPT: OnceLock<usvg::Options<'static>> = OnceLock::new();
    OPT.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        usvg::Options { fontdb: Arc::new(db), ..Default::default() }
    })
}

pub fn import_svg_data(data: &[u8]) -> Result<SvgImport, ImportError> {
    let tree = usvg::Tree::from_data(data, options()).map_err(|e| ImportError::Svg(e.to_string()))?;
    let size = tree.size();
    let mut out = SvgImport {
        elements: Vec::new(),
        bitmaps: Vec::new(),
        width: size.width() as f64,
        height: size.height() as f64,
        warnings: Vec::new(),
    };
    walk(tree.root(), 1.0, &mut out);
    out.warnings.dedup();
    Ok(out)
}

pub fn import_svg(path: &std::path::Path) -> Result<SvgImport, ImportError> {
    import_svg_data(&std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_shapes_and_gradients() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50">
          <defs><linearGradient id="g"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient></defs>
          <g transform="translate(10,5)" opacity="0.5">
            <rect width="20" height="10" fill="#ff0000"/>
            <circle cx="50" cy="20" r="10" fill="url(#g)" stroke="blue" stroke-width="2"/>
          </g></svg>"##;
        let r = import_svg_data(svg).unwrap();
        assert_eq!((r.width, r.height), (100.0, 50.0));
        assert_eq!(r.elements.len(), 2);
        let ElementKind::Shape(s) = &r.elements[0].kind else { panic!() };
        let c = s.paths[0].fill.as_ref().unwrap().primary_color();
        assert!((c.a - 0.5).abs() < 0.01 && c.r > 0.99);
        assert!((r.elements[0].transform.tx - 10.0).abs() < 1e-6);
        let ElementKind::Shape(s2) = &r.elements[1].kind else { panic!() };
        assert!(matches!(s2.paths[0].fill, Some(Fill::Linear { .. })));
        assert!(s2.paths[0].stroke.is_some());
    }
}
