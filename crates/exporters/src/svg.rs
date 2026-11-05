//! SVG export of evaluated frames (vector paths, gradients, bitmaps, text as
//! outlines, filters, blend modes and masks).

use crate::{ExportError, ExportOptions};
use animara_animation::{DisplayItem, EvalOptions, evaluate_document_frame, scene_at_global_frame};
use animara_document::{BlendMode, Color, ColorTransform, Document, Fill, Filter, LineCap, LineJoin, Shape, SpreadMode, StrokeStyle};
use animara_geometry::{Affine, FillRule};
use base64::Engine as _;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

struct W<'a> {
    doc: &'a Document,
    defs: String,
    body: String,
    next: u32,
}

fn mat(m: &Affine) -> String {
    format!("matrix({} {} {} {} {} {})", fmt(m.a), fmt(m.b), fmt(m.c), fmt(m.d), fmt(m.tx), fmt(m.ty))
}

fn fmt(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn rgb(c: Color) -> String {
    let [r, g, b, _] = c.to_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl W<'_> {
    fn id(&mut self, p: &str) -> String {
        self.next += 1;
        format!("{p}{}", self.next)
    }

    /// Returns a paint attribute value and opacity.
    fn paint(&mut self, f: &Fill, ct: &ColorTransform, mask_white: bool) -> (String, f32) {
        if mask_white {
            return ("#ffffff".into(), 1.0);
        }
        match f {
            Fill::Solid { color } => {
                let c = ct.apply(*color);
                (rgb(c), c.a)
            }
            Fill::Linear { stops, matrix, spread } | Fill::Radial { stops, matrix, spread, .. } => {
                let id = self.id("g");
                let sm = match spread {
                    SpreadMode::Pad => "pad",
                    SpreadMode::Reflect => "reflect",
                    SpreadMode::Repeat => "repeat",
                };
                let mut st = String::new();
                for s in stops {
                    let c = ct.apply(s.color);
                    let _ = write!(
                        st,
                        "<stop offset=\"{}\" stop-color=\"{}\" stop-opacity=\"{}\"/>",
                        fmt(s.offset as f64),
                        rgb(c),
                        fmt(c.a as f64)
                    );
                }
                match f {
                    Fill::Linear { .. } => {
                        let _ = write!(
                            self.defs,
                            "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"-1\" y1=\"0\" x2=\"1\" y2=\"0\" spreadMethod=\"{sm}\" gradientTransform=\"{}\">{st}</linearGradient>",
                            mat(matrix)
                        );
                    }
                    Fill::Radial { focal, .. } => {
                        let _ = write!(
                            self.defs,
                            "<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"0\" cy=\"0\" r=\"1\" fx=\"{}\" fy=\"0\" spreadMethod=\"{sm}\" gradientTransform=\"{}\">{st}</radialGradient>",
                            fmt(*focal),
                            mat(matrix)
                        );
                    }
                    _ => unreachable!(),
                }
                (format!("url(#{id})"), 1.0)
            }
            Fill::Bitmap { asset, matrix, .. } => match self.doc.library.bitmap(*asset) {
                Some(b) => {
                    let id = self.id("p");
                    let data = base64::engine::general_purpose::STANDARD.encode(b.data.as_slice());
                    let mime = if b.format == "jpg" { "jpeg" } else { b.format.as_str() };
                    let _ = write!(
                        self.defs,
                        "<pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" width=\"{}\" height=\"{}\" patternTransform=\"{}\"><image width=\"{}\" height=\"{}\" xlink:href=\"data:image/{mime};base64,{data}\"/></pattern>",
                        b.width,
                        b.height,
                        mat(matrix),
                        b.width,
                        b.height
                    );
                    (format!("url(#{id})"), 1.0)
                }
                None => ("#808080".into(), 1.0),
            },
        }
    }

    fn shape(&mut self, s: &Shape, m: &Affine, ct: &ColorTransform, mask_white: bool) {
        for sp in &s.paths {
            if sp.path.is_empty() {
                continue;
            }
            let mut attrs = String::new();
            match &sp.fill {
                Some(f) => {
                    let (p, o) = self.paint(f, ct, mask_white);
                    let _ = write!(attrs, " fill=\"{p}\"");
                    if o < 1.0 {
                        let _ = write!(attrs, " fill-opacity=\"{}\"", fmt(o as f64));
                    }
                    if sp.fill_rule == FillRule::EvenOdd {
                        attrs.push_str(" fill-rule=\"evenodd\"");
                    }
                }
                None => attrs.push_str(" fill=\"none\""),
            }
            if let Some(st) = &sp.stroke
                && !mask_white
            {
                let (p, o) = self.paint(&st.fill, ct, false);
                let w = match st.style {
                    StrokeStyle::Hairline => 1.0 / m.mean_scale().max(1e-9),
                    _ => st.width,
                };
                let _ = write!(attrs, " stroke=\"{p}\" stroke-width=\"{}\"", fmt(w));
                if o < 1.0 {
                    let _ = write!(attrs, " stroke-opacity=\"{}\"", fmt(o as f64));
                }
                let _ = write!(
                    attrs,
                    " stroke-linecap=\"{}\" stroke-linejoin=\"{}\"",
                    match st.cap {
                        LineCap::Butt => "butt",
                        LineCap::Round => "round",
                        LineCap::Square => "square",
                    },
                    match st.join {
                        LineJoin::Miter => "miter",
                        LineJoin::Round => "round",
                        LineJoin::Bevel => "bevel",
                    }
                );
                match st.style {
                    StrokeStyle::Dashed { dash, gap } => {
                        let _ = write!(attrs, " stroke-dasharray=\"{} {}\"", fmt(dash), fmt(gap));
                    }
                    StrokeStyle::Dotted { spacing } => {
                        let _ = write!(attrs, " stroke-dasharray=\"0.01 {}\"", fmt(spacing + w));
                    }
                    _ => {}
                }
                if !st.scale_with_object || matches!(st.style, StrokeStyle::Hairline) {
                    attrs.push_str(" vector-effect=\"non-scaling-stroke\"");
                }
            }
            let _ = write!(self.body, "<path transform=\"{}\" d=\"{}\"{attrs}/>", mat(m), sp.path.to_svg());
        }
    }

    fn filter(&mut self, filters: &[Filter], scale: f64) -> Option<String> {
        if filters.is_empty() {
            return None;
        }
        let id = self.id("f");
        let mut f =
            format!("<filter id=\"{id}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\" color-interpolation-filters=\"sRGB\">");
        let mut input = "SourceGraphic".to_string();
        for (i, flt) in filters.iter().enumerate() {
            let out = format!("r{i}");
            match *flt {
                Filter::Blur { blur_x, blur_y, .. } => {
                    let _ = write!(
                        f,
                        "<feGaussianBlur in=\"{input}\" stdDeviation=\"{} {}\" result=\"{out}\"/>",
                        fmt(blur_x * scale / 2.0),
                        fmt(blur_y * scale / 2.0)
                    );
                }
                Filter::DropShadow { blur_x, blur_y, angle, distance, color, strength, .. } => {
                    let a = angle.to_radians();
                    let _ = write!(
                        f,
                        "<feDropShadow in=\"{input}\" dx=\"{}\" dy=\"{}\" stdDeviation=\"{} {}\" flood-color=\"{}\" flood-opacity=\"{}\" result=\"{out}\"/>",
                        fmt(a.cos() * distance * scale),
                        fmt(a.sin() * distance * scale),
                        fmt(blur_x * scale / 2.0),
                        fmt(blur_y * scale / 2.0),
                        rgb(color),
                        fmt((color.a * strength).min(1.0) as f64)
                    );
                }
                Filter::Glow { blur_x, blur_y, color, strength, .. } => {
                    let _ = write!(
                        f,
                        "<feDropShadow in=\"{input}\" dx=\"0\" dy=\"0\" stdDeviation=\"{} {}\" flood-color=\"{}\" flood-opacity=\"{}\" result=\"{out}\"/>",
                        fmt(blur_x * scale / 2.0),
                        fmt(blur_y * scale / 2.0),
                        rgb(color),
                        fmt((color.a * strength).min(1.0) as f64)
                    );
                }
                Filter::AdjustColor { saturation, hue, brightness, contrast } => {
                    let c = 1.0 + contrast as f64 / 100.0;
                    let b = brightness as f64 / 100.0 + 0.5 - 0.5 * c;
                    let _ = write!(
                        f,
                        "<feColorMatrix in=\"{input}\" type=\"saturate\" values=\"{}\" result=\"{out}a\"/><feColorMatrix in=\"{out}a\" type=\"hueRotate\" values=\"{}\" result=\"{out}b\"/><feComponentTransfer in=\"{out}b\" result=\"{out}\"><feFuncR type=\"linear\" slope=\"{c}\" intercept=\"{b}\"/><feFuncG type=\"linear\" slope=\"{c}\" intercept=\"{b}\"/><feFuncB type=\"linear\" slope=\"{c}\" intercept=\"{b}\"/></feComponentTransfer>",
                        fmt(1.0 + saturation as f64 / 100.0),
                        fmt(hue as f64)
                    );
                }
            }
            input = out;
        }
        f.push_str("</filter>");
        self.defs.push_str(&f);
        Some(id)
    }

    fn items(&mut self, items: &[DisplayItem], mask_white: bool) {
        for it in items {
            match it {
                DisplayItem::Shape { shape, transform, color, .. } => self.shape(shape, transform, color, mask_white),
                DisplayItem::Text { text, transform, color, .. } => {
                    let layout = animara_text::fonts().layout(text);
                    for (p, c) in layout.to_paths() {
                        let c = color.apply(c);
                        let fill = if mask_white { "#ffffff".to_string() } else { rgb(c) };
                        let _ = write!(
                            self.body,
                            "<path transform=\"{}\" d=\"{}\" fill=\"{fill}\" fill-opacity=\"{}\"/>",
                            mat(transform),
                            p.to_svg(),
                            fmt(if mask_white { 1.0 } else { c.a as f64 })
                        );
                    }
                }
                DisplayItem::Bitmap { bitmap, transform, color, .. } => {
                    if let Some(b) = self.doc.library.bitmap(bitmap.asset) {
                        let data = base64::engine::general_purpose::STANDARD.encode(b.data.as_slice());
                        let mime = if b.format == "jpg" { "jpeg" } else { b.format.as_str() };
                        let mut extra = String::new();
                        if !color.is_identity() {
                            let id = self.id("c");
                            let (m, a) = (color.mul, color.add);
                            let _ = write!(
                                self.defs,
                                "<filter id=\"{id}\"><feColorMatrix type=\"matrix\" values=\"{} 0 0 0 {} 0 {} 0 0 {} 0 0 {} 0 {} 0 0 0 {} {}\"/></filter>",
                                m[0], a[0], m[1], a[1], m[2], a[2], m[3], a[3]
                            );
                            extra = format!(" filter=\"url(#{id})\"");
                        }
                        let _ = write!(
                            self.body,
                            "<image transform=\"{}\" width=\"{}\" height=\"{}\" xlink:href=\"data:image/{mime};base64,{data}\"{extra}/>",
                            mat(transform),
                            b.width,
                            b.height
                        );
                    }
                }
                DisplayItem::Group { items, filters, blend, scale, .. } => {
                    let fid = if mask_white { None } else { self.filter(filters, *scale) };
                    let mut attrs = String::new();
                    if let Some(f) = fid {
                        let _ = write!(attrs, " filter=\"url(#{f})\"");
                    }
                    let mode = match blend {
                        BlendMode::Multiply => Some("multiply"),
                        BlendMode::Screen => Some("screen"),
                        BlendMode::Overlay => Some("overlay"),
                        BlendMode::HardLight => Some("hard-light"),
                        BlendMode::Lighten => Some("lighten"),
                        BlendMode::Darken => Some("darken"),
                        BlendMode::Difference | BlendMode::Invert => Some("difference"),
                        BlendMode::Add => Some("plus-lighter"),
                        BlendMode::Subtract => Some("exclusion"),
                        _ => None,
                    };
                    if let Some(m) = mode {
                        let _ = write!(attrs, " style=\"mix-blend-mode:{m}\"");
                    }
                    let _ = write!(self.body, "<g{attrs}>");
                    self.items(items, mask_white);
                    self.body.push_str("</g>");
                }
                DisplayItem::Masked { mask, content } => {
                    let id = self.id("m");
                    let saved = std::mem::take(&mut self.body);
                    self.items(mask, true);
                    let mask_body = std::mem::replace(&mut self.body, saved);
                    let _ = write!(self.defs, "<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\">{mask_body}</mask>");
                    let _ = write!(self.body, "<g mask=\"url(#{id})\">");
                    self.items(content, mask_white);
                    self.body.push_str("</g>");
                }
            }
        }
    }
}

/// SVG document for a scene frame.
pub fn svg_for_frame(doc: &Document, scene: usize, frame: u32, scale: f64, transparent: bool) -> String {
    let dl = evaluate_document_frame(doc, scene, frame, &EvalOptions::export());
    let mut w = W { doc, defs: String::new(), body: String::new(), next: 0 };
    w.items(&dl.items, false);
    let (sw, sh) = (doc.width * scale, doc.height * scale);
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
        fmt(sw),
        fmt(sh),
        fmt(doc.width),
        fmt(doc.height)
    );
    if !doc.metadata.title.is_empty() {
        let _ = write!(out, "<title>{}</title>", esc(&doc.metadata.title));
    }
    if !w.defs.is_empty() {
        let _ = write!(out, "<defs>{}</defs>", w.defs);
    }
    if !transparent {
        let bg = doc.background;
        let _ = write!(out, "<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>", fmt(doc.width), fmt(doc.height), rgb(bg));
    }
    let _ = write!(out, "<g transform=\"{}\">{}</g></svg>", mat(&dl.view), w.body);
    out
}

pub fn export_svg_frame(doc: &Document, global: u32, path: &Path, opts: &ExportOptions) -> Result<(), ExportError> {
    let (s, f) = scene_at_global_frame(doc, global);
    std::fs::write(path, svg_for_frame(doc, s, f, opts.scale, opts.transparent))?;
    Ok(())
}

pub fn export_svg_sequence(doc: &Document, dir: &Path, base: &str, opts: &ExportOptions) -> Result<Vec<PathBuf>, ExportError> {
    std::fs::create_dir_all(dir)?;
    let (a, b) = opts.frames(doc);
    let mut out = Vec::new();
    for g in a..=b {
        let p = dir.join(format!("{base}_{:04}.svg", g - a + 1));
        export_svg_frame(doc, g, &p, opts)?;
        out.push(p);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_is_valid_and_reimportable() {
        let doc = crate::test_doc::sample();
        let s = svg_for_frame(&doc, 0, 5, 1.0, false);
        assert!(s.starts_with("<svg") && s.contains("linearGradient"));
        // Round trip through the SVG importer (usvg) proves it parses.
        let imp = animara_importers::import_svg_data(s.as_bytes()).unwrap();
        assert!(imp.elements.len() >= 3, "{}", imp.elements.len());
        assert_eq!((imp.width, imp.height), (160.0, 120.0));
    }
}
