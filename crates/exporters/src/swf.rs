//! SWF export (uncompressed `FWS`, version 10) following the public SWF file
//! format specification. Frames are baked to DefineShape3 characters in stage
//! coordinates; identical shapes are shared. Supported: solid/gradient/bitmap
//! fills, strokes, text (as outlines), masks (clip depths), color effects
//! (baked) and streamed uncompressed PCM audio. Filters and blend modes are
//! reported as warnings (they would require PlaceObject3).

use crate::{ExportError, ExportOptions, cubic_to_quads};
use animara_animation::{DisplayItem, EvalOptions, evaluate_document_frame, scene_at_global_frame};
use animara_document::{ColorTransform, Document, Fill, Shape, ShapePath, SpreadMode, StrokeStyle};
use animara_geometry::{Affine, Path, Segment, Vec2};
use std::collections::HashMap;

struct Bits {
    buf: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    fn new() -> Bits {
        Bits { buf: Vec::new(), acc: 0, n: 0 }
    }
    fn ub(&mut self, nbits: u32, v: u32) {
        for i in (0..nbits).rev() {
            self.acc = (self.acc << 1) | ((v >> i) & 1);
            self.n += 1;
            if self.n == 8 {
                self.buf.push(self.acc as u8);
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    fn sb(&mut self, nbits: u32, v: i32) {
        self.ub(nbits, (v as u32) & ((1u64 << nbits) - 1) as u32);
    }
    fn align(&mut self) {
        if self.n > 0 {
            let pad = 8 - self.n;
            self.ub(pad, 0);
        }
    }
    fn finish(mut self) -> Vec<u8> {
        self.align();
        self.buf
    }
}

fn sbits(v: i32) -> u32 {
    if v == 0 {
        return 1;
    }
    let mag = if v < 0 { !v } else { v } as u32;
    33 - mag.leading_zeros()
}

fn rect(b: &mut Bits, x0: i32, x1: i32, y0: i32, y1: i32) {
    let n = [x0, x1, y0, y1].iter().map(|v| sbits(*v)).max().unwrap_or(1);
    b.ub(5, n);
    for v in [x0, x1, y0, y1] {
        b.sb(n, v);
    }
}

fn matrix(b: &mut Bits, m: &Affine) {
    let fixed = |v: f64| (v * 65536.0).round() as i32;
    let has_scale = m.a != 1.0 || m.d != 1.0;
    b.ub(1, has_scale as u32);
    if has_scale {
        let (sx, sy) = (fixed(m.a), fixed(m.d));
        let n = sbits(sx).max(sbits(sy));
        b.ub(5, n);
        b.sb(n, sx);
        b.sb(n, sy);
    }
    let has_rot = m.b != 0.0 || m.c != 0.0;
    b.ub(1, has_rot as u32);
    if has_rot {
        let (r0, r1) = (fixed(m.b), fixed(m.c));
        let n = sbits(r0).max(sbits(r1));
        b.ub(5, n);
        b.sb(n, r0);
        b.sb(n, r1);
    }
    let (tx, ty) = ((m.tx * 20.0).round() as i32, (m.ty * 20.0).round() as i32);
    let n = if tx == 0 && ty == 0 { 0 } else { sbits(tx).max(sbits(ty)) };
    b.ub(5, n);
    if n > 0 {
        b.sb(n, tx);
        b.sb(n, ty);
    }
}

fn tag(out: &mut Vec<u8>, code: u16, body: &[u8]) {
    if body.len() < 63 && !matches!(code, 36 | 19) {
        out.extend_from_slice(&((code << 6) | body.len() as u16).to_le_bytes());
    } else {
        out.extend_from_slice(&((code << 6) | 0x3f).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    }
    out.extend_from_slice(body);
}

fn rgba(c: animara_document::Color) -> [u8; 4] {
    c.to_rgba8()
}

struct Ctx<'a> {
    doc: &'a Document,
    tags: Vec<u8>,
    next_id: u16,
    shapes: HashMap<Vec<u8>, u16>,
    bitmaps: HashMap<animara_document::Id, u16>,
    warnings: Vec<String>,
}

/// One placement for a frame: character id and optional clip depth offset.
#[derive(Clone, PartialEq)]
struct Place {
    id: u16,
    clip_count: Option<u16>,
}

impl Ctx<'_> {
    fn bitmap(&mut self, id: animara_document::Id) -> Option<u16> {
        if let Some(c) = self.bitmaps.get(&id) {
            return Some(*c);
        }
        let b = self.doc.library.bitmap(id)?;
        let img = image::load_from_memory(&b.data).ok()?.to_rgba8();
        let mut argb = Vec::with_capacity(img.len());
        for p in img.pixels() {
            let [r, g, bb, a] = p.0;
            let pm = |v: u8| ((v as u32 * a as u32 + 127) / 255) as u8;
            argb.extend_from_slice(&[a, pm(r), pm(g), pm(bb)]);
        }
        let cid = self.next_id;
        self.next_id += 1;
        let mut body = Vec::new();
        body.extend_from_slice(&cid.to_le_bytes());
        body.push(5);
        body.extend_from_slice(&(img.width() as u16).to_le_bytes());
        body.extend_from_slice(&(img.height() as u16).to_le_bytes());
        body.extend(compress(&argb));
        tag(&mut self.tags, 36, &body);
        self.bitmaps.insert(id, cid);
        Some(cid)
    }

    fn fill_style(&mut self, f: &Fill, ct: &ColorTransform, m: &Affine, out: &mut Vec<u8>) {
        match f {
            Fill::Solid { color } => {
                out.push(0x00);
                out.extend(rgba(ct.apply(*color)));
            }
            Fill::Linear { stops, matrix: gm, spread } | Fill::Radial { stops, matrix: gm, spread, .. } => {
                out.push(if matches!(f, Fill::Linear { .. }) { 0x10 } else { 0x12 });
                // SWF gradients span ±16384 twips; ours span ±1 unit.
                let world = *m * *gm * Affine::scale(1.0 / 819.2, 1.0 / 819.2);
                let mut b = Bits::new();
                matrix(&mut b, &Affine { a: world.a * 20.0, b: world.b * 20.0, c: world.c * 20.0, d: world.d * 20.0, ..world });
                out.extend(b.finish());
                let sp = match spread {
                    SpreadMode::Pad => 0,
                    SpreadMode::Reflect => 1,
                    SpreadMode::Repeat => 2,
                };
                let n = stops.len().clamp(1, 15);
                out.push((sp << 6) | n as u8);
                for s in stops.iter().take(n) {
                    out.push((s.offset.clamp(0.0, 1.0) * 255.0).round() as u8);
                    out.extend(rgba(ct.apply(s.color)));
                }
            }
            Fill::Bitmap { asset, matrix: bm, repeat, smooth } => match self.bitmap(*asset) {
                Some(cid) => {
                    out.push(match (*repeat, *smooth) {
                        (true, true) => 0x40,
                        (false, true) => 0x41,
                        (true, false) => 0x42,
                        (false, false) => 0x43,
                    });
                    out.extend_from_slice(&cid.to_le_bytes());
                    let w = *m * *bm;
                    let mut b = Bits::new();
                    matrix(&mut b, &Affine { a: w.a * 20.0, b: w.b * 20.0, c: w.c * 20.0, d: w.d * 20.0, ..w });
                    out.extend(b.finish());
                }
                None => {
                    out.push(0x00);
                    out.extend([128, 128, 128, 255]);
                }
            },
        }
    }

    /// Define one shape path (already in stage coordinates via `m`).
    fn define(&mut self, sp: &ShapePath, m: &Affine, ct: &ColorTransform) -> Option<u16> {
        let world = sp.path.transform(m);
        let bounds = sp.bounds().is_empty().then(animara_geometry::Rect::default).unwrap_or_else(|| {
            let b = world.bounds();
            b.inflate(sp.stroke.as_ref().map(|s| s.width * m.mean_scale()).unwrap_or(0.0) + 1.0)
        });
        if bounds.is_empty() {
            return None;
        }
        let mut body = Vec::new();
        // Fill styles.
        let mut fills = Vec::new();
        let has_fill = sp.fill.is_some();
        if let Some(f) = &sp.fill {
            self.fill_style(f, ct, m, &mut fills);
        }
        body.push(has_fill as u8);
        body.extend(fills);
        // Line styles.
        let has_line = sp.stroke.is_some();
        body.push(has_line as u8);
        if let Some(s) = &sp.stroke {
            let w = match s.style {
                StrokeStyle::Hairline => 1.0,
                _ => s.width * if s.scale_with_object { m.mean_scale() } else { 1.0 },
            };
            body.extend_from_slice(&((w * 20.0).round().clamp(1.0, 65535.0) as u16).to_le_bytes());
            body.extend(rgba(ct.apply(s.fill.primary_color())));
            if !matches!(s.fill, Fill::Solid { .. }) {
                self.warnings.push("gradient strokes are exported to SWF as solid strokes".into());
            }
        }
        let mut b = Bits::new();
        b.ub(4, 1);
        b.ub(4, 1);
        for sub in world.subpaths() {
            // Orientation decides which side carries the fill.
            let area: f64 = sub.segments.iter().map(|s| s.start().cross(s.end())).sum();
            let (f0, f1) = if !has_fill {
                (0, 0)
            } else if area >= 0.0 {
                (0, 1)
            } else {
                (1, 0)
            };
            let tw = |v: Vec2| ((v.x * 20.0).round() as i32, (v.y * 20.0).round() as i32);
            let (sx, sy) = tw(sub.start);
            // Style change: line, fill1, fill0, moveTo.
            b.ub(1, 0);
            b.ub(1, 0);
            b.ub(1, has_line as u32);
            b.ub(1, 1);
            b.ub(1, 1);
            b.ub(1, 1);
            let n = sbits(sx).max(sbits(sy));
            b.ub(5, n);
            b.sb(n, sx);
            b.sb(n, sy);
            b.ub(1, f0);
            b.ub(1, f1);
            if has_line {
                b.ub(1, 1);
            }
            let mut cur = (sx, sy);
            let emit_line = |b: &mut Bits, cur: &mut (i32, i32), to: (i32, i32)| {
                let (dx, dy) = (to.0 - cur.0, to.1 - cur.1);
                if dx == 0 && dy == 0 {
                    return;
                }
                let n = sbits(dx).max(sbits(dy)).max(2);
                b.ub(1, 1);
                b.ub(1, 1);
                b.ub(4, n - 2);
                b.ub(1, 1);
                b.sb(n, dx);
                b.sb(n, dy);
                *cur = to;
            };
            let emit_curve = |b: &mut Bits, cur: &mut (i32, i32), c: (i32, i32), to: (i32, i32)| {
                let (cx, cy) = (c.0 - cur.0, c.1 - cur.1);
                let (ax, ay) = (to.0 - c.0, to.1 - c.1);
                let n = [cx, cy, ax, ay].iter().map(|v| sbits(*v)).max().unwrap_or(2).max(2);
                b.ub(1, 1);
                b.ub(1, 0);
                b.ub(4, n - 2);
                for v in [cx, cy, ax, ay] {
                    b.sb(n, v);
                }
                *cur = to;
            };
            let mut segs = sub.segments.clone();
            if !sub.closed
                && has_fill
                && let (Some(f), Some(l)) = (segs.first().map(|s| s.start()), segs.last().map(|s| s.end()))
            {
                segs.push(Segment::Line(l, f));
            }
            for s in &segs {
                match *s {
                    Segment::Line(_, e) => emit_line(&mut b, &mut cur, tw(e)),
                    Segment::Quad(_, c, e) => emit_curve(&mut b, &mut cur, tw(c), tw(e)),
                    Segment::Cubic(a, c1, c2, e) => {
                        for (c, e2) in cubic_to_quads(a, c1, c2, e) {
                            emit_curve(&mut b, &mut cur, tw(c), tw(e2));
                        }
                    }
                }
            }
        }
        b.ub(6, 0);
        body.extend(b.finish());
        let mut hdr = Bits::new();
        rect(
            &mut hdr,
            (bounds.min.x * 20.0) as i32,
            (bounds.max.x * 20.0) as i32,
            (bounds.min.y * 20.0) as i32,
            (bounds.max.y * 20.0) as i32,
        );
        let mut key = hdr.finish();
        key.extend(&body);
        if let Some(id) = self.shapes.get(&key) {
            return Some(*id);
        }
        let id = self.next_id;
        self.next_id += 1;
        let mut full = id.to_le_bytes().to_vec();
        full.extend(&key);
        tag(&mut self.tags, 32, &full);
        self.shapes.insert(key, id);
        Some(id)
    }

    fn items(&mut self, items: &[DisplayItem], out: &mut Vec<Place>) {
        for it in items {
            match it {
                DisplayItem::Shape { shape, transform, color, .. } => {
                    for sp in &shape.paths {
                        if let Some(id) = self.define(sp, transform, color) {
                            out.push(Place { id, clip_count: None });
                        }
                    }
                }
                DisplayItem::Text { text, transform, color, .. } => {
                    let s = animara_text::text_to_shape(text);
                    self.items(&[DisplayItem::Shape { shape: s, transform: *transform, color: *color, outline: None, source: None }], out);
                }
                DisplayItem::Bitmap { bitmap, transform, color, .. } => {
                    if let Some(b) = self.doc.library.bitmap(bitmap.asset) {
                        let sp = ShapePath::filled(
                            Path::rect(animara_geometry::Rect::new(0.0, 0.0, b.width as f64, b.height as f64)),
                            Fill::Bitmap { asset: bitmap.asset, matrix: Affine::IDENTITY, smooth: b.smoothing, repeat: false },
                        );
                        if let Some(id) = self.define(&sp, transform, color) {
                            out.push(Place { id, clip_count: None });
                        }
                    }
                }
                DisplayItem::Group { items, filters, blend, .. } => {
                    if !filters.is_empty() || *blend != animara_document::BlendMode::Normal {
                        self.warnings.push("filters and blend modes are not exported to SWF".into());
                    }
                    self.items(items, out);
                }
                DisplayItem::Masked { mask, content } => {
                    // Merge every mask path into one clip character.
                    let mut paths = Vec::new();
                    collect_paths(mask, &mut paths);
                    let mut merged = Path::new();
                    for p in &paths {
                        merged.extend(p);
                    }
                    let sp = ShapePath::filled(merged, Fill::solid(animara_document::Color::BLACK));
                    let mut inner = Vec::new();
                    self.items(content, &mut inner);
                    if let Some(id) = self.define(&sp, &Affine::IDENTITY, &ColorTransform::IDENTITY) {
                        out.push(Place { id, clip_count: Some(inner.len() as u16) });
                    }
                    out.extend(inner);
                }
            }
        }
    }
}

fn collect_paths(items: &[DisplayItem], out: &mut Vec<Path>) {
    for it in items {
        match it {
            DisplayItem::Shape { shape, transform, .. } => {
                out.extend(shape.paths.iter().filter(|p| p.fill.is_some()).map(|p| p.path.transform(transform)))
            }
            DisplayItem::Text { text, transform, .. } => {
                let s: Shape = animara_text::text_to_shape(text);
                out.extend(s.paths.iter().map(|p| p.path.transform(transform)));
            }
            DisplayItem::Group { items, .. } => collect_paths(items, out),
            DisplayItem::Masked { content, .. } => collect_paths(content, out),
            DisplayItem::Bitmap { .. } => {}
        }
    }
}

fn compress(data: &[u8]) -> Vec<u8> {
    zlib_stored::compress(data)
}

/// zlib stream for DefineBitsLossless2 built from stored deflate blocks
mod zlib_stored {
    /// Produce a valid zlib stream using stored (uncompressed) deflate blocks.
    pub fn compress(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x78, 0x01];
        let mut chunks = data.chunks(65535).peekable();
        if chunks.peek().is_none() {
            out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
        }
        while let Some(c) = chunks.next() {
            out.push(if chunks.peek().is_none() { 1 } else { 0 });
            let len = c.len() as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(c);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        out.extend_from_slice(&((b << 16) | a).to_be_bytes());
        out
    }
}

/// Export the movie as SWF. Returns warnings.
pub fn export_swf(doc: &Document, path: &std::path::Path, opts: &ExportOptions) -> Result<Vec<String>, ExportError> {
    let (a, b) = opts.frames(doc);
    let mut ctx = Ctx { doc, tags: Vec::new(), next_id: 1, shapes: HashMap::new(), bitmaps: HashMap::new(), warnings: Vec::new() };
    // FileAttributes: AS1/2, no network access needed.
    tag(&mut ctx.tags, 69, &0u32.to_le_bytes());
    let [r, g, bl, _] = doc.background.to_rgba8();
    tag(&mut ctx.tags, 9, &[r, g, bl]);
    // Streamed audio (uncompressed 16-bit stereo PCM, 44.1 kHz).
    let audio = (opts.include_audio && animara_media::has_audio(doc)).then(|| animara_media::mixdown_document(doc, 44100));
    let spf = (44100.0 / doc.fps.max(1.0)).round() as u16;
    if audio.is_some() {
        let flags = (3 << 2) | (1 << 1) | 1; // 44kHz, 16-bit, stereo
        let mut body = vec![flags, (3 << 4) | flags];
        body.extend_from_slice(&spf.to_le_bytes());
        tag(&mut ctx.tags, 45, &body);
    }
    let mut prev: Vec<Place> = Vec::new();
    let mut audio_pos = 0usize;
    for gf in a..=b {
        let (s, f) = scene_at_global_frame(doc, gf);
        let dl = evaluate_document_frame(doc, s, f, &EvalOptions::export());
        let items: Vec<DisplayItem> = dl.items.iter().cloned().map(|i| animara_animation::transform_item(i, &dl.view)).collect();
        let mut places = Vec::new();
        ctx.items(&items, &mut places);
        if places != prev {
            for d in 0..prev.len() {
                tag(&mut ctx.tags, 28, &((d + 1) as u16).to_le_bytes());
            }
            for (d, p) in places.iter().enumerate() {
                let mut body = Vec::new();
                let flags = 0x02 | 0x04 | if p.clip_count.is_some() { 0x40 } else { 0 };
                body.push(flags);
                body.extend_from_slice(&((d + 1) as u16).to_le_bytes());
                body.extend_from_slice(&p.id.to_le_bytes());
                let mut mb = Bits::new();
                matrix(&mut mb, &Affine::IDENTITY);
                body.extend(mb.finish());
                if let Some(n) = p.clip_count {
                    body.extend_from_slice(&((d + 1) as u16 + n).to_le_bytes());
                }
                tag(&mut ctx.tags, 26, &body);
            }
            prev = places;
        }
        if let Some(pcm) = &audio {
            let mut block = Vec::with_capacity(spf as usize * 4);
            for i in 0..spf as usize {
                for c in 0..2 {
                    let v = pcm.samples.get((audio_pos + i) * 2 + c).copied().unwrap_or(0.0);
                    block.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
                }
            }
            audio_pos += spf as usize;
            tag(&mut ctx.tags, 19, &block);
        }
        tag(&mut ctx.tags, 1, &[]);
    }
    tag(&mut ctx.tags, 0, &[]);
    let mut hdr = Bits::new();
    rect(&mut hdr, 0, (doc.width * 20.0).round() as i32, 0, (doc.height * 20.0).round() as i32);
    let mut body = hdr.finish();
    let fps = doc.fps.clamp(0.01, 255.0);
    body.push(((fps.fract() * 256.0) as u32).min(255) as u8);
    body.push(fps.trunc() as u8);
    body.extend_from_slice(&((b - a + 1) as u16).to_le_bytes());
    body.extend(ctx.tags);
    let mut out = b"FWS".to_vec();
    out.push(10);
    out.extend_from_slice(&((body.len() + 8) as u32).to_le_bytes());
    out.extend(body);
    std::fs::write(path, out)?;
    let mut w = ctx.warnings;
    w.sort();
    w.dedup();
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_writer_signed() {
        let mut b = Bits::new();
        b.sb(4, -1);
        b.ub(4, 3);
        assert_eq!(b.finish(), vec![0xF3]);
        assert_eq!(sbits(0), 1);
        assert_eq!(sbits(-1), 1);
        assert_eq!(sbits(1), 2);
        assert_eq!(sbits(-200), 9);
    }

    #[test]
    fn swf_header_and_tags_are_consistent() {
        let doc = crate::test_doc::sample();
        let p = std::env::temp_dir().join(format!("animara-swf-{}.swf", std::process::id()));
        export_swf(&doc, &p, &ExportOptions::default()).unwrap();
        let data = std::fs::read(&p).unwrap();
        assert_eq!(&data[0..3], b"FWS");
        assert_eq!(u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize, data.len());
        // Walk the tag stream and count ShowFrame tags.
        let nbits = (data[8] >> 3) as usize;
        let rect_bytes = (5 + nbits * 4).div_ceil(8);
        let mut i = 8 + rect_bytes + 4;
        let frames = u16::from_le_bytes([data[8 + rect_bytes + 2], data[8 + rect_bytes + 3]]);
        assert_eq!(frames, 12);
        let mut shows = 0;
        loop {
            let h = u16::from_le_bytes([data[i], data[i + 1]]);
            i += 2;
            let code = h >> 6;
            let mut len = (h & 0x3f) as usize;
            if len == 0x3f {
                len = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
                i += 4;
            }
            i += len;
            if code == 1 {
                shows += 1;
            }
            if code == 0 {
                break;
            }
        }
        assert_eq!(i, data.len());
        assert_eq!(shows, 12);
    }

    #[test]
    fn stored_zlib_roundtrips() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let z = zlib_stored::compress(&data);
        let mut dec = Vec::new();
        use std::io::Read;
        let mut zr = zip_test_inflate(&z);
        zr.read_to_end(&mut dec).unwrap();
        assert_eq!(dec, data);
    }

    fn zip_test_inflate(z: &[u8]) -> impl std::io::Read + '_ {
        // Skip zlib header/trailer and parse stored blocks manually.
        struct R(Vec<u8>, usize);
        impl std::io::Read for R {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let n = buf.len().min(self.0.len() - self.1);
                buf[..n].copy_from_slice(&self.0[self.1..self.1 + n]);
                self.1 += n;
                Ok(n)
            }
        }
        let mut out = Vec::new();
        let mut i = 2;
        loop {
            let last = z[i] & 1;
            let len = u16::from_le_bytes([z[i + 1], z[i + 2]]) as usize;
            out.extend_from_slice(&z[i + 5..i + 5 + len]);
            i += 5 + len;
            if last == 1 {
                break;
            }
        }
        R(out, 0)
    }
}
