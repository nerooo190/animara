//! Export pipelines. Every exporter produces real files:
//!
//! * Raster: PNG, PNG sequence, animated GIF, APNG, sprite sheet (PNG + JSON)
//! * Video (via external ffmpeg): MP4/H.264, WebM/VP9 (alpha), MOV/ProRes 4444 (alpha), animated WebP
//! * Vector: SVG frame / sequence, Lottie JSON, HTML5 Canvas (self-contained page), SWF
//! * Document: XFL folder and ZIP-based FLA

mod html5;
mod lottie;
mod svg;
mod swf;
mod xfl;

use animara_animation::{EvalOptions, evaluate_document_frame, scene_at_global_frame};
use animara_document::Document;
use animara_renderer::{RenderCache, pixmap_to_rgba, render_display_list, tiny_skia::Pixmap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub use html5::export_html5;
pub use lottie::export_lottie;
pub use svg::{export_svg_frame, export_svg_sequence, svg_for_frame};
pub use swf::export_swf;
pub use xfl::{export_fla, export_xfl, xfl_files};

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("encoding error: {0}")]
    Encode(String),
    #[error("ffmpeg is required for this format but was not found on PATH")]
    NoFfmpeg,
    #[error("ffmpeg failed: {0}")]
    Ffmpeg(String),
    #[error("nothing to export: {0}")]
    Empty(String),
}

/// Common export settings.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Output pixels per document pixel.
    pub scale: f64,
    /// Global frame range (scenes back to back), inclusive. `None` = whole movie.
    pub range: Option<(u32, u32)>,
    pub transparent: bool,
    /// 1..=100 quality for lossy formats.
    pub quality: u8,
    /// 0 = loop forever (GIF/APNG/WebP), otherwise play count.
    pub loop_count: u16,
    pub include_audio: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions { scale: 1.0, range: None, transparent: false, quality: 90, loop_count: 0, include_audio: true }
    }
}

impl ExportOptions {
    pub fn frames(&self, doc: &Document) -> (u32, u32) {
        let total = doc.total_frames().max(1);
        match self.range {
            Some((a, b)) => (a.min(total - 1), b.min(total - 1).max(a.min(total - 1))),
            None => (0, total - 1),
        }
    }
}

/// Render one global frame.
pub fn render_global_frame(doc: &Document, global: u32, opts: &ExportOptions, cache: &mut RenderCache) -> Result<Pixmap, ExportError> {
    let (scene, f) = scene_at_global_frame(doc, global);
    let dl = evaluate_document_frame(doc, scene, f, &EvalOptions::export());
    render_display_list(&dl, doc, cache, opts.scale, opts.transparent).ok_or_else(|| ExportError::Encode("invalid output size".into()))
}

/// Render every frame in range, calling `sink` with straight RGBA bytes.
pub fn for_each_frame(
    doc: &Document,
    opts: &ExportOptions,
    mut sink: impl FnMut(u32, &Pixmap) -> Result<(), ExportError>,
) -> Result<(u32, u32), ExportError> {
    let (a, b) = opts.frames(doc);
    let mut cache = RenderCache::new();
    let mut size = (0, 0);
    for g in a..=b {
        let pm = render_global_frame(doc, g, opts, &mut cache)?;
        size = (pm.width(), pm.height());
        sink(g - a, &pm)?;
    }
    Ok(size)
}

pub fn export_png(doc: &Document, global: u32, path: &Path, opts: &ExportOptions) -> Result<(), ExportError> {
    let pm = render_global_frame(doc, global, opts, &mut RenderCache::new())?;
    pm.save_png(path).map_err(|e| ExportError::Encode(e.to_string()))
}

/// `base_0001.png`, `base_0002.png`, … in `dir`.
pub fn export_png_sequence(doc: &Document, dir: &Path, base: &str, opts: &ExportOptions) -> Result<Vec<PathBuf>, ExportError> {
    std::fs::create_dir_all(dir)?;
    let mut out = Vec::new();
    for_each_frame(doc, opts, |i, pm| {
        let p = dir.join(format!("{base}_{:04}.png", i + 1));
        pm.save_png(&p).map_err(|e| ExportError::Encode(e.to_string()))?;
        out.push(p);
        Ok(())
    })?;
    Ok(out)
}

fn frame_delay_cs(doc: &Document) -> u16 {
    (100.0 / doc.fps.max(0.01)).round().clamp(1.0, 65535.0) as u16
}

pub fn export_gif(doc: &Document, path: &Path, opts: &ExportOptions) -> Result<(), ExportError> {
    let file = std::fs::File::create(path)?;
    let mut enc: Option<gif::Encoder<std::fs::File>> = None;
    let mut file = Some(file);
    let delay = frame_delay_cs(doc);
    // NeuQuant speed: 1 = best, 30 = fastest. Map quality to speed.
    let speed = (31 - (opts.quality as i32 * 30 / 100)).clamp(1, 30);
    for_each_frame(doc, opts, |_, pm| {
        let (w, h) = (pm.width() as u16, pm.height() as u16);
        if enc.is_none() {
            let mut e = gif::Encoder::new(file.take().unwrap(), w, h, &[]).map_err(|e| ExportError::Encode(e.to_string()))?;
            e.set_repeat(if opts.loop_count == 0 { gif::Repeat::Infinite } else { gif::Repeat::Finite(opts.loop_count) })
                .map_err(|e| ExportError::Encode(e.to_string()))?;
            enc = Some(e);
        }
        let mut rgba = pixmap_to_rgba(pm);
        let mut frame = gif::Frame::from_rgba_speed(w, h, &mut rgba, speed);
        frame.delay = delay;
        frame.dispose = gif::DisposalMethod::Background;
        enc.as_mut().unwrap().write_frame(&frame).map_err(|e| ExportError::Encode(e.to_string()))
    })?;
    Ok(())
}

pub fn export_apng(doc: &Document, path: &Path, opts: &ExportOptions) -> Result<(), ExportError> {
    let (a, b) = opts.frames(doc);
    let n = b - a + 1;
    let mut frames = Vec::new();
    let (w, h) = for_each_frame(doc, opts, |_, pm| {
        frames.push(pixmap_to_rgba(pm));
        Ok(())
    })?;
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_animated(n, opts.loop_count as u32).map_err(|e| ExportError::Encode(e.to_string()))?;
    let fps = doc.fps.max(0.01);
    let (num, den) = ((1000.0 / fps).round() as u16, 1000u16);
    enc.set_frame_delay(num.max(1), den).map_err(|e| ExportError::Encode(e.to_string()))?;
    let mut writer = enc.write_header().map_err(|e| ExportError::Encode(e.to_string()))?;
    for f in frames {
        writer.write_image_data(&f).map_err(|e| ExportError::Encode(e.to_string()))?;
    }
    writer.finish().map_err(|e| ExportError::Encode(e.to_string()))?;
    Ok(())
}

/// Sprite sheet: frames packed in a grid + JSON metadata (hash format).
pub fn export_sprite_sheet(doc: &Document, png_path: &Path, opts: &ExportOptions) -> Result<PathBuf, ExportError> {
    let mut frames = Vec::new();
    let (w, h) = for_each_frame(doc, opts, |_, pm| {
        frames.push(pm.clone());
        Ok(())
    })?;
    if frames.is_empty() {
        return Err(ExportError::Empty("no frames".into()));
    }
    let n = frames.len() as u32;
    let cols = (n as f64).sqrt().ceil() as u32;
    let rows = n.div_ceil(cols);
    let mut sheet = Pixmap::new(cols * w, rows * h).ok_or_else(|| ExportError::Encode("sheet too large".into()))?;
    let mut json_frames = serde_json::Map::new();
    let stem = png_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "sheet".into());
    for (i, f) in frames.iter().enumerate() {
        let (x, y) = ((i as u32 % cols) * w, (i as u32 / cols) * h);
        sheet.draw_pixmap(x as i32, y as i32, f.as_ref(), &Default::default(), animara_renderer::tiny_skia::Transform::identity(), None);
        json_frames.insert(
            format!("{stem}_{:04}", i + 1),
            serde_json::json!({ "frame": {"x": x, "y": y, "w": w, "h": h}, "rotated": false, "trimmed": false,
                "spriteSourceSize": {"x": 0, "y": 0, "w": w, "h": h}, "sourceSize": {"w": w, "h": h} }),
        );
    }
    sheet.save_png(png_path).map_err(|e| ExportError::Encode(e.to_string()))?;
    let meta = serde_json::json!({
        "frames": json_frames,
        "meta": { "app": "Animara", "version": env!("CARGO_PKG_VERSION"), "image": png_path.file_name().map(|s| s.to_string_lossy().into_owned()),
                  "format": "RGBA8888", "size": {"w": cols * w, "h": rows * h}, "scale": opts.scale.to_string(), "frameRate": doc.fps }
    });
    let json_path = png_path.with_extension("json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&meta).unwrap_or_default())?;
    Ok(json_path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    Mp4,
    WebM,
    Mov,
    WebP,
}

/// Encode video by piping raw RGBA frames into ffmpeg (audio is mixed from the timeline).
pub fn export_video(doc: &Document, path: &Path, format: VideoFormat, opts: &ExportOptions) -> Result<(), ExportError> {
    if !animara_media::ffmpeg_available() {
        return Err(ExportError::NoFfmpeg);
    }
    let (a, b) = opts.frames(doc);
    let mut cache = RenderCache::new();
    let first = render_global_frame(doc, a, opts, &mut cache)?;
    let (w, h) = (first.width(), first.height());
    let alpha = opts.transparent && matches!(format, VideoFormat::WebM | VideoFormat::Mov | VideoFormat::WebP);
    let audio = if opts.include_audio && format != VideoFormat::WebP && animara_media::has_audio(doc) {
        let (s0, f0) = scene_at_global_frame(doc, a);
        let (s1, f1) = scene_at_global_frame(doc, b);
        let pcm = if s0 == s1 {
            animara_media::mixdown_timeline(doc, &doc.scenes[s0].timeline, f0, f1, 44100)
        } else {
            animara_media::mixdown_document(doc, 44100)
        };
        let p = std::env::temp_dir().join(format!("animara-export-{}.wav", std::process::id()));
        std::fs::write(&p, animara_media::encode_wav(&pcm))?;
        Some(p)
    } else {
        None
    };
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &doc.fps.to_string(), "-i", "-"]);
    if let Some(a) = &audio {
        cmd.arg("-i").arg(a);
    }
    let crf = |lo: u32, hi: u32| (hi as i32 - ((hi - lo) as i32 * opts.quality as i32 / 100)).to_string();
    match format {
        VideoFormat::Mp4 => {
            cmd.args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", &crf(14, 35), "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2"]);
            if audio.is_some() {
                cmd.args(["-c:a", "aac", "-b:a", "192k"]);
            }
            cmd.args(["-movflags", "+faststart"]);
        }
        VideoFormat::WebM => {
            cmd.args(["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", &crf(15, 50), "-pix_fmt", if alpha { "yuva420p" } else { "yuv420p" }]);
            if audio.is_some() {
                cmd.args(["-c:a", "libopus"]);
            }
        }
        VideoFormat::Mov => {
            cmd.args(["-c:v", "prores_ks", "-profile:v", "4", "-pix_fmt", if alpha { "yuva444p10le" } else { "yuv444p10le" }]);
            if audio.is_some() {
                cmd.args(["-c:a", "pcm_s16le"]);
            }
        }
        VideoFormat::WebP => {
            cmd.args([
                "-c:v",
                "libwebp_anim",
                "-lossless",
                if opts.quality >= 100 { "1" } else { "0" },
                "-q:v",
                &opts.quality.to_string(),
                "-loop",
                &opts.loop_count.to_string(),
            ]);
            if alpha {
                cmd.args(["-pix_fmt", "yuva420p"]);
            }
        }
    }
    if audio.is_some() {
        cmd.arg("-shortest");
    }
    cmd.arg(path).stdin(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    {
        let stdin = child.stdin.as_mut().ok_or_else(|| ExportError::Ffmpeg("no stdin".into()))?;
        stdin.write_all(&pixmap_to_rgba(&first))?;
        for g in a + 1..=b {
            let pm = render_global_frame(doc, g, opts, &mut cache)?;
            if stdin.write_all(&pixmap_to_rgba(&pm)).is_err() {
                break;
            }
        }
    }
    drop(child.stdin.take());
    let out = child.wait_with_output()?;
    if let Some(a) = audio {
        let _ = std::fs::remove_file(a);
    }
    if !out.status.success() {
        return Err(ExportError::Ffmpeg(String::from_utf8_lossy(&out.stderr).into_owned()));
    }
    Ok(())
}

/// Export the timeline's mixed audio as WAV.
pub fn export_audio_wav(doc: &Document, path: &Path) -> Result<(), ExportError> {
    std::fs::write(path, animara_media::encode_wav(&animara_media::mixdown_document(doc, 44100)))?;
    Ok(())
}

/// Convert a cubic Bézier to quadratic segments (for formats that only support quadratics).
pub(crate) fn cubic_to_quads(
    p0: animara_geometry::Vec2,
    c1: animara_geometry::Vec2,
    c2: animara_geometry::Vec2,
    p3: animara_geometry::Vec2,
) -> Vec<(animara_geometry::Vec2, animara_geometry::Vec2)> {
    use animara_geometry::Segment;
    let len = p0.distance(c1) + c1.distance(c2) + c2.distance(p3);
    let n = ((len / 20.0).ceil() as usize).clamp(1, 16);
    let seg = Segment::Cubic(p0, c1, c2, p3);
    (0..n)
        .map(|i| {
            let t0 = i as f64 / n as f64;
            let t1 = (i + 1) as f64 / n as f64;
            let a = seg.eval(t0);
            let b = seg.eval(t1);
            let da = seg.derivative(t0) * ((t1 - t0) / 3.0);
            let db = seg.derivative(t1) * ((t1 - t0) / 3.0);
            // Midpoint of the two tangent-implied quadratic controls.
            let ctrl = ((a + da * 1.5) + (b - db * 1.5)) * 0.5;
            (ctrl, b)
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod test_doc {
    use animara_document::*;
    use animara_geometry::{Affine, Path, Rect};

    /// A small animated document exercising tweens, symbols, text, gradients and a mask.
    pub fn sample() -> Document {
        let mut doc = Document::new(160.0, 120.0, 12.0);
        let mut sym = Symbol::new("Box", SymbolKind::Graphic);
        sym.timeline.layers[0].frames[0].elements.push(Element::shape(vec![ShapePath {
            path: Path::rect(Rect::new(-10.0, -10.0, 10.0, 10.0)),
            fill: Some(Fill::Linear {
                stops: vec![
                    GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
                    GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 1.0) },
                ],
                matrix: Affine::scale(10.0, 10.0),
                spread: SpreadMode::Pad,
            }),
            stroke: Some(Stroke::solid(Color::BLACK, 2.0)),
            fill_rule: animara_geometry::FillRule::NonZero,
            primitive: None,
        }]));
        let sid = doc.library.add_symbol(sym);
        let l = &mut doc.scenes[0].timeline.layers[0];
        let e = Element::new(ElementKind::Instance(SymbolInstance::new(sid))).with_transform(Affine::translate(20.0, 60.0));
        l.frames[0].elements.push(e);
        l.insert_keyframe(11, None);
        l.frames[1].elements[0].transform = Affine::translate(140.0, 60.0) * Affine::rotate(1.0);
        l.frames[0].tween = Some(Tween::new(TweenKind::Classic));
        l.frames[0].script = "stop();".into();
        let tid = doc.scenes[0].timeline.add_layer("Text", 0);
        let t = doc.scenes[0].timeline.layer_mut(tid).unwrap();
        t.frames[0].elements.push(
            Element::new(ElementKind::Text(TextElement::new("Hi", TextStyle::default()))).with_transform(Affine::translate(5.0, 5.0)),
        );
        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("animara-export-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn gif_apng_png_sequence_and_sheet() {
        let doc = test_doc::sample();
        let opts = ExportOptions::default();
        let g = tmp("a.gif");
        export_gif(&doc, &g, &opts).unwrap();
        let dec = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(std::fs::File::open(&g).unwrap())).unwrap();
        use image::AnimationDecoder;
        assert_eq!(dec.into_frames().count(), 12);
        let a = tmp("a.png");
        export_apng(&doc, &a, &opts).unwrap();
        let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&a).unwrap()));
        let r = dec.read_info().unwrap();
        assert_eq!(r.info().animation_control().unwrap().num_frames, 12);
        let seq = export_png_sequence(&doc, &tmp("seq"), "f", &ExportOptions { range: Some((0, 2)), ..opts.clone() }).unwrap();
        assert_eq!(seq.len(), 3);
        let json = export_sprite_sheet(&doc, &tmp("sheet.png"), &opts).unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
        assert_eq!(v["frames"].as_object().unwrap().len(), 12);
        assert_eq!(v["meta"]["size"]["w"], 160 * 4);
    }

    #[test]
    fn video_when_ffmpeg_present() {
        if !animara_media::ffmpeg_available() {
            return;
        }
        let doc = test_doc::sample();
        let p = tmp("v.mp4");
        export_video(&doc, &p, VideoFormat::Mp4, &ExportOptions::default()).unwrap();
        assert!(std::fs::metadata(&p).unwrap().len() > 500);
        let p = tmp("v.webm");
        export_video(&doc, &p, VideoFormat::WebM, &ExportOptions { transparent: true, ..Default::default() }).unwrap();
        assert!(std::fs::metadata(&p).unwrap().len() > 200);
    }

    #[test]
    fn cubic_quads_end_at_endpoint() {
        use animara_geometry::Vec2;
        let q = cubic_to_quads(Vec2::new(0.0, 0.0), Vec2::new(0.0, 50.0), Vec2::new(100.0, 50.0), Vec2::new(100.0, 0.0));
        assert_eq!(q.last().unwrap().1, Vec2::new(100.0, 0.0));
        assert!(q.len() > 1);
    }
}
