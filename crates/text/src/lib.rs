//! Text engine: system font discovery, OpenType shaping (kerning, ligatures,
//! complex scripts), Unicode bidirectional layout, line wrapping, tracking,
//! leading, alignment and glyph-outline extraction (text → vector paths).

use animara_document::{Color, TextAlign, TextElement, TextStyle};
use animara_geometry::{Affine, Path, Rect, Vec2};
use rustybuzz::ttf_parser;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};
use unicode_bidi::{BidiInfo, Level};

/// A positioned glyph outline in text-local coordinates.
#[derive(Debug, Clone)]
pub struct PositionedGlyph {
    pub path: Path,
    pub color: Color,
    /// Byte index of the source character in the plain text.
    pub cluster: usize,
}

#[derive(Debug, Clone)]
pub struct LineInfo {
    pub top: f64,
    pub baseline: f64,
    pub height: f64,
    pub x: f64,
    pub width: f64,
    /// Byte range in the plain text.
    pub range: std::ops::Range<usize>,
    /// Caret x positions for each char boundary in `range` (logical order), relative to the box.
    pub carets: Vec<(usize, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct TextLayout {
    pub glyphs: Vec<PositionedGlyph>,
    pub lines: Vec<LineInfo>,
    pub bounds: Rect,
    /// Box size used for alignment.
    pub box_width: f64,
}

impl TextLayout {
    /// Caret position (x, top, height) for a byte offset in the plain text.
    pub fn caret(&self, byte: usize) -> (f64, f64, f64) {
        for (li, l) in self.lines.iter().enumerate() {
            let last = li + 1 == self.lines.len();
            if byte >= l.range.start && (byte <= l.range.end && (byte < l.range.end || last || l.range.is_empty())) {
                let x = l.carets.iter().find(|(b, _)| *b == byte).map(|c| c.1).unwrap_or(l.x + l.width);
                return (x, l.top, l.height);
            }
            if byte <= l.range.end && !last && self.lines[li + 1].range.start > byte {
                return (l.x + l.width, l.top, l.height);
            }
        }
        self.lines.last().map(|l| (l.x + l.width, l.top, l.height)).unwrap_or((0.0, 0.0, 20.0))
    }
    /// Byte offset nearest to a point (for click-to-place caret).
    pub fn hit(&self, p: Vec2) -> usize {
        let line = self.lines.iter().min_by(|a, b| {
            let da = if p.y < a.top {
                a.top - p.y
            } else if p.y > a.top + a.height {
                p.y - a.top - a.height
            } else {
                0.0
            };
            let db = if p.y < b.top {
                b.top - p.y
            } else if p.y > b.top + b.height {
                p.y - b.top - b.height
            } else {
                0.0
            };
            da.total_cmp(&db)
        });
        match line {
            Some(l) => l.carets.iter().min_by(|a, b| (a.1 - p.x).abs().total_cmp(&(b.1 - p.x).abs())).map(|c| c.0).unwrap_or(l.range.start),
            None => 0,
        }
    }
    /// All glyph outlines merged into one path per color.
    pub fn to_paths(&self) -> Vec<(Path, Color)> {
        let mut out: Vec<(Path, Color)> = Vec::new();
        for g in &self.glyphs {
            match out.last_mut() {
                Some((p, c)) if *c == g.color => p.extend(&g.path),
                _ => out.push((g.path.clone(), g.color)),
            }
        }
        out
    }
}

struct LoadedFace {
    data: Arc<Vec<u8>>,
    index: u32,
}

/// Font database with shaping and layout.
pub struct FontLibrary {
    db: fontdb::Database,
    faces: Mutex<HashMap<fontdb::ID, Arc<LoadedFace>>>,
    fallback_ids: Vec<fontdb::ID>,
    cache: Mutex<HashMap<u64, Arc<TextLayout>>>,
}

static GLOBAL: OnceLock<FontLibrary> = OnceLock::new();

/// Shared font library (system fonts are scanned once).
pub fn fonts() -> &'static FontLibrary {
    GLOBAL.get_or_init(FontLibrary::new)
}

const FALLBACK_FAMILIES: &[&str] = &[
    "Noto Sans",
    "DejaVu Sans",
    "Liberation Sans",
    "Arial",
    "Segoe UI",
    "Helvetica",
    "Noto Sans Arabic",
    "Noto Naskh Arabic",
    "Noto Sans Hebrew",
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Noto Sans Devanagari",
    "Noto Sans Thai",
    "Microsoft YaHei",
    "Tahoma",
    "Noto Color Emoji",
    "Noto Emoji",
];

impl FontLibrary {
    pub fn new() -> FontLibrary {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        // Bundled fallbacks guarantee text renders on systems without fonts.
        db.load_font_data(epaint_default_fonts::UBUNTU_LIGHT.to_vec());
        db.load_font_data(epaint_default_fonts::HACK_REGULAR.to_vec());
        db.set_sans_serif_family(Self::first_available(
            &db,
            &["Noto Sans", "DejaVu Sans", "Liberation Sans", "Arial", "Segoe UI", "Ubuntu"],
        ));
        db.set_serif_family(Self::first_available(&db, &["Noto Serif", "DejaVu Serif", "Liberation Serif", "Times New Roman", "Ubuntu"]));
        db.set_monospace_family(Self::first_available(&db, &["Noto Sans Mono", "DejaVu Sans Mono", "Liberation Mono", "Consolas", "Hack"]));
        let mut fallback_ids = Vec::new();
        for fam in FALLBACK_FAMILIES.iter().chain(["Ubuntu", "Hack"].iter()) {
            let q = fontdb::Query { families: &[fontdb::Family::Name(fam)], ..Default::default() };
            if let Some(id) = db.query(&q) {
                fallback_ids.push(id);
            }
        }
        FontLibrary { db, faces: Mutex::new(HashMap::new()), fallback_ids, cache: Mutex::new(HashMap::new()) }
    }

    fn first_available(db: &fontdb::Database, names: &[&str]) -> String {
        for n in names {
            if db.faces().any(|f| f.families.iter().any(|(fam, _)| fam == n)) {
                return n.to_string();
            }
        }
        "Ubuntu".to_string()
    }

    /// Raw font file bytes for the first face of a family (used to give the UI
    /// fallback fonts for symbols). Returns `None` when the family is not installed.
    pub fn family_bytes(&self, family: &str) -> Option<(Vec<u8>, u32)> {
        let q = fontdb::Query { families: &[fontdb::Family::Name(family)], ..Default::default() };
        let id = self.db.query(&q)?;
        let name_ok = self.db.face(id).is_some_and(|f| f.families.iter().any(|(n, _)| n == family));
        if !name_ok {
            return None;
        }
        let f = self.load(id)?;
        Some((f.data.as_ref().clone(), f.index))
    }

    /// Sorted list of available font family names (plus generic families).
    pub fn families(&self) -> Vec<String> {
        let mut v: Vec<String> = self.db.faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone())).collect();
        v.sort();
        v.dedup();
        let mut out = vec!["sans-serif".to_string(), "serif".to_string(), "monospace".to_string()];
        out.extend(v);
        out
    }

    fn query(&self, style: &TextStyle) -> Option<fontdb::ID> {
        let family = match style.font_family.as_str() {
            "sans-serif" | "_sans" | "" => fontdb::Family::SansSerif,
            "serif" | "_serif" => fontdb::Family::Serif,
            "monospace" | "_typewriter" => fontdb::Family::Monospace,
            name => fontdb::Family::Name(name),
        };
        let q = fontdb::Query {
            families: &[family, fontdb::Family::SansSerif],
            weight: if style.bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
            style: if style.italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
            stretch: fontdb::Stretch::Normal,
        };
        self.db.query(&q).or_else(|| self.fallback_ids.first().copied())
    }

    fn load(&self, id: fontdb::ID) -> Option<Arc<LoadedFace>> {
        if let Some(f) = self.faces.lock().ok()?.get(&id) {
            return Some(f.clone());
        }
        let face = self.db.with_face_data(id, |data, index| Arc::new(LoadedFace { data: Arc::new(data.to_vec()), index }))?;
        self.faces.lock().ok()?.insert(id, face.clone());
        Some(face)
    }

    fn is_italic(&self, id: fontdb::ID) -> bool {
        self.db.face(id).is_some_and(|f| f.style != fontdb::Style::Normal)
    }

    fn covers(&self, id: fontdb::ID, c: char) -> bool {
        let Some(f) = self.load(id) else { return false };
        ttf_parser::Face::parse(&f.data, f.index).ok().and_then(|face| face.glyph_index(c)).is_some()
    }

    /// Choose a face for a character: the styled face when it has the glyph, else a fallback.
    fn face_for_char(&self, primary: fontdb::ID, c: char) -> fontdb::ID {
        if c.is_whitespace() || c.is_control() || self.covers(primary, c) {
            return primary;
        }
        self.fallback_ids.iter().copied().find(|&id| self.covers(id, c)).unwrap_or(primary)
    }

    /// Lay out a text element (cached).
    pub fn layout(&self, text: &TextElement) -> Arc<TextLayout> {
        let key = {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            format!("{text:?}").hash(&mut h);
            h.finish()
        };
        if let Some(l) = self.cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
            return l;
        }
        let layout = Arc::new(self.layout_uncached(text));
        if let Ok(mut c) = self.cache.lock() {
            if c.len() > 2048 {
                c.clear();
            }
            c.insert(key, layout.clone());
        }
        layout
    }

    fn layout_uncached(&self, text: &TextElement) -> TextLayout {
        let plain = text.plain_text();
        // Style lookup by byte index.
        let mut style_ranges: Vec<(std::ops::Range<usize>, &TextStyle)> = Vec::new();
        let mut pos = 0;
        for r in &text.runs {
            style_ranges.push((pos..pos + r.text.len(), &r.style));
            pos += r.text.len();
        }
        let default_style = text.style();
        let style_at =
            |b: usize| -> &TextStyle { style_ranges.iter().find(|(r, _)| r.contains(&b)).map(|(_, s)| *s).unwrap_or(&default_style) };

        let mut layout = TextLayout { box_width: text.width.unwrap_or(0.0), ..Default::default() };
        let mut y = 0.0;
        let mut para_start = 0;
        let mut max_width: f64 = 0.0;
        let mut pending_lines: Vec<(LineInfo, Vec<PositionedGlyph>)> = Vec::new();
        for para in plain.split('\n') {
            let base = para_start;
            para_start += para.len() + 1;
            let lines = self.layout_paragraph(para, base, &style_at, text, &mut y);
            for (l, g) in lines {
                max_width = max_width.max(l.width);
                pending_lines.push((l, g));
            }
        }
        let box_w = text.width.unwrap_or(max_width);
        layout.box_width = box_w;
        for (mut l, glyphs) in pending_lines {
            let rtl_para = text.rtl;
            let dx = match text.align {
                TextAlign::Left | TextAlign::Justify => {
                    if rtl_para {
                        box_w - l.width
                    } else {
                        0.0
                    }
                }
                TextAlign::Center => (box_w - l.width) / 2.0,
                TextAlign::Right => box_w - l.width,
            };
            l.x += dx;
            for c in &mut l.carets {
                c.1 += dx;
            }
            let m = Affine::translate(dx, 0.0);
            for mut g in glyphs {
                g.path = g.path.transform(&m);
                layout.bounds = layout.bounds.union(&g.path.bounds());
                layout.glyphs.push(g);
            }
            layout.lines.push(l);
        }
        let total_h = y;
        layout.bounds = layout.bounds.union(&Rect::new(0.0, 0.0, box_w.max(1.0), total_h.max(1.0)));
        layout
    }

    #[allow(clippy::type_complexity)]
    fn layout_paragraph<'s>(
        &self,
        para: &str,
        base: usize,
        style_at: &dyn Fn(usize) -> &'s TextStyle,
        text: &TextElement,
        y: &mut f64,
    ) -> Vec<(LineInfo, Vec<PositionedGlyph>)> {
        let first_style = style_at(base).clone();
        let primary = self.query(&first_style);
        let line_metrics = |style: &TextStyle| -> (f64, f64) {
            let id = self.query(style);
            let (asc, desc, gap, upem) = id
                .and_then(|id| self.load(id))
                .and_then(|f| {
                    let face = ttf_parser::Face::parse(&f.data, f.index).ok()?;
                    Some((face.ascender() as f64, face.descender() as f64, face.line_gap() as f64, face.units_per_em() as f64))
                })
                .unwrap_or((800.0, -200.0, 0.0, 1000.0));
            let s = style.size / upem;
            (asc * s, (asc - desc + gap) * s)
        };
        if para.is_empty() {
            let (asc, h) = line_metrics(&first_style);
            let info =
                LineInfo { top: *y, baseline: *y + asc, height: h, x: 0.0, width: 0.0, range: base..base, carets: vec![(base, 0.0)] };
            *y += h + text.line_spacing;
            let _ = primary;
            return vec![(info, Vec::new())];
        }
        let level = if text.rtl { Some(Level::rtl()) } else { None };
        let bidi = BidiInfo::new(para, level);
        let levels = &bidi.levels;

        // Segment by style, face and bidi level.
        struct Seg {
            range: std::ops::Range<usize>,
            face: fontdb::ID,
            level: Level,
            style: TextStyle,
        }
        let mut segs: Vec<Seg> = Vec::new();
        for (bi, c) in para.char_indices() {
            let style = style_at(base + bi).clone();
            let prim = self.query(&style).unwrap_or_else(|| self.fallback_ids[0]);
            let face = self.face_for_char(prim, c);
            let lvl = levels[bi];
            match segs.last_mut() {
                Some(s) if s.face == face && s.level == lvl && s.style == style && s.range.end == bi => s.range.end = bi + c.len_utf8(),
                _ => segs.push(Seg { range: bi..bi + c.len_utf8(), face, level: lvl, style }),
            }
        }

        // Shape segments.
        struct Glyph {
            id: u16,
            cluster: usize,
            advance: f64,
            dx: f64,
            dy: f64,
            seg: usize,
        }
        let mut shaped: Vec<Vec<Glyph>> = Vec::new();
        let mut cluster_adv: HashMap<usize, f64> = HashMap::new();
        for (si, s) in segs.iter().enumerate() {
            let mut glyphs = Vec::new();
            if let Some(f) = self.load(s.face)
                && let Some(face) = rustybuzz::Face::from_slice(&f.data, f.index)
            {
                let upem = face.units_per_em() as f64;
                let scale = s.style.size / upem;
                let mut buf = rustybuzz::UnicodeBuffer::new();
                buf.push_str(&para[s.range.clone()]);
                buf.set_direction(if s.level.is_rtl() { rustybuzz::Direction::RightToLeft } else { rustybuzz::Direction::LeftToRight });
                buf.guess_segment_properties();
                let features =
                    if s.style.kerning { vec![] } else { vec![rustybuzz::Feature::new(ttf_parser::Tag::from_bytes(b"kern"), 0, ..)] };
                let out = rustybuzz::shape(&face, &features, buf);
                for (info, p) in out.glyph_infos().iter().zip(out.glyph_positions()) {
                    let cluster = s.range.start + info.cluster as usize;
                    let advance = p.x_advance as f64 * scale + s.style.letter_spacing;
                    *cluster_adv.entry(cluster).or_insert(0.0) += advance;
                    glyphs.push(Glyph {
                        id: info.glyph_id as u16,
                        cluster,
                        advance,
                        dx: p.x_offset as f64 * scale,
                        dy: p.y_offset as f64 * scale,
                        seg: si,
                    });
                }
            }
            shaped.push(glyphs);
        }

        // Greedy line breaking in logical order.
        let chars: Vec<(usize, char)> = para.char_indices().collect();
        let mut lines: Vec<std::ops::Range<usize>> = Vec::new();
        let max_w = text.width.filter(|w| *w > 0.0);
        let mut line_start = 0;
        let mut width = 0.0;
        let mut last_break: Option<usize> = None;
        let mut i = 0;
        while i < chars.len() {
            let (b, c) = chars[i];
            let adv = cluster_adv.get(&b).copied().unwrap_or(0.0);
            if let Some(mw) = max_w
                && width + adv > mw
                && b > line_start
                && !c.is_whitespace()
            {
                let brk = last_break.filter(|&lb| lb > line_start).unwrap_or(b);
                lines.push(line_start..brk);
                line_start = brk;
                // Restart measuring from the break.
                i = chars.iter().position(|(cb, _)| *cb == brk).unwrap_or(i);
                width = 0.0;
                last_break = None;
                continue;
            }
            width += adv;
            if c.is_whitespace() {
                last_break = Some(b + c.len_utf8());
            }
            i += 1;
        }
        lines.push(line_start..para.len());

        let mut result = Vec::new();
        for range in lines {
            let mut asc_max: f64 = 0.0;
            let mut h_max: f64 = 0.0;
            for s in &segs {
                if s.range.start < range.end && s.range.end > range.start {
                    let (a, h) = line_metrics(&s.style);
                    asc_max = asc_max.max(a);
                    h_max = h_max.max(h);
                }
            }
            if h_max == 0.0 {
                let (a, h) = line_metrics(&first_style);
                asc_max = a;
                h_max = h;
            }
            let baseline = *y + asc_max;
            let para_info = &bidi.paragraphs[0];
            let (runs_levels, runs) = bidi.visual_runs(para_info, range.clone());
            let _ = runs_levels;
            let mut x = 0.0;
            let mut glyph_out = Vec::new();
            let mut carets: Vec<(usize, f64)> = Vec::new();
            for run in runs {
                let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
                let mut seg_ids: Vec<usize> =
                    segs.iter().enumerate().filter(|(_, s)| s.range.start < run.end && s.range.end > run.start).map(|(i, _)| i).collect();
                if rtl {
                    seg_ids.reverse();
                }
                for si in seg_ids {
                    let s = &segs[si];
                    let face_data = self.load(s.face);
                    let face = face_data.as_ref().and_then(|f| ttf_parser::Face::parse(&f.data, f.index).ok());
                    let synth_italic = s.style.italic && !self.is_italic(s.face);
                    for g in shaped[si].iter().filter(|g| g.cluster >= run.start && g.cluster < run.end && g.seg == si) {
                        let gx = x;
                        if !carets.iter().any(|c| c.0 == base + g.cluster) {
                            if rtl {
                                carets.push((base + g.cluster, gx + g.advance));
                            } else {
                                carets.push((base + g.cluster, gx));
                            }
                        }
                        if let Some(face) = &face {
                            let upem = face.units_per_em() as f64;
                            let scale = s.style.size / upem;
                            let mut builder = PathBuilder { path: Path::new() };
                            if face.outline_glyph(ttf_parser::GlyphId(g.id), &mut builder).is_some() {
                                let mut m = Affine::translate(gx + g.dx, baseline - g.dy) * Affine::scale(scale, -scale);
                                if synth_italic {
                                    m = Affine::translate(gx + g.dx, baseline - g.dy)
                                        * Affine::new(1.0, 0.0, -0.2, 1.0, 0.0, 0.0)
                                        * Affine::scale(scale, -scale);
                                }
                                let mut path = builder.path.transform(&m);
                                if s.style.bold && self.db.face(s.face).is_some_and(|f| f.weight.0 < 600) {
                                    // Synthetic bold: double the outline with a tiny offset.
                                    let off = path.translate(Vec2::new(s.style.size * 0.03, 0.0));
                                    path.extend(&off);
                                }
                                glyph_out.push(PositionedGlyph { path, color: s.style.color, cluster: base + g.cluster });
                            }
                        }
                        x += g.advance;
                    }
                }
            }
            // Carets for remaining boundaries (end of line).
            carets.push((base + range.end, x));
            carets.sort_by_key(|c| c.0);
            let info = LineInfo { top: *y, baseline, height: h_max, x: 0.0, width: x, range: base + range.start..base + range.end, carets };
            *y += h_max + text.line_spacing;
            result.push((info, glyph_out));
        }
        result
    }
}

impl Default for FontLibrary {
    fn default() -> Self {
        FontLibrary::new()
    }
}

struct PathBuilder {
    path: Path,
}

impl ttf_parser::OutlineBuilder for PathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(Vec2::new(x as f64, y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(Vec2::new(x as f64, y as f64));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quad_to(Vec2::new(x1 as f64, y1 as f64), Vec2::new(x as f64, y as f64));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.cubic_to(Vec2::new(x1 as f64, y1 as f64), Vec2::new(x2 as f64, y2 as f64), Vec2::new(x as f64, y as f64));
    }
    fn close(&mut self) {
        self.path.close();
    }
}

/// Convert text to filled shape paths (Modify ▸ Break Apart twice).
pub fn text_to_shape(text: &TextElement) -> animara_document::Shape {
    let layout = fonts().layout(text);
    animara_document::Shape {
        paths: layout
            .to_paths()
            .into_iter()
            .map(|(p, c)| animara_document::ShapePath::filled(p, animara_document::Fill::solid(c)))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_produces_glyphs_and_wraps() {
        let mut t = TextElement::new("Hello world, this wraps", TextStyle::default());
        let l = fonts().layout(&t);
        assert!(!l.glyphs.is_empty());
        assert_eq!(l.lines.len(), 1);
        t.width = Some(80.0);
        let l2 = fonts().layout(&t);
        assert!(l2.lines.len() >= 2, "{}", l2.lines.len());
        for line in &l2.lines {
            assert!(line.width <= 80.0 + 30.0);
        }
    }

    #[test]
    fn tracking_increases_width() {
        let t = TextElement::new("AVATAR", TextStyle::default());
        let mut wide = t.clone();
        wide.set_style_all(|s| s.letter_spacing = 10.0);
        let a = fonts().layout(&t).lines[0].width;
        let b = fonts().layout(&wide).lines[0].width;
        assert!((b - a - 60.0).abs() < 1.0, "{a} {b}");
    }

    #[test]
    fn newlines_and_rtl() {
        let mut t = TextElement::new("abc\nשלום", TextStyle::default());
        t.rtl = true;
        let l = fonts().layout(&t);
        assert_eq!(l.lines.len(), 2);
        let (x0, ..) = l.caret(0);
        let (_, top1, _) = l.caret(5);
        assert!(top1 > 0.0);
        assert!(x0.is_finite());
    }

    #[test]
    fn text_to_shape_has_paths() {
        let s = text_to_shape(&TextElement::new("Hi", TextStyle::default()));
        assert!(!s.paths.is_empty());
        assert!(s.bounds().width() > 5.0);
    }
}
