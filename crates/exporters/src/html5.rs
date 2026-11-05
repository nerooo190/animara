//! HTML5 Canvas export: a single self-contained HTML page with baked vector
//! frames and a small JavaScript player (looping, frame actions `stop()`,
//! `play()`, `gotoAndPlay/Stop`, `nextFrame/prevFrame`, click handlers on named
//! instances, synchronized audio).

use crate::{ExportError, ExportOptions};
use animara_animation::{DisplayItem, EvalOptions, evaluate_document_frame, scene_at_global_frame};
use animara_document::{BlendMode, ColorTransform, Document, ElementKind, Fill, Filter, LineCap, LineJoin, StrokeStyle};
use animara_geometry::Affine;
use base64::Engine as _;
use serde_json::{Value, json};
use std::collections::HashMap;

fn m(a: &Affine) -> Value {
    let r = |v: f64| (v * 1000.0).round() / 1000.0;
    json!([r(a.a), r(a.b), r(a.c), r(a.d), r(a.tx), r(a.ty)])
}

fn css(c: animara_document::Color) -> String {
    let [r, g, b, _] = c.to_rgba8();
    format!("rgba({r},{g},{b},{:.3})", c.a)
}

fn fill_spec(f: &Fill, ct: &ColorTransform) -> Value {
    match f {
        Fill::Solid { color } => json!({"t": "c", "c": css(ct.apply(*color))}),
        Fill::Linear { stops, matrix, .. } => json!({"t": "l", "m": m(matrix), "st": stops.iter().map(|s| json!([s.offset, css(ct.apply(s.color))])).collect::<Vec<_>>()}),
        Fill::Radial { stops, matrix, focal, .. } => {
            json!({"t": "r", "m": m(matrix), "fo": focal, "st": stops.iter().map(|s| json!([s.offset, css(ct.apply(s.color))])).collect::<Vec<_>>()})
        }
        Fill::Bitmap { .. } => json!({"t": "c", "c": "rgba(128,128,128,1)"}),
    }
}

struct Ctx<'a> {
    doc: &'a Document,
    images: Vec<(animara_document::Id, String)>,
}

impl Ctx<'_> {
    fn image(&mut self, id: animara_document::Id) -> Option<usize> {
        if let Some(i) = self.images.iter().position(|x| x.0 == id) {
            return Some(i);
        }
        let b = self.doc.library.bitmap(id)?;
        let mime = if b.format == "jpg" { "jpeg" } else { b.format.as_str() };
        self.images.push((id, format!("data:image/{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(b.data.as_slice()))));
        Some(self.images.len() - 1)
    }

    fn ops(&mut self, items: &[DisplayItem]) -> Vec<Value> {
        let mut out = Vec::new();
        for it in items {
            match it {
                DisplayItem::Shape { shape, transform, color, .. } => {
                    for sp in &shape.paths {
                        let stroke = sp.stroke.as_ref().map(|s| {
                            json!({"w": if matches!(s.style, StrokeStyle::Hairline) { 1.0 / transform.mean_scale().max(1e-9) } else { s.width },
                                "c": fill_spec(&s.fill, color),
                                "cap": match s.cap { LineCap::Butt => "butt", LineCap::Round => "round", LineCap::Square => "square" },
                                "join": match s.join { LineJoin::Miter => "miter", LineJoin::Round => "round", LineJoin::Bevel => "bevel" },
                                "dash": match s.style { StrokeStyle::Dashed { dash, gap } => json!([dash, gap]), StrokeStyle::Dotted { spacing } => json!([0.01, spacing + s.width]), _ => json!([]) }})
                        });
                        out.push(json!({"k": "p", "d": sp.path.to_svg(), "m": m(transform), "f": sp.fill.as_ref().map(|f| fill_spec(f, color)), "s": stroke,
                            "r": if sp.fill_rule == animara_geometry::FillRule::EvenOdd { "evenodd" } else { "nonzero" }}));
                    }
                }
                DisplayItem::Text { text, transform, color, .. } => {
                    for (p, c) in animara_text::fonts().layout(text).to_paths() {
                        out.push(json!({"k": "p", "d": p.to_svg(), "m": m(transform), "f": {"t": "c", "c": css(color.apply(c))}, "s": null, "r": "nonzero"}));
                    }
                }
                DisplayItem::Bitmap { bitmap, transform, color, .. } => {
                    if let Some(i) = self.image(bitmap.asset) {
                        out.push(json!({"k": "i", "img": i, "m": m(transform), "a": color.mul[3]}));
                    }
                }
                DisplayItem::Group { items, filters, blend, scale, .. } => {
                    let flt: Vec<String> = filters
                        .iter()
                        .map(|f| match *f {
                            Filter::Blur { blur_x, blur_y, .. } => format!("blur({:.1}px)", (blur_x + blur_y) / 4.0 * scale),
                            Filter::DropShadow { angle, distance, blur_x, color, .. } => {
                                let a = angle.to_radians();
                                format!("drop-shadow({:.1}px {:.1}px {:.1}px {})", a.cos() * distance * scale, a.sin() * distance * scale, blur_x / 2.0 * scale, css(color))
                            }
                            Filter::Glow { blur_x, color, .. } => format!("drop-shadow(0 0 {:.1}px {})", blur_x / 2.0 * scale, css(color)),
                            Filter::AdjustColor { brightness, contrast, saturation, hue } => format!(
                                "brightness({:.2}) contrast({:.2}) saturate({:.2}) hue-rotate({:.0}deg)",
                                1.0 + brightness / 100.0,
                                1.0 + contrast / 100.0,
                                1.0 + saturation / 100.0,
                                hue
                            ),
                        })
                        .collect();
                    let bm = match blend {
                        BlendMode::Multiply => "multiply",
                        BlendMode::Screen => "screen",
                        BlendMode::Overlay => "overlay",
                        BlendMode::HardLight => "hard-light",
                        BlendMode::Lighten => "lighten",
                        BlendMode::Darken => "darken",
                        BlendMode::Difference | BlendMode::Invert => "difference",
                        BlendMode::Add => "lighter",
                        BlendMode::Subtract => "exclusion",
                        BlendMode::Alpha => "destination-in",
                        BlendMode::Erase => "destination-out",
                        _ => "source-over",
                    };
                    out.push(json!({"k": "g", "ops": self.ops(items), "flt": flt.join(" "), "bm": bm}));
                }
                DisplayItem::Masked { mask, content } => {
                    out.push(json!({"k": "m", "mask": self.ops(mask), "ops": self.ops(content)}));
                }
            }
        }
        out
    }
}

/// Parse the timeline-control subset of frame scripts into JSON actions.
fn parse_actions(src: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let s = src;
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < s.len() {
        let rest = &s[i..];
        let take_arg = |r: &str, name: &str| -> Option<(String, usize)> {
            let start = name.len() + 1;
            let end = r[start..].find(')')?;
            Some((r[start..start + end].trim().to_string(), start + end + 1))
        };
        let frame_arg = |a: &str| -> Value {
            let t = a.trim().trim_matches('"').trim_matches('\'');
            match t.parse::<u32>() {
                Ok(n) => json!(n),
                Err(_) => json!(t),
            }
        };
        if rest.starts_with("on_click(") {
            // on_click("name", || { ... });
            let Some(q1) = rest.find('"') else { break };
            let Some(q2) = rest[q1 + 1..].find('"') else { break };
            let name = &rest[q1 + 1..q1 + 1 + q2];
            let Some(b0) = rest.find('{') else { break };
            let mut depth = 0;
            let mut end = b0;
            for (k, ch) in rest[b0..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = b0 + k;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            out.push(json!(["click", name, parse_actions(&rest[b0 + 1..end])]));
            i += end + 1;
            continue;
        }
        let mut matched = false;
        for name in ["gotoAndPlay", "gotoAndStop"] {
            if rest.starts_with(&format!("{name}(")) {
                if let Some((a, len)) = take_arg(rest, name) {
                    out.push(json!(["goto", frame_arg(&a), name == "gotoAndPlay"]));
                    i += len;
                    matched = true;
                }
                break;
            }
        }
        if matched {
            continue;
        }
        for (name, act) in [("stop()", "stop"), ("play()", "play"), ("nextFrame()", "next"), ("prevFrame()", "prev")] {
            if rest.starts_with(name) && (i == 0 || !(bytes[i - 1] as char).is_alphanumeric() && bytes[i - 1] != b'.') {
                out.push(json!([act]));
                i += name.len();
                matched = true;
                break;
            }
        }
        if !matched {
            i += rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        }
    }
    out
}

const PLAYER_JS: &str = r#"
(function(){
const D=JSON.parse(document.getElementById('anim-data').textContent);
const cv=document.getElementById('stage');const ctx=cv.getContext('2d');
const dpr=window.devicePixelRatio||1;cv.width=D.w*dpr;cv.height=D.h*dpr;
const imgs=D.images.map(s=>{const i=new Image();i.src=s;return i;});
const paths={};function P(d){return paths[d]||(paths[d]=new Path2D(d));}
let frame=0,playing=true,last=performance.now(),acc=0;const audio=D.audio?new Audio(D.audio):null;
function grad(c,f){if(f.t==='c')return f.c;const g=f.m;let gr;
 if(f.t==='l'){gr=c.createLinearGradient(-g[0]+g[4],-g[1]+g[5],g[0]+g[4],g[1]+g[5]);}
 else{const r=Math.hypot(g[0],g[1]);gr=c.createRadialGradient(g[4]+g[0]*f.fo,g[5]+g[1]*f.fo,0,g[4],g[5],r);}
 for(const s of f.st)gr.addColorStop(Math.min(1,Math.max(0,s[0])),s[1]);return gr;}
function draw(c,ops,base){for(const o of ops){
 if(o.k==='p'){c.setTransform(base.multiply(new DOMMatrix(o.m)));const p=P(o.d);
  if(o.f){c.fillStyle=grad(c,o.f);c.fill(p,o.r);}
  if(o.s){c.strokeStyle=grad(c,o.s.c);c.lineWidth=o.s.w;c.lineCap=o.s.cap;c.lineJoin=o.s.join;c.setLineDash(o.s.dash);c.stroke(p);c.setLineDash([]);}}
 else if(o.k==='i'){c.setTransform(base.multiply(new DOMMatrix(o.m)));c.globalAlpha=o.a;if(imgs[o.img].complete)c.drawImage(imgs[o.img],0,0);c.globalAlpha=1;}
 else if(o.k==='g'){const off=document.createElement('canvas');off.width=cv.width;off.height=cv.height;const oc=off.getContext('2d');
  draw(oc,o.ops,base);c.save();c.setTransform(1,0,0,1,0,0);c.filter=o.flt||'none';c.globalCompositeOperation=o.bm;c.drawImage(off,0,0);c.restore();}
 else if(o.k==='m'){const a=document.createElement('canvas');a.width=cv.width;a.height=cv.height;const ac=a.getContext('2d');draw(ac,o.ops,base);
  const b=document.createElement('canvas');b.width=cv.width;b.height=cv.height;const bc=b.getContext('2d');draw(bc,o.mask,base);
  ac.setTransform(1,0,0,1,0,0);ac.globalCompositeOperation='destination-in';ac.drawImage(b,0,0);
  c.save();c.setTransform(1,0,0,1,0,0);c.drawImage(a,0,0);c.restore();}}}
function render(){const base=new DOMMatrix([dpr,0,0,dpr,0,0]).multiply(new DOMMatrix(D.views[frame]));
 ctx.setTransform(1,0,0,1,0,0);ctx.clearRect(0,0,cv.width,cv.height);
 if(D.bg){ctx.fillStyle=D.bg;ctx.fillRect(0,0,cv.width,cv.height);}draw(ctx,D.frames[D.seq[frame]],base);}
function resolve(f){if(typeof f==='number')return Math.max(0,Math.min(D.seq.length-1,f-1));const l=D.labels[f];return l===undefined?frame:l;}
function syncAudio(){if(!audio)return;if(playing){audio.currentTime=frame/D.fps;audio.play().catch(()=>{});}else audio.pause();}
function run(acts){let jumped=false;for(const a of acts){
 if(a[0]==='stop'){playing=false;syncAudio();}else if(a[0]==='play'){playing=true;syncAudio();}
 else if(a[0]==='goto'){frame=resolve(a[1]);playing=a[2];jumped=true;enter();syncAudio();}
 else if(a[0]==='next'){frame=Math.min(frame+1,D.seq.length-1);playing=false;jumped=true;enter();}
 else if(a[0]==='prev'){frame=Math.max(frame-1,0);playing=false;jumped=true;enter();}}return jumped;}
let handlers={};
function enter(){const acts=D.actions[frame];if(!acts)return;const flat=[];for(const a of acts){if(a[0]==='click'){handlers[a[1]]=a[2];}else flat.push(a);}run(flat);}
cv.addEventListener('click',e=>{const r=cv.getBoundingClientRect();const x=(e.clientX-r.left)*D.w/r.width,y=(e.clientY-r.top)*D.h/r.height;
 const hits=D.hits[frame]||[];for(let i=hits.length-1;i>=0;i--){const h=hits[i];if(x>=h[1]&&x<=h[3]&&y>=h[2]&&y<=h[4]&&handlers[h[0]]){run(handlers[h[0]]);render();return;}}
 if(!audio||!audio.paused)return;syncAudio();});
cv.addEventListener('mousemove',e=>{const r=cv.getBoundingClientRect();const x=(e.clientX-r.left)*D.w/r.width,y=(e.clientY-r.top)*D.h/r.height;
 cv.style.cursor=(D.hits[frame]||[]).some(h=>handlers[h[0]]&&x>=h[1]&&x<=h[3]&&y>=h[2]&&y<=h[4])?'pointer':'default';});
function tick(now){acc+=now-last;last=now;const step=1000/D.fps;
 while(acc>=step){acc-=step;if(playing){let n=frame+1;if(n>=D.seq.length){if(!D.loop){playing=false;n=frame;}else{n=0;syncAudio();}}
  if(n!==frame){frame=n;enter();}}}
 render();requestAnimationFrame(tick);}
enter();syncAudio();render();requestAnimationFrame(tick);
window.animara={play:()=>{playing=true;syncAudio();},stop:()=>{playing=false;syncAudio();},gotoAndStop:f=>{frame=resolve(f);playing=false;render();},get frame(){return frame+1;}};
})();
"#;

/// Build the HTML page.
pub fn html5_page(doc: &Document, opts: &ExportOptions) -> String {
    let (a, b) = opts.frames(doc);
    let mut ctx = Ctx { doc, images: Vec::new() };
    let mut uniq: Vec<Value> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut seq = Vec::new();
    let mut views = Vec::new();
    let mut actions = serde_json::Map::new();
    let mut hits = serde_json::Map::new();
    let mut labels = serde_json::Map::new();
    for g in a..=b {
        let (s, f) = scene_at_global_frame(doc, g);
        let dl = evaluate_document_frame(doc, s, f, &EvalOptions::export());
        let ops = Value::Array(ctx.ops(&dl.items));
        let key = ops.to_string();
        let idx = *index.entry(key).or_insert_with(|| {
            uniq.push(ops);
            uniq.len() - 1
        });
        seq.push(idx);
        views.push(m(&dl.view));
        let tl = &doc.scenes[s].timeline;
        let mut acts = Vec::new();
        for layer in &tl.layers {
            if let Some(k) = layer.keyframe_at(f)
                && k.start == f
            {
                if !k.script.is_empty() {
                    acts.extend(parse_actions(&k.script));
                }
                if let Some(l) = &k.label {
                    labels.insert(l.name.clone(), json!(g - a));
                }
            }
        }
        if !acts.is_empty() {
            actions.insert((g - a).to_string(), Value::Array(acts));
        }
        // Hit boxes of named instances on this frame.
        let mut hb = Vec::new();
        for li in 0..tl.layers.len() {
            let (els, _) = animara_animation::layer_elements_at(doc, tl, li, f);
            for e in els {
                let name = match &e.kind {
                    ElementKind::Instance(i) => i.instance_name.clone(),
                    _ => String::new(),
                };
                if !name.is_empty() {
                    let bb = dl.view.transform_rect(&animara_renderer::bounds::element_bounds(doc, &e, &Affine::IDENTITY));
                    hb.push(json!([name, bb.min.x, bb.min.y, bb.max.x, bb.max.y]));
                }
            }
        }
        if !hb.is_empty() {
            hits.insert((g - a).to_string(), Value::Array(hb));
        }
    }
    let audio = if opts.include_audio && animara_media::has_audio(doc) {
        let pcm = animara_media::mixdown_document(doc, 22050);
        Some(format!("data:audio/wav;base64,{}", base64::engine::general_purpose::STANDARD.encode(animara_media::encode_wav(&pcm))))
    } else {
        None
    };
    let data = json!({
        "w": doc.width, "h": doc.height, "fps": doc.fps, "loop": doc.publish.loop_playback,
        "bg": if opts.transparent { Value::Null } else { json!(css(doc.background)) },
        "frames": uniq, "seq": seq, "views": views, "images": ctx.images.iter().map(|x| x.1.clone()).collect::<Vec<_>>(),
        "actions": actions, "hits": hits, "labels": labels, "audio": audio,
    });
    let title = if doc.metadata.title.is_empty() { doc.name.clone() } else { doc.metadata.title.clone() };
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;");
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n<meta name=\"generator\" content=\"Animara {}\"><title>{}</title>\n<style>html,body{{margin:0;height:100%;background:#222;display:flex;align-items:center;justify-content:center}}canvas{{max-width:100vw;max-height:100vh;width:{}px;aspect-ratio:{}/{};}}</style></head>\n<body><canvas id=\"stage\"></canvas>\n<script type=\"application/json\" id=\"anim-data\">{}</script>\n<script>{}</script></body></html>\n",
        env!("CARGO_PKG_VERSION"),
        esc(&title),
        doc.width,
        doc.width,
        doc.height,
        data.to_string().replace("</", "<\\/"),
        PLAYER_JS
    )
}

pub fn export_html5(doc: &Document, path: &std::path::Path, opts: &ExportOptions) -> Result<(), ExportError> {
    std::fs::write(path, html5_page(doc, opts))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_parse() {
        let a = parse_actions("stop();\non_click(\"btn\", || {\n gotoAndPlay(\"intro\");\n});\ngotoAndStop(5);");
        assert_eq!(a[0], json!(["stop"]));
        assert_eq!(a[1], json!(["click", "btn", [["goto", "intro", true]]]));
        assert_eq!(a[2], json!(["goto", 5, false]));
    }

    #[test]
    fn page_contains_frames_and_player() {
        let doc = crate::test_doc::sample();
        let html = html5_page(&doc, &ExportOptions::default());
        assert!(html.contains("<canvas id=\"stage\">") && html.contains("requestAnimationFrame"));
        let start = html.find("id=\"anim-data\">").unwrap() + 15;
        let end = html[start..].find("</script>").unwrap() + start;
        let v: Value = serde_json::from_str(&html[start..end].replace("<\\/", "</")).unwrap();
        assert_eq!(v["seq"].as_array().unwrap().len(), 12);
        assert_eq!(v["actions"]["0"], json!([["stop"]]));
    }
}
