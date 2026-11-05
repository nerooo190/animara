//! XFL / FLA export. Produces a DOMDocument.xml with LIBRARY symbol files and
//! media, mirroring the structures read by `animara-importers`.

use crate::{ExportError, cubic_to_quads};
use animara_document::{
    BlendMode, Color, ColorEffect, Document, Element, ElementKind, Fill, Filter, LabelKind, LayerKind, LineCap, LineJoin, LoopMode,
    RotateMode, Shape, SoundSync, SpreadMode, StrokeStyle, SymbolKind, TextAlign, TextKind, Timeline, TweenKind,
};
use animara_geometry::{Affine, Segment, Vec2};
use std::fmt::Write as _;
use std::io::Write as _;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn hex(c: Color) -> String {
    let [r, g, b, _] = c.to_rgba8();
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn alpha_attr(c: Color) -> String {
    if c.a < 1.0 { format!(" alpha=\"{}\"", c.a) } else { String::new() }
}

fn num(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".into() } else { s }
}

fn matrix_xml(m: &Affine) -> String {
    if m.approx_eq(&Affine::IDENTITY, 1e-12) {
        return String::new();
    }
    let mut a = String::new();
    for (k, v, d) in [("a", m.a, 1.0), ("b", m.b, 0.0), ("c", m.c, 0.0), ("d", m.d, 1.0), ("tx", m.tx, 0.0), ("ty", m.ty, 0.0)] {
        if (v - d).abs() > 1e-12 {
            let _ = write!(a, " {k}=\"{}\"", num(v));
        }
    }
    format!("<matrix><Matrix{a}/></matrix>")
}

fn tw(v: f64) -> String {
    num((v * 20.0 * 100.0).round() / 100.0)
}

fn library_name(folder: &str, name: &str) -> String {
    if folder.is_empty() { name.to_string() } else { format!("{folder}/{name}") }
}

struct Ctx<'a> {
    doc: &'a Document,
}

impl Ctx<'_> {
    fn symbol_name(&self, id: animara_document::Id) -> String {
        self.doc.library.symbol(id).map(|s| library_name(&s.folder, &s.name)).unwrap_or_default()
    }

    fn fill_xml(&self, f: &Fill, m: &Affine) -> String {
        match f {
            Fill::Solid { color } => format!("<SolidColor color=\"{}\"{}/>", hex(*color), alpha_attr(*color)),
            Fill::Linear { stops, matrix, spread } | Fill::Radial { stops, matrix, spread, .. } => {
                let gm = *m * *matrix * Affine::scale(1.0 / 819.2, 1.0 / 819.2);
                let tag = if matches!(f, Fill::Linear { .. }) { "LinearGradient" } else { "RadialGradient" };
                let sm = match spread {
                    SpreadMode::Pad => "",
                    SpreadMode::Reflect => " spreadMethod=\"reflect\"",
                    SpreadMode::Repeat => " spreadMethod=\"repeat\"",
                };
                let focal = match f {
                    Fill::Radial { focal, .. } if *focal != 0.0 => format!(" focalPointRatio=\"{}\"", num(*focal)),
                    _ => String::new(),
                };
                let mut s = format!("<{tag}{sm}{focal}>{}", if matrix_xml(&gm).is_empty() { "<matrix><Matrix/></matrix>".to_string() } else { matrix_xml(&gm) });
                for st in stops {
                    let _ = write!(s, "<GradientEntry color=\"{}\"{} ratio=\"{}\"/>", hex(st.color), alpha_attr(st.color), num(st.offset as f64));
                }
                let _ = write!(s, "</{tag}>");
                s
            }
            Fill::Bitmap { asset, matrix, smooth, repeat } => {
                let name = self.doc.library.bitmap(*asset).map(|b| library_name(&b.folder, &b.name)).unwrap_or_default();
                let bm = *m * *matrix * Affine::scale(20.0, 20.0);
                format!(
                    "<BitmapFill bitmapPath=\"{}\"{}{}>{}</BitmapFill>",
                    esc(&name),
                    if *repeat { "" } else { " bitmapIsClipped=\"true\"" },
                    if *smooth { "" } else { " bitmapIsSmoothed=\"false\"" },
                    matrix_xml(&bm)
                )
            }
        }
    }

    fn shape_xml(&self, s: &Shape, m: &Affine) -> String {
        let mut fills = String::new();
        let mut strokes = String::new();
        let mut edges = String::new();
        for (i, sp) in s.paths.iter().enumerate() {
            let idx = i + 1;
            if let Some(f) = &sp.fill {
                let _ = write!(fills, "<FillStyle index=\"{idx}\">{}</FillStyle>", self.fill_xml(f, m));
            }
            if let Some(st) = &sp.stroke {
                let caps = match st.cap {
                    LineCap::Butt => " caps=\"none\"",
                    LineCap::Square => " caps=\"square\"",
                    LineCap::Round => "",
                };
                let joints = match st.join {
                    LineJoin::Miter => format!(" joints=\"miter\" miterLimit=\"{}\"", num(st.miter_limit)),
                    LineJoin::Bevel => " joints=\"bevel\"".into(),
                    LineJoin::Round => String::new(),
                };
                let scale = if st.scale_with_object { "" } else { " scaleMode=\"none\"" };
                let w = num(st.width * if st.scale_with_object { m.mean_scale() } else { 1.0 });
                let inner = format!("<fill>{}</fill>", self.fill_xml(&st.fill, m));
                let body = match st.style {
                    StrokeStyle::Dashed { dash, gap } => format!("<DashedStroke weight=\"{w}\" dash1=\"{}\" dash2=\"{}\"{caps}{joints}{scale}>{inner}</DashedStroke>", num(dash), num(gap)),
                    StrokeStyle::Dotted { spacing } => format!("<DottedStroke weight=\"{w}\" dotSpace=\"{}\"{caps}{joints}{scale}>{inner}</DottedStroke>", num(spacing)),
                    StrokeStyle::Hairline => format!("<SolidStroke weight=\"1\" solidStyle=\"hairline\"{caps}{joints}>{inner}</SolidStroke>"),
                    StrokeStyle::Solid => format!("<SolidStroke weight=\"{w}\"{caps}{joints}{scale}>{inner}</SolidStroke>"),
                };
                let _ = write!(strokes, "<StrokeStyle index=\"{idx}\">{body}</StrokeStyle>");
            }
            let world = sp.path.transform(m);
            for sub in world.subpaths() {
                let area: f64 = sub.segments.iter().map(|s| s.start().cross(s.end())).sum();
                let mut d = format!("!{} {}", tw(sub.start.x), tw(sub.start.y));
                let mut segs = sub.segments.clone();
                if !sub.closed && sp.fill.is_some() && let (Some(f), Some(l)) = (segs.first().map(|s| s.start()), segs.last().map(|s| s.end())) {
                    segs.push(Segment::Line(l, f));
                }
                for s in &segs {
                    match *s {
                        Segment::Line(_, e) => {
                            let _ = write!(d, "|{} {}", tw(e.x), tw(e.y));
                        }
                        Segment::Quad(_, c, e) => {
                            let _ = write!(d, "[{} {} {} {}", tw(c.x), tw(c.y), tw(e.x), tw(e.y));
                        }
                        Segment::Cubic(a, c1, c2, e) => {
                            for (c, e2) in cubic_to_quads(a, c1, c2, e) {
                                let _ = write!(d, "[{} {} {} {}", tw(c.x), tw(c.y), tw(e2.x), tw(e2.y));
                            }
                        }
                    }
                }
                let fill_attr = match &sp.fill {
                    Some(_) if area >= 0.0 => format!(" fillStyle1=\"{idx}\""),
                    Some(_) => format!(" fillStyle0=\"{idx}\""),
                    None => String::new(),
                };
                let stroke_attr = if sp.stroke.is_some() { format!(" strokeStyle=\"{idx}\"") } else { String::new() };
                let _ = write!(edges, "<Edge{fill_attr}{stroke_attr} edges=\"{d}\"/>");
            }
        }
        format!("<DOMShape><fills>{fills}</fills><strokes>{strokes}</strokes><edges>{edges}</edges></DOMShape>")
    }

    fn element_xml(&self, e: &Element, parent: &Affine) -> String {
        let m = *parent * e.transform;
        match &e.kind {
            ElementKind::Shape(s) => self.shape_xml(s, &m),
            ElementKind::Group { children } => {
                let inner: String = children.iter().map(|c| self.element_xml(c, &m)).collect();
                format!("<DOMGroup><members>{inner}</members></DOMGroup>")
            }
            ElementKind::Bitmap(b) => {
                let name = self.doc.library.bitmap(b.asset).map(|a| library_name(&a.folder, &a.name)).unwrap_or_default();
                format!("<DOMBitmapInstance libraryItemName=\"{}\">{}</DOMBitmapInstance>", esc(&name), matrix_xml(&m))
            }
            ElementKind::Instance(i) => {
                let kind = self.doc.library.symbol(i.symbol).map(|s| s.kind).unwrap_or_default();
                let mut attrs = format!(" libraryItemName=\"{}\"", esc(&self.symbol_name(i.symbol)));
                match kind {
                    SymbolKind::Graphic => {
                        attrs.push_str(" symbolType=\"graphic\"");
                        let lp = match i.loop_mode {
                            LoopMode::Loop => "loop",
                            LoopMode::PlayOnce => "play once",
                            LoopMode::SingleFrame => "single frame",
                            LoopMode::LoopReverse => "loop reverse",
                            LoopMode::PlayOnceReverse => "play once reverse",
                        };
                        let _ = write!(attrs, " loop=\"{lp}\"");
                        if i.first_frame > 0 {
                            let _ = write!(attrs, " firstFrame=\"{}\"", i.first_frame);
                        }
                    }
                    SymbolKind::Button => attrs.push_str(" symbolType=\"button\""),
                    SymbolKind::MovieClip => {}
                }
                if !i.instance_name.is_empty() {
                    let _ = write!(attrs, " name=\"{}\"", esc(&i.instance_name));
                }
                if !i.visible {
                    attrs.push_str(" isVisible=\"false\"");
                }
                if i.cache_as_bitmap {
                    attrs.push_str(" cacheAsBitmap=\"true\"");
                }
                let bm = match i.blend {
                    BlendMode::Normal => None,
                    other => Some(format!("{other:?}").to_lowercase()),
                };
                if let Some(b) = bm {
                    let _ = write!(attrs, " blendMode=\"{b}\"");
                }
                let mut inner = matrix_xml(&m);
                if e.pivot != Vec2::ZERO {
                    let _ = write!(inner, "<transformationPoint><Point x=\"{}\" y=\"{}\"/></transformationPoint>", num(e.pivot.x), num(e.pivot.y));
                }
                match i.color_effect {
                    ColorEffect::None => {}
                    ColorEffect::Brightness { amount } => {
                        let _ = write!(inner, "<color><Color brightness=\"{}\"/></color>", num(amount as f64));
                    }
                    ColorEffect::Tint { color, amount } => {
                        let _ = write!(inner, "<color><Color tintMultiplier=\"{}\" tintColor=\"{}\"/></color>", num(amount as f64), hex(color));
                    }
                    ColorEffect::Alpha { alpha } => {
                        let _ = write!(inner, "<color><Color alphaMultiplier=\"{}\"/></color>", num(alpha as f64));
                    }
                    ColorEffect::Advanced { transform: t } => {
                        let _ = write!(
                            inner,
                            "<color><Color redMultiplier=\"{}\" greenMultiplier=\"{}\" blueMultiplier=\"{}\" alphaMultiplier=\"{}\" redOffset=\"{}\" greenOffset=\"{}\" blueOffset=\"{}\" alphaOffset=\"{}\"/></color>",
                            num(t.mul[0] as f64), num(t.mul[1] as f64), num(t.mul[2] as f64), num(t.mul[3] as f64),
                            num(t.add[0] as f64 * 255.0), num(t.add[1] as f64 * 255.0), num(t.add[2] as f64 * 255.0), num(t.add[3] as f64 * 255.0)
                        );
                    }
                }
                if !i.filters.is_empty() {
                    inner.push_str("<filters>");
                    for f in &i.filters {
                        let _ = match *f {
                            Filter::Blur { blur_x, blur_y, quality } => write!(inner, "<BlurFilter blurX=\"{}\" blurY=\"{}\" quality=\"{quality}\"/>", num(blur_x), num(blur_y)),
                            Filter::DropShadow { blur_x, blur_y, strength, quality, angle, distance, color, knockout, inner: inn, hide_object } => write!(
                                inner,
                                "<DropShadowFilter blurX=\"{}\" blurY=\"{}\" strength=\"{}\" quality=\"{quality}\" angle=\"{}\" distance=\"{}\" color=\"{}\"{} knockout=\"{knockout}\" inner=\"{inn}\" hideObject=\"{hide_object}\"/>",
                                num(blur_x), num(blur_y), num(strength as f64), num(angle), num(distance), hex(color), alpha_attr(color)
                            ),
                            Filter::Glow { blur_x, blur_y, strength, quality, color, knockout, inner: inn } => write!(
                                inner,
                                "<GlowFilter blurX=\"{}\" blurY=\"{}\" strength=\"{}\" quality=\"{quality}\" color=\"{}\"{} knockout=\"{knockout}\" inner=\"{inn}\"/>",
                                num(blur_x), num(blur_y), num(strength as f64), hex(color), alpha_attr(color)
                            ),
                            Filter::AdjustColor { brightness, contrast, saturation, hue } => write!(
                                inner,
                                "<AdjustColorFilter brightness=\"{}\" contrast=\"{}\" saturation=\"{}\" hue=\"{}\"/>",
                                num(brightness as f64), num(contrast as f64), num(saturation as f64), num(hue as f64)
                            ),
                        };
                    }
                    inner.push_str("</filters>");
                }
                format!("<DOMSymbolInstance{attrs}>{inner}</DOMSymbolInstance>")
            }
            ElementKind::Text(t) => {
                let tag = match t.kind {
                    TextKind::Static => "DOMStaticText",
                    TextKind::Dynamic => "DOMDynamicText",
                    TextKind::Input => "DOMInputText",
                };
                let layout_w = t.width.unwrap_or_else(|| animara_text::fonts().layout(t).bounds.width());
                let mut attrs = format!(" width=\"{}\"", num(layout_w));
                if t.width.is_none() {
                    attrs.push_str(" autoExpand=\"true\"");
                }
                if !t.instance_name.is_empty() {
                    let _ = write!(attrs, " name=\"{}\"", esc(&t.instance_name));
                }
                let align = match t.align {
                    TextAlign::Left => "left",
                    TextAlign::Center => "center",
                    TextAlign::Right => "right",
                    TextAlign::Justify => "justify",
                };
                let mut runs = String::new();
                for r in &t.runs {
                    let face = match (r.style.bold, r.style.italic) {
                        (true, true) => format!("{}-BoldItalic", r.style.font_family),
                        (true, false) => format!("{}-Bold", r.style.font_family),
                        (false, true) => format!("{}-Italic", r.style.font_family),
                        _ => r.style.font_family.clone(),
                    };
                    let _ = write!(
                        runs,
                        "<DOMTextRun><characters>{}</characters><textAttrs><DOMTextAttrs alignment=\"{align}\" face=\"{}\" size=\"{}\" fillColor=\"{}\"{} letterSpacing=\"{}\" lineSpacing=\"{}\"{}/></textAttrs></DOMTextRun>",
                        esc(&r.text).replace('\n', "&#xD;"),
                        esc(&face),
                        num(r.style.size),
                        hex(r.style.color),
                        alpha_attr(r.style.color),
                        num(r.style.letter_spacing),
                        num(t.line_spacing),
                        if r.style.kerning { "" } else { " autoKern=\"false\"" }
                    );
                }
                format!("<{tag}{attrs}>{}<textRuns>{runs}</textRuns></{tag}>", matrix_xml(&m))
            }
        }
    }

    fn timeline_xml(&self, tl: &Timeline, name: &str) -> String {
        let mut s = format!("<DOMTimeline name=\"{}\"><layers>", esc(name));
        for l in &tl.layers {
            let mut attrs = format!(" name=\"{}\" color=\"{}\"", esc(&l.name), hex(l.color));
            let kind = match l.kind {
                LayerKind::Guide => Some("guide"),
                LayerKind::Mask => Some("mask"),
                LayerKind::Folder => Some("folder"),
                LayerKind::Camera => Some("camera"),
                _ => None,
            };
            if let Some(k) = kind {
                let _ = write!(attrs, " layerType=\"{k}\"");
            }
            let parent = l.mask_layer.or(l.guide_layer).or(l.folder).or(l.parent_layer).and_then(|p| tl.layer_index(p));
            if let Some(p) = parent {
                let _ = write!(attrs, " parentLayerIndex=\"{p}\"");
            }
            if !l.visible {
                attrs.push_str(" visible=\"false\"");
            }
            if l.locked {
                attrs.push_str(" locked=\"true\"");
            }
            if l.outline {
                attrs.push_str(" outline=\"true\"");
            }
            if l.kind == LayerKind::Folder && !l.expanded {
                attrs.push_str(" open=\"false\"");
            }
            if l.depth != 0.0 {
                let _ = write!(attrs, " layerDepth=\"{}\"", num(l.depth));
            }
            let _ = write!(s, "<DOMLayer{attrs}><frames>");
            for k in &l.frames {
                let mut fa = format!(" index=\"{}\"", k.start);
                if k.duration > 1 {
                    let _ = write!(fa, " duration=\"{}\"", k.duration);
                }
                if let Some(t) = &k.tween {
                    let tt = match t.kind {
                        TweenKind::Classic => "motion",
                        TweenKind::Shape => "shape",
                        TweenKind::Motion => "motion object",
                    };
                    let _ = write!(fa, " tweenType=\"{tt}\"");
                    if let animara_document::Ease::Classic { strength } = t.ease {
                        let _ = write!(fa, " acceleration=\"{strength}\"");
                    }
                    let rot = match t.rotate {
                        RotateMode::Auto => None,
                        RotateMode::None => Some("none"),
                        RotateMode::Clockwise => Some("clockwise"),
                        RotateMode::CounterClockwise => Some("counter-clockwise"),
                    };
                    if let Some(r) = rot {
                        let _ = write!(fa, " motionTweenRotate=\"{r}\"");
                    }
                    if t.rotate_times > 0 {
                        let _ = write!(fa, " motionTweenRotateTimes=\"{}\"", t.rotate_times);
                    }
                    if t.orient_to_path {
                        fa.push_str(" motionTweenOrientToPath=\"true\"");
                    }
                    if !t.scale {
                        fa.push_str(" motionTweenScale=\"false\"");
                    }
                }
                if let Some(lb) = &k.label {
                    let lt = match lb.kind {
                        LabelKind::Name => "name",
                        LabelKind::Comment => "comment",
                        LabelKind::Anchor => "anchor",
                    };
                    let _ = write!(fa, " name=\"{}\" labelType=\"{lt}\"", esc(&lb.name));
                }
                if let Some(snd) = &k.sound
                    && let Some(a) = self.doc.library.sound(snd.asset)
                {
                    let sync = match snd.sync {
                        SoundSync::Event => "event",
                        SoundSync::Start => "start",
                        SoundSync::Stop => "stop",
                        SoundSync::Stream => "stream",
                    };
                    let _ = write!(fa, " soundName=\"{}\" soundSync=\"{sync}\" soundLoop=\"{}\"", esc(&library_name(&a.folder, &a.name)), snd.loops + 1);
                }
                let _ = write!(s, "<DOMFrame{fa} keyMode=\"9728\">");
                if !k.script.is_empty() {
                    let _ = write!(s, "<Actionscript><script><![CDATA[{}]]></script></Actionscript>", k.script.replace("]]>", "]]]]><![CDATA[>"));
                }
                s.push_str("<elements>");
                for e in &k.elements {
                    s.push_str(&self.element_xml(e, &Affine::IDENTITY));
                }
                s.push_str("</elements></DOMFrame>");
            }
            s.push_str("</frames></DOMLayer>");
        }
        s.push_str("</layers></DOMTimeline>");
        s
    }
}

fn path_safe(s: &str) -> String {
    s.chars().map(|c| if matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect()
}

/// All XFL files as (relative path, bytes).
pub fn xfl_files(doc: &Document) -> Vec<(String, Vec<u8>)> {
    let ctx = Ctx { doc };
    let mut files = Vec::new();
    let mut dom = format!(
        "<DOMDocument xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns=\"http://ns.adobe.com/xfl/2008/\" width=\"{}\" height=\"{}\" frameRate=\"{}\" backgroundColor=\"{}\" xflVersion=\"2.97\" creatorInfo=\"Animara {}\" platform=\"{}\">",
        num(doc.width),
        num(doc.height),
        num(doc.fps),
        hex(doc.background),
        env!("CARGO_PKG_VERSION"),
        if cfg!(target_os = "macos") { "Macintosh" } else { "Windows" }
    );
    let folders = doc.library.all_folders();
    if !folders.is_empty() {
        dom.push_str("<folders>");
        for f in &folders {
            let _ = write!(dom, "<DOMFolderItem name=\"{}\" isExpanded=\"true\"/>", esc(f));
        }
        dom.push_str("</folders>");
    }
    if !doc.library.bitmaps.is_empty() || !doc.library.sounds.is_empty() {
        dom.push_str("<media>");
        for b in &doc.library.bitmaps {
            let name = library_name(&b.folder, &b.name);
            let href = path_safe(&name);
            let _ = write!(dom, "<DOMBitmapItem name=\"{}\" href=\"{}\" allowSmoothing=\"{}\" frameRight=\"{}\" frameBottom=\"{}\"/>", esc(&name), esc(&href), b.smoothing, b.width * 20, b.height * 20);
            files.push((format!("LIBRARY/{href}"), b.data.as_ref().clone()));
        }
        for s in &doc.library.sounds {
            let name = library_name(&s.folder, &s.name);
            let href = path_safe(&name);
            let _ = write!(dom, "<DOMSoundItem name=\"{}\" href=\"{}\" sampleRate=\"{}\" sampleCount=\"{}\"/>", esc(&name), esc(&href), s.sample_rate, (s.duration * s.sample_rate as f64) as u64);
            files.push((format!("LIBRARY/{href}"), s.data.as_ref().clone()));
        }
        dom.push_str("</media>");
    }
    if !doc.library.symbols.is_empty() {
        dom.push_str("<symbols>");
        for s in &doc.library.symbols {
            let name = library_name(&s.folder, &s.name);
            let href = format!("{}.xml", path_safe(&name));
            let _ = write!(dom, "<Include href=\"{}\" loadImmediate=\"false\"/>", esc(&href));
            let kind = match s.kind {
                SymbolKind::Graphic => " symbolType=\"graphic\"",
                SymbolKind::Button => " symbolType=\"button\"",
                SymbolKind::MovieClip => "",
            };
            let linkage = if s.linkage.is_empty() { String::new() } else { format!(" linkageExportForAS=\"true\" linkageClassName=\"{}\"", esc(&s.linkage)) };
            let xml = format!(
                "<DOMSymbolItem xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns=\"http://ns.adobe.com/xfl/2008/\" name=\"{}\"{kind}{linkage}><timeline>{}</timeline></DOMSymbolItem>",
                esc(&name),
                ctx.timeline_xml(&s.timeline, &s.name)
            );
            files.push((format!("LIBRARY/{href}"), xml.into_bytes()));
        }
        dom.push_str("</symbols>");
    }
    dom.push_str("<timelines>");
    for sc in &doc.scenes {
        dom.push_str(&ctx.timeline_xml(&sc.timeline, &sc.name));
    }
    dom.push_str("</timelines></DOMDocument>");
    files.insert(0, ("DOMDocument.xml".into(), dom.into_bytes()));
    files.push(("META-INF/metadata.xml".into(), format!("<metadata generator=\"Animara {}\" title=\"{}\"/>", env!("CARGO_PKG_VERSION"), esc(&doc.metadata.title)).into_bytes()));
    files
}

/// Write an XFL folder `dir/<name>/` containing `<name>.xfl`.
pub fn export_xfl(doc: &Document, xfl_path: &std::path::Path) -> Result<std::path::PathBuf, ExportError> {
    let stem = xfl_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "document".into());
    let dir = xfl_path.parent().unwrap_or(std::path::Path::new(".")).join(&stem);
    std::fs::create_dir_all(&dir)?;
    for (p, data) in xfl_files(doc) {
        let full = dir.join(&p);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(full, data)?;
    }
    let marker = dir.join(format!("{stem}.xfl"));
    std::fs::write(&marker, "PROXY-CS5")?;
    Ok(marker)
}

/// Write a ZIP-based FLA (XFL in an archive).
pub fn export_fla(doc: &Document, path: &std::path::Path) -> Result<(), ExportError> {
    let f = std::fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (p, data) in xfl_files(doc) {
        zip.start_file(p, opts).map_err(|e| ExportError::Encode(e.to_string()))?;
        zip.write_all(&data)?;
    }
    zip.start_file("mimetype", opts).map_err(|e| ExportError::Encode(e.to_string()))?;
    zip.write_all(b"application/vnd.adobe.xfl")?;
    zip.finish().map_err(|e| ExportError::Encode(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xfl_roundtrip_through_importer() {
        let doc = crate::test_doc::sample();
        let dir = std::env::temp_dir().join(format!("animara-xfl-{}", std::process::id()));
        let marker = export_xfl(&doc, &dir.join("sample.xfl")).unwrap();
        let imp = animara_importers::import_xfl(&marker).unwrap();
        let d = &imp.document;
        assert_eq!((d.width, d.height, d.fps), (doc.width, doc.height, doc.fps));
        assert_eq!(d.scenes.len(), 1);
        let (a, b) = (&doc.scenes[0].timeline, &d.scenes[0].timeline);
        assert_eq!(a.layers.len(), b.layers.len());
        for (la, lb) in a.layers.iter().zip(&b.layers) {
            assert_eq!(la.name, lb.name);
            assert_eq!(la.frames.len(), lb.frames.len());
            for (ka, kb) in la.frames.iter().zip(&lb.frames) {
                assert_eq!((ka.start, ka.duration), (kb.start, kb.duration));
                assert_eq!(ka.tween.as_ref().map(|t| t.kind), kb.tween.as_ref().map(|t| t.kind));
                assert_eq!(ka.script, kb.script);
                assert_eq!(ka.elements.len(), kb.elements.len());
            }
        }
        assert_eq!(d.library.symbols[0].name, "Box");
        // Geometry survives: rendered frames match closely.
        let mut c1 = animara_renderer::RenderCache::new();
        let mut c2 = animara_renderer::RenderCache::new();
        let p1 = animara_renderer::render_frame(&doc, 0, 6, 1.0, false, &mut c1).unwrap();
        let p2 = animara_renderer::render_frame(d, 0, 6, 1.0, false, &mut c2).unwrap();
        let diff: u64 = p1.data().iter().zip(p2.data()).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as u64).sum();
        let mean = diff as f64 / p1.data().len() as f64;
        assert!(mean < 2.0, "mean pixel difference {mean}");
        // FLA (zip) also imports.
        let fla = dir.join("sample.fla");
        export_fla(&doc, &fla).unwrap();
        let imp2 = animara_importers::import_fla(&fla).unwrap();
        assert_eq!(imp2.document.scenes[0].timeline.layers.len(), a.layers.len());
    }
}
