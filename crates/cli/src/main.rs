//! `animara-cli` — headless rendering, export, conversion and validation.

use animara_document::Document;
use animara_exporters::{ExportOptions, VideoFormat};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "animara-cli — Animara command-line tools

USAGE:
  animara-cli info <file>
  animara-cli validate <file>
  animara-cli render <file> --frame <n> --out <image.png> [--scale <s>] [--transparent]
  animara-cli export <file> --out <path> [--format <fmt>] [--scale <s>] [--range <a-b>]
                     [--transparent] [--quality <1-100>] [--no-audio]
  animara-cli convert <input> <output>       (.anim / .xfl / .fla ↔ .anim / .xfl / .fla)
  animara-cli check-scripts <file>
  animara-cli bench [--layers <n>] [--shapes <n>] [--frames <n>] [--save <stress.anim>]
  animara-cli --version

FORMATS (default inferred from --out extension):
  png, png-seq, gif, apng, webp, mp4, webm, mov, svg, svg-seq, sprite, lottie, html, swf, xfl, fla

INPUT: .anim (native), .fla (CS5+), .xfl, .svg";

fn load(path: &Path) -> Result<Document, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "fla" => animara_importers::import_fla(path).map(|r| report(r.document, &r.warnings)).map_err(|e| e.to_string()),
        "xfl" => animara_importers::import_xfl(path).map(|r| report(r.document, &r.warnings)).map_err(|e| e.to_string()),
        "svg" => animara_importers::import_svg(path)
            .map(|r| {
                let mut d = Document::new(r.width.max(1.0), r.height.max(1.0), 24.0);
                d.library.bitmaps = r.bitmaps;
                d.scenes[0].timeline.layers[0].frames[0].elements = r.elements;
                report(d, &r.warnings)
            })
            .map_err(|e| e.to_string()),
        _ => animara_document::load_document(path).map_err(|e| e.to_string()),
    }
}

fn report(d: Document, warnings: &[String]) -> Document {
    for w in warnings {
        eprintln!("warning: {w}");
    }
    d
}

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn opts(args: &[String]) -> Result<ExportOptions, String> {
    let mut o = ExportOptions::default();
    if let Some(s) = arg(args, "--scale") {
        o.scale = s.parse().map_err(|_| "invalid --scale")?;
    }
    if let Some(r) = arg(args, "--range") {
        let (a, b) = r.split_once('-').ok_or("--range expects a-b")?;
        let a: u32 = a.parse().map_err(|_| "invalid range start")?;
        let b: u32 = b.parse().map_err(|_| "invalid range end")?;
        o.range = Some((a.max(1) - 1, b.max(1) - 1));
    }
    if let Some(q) = arg(args, "--quality") {
        o.quality = q.parse().map_err(|_| "invalid --quality")?;
    }
    o.transparent = args.iter().any(|a| a == "--transparent");
    o.include_audio = !args.iter().any(|a| a == "--no-audio");
    Ok(o)
}

fn run(args: Vec<String>) -> Result<(), String> {
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("--help");
    match cmd {
        "--version" | "-V" => println!("animara-cli {}", env!("CARGO_PKG_VERSION")),
        "info" => {
            let d = load(Path::new(args.get(1).ok_or(USAGE)?))?;
            println!("name: {}", d.name);
            println!("stage: {}x{} @ {} fps, background {}", d.width, d.height, d.fps, d.background.to_hex());
            println!("scenes: {} ({} frames total)", d.scenes.len(), d.total_frames());
            for s in &d.scenes {
                println!("  scene '{}': {} layers, {} frames", s.name, s.timeline.layers.len(), s.timeline.length());
            }
            println!(
                "library: {} symbols, {} bitmaps, {} sounds",
                d.library.symbols.len(),
                d.library.bitmaps.len(),
                d.library.sounds.len()
            );
        }
        "validate" => {
            let d = load(Path::new(args.get(1).ok_or(USAGE)?))?;
            let p = d.validate();
            if p.is_empty() {
                println!("valid");
            } else {
                for x in &p {
                    println!("problem: {x}");
                }
                return Err(format!("{} problem(s)", p.len()));
            }
        }
        "check-scripts" => {
            let d = load(Path::new(args.get(1).ok_or(USAGE)?))?;
            let mut errors = 0;
            let tls = d.scenes.iter().map(|s| (&s.name, &s.timeline)).chain(d.library.symbols.iter().map(|s| (&s.name, &s.timeline)));
            for (name, tl) in tls {
                for l in &tl.layers {
                    for k in &l.frames {
                        if let Err(e) = animara_scripting::check_syntax(&k.script) {
                            errors += 1;
                            println!("{name} / {} / frame {}: {e}", l.name, k.start + 1);
                        }
                    }
                }
            }
            if errors > 0 {
                return Err(format!("{errors} script error(s)"));
            }
            println!("all scripts compile");
        }
        "render" => {
            let d = load(Path::new(args.get(1).ok_or(USAGE)?))?;
            let frame: u32 = arg(&args, "--frame").unwrap_or("1").parse().map_err(|_| "invalid --frame")?;
            let out = arg(&args, "--out").ok_or("--out is required")?;
            animara_exporters::export_png(&d, frame.max(1) - 1, Path::new(out), &opts(&args)?).map_err(|e| e.to_string())?;
            println!("wrote {out}");
        }
        "export" => {
            let d = load(Path::new(args.get(1).ok_or(USAGE)?))?;
            let out = PathBuf::from(arg(&args, "--out").ok_or("--out is required")?);
            let ext = out.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let fmt = arg(&args, "--format").map(String::from).unwrap_or_else(|| match ext.as_str() {
                "json" => "lottie".into(),
                "html" | "htm" => "html".into(),
                "" => "png-seq".into(),
                e => e.to_string(),
            });
            let o = opts(&args)?;
            let base = d.name.clone();
            let e = |e: animara_exporters::ExportError| e.to_string();
            let notes: Vec<String> = match fmt.as_str() {
                "png" => animara_exporters::export_png(&d, o.range.map(|r| r.0).unwrap_or(0), &out, &o).map(|_| vec![]).map_err(e)?,
                "png-seq" => {
                    animara_exporters::export_png_sequence(&d, &out, &base, &o).map(|v| vec![format!("{} files", v.len())]).map_err(e)?
                }
                "gif" => animara_exporters::export_gif(&d, &out, &o).map(|_| vec![]).map_err(e)?,
                "apng" => animara_exporters::export_apng(&d, &out, &o).map(|_| vec![]).map_err(e)?,
                "webp" => animara_exporters::export_video(&d, &out, VideoFormat::WebP, &o).map(|_| vec![]).map_err(e)?,
                "mp4" => animara_exporters::export_video(&d, &out, VideoFormat::Mp4, &o).map(|_| vec![]).map_err(e)?,
                "webm" => animara_exporters::export_video(&d, &out, VideoFormat::WebM, &o).map(|_| vec![]).map_err(e)?,
                "mov" => animara_exporters::export_video(&d, &out, VideoFormat::Mov, &o).map(|_| vec![]).map_err(e)?,
                "svg" => animara_exporters::export_svg_frame(&d, o.range.map(|r| r.0).unwrap_or(0), &out, &o).map(|_| vec![]).map_err(e)?,
                "svg-seq" => {
                    animara_exporters::export_svg_sequence(&d, &out, &base, &o).map(|v| vec![format!("{} files", v.len())]).map_err(e)?
                }
                "sprite" => {
                    animara_exporters::export_sprite_sheet(&d, &out, &o).map(|j| vec![format!("metadata {}", j.display())]).map_err(e)?
                }
                "lottie" => animara_exporters::export_lottie(&d, &out, &o).map_err(e)?,
                "html" => animara_exporters::export_html5(&d, &out, &o).map(|_| vec![]).map_err(e)?,
                "swf" => animara_exporters::export_swf(&d, &out, &o).map_err(e)?,
                "xfl" => animara_exporters::export_xfl(&d, &out).map(|p| vec![format!("wrote {}", p.display())]).map_err(e)?,
                "fla" => animara_exporters::export_fla(&d, &out).map(|_| vec![]).map_err(e)?,
                other => return Err(format!("unknown format '{other}'")),
            };
            for n in notes {
                eprintln!("note: {n}");
            }
            println!("exported {fmt} → {}", out.display());
        }
        "bench" => bench(&args)?,
        "convert" => {
            let input = Path::new(args.get(1).ok_or(USAGE)?);
            let output = Path::new(args.get(2).ok_or(USAGE)?);
            let d = load(input)?;
            match output.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
                Some("xfl") => {
                    animara_exporters::export_xfl(&d, output).map_err(|e| e.to_string())?;
                }
                Some("fla") => animara_exporters::export_fla(&d, output).map_err(|e| e.to_string())?,
                _ => animara_document::save_document(&d, output).map_err(|e| e.to_string())?,
            }
            println!("converted {} → {}", input.display(), output.display());
        }
        _ => {
            println!("{USAGE}");
        }
    }
    Ok(())
}

/// Synthetic stress document: `layers` layers × `shapes` tweened shapes inside nested symbols.
fn stress_doc(layers: usize, shapes: usize, frames: u32) -> Document {
    use animara_document::*;
    use animara_geometry::{Affine, Path, Rect, Vec2};
    let mut doc = Document::new(1920.0, 1080.0, 30.0);
    // A nested symbol: a star inside a graphic symbol inside a movie clip.
    let mut inner = Symbol::new("Star", SymbolKind::Graphic);
    inner.timeline.layers[0].frames[0].elements.push(Element::shape(vec![ShapePath {
        path: Path::star(Vec2::ZERO, 30.0, 5, 0.5, 0.0),
        fill: Some(Fill::Radial {
            stops: vec![GradientStop { offset: 0.0, color: Color::WHITE }, GradientStop { offset: 1.0, color: Color::rgb(0.9, 0.3, 0.1) }],
            matrix: Affine::scale(30.0, 30.0),
            spread: SpreadMode::Pad,
            focal: 0.0,
        }),
        stroke: Some(Stroke::solid(Color::BLACK, 2.0)),
        fill_rule: animara_geometry::FillRule::NonZero,
        primitive: None,
    }]));
    let inner_id = doc.library.add_symbol(inner);
    let mut outer = Symbol::new("Spinner", SymbolKind::Graphic);
    outer.timeline.layers[0].frames[0].elements.push(Element::new(ElementKind::Instance(SymbolInstance::new(inner_id))));
    let outer_id = doc.library.add_symbol(outer);
    let tl = &mut doc.scenes[0].timeline;
    tl.layers.clear();
    for l in 0..layers {
        let mut layer = Layer::new(&format!("Layer {l}"));
        layer.frames = vec![Keyframe::new(0, frames - 1), Keyframe::new(frames - 1, 1)];
        for s in 0..shapes {
            let x = (s * 97 % 1800) as f64 + 60.0;
            let y = ((l * 131 + s * 53) % 1000) as f64 + 40.0;
            let e = if s % 3 == 0 {
                Element::new(ElementKind::Instance(SymbolInstance::new(outer_id))).with_transform(Affine::translate(x, y))
            } else {
                Element::shape(vec![ShapePath {
                    path: Path::ellipse(Rect::new(-15.0, -10.0, 15.0, 10.0)),
                    fill: Some(Fill::solid(Color::from_hsv((s * 37 % 360) as f32, 0.7, 0.9, 0.9))),
                    stroke: Some(Stroke::solid(Color::BLACK, 1.0)),
                    fill_rule: animara_geometry::FillRule::NonZero,
                    primitive: None,
                }])
                .with_transform(Affine::translate(x, y))
            };
            let mut end = e.clone();
            end.transform = Affine::translate(1920.0 - x, 1080.0 - y) * Affine::rotate(3.0);
            layer.frames[0].elements.push(e);
            layer.frames[1].elements.push(end);
        }
        layer.frames[0].tween = Some(Tween::new(TweenKind::Classic));
        tl.layers.push(layer);
    }
    doc
}

fn bench(args: &[String]) -> Result<(), String> {
    use std::time::Instant;
    let layers: usize = arg(args, "--layers").unwrap_or("20").parse().map_err(|_| "invalid --layers")?;
    let shapes: usize = arg(args, "--shapes").unwrap_or("50").parse().map_err(|_| "invalid --shapes")?;
    let frames: u32 = arg(args, "--frames").unwrap_or("60").parse().map_err(|_| "invalid --frames")?;
    let doc = stress_doc(layers, shapes, frames.max(2));
    if let Some(out) = arg(args, "--save") {
        animara_document::save_document(&doc, Path::new(out)).map_err(|e| e.to_string())?;
    }
    println!("stress document: {layers} layers × {shapes} elements (1/3 nested symbol instances), {frames} frames, 1920×1080");
    let opts = animara_animation::EvalOptions::export();
    let t = Instant::now();
    let mut items = 0;
    for f in 0..frames {
        items += animara_animation::evaluate_document_frame(&doc, 0, f, &opts).items.len();
    }
    let eval = t.elapsed().as_secs_f64() / frames as f64;
    let mut cache = animara_renderer::RenderCache::new();
    let t = Instant::now();
    for f in 0..frames {
        animara_renderer::render_frame(&doc, 0, f, 1.0, false, &mut cache).ok_or("render failed")?;
    }
    let render = t.elapsed().as_secs_f64() / frames as f64;
    let t = Instant::now();
    let bytes = animara_document::save_to_bytes(&doc).map_err(|e| e.to_string())?;
    let save = t.elapsed().as_secs_f64();
    let t = Instant::now();
    animara_document::load_from_bytes(&bytes).map_err(|e| e.to_string())?;
    let load = t.elapsed().as_secs_f64();
    println!(
        "evaluate frame (tweens + nested symbols): {:8.3} ms/frame  ({} top-level items/frame)",
        eval * 1000.0,
        items / frames as usize
    );
    println!("render frame 1920×1080 (CPU, tiny-skia):  {:8.3} ms/frame  ({:.1} fps)", render * 1000.0, 1.0 / render);
    println!("save .anim ({} KiB):                     {:8.3} ms", bytes.len() / 1024, save * 1000.0);
    println!("load .anim:                               {:8.3} ms", load * 1000.0);
    Ok(())
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
