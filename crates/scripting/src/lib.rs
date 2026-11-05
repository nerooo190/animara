//! Scripting for Animara, built on the sandboxed Rhai engine.
//!
//! Frame scripts use an ActionScript-like timeline API (`stop()`, `play()`,
//! `gotoAndPlay(10)`, `gotoAndStop("label")`, `trace(..)`, `clip("name").stop()`,
//! `on_click("button", || { .. })`). Scripts cannot touch the file system or
//! network: Rhai exposes no I/O and every run is bounded by operation limits.
//!
//! Plugins are Rhai files defining `fn run(doc)`; they receive the document as
//! a map, return a (modified) map, and the host validates the result.

use animara_document::{Document, fresh_id};
use rhai::{AST, Dynamic, Engine, EvalAltResult, FnPtr, Map, Scope};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Which timeline a command applies to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// The timeline running the script.
    This,
    Root,
    /// A movie clip / button / text field by instance name.
    Named(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrameRef {
    /// 1-based frame number (as authors see it).
    Number(u32),
    Label(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptCmd {
    Stop(Target),
    Play(Target),
    Goto { target: Target, frame: FrameRef, play: bool },
    NextFrame(Target),
    PrevFrame(Target),
    NextScene,
    PrevScene,
    Trace(String),
    SetProp { target: Target, prop: String, value: f64 },
    SetText { target: Target, text: String },
    StopAllSounds,
    Navigate(String),
}

/// Names exposed to scripts (used for syntax highlighting and docs).
pub const API_FUNCTIONS: &[&str] = &[
    "stop", "play", "gotoAndPlay", "gotoAndStop", "nextFrame", "prevFrame", "nextScene", "prevScene", "trace", "clip", "root",
    "on_click", "on_enter_frame", "on_key", "currentFrame", "totalFrames", "mouseX", "mouseY", "random", "stopAllSounds",
    "navigateToURL", "set_x", "set_y", "set_alpha", "set_rotation", "set_scale", "set_visible", "set_text", "x", "y",
];

/// Snippets offered in the Actions panel.
pub const SNIPPETS: &[(&str, &str)] = &[
    ("Stop at this frame", "stop();\n"),
    ("Click to go to frame and play", "on_click(\"button_1\", || {\n    gotoAndPlay(1);\n});\n"),
    ("Click to go to frame and stop", "on_click(\"button_1\", || {\n    gotoAndStop(1);\n});\n"),
    ("Click to go to next frame", "on_click(\"button_1\", || {\n    nextFrame();\n});\n"),
    ("Play a movie clip", "clip(\"movieClip_1\").play();\n"),
    ("Stop a movie clip", "clip(\"movieClip_1\").stop();\n"),
    ("Show / hide object on click", "on_click(\"button_1\", || {\n    clip(\"movieClip_1\").set_visible(false);\n});\n"),
    ("Move object every frame", "on_enter_frame(\"movieClip_1\", || {\n    let c = clip(\"movieClip_1\");\n    c.set_x(c.x() + 5.0);\n});\n"),
    ("Keyboard: arrow right next frame", "on_key(\"ArrowRight\", || {\n    nextFrame();\n});\n"),
    ("Set dynamic text", "clip(\"label_txt\").set_text(\"Hello\");\n"),
    ("Trace current frame", "trace(\"frame \" + currentFrame());\n"),
    ("Open web page on click", "on_click(\"button_1\", || {\n    navigateToURL(\"https://example.com\");\n});\n"),
];

/// Movie-clip handle exposed to scripts.
#[derive(Debug, Clone)]
pub struct Clip {
    target: Target,
}

#[derive(Clone)]
struct Handler {
    event: String,
    target: String,
    func: FnPtr,
    ast: Arc<AST>,
}

/// Values the runtime exposes read-only to scripts.
#[derive(Debug, Clone, Default)]
pub struct RuntimeInfo {
    pub current_frame: u32,
    pub total_frames: u32,
    pub mouse: (f64, f64),
    /// Instance name → (x, y) for property reads.
    pub positions: HashMap<String, (f64, f64)>,
}

#[derive(Default)]
struct Shared {
    queue: Vec<ScriptCmd>,
    this: Option<Target>,
    current_ast: Option<Arc<AST>>,
    handlers: Vec<Handler>,
    info: RuntimeInfo,
}

/// Scripting host for one running movie.
pub struct ScriptHost {
    engine: Engine,
    cache: HashMap<u64, Arc<AST>>,
    scope: Scope<'static>,
    shared: Arc<Mutex<Shared>>,
}

fn lock(s: &Arc<Mutex<Shared>>) -> std::sync::MutexGuard<'_, Shared> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}

fn frame_ref(v: Dynamic) -> FrameRef {
    if let Ok(i) = v.as_int() {
        FrameRef::Number(i.max(1) as u32)
    } else if let Ok(f) = v.as_float() {
        FrameRef::Number(f.max(1.0) as u32)
    } else {
        FrameRef::Label(v.to_string())
    }
}

/// Light compatibility pass so simple ActionScript-style statements run: `var` → `let`,
/// `this.` prefixes dropped, AS `:Type` annotations on `let` removed.
pub fn preprocess(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let mut l = line.replace("this.", "");
        let trimmed = l.trim_start();
        if let Some(rest) = trimmed.strip_prefix("var ") {
            let indent = &l[..l.len() - trimmed.len()];
            // Drop a type annotation: `var x:Number = 1` → `let x = 1`
            let rest = match rest.find(':') {
                Some(c) if rest[..c].chars().all(|ch| ch.is_alphanumeric() || ch == '_') => {
                    let after = &rest[c + 1..];
                    let eq = after.find('=').map(|i| &after[i..]).unwrap_or(";");
                    format!("{} {}", &rest[..c], eq)
                }
                _ => rest.to_string(),
            };
            l = format!("{indent}let {rest}");
        }
        out.push_str(&l);
        out.push('\n');
    }
    out
}

fn sandboxed_engine() -> Engine {
    let mut e = Engine::new();
    e.set_max_operations(2_000_000);
    e.set_max_call_levels(64);
    e.set_max_expr_depths(64, 64);
    e.set_max_string_size(1 << 20);
    e.set_max_array_size(100_000);
    e.set_max_map_size(100_000);
    e
}

impl Default for ScriptHost {
    fn default() -> Self {
        ScriptHost::new()
    }
}

impl ScriptHost {
    pub fn new() -> ScriptHost {
        let mut engine = sandboxed_engine();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let this = |s: &Arc<Mutex<Shared>>| lock(s).this.clone().unwrap_or(Target::Root);
        macro_rules! push {
            ($s:expr, $cmd:expr) => {{
                // Build the command before locking: building it may itself lock.
                let c = $cmd;
                lock(&$s).queue.push(c)
            }};
        }
        let s = shared.clone();
        engine.register_fn("stop", move || push!(s, ScriptCmd::Stop(this(&s))));
        let s = shared.clone();
        engine.register_fn("play", move || push!(s, ScriptCmd::Play(this(&s))));
        let s = shared.clone();
        engine.register_fn("gotoAndPlay", move |f: Dynamic| push!(s, ScriptCmd::Goto { target: this(&s), frame: frame_ref(f), play: true }));
        let s = shared.clone();
        engine.register_fn("gotoAndStop", move |f: Dynamic| push!(s, ScriptCmd::Goto { target: this(&s), frame: frame_ref(f), play: false }));
        let s = shared.clone();
        engine.register_fn("nextFrame", move || push!(s, ScriptCmd::NextFrame(this(&s))));
        let s = shared.clone();
        engine.register_fn("prevFrame", move || push!(s, ScriptCmd::PrevFrame(this(&s))));
        let s = shared.clone();
        engine.register_fn("nextScene", move || push!(s, ScriptCmd::NextScene));
        let s = shared.clone();
        engine.register_fn("prevScene", move || push!(s, ScriptCmd::PrevScene));
        let s = shared.clone();
        engine.register_fn("stopAllSounds", move || push!(s, ScriptCmd::StopAllSounds));
        let s = shared.clone();
        engine.register_fn("navigateToURL", move |u: &str| push!(s, ScriptCmd::Navigate(u.to_string())));
        let s = shared.clone();
        engine.register_fn("trace", move |v: Dynamic| push!(s, ScriptCmd::Trace(v.to_string())));
        let s = shared.clone();
        engine.register_fn("currentFrame", move || lock(&s).info.current_frame as i64 + 1);
        let s = shared.clone();
        engine.register_fn("totalFrames", move || lock(&s).info.total_frames as i64);
        let s = shared.clone();
        engine.register_fn("mouseX", move || lock(&s).info.mouse.0);
        let s = shared.clone();
        engine.register_fn("mouseY", move || lock(&s).info.mouse.1);
        engine.register_fn("random", || {
            // xorshift seeded from a fresh id: deterministic enough, no OS access needed.
            let mut x = fresh_id().0.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 11) as f64 / (1u64 << 53) as f64
        });
        // Event handlers.
        for ev in ["on_click", "on_enter_frame", "on_mouse_over", "on_mouse_out"] {
            let s = shared.clone();
            let event = ev.trim_start_matches("on_").to_string();
            engine.register_fn(ev, move |target: &str, f: FnPtr| {
                let mut g = lock(&s);
                if let Some(ast) = g.current_ast.clone() {
                    g.handlers.push(Handler { event: event.clone(), target: target.to_string(), func: f, ast });
                }
            });
        }
        let s = shared.clone();
        engine.register_fn("on_key", move |key: &str, f: FnPtr| {
            let mut g = lock(&s);
            if let Some(ast) = g.current_ast.clone() {
                g.handlers.push(Handler { event: "key".into(), target: key.to_string(), func: f, ast });
            }
        });
        // Clip handles.
        engine.register_type_with_name::<Clip>("Clip");
        engine.register_fn("clip", |name: &str| Clip { target: Target::Named(name.to_string()) });
        engine.register_fn("root", || Clip { target: Target::Root });
        let s = shared.clone();
        engine.register_fn("stop", move |c: &mut Clip| push!(s, ScriptCmd::Stop(c.target.clone())));
        let s = shared.clone();
        engine.register_fn("play", move |c: &mut Clip| push!(s, ScriptCmd::Play(c.target.clone())));
        let s = shared.clone();
        engine.register_fn("gotoAndPlay", move |c: &mut Clip, f: Dynamic| push!(s, ScriptCmd::Goto { target: c.target.clone(), frame: frame_ref(f), play: true }));
        let s = shared.clone();
        engine.register_fn("gotoAndStop", move |c: &mut Clip, f: Dynamic| push!(s, ScriptCmd::Goto { target: c.target.clone(), frame: frame_ref(f), play: false }));
        let s = shared.clone();
        engine.register_fn("nextFrame", move |c: &mut Clip| push!(s, ScriptCmd::NextFrame(c.target.clone())));
        let s = shared.clone();
        engine.register_fn("prevFrame", move |c: &mut Clip| push!(s, ScriptCmd::PrevFrame(c.target.clone())));
        for prop in ["x", "y", "alpha", "rotation", "scale", "scale_x", "scale_y"] {
            let s = shared.clone();
            let p = prop.to_string();
            engine.register_fn(format!("set_{prop}"), move |c: &mut Clip, v: f64| push!(s, ScriptCmd::SetProp { target: c.target.clone(), prop: p.clone(), value: v }));
        }
        let s = shared.clone();
        engine.register_fn("set_visible", move |c: &mut Clip, v: bool| {
            push!(s, ScriptCmd::SetProp { target: c.target.clone(), prop: "visible".into(), value: if v { 1.0 } else { 0.0 } })
        });
        let s = shared.clone();
        engine.register_fn("set_text", move |c: &mut Clip, t: Dynamic| push!(s, ScriptCmd::SetText { target: c.target.clone(), text: t.to_string() }));
        let s = shared.clone();
        engine.register_fn("x", move |c: &mut Clip| match &c.target {
            Target::Named(n) => lock(&s).info.positions.get(n).map(|p| p.0).unwrap_or(0.0),
            _ => 0.0,
        });
        let s = shared.clone();
        engine.register_fn("y", move |c: &mut Clip| match &c.target {
            Target::Named(n) => lock(&s).info.positions.get(n).map(|p| p.1).unwrap_or(0.0),
            _ => 0.0,
        });
        ScriptHost { engine, cache: HashMap::new(), scope: Scope::new(), shared }
    }

    pub fn set_info(&mut self, info: RuntimeInfo) {
        lock(&self.shared).info = info;
    }

    fn compile(&mut self, script: &str) -> Result<Arc<AST>, String> {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        script.hash(&mut h);
        let key = h.finish();
        if let Some(a) = self.cache.get(&key) {
            return Ok(a.clone());
        }
        let ast = self.engine.compile(preprocess(script)).map_err(|e| e.to_string())?;
        let ast = Arc::new(ast);
        self.cache.insert(key, ast.clone());
        Ok(ast)
    }

    /// Run a frame script on behalf of `this` timeline; returns queued commands.
    pub fn run(&mut self, script: &str, this: Target) -> Result<Vec<ScriptCmd>, String> {
        if script.trim().is_empty() {
            return Ok(Vec::new());
        }
        let ast = self.compile(script)?;
        {
            let mut g = lock(&self.shared);
            g.this = Some(this);
            g.current_ast = Some(ast.clone());
        }
        let r = self.engine.run_ast_with_scope(&mut self.scope, &ast);
        let mut g = lock(&self.shared);
        g.current_ast = None;
        let cmds = std::mem::take(&mut g.queue);
        r.map_err(|e| e.to_string())?;
        Ok(cmds)
    }

    /// Fire an event (`click`, `enter_frame`, `key`, `mouse_over`, `mouse_out`) for a target name.
    pub fn fire(&mut self, event: &str, target: &str) -> Result<Vec<ScriptCmd>, String> {
        let handlers: Vec<Handler> = lock(&self.shared).handlers.iter().filter(|h| h.event == event && h.target == target).cloned().collect();
        let mut err = None;
        for h in handlers {
            lock(&self.shared).this = Some(Target::Root);
            let r: Result<Dynamic, Box<EvalAltResult>> = h.func.call(&self.engine, &h.ast, ());
            if let Err(e) = r {
                err = Some(e.to_string());
            }
        }
        let cmds = std::mem::take(&mut lock(&self.shared).queue);
        match err {
            Some(e) => Err(e),
            None => Ok(cmds),
        }
    }

    pub fn has_handlers(&self, event: &str) -> bool {
        lock(&self.shared).handlers.iter().any(|h| h.event == event)
    }

    pub fn handler_targets(&self, event: &str) -> Vec<String> {
        lock(&self.shared).handlers.iter().filter(|h| h.event == event).map(|h| h.target.clone()).collect()
    }

    /// Forget handlers and variables (e.g. when restarting the movie).
    pub fn reset(&mut self) {
        let mut g = lock(&self.shared);
        g.handlers.clear();
        g.queue.clear();
        drop(g);
        self.scope = Scope::new();
    }
}

/// Compile-check a script (Debug ▸ Check syntax).
pub fn check_syntax(script: &str) -> Result<(), String> {
    let host = ScriptHost::new();
    host.engine.compile(preprocess(script)).map(|_| ()).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ plugins

#[derive(Debug, Clone)]
pub struct Plugin {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub source: String,
}

/// Load `*.rhai` plugins from a directory. The first `//` comment line is the description.
pub fn load_plugins(dir: &Path) -> Vec<Plugin> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Plugin> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rhai"))
        .filter_map(|p| {
            let source = std::fs::read_to_string(&p).ok()?;
            let description = source.lines().next().and_then(|l| l.strip_prefix("//")).map(|s| s.trim().to_string()).unwrap_or_default();
            Some(Plugin { name: p.file_stem()?.to_string_lossy().into_owned(), description, path: p, source })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Built-in example plugins written to the plugin folder on first run.
pub const EXAMPLE_PLUGINS: &[(&str, &str)] = &[
    (
        "add_grid_of_dots.rhai",
        "// Adds a grid of dots to the first layer of the first scene.\nfn run(doc) {\n    let layer = doc.scenes[0].timeline.layers[0];\n    let key = layer.frames[0];\n    for y in 0..5 {\n        for x in 0..8 {\n            key.elements.push(ellipse_shape(40.0 + x * 60.0, 40.0 + y * 60.0, 12.0, \"#2680EB\"));\n        }\n    }\n    layer.frames[0] = key;\n    doc.scenes[0].timeline.layers[0] = layer;\n    log(\"added 40 dots\");\n    doc\n}\n",
    ),
    (
        "rename_layers_uppercase.rhai",
        "// Converts every layer name in every scene to upper case.\nfn run(doc) {\n    for i in 0..doc.scenes.len() {\n        let scene = doc.scenes[i];\n        for j in 0..scene.timeline.layers.len() {\n            let l = scene.timeline.layers[j];\n            l.name = l.name.to_upper();\n            scene.timeline.layers[j] = l;\n        }\n        doc.scenes[i] = scene;\n    }\n    doc\n}\n",
    ),
];

fn element_json(path: animara_geometry::Path, color: &str) -> Dynamic {
    let c = animara_document::Color::from_hex(color).unwrap_or(animara_document::Color::BLACK);
    let e = animara_document::Element::shape(vec![animara_document::ShapePath::filled(path, animara_document::Fill::solid(c))]);
    rhai::serde::to_dynamic(&e).unwrap_or(Dynamic::UNIT)
}

/// Run a plugin's `run(doc)` function. Returns the new document and log lines.
pub fn run_plugin(plugin: &Plugin, doc: &Document) -> Result<(Document, Vec<String>), String> {
    let mut engine = sandboxed_engine();
    engine.set_max_operations(50_000_000);
    let logs = Arc::new(Mutex::new(Vec::<String>::new()));
    let l = logs.clone();
    engine.register_fn("log", move |v: Dynamic| l.lock().unwrap_or_else(|e| e.into_inner()).push(v.to_string()));
    engine.register_fn("fresh_id", || fresh_id().0 as i64);
    engine.register_fn("rect_shape", |x: f64, y: f64, w: f64, h: f64, color: &str| {
        element_json(animara_geometry::Path::rect(animara_geometry::Rect::from_xywh(x, y, w, h)), color)
    });
    engine.register_fn("ellipse_shape", |x: f64, y: f64, r: f64, color: &str| {
        element_json(animara_geometry::Path::ellipse(animara_geometry::Rect::new(x - r, y - r, x + r, y + r)), color)
    });
    engine.register_fn("path_shape", |svg: &str, color: &str| {
        element_json(animara_geometry::Path::parse_svg(svg).unwrap_or_default(), color)
    });
    let ast = engine.compile(&plugin.source).map_err(|e| e.to_string())?;
    let input: Dynamic = rhai::serde::to_dynamic(doc).map_err(|e| e.to_string())?;
    let result: Dynamic = engine.call_fn(&mut Scope::new(), &ast, "run", (input,)).map_err(|e| e.to_string())?;
    let mut out: Document = rhai::serde::from_dynamic(&result).map_err(|e| format!("plugin returned an invalid document: {e}"))?;
    // Media bytes are not exposed to plugins; restore them by id.
    for b in &mut out.library.bitmaps {
        if let Some(orig) = doc.library.bitmap(b.id) {
            b.data = orig.data.clone();
        }
    }
    for s in &mut out.library.sounds {
        if let Some(orig) = doc.library.sound(s.id) {
            s.data = orig.data.clone();
        }
    }
    let problems = out.validate();
    if !problems.is_empty() {
        return Err(format!("plugin produced an invalid document: {}", problems.join("; ")));
    }
    animara_document::bump_ids_above(out.max_id());
    let lines = logs.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Ok((out, lines))
}

/// Convert a map produced by a plugin into a JSON value (diagnostics).
pub fn map_to_json(m: &Map) -> serde_json::Value {
    rhai::serde::from_dynamic::<serde_json::Value>(&Dynamic::from_map(m.clone())).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_commands() {
        let mut h = ScriptHost::new();
        let cmds = h.run("stop();\ngotoAndPlay(\"intro\");\nclip(\"ball\").gotoAndStop(3);\ntrace(1 + 2);", Target::Root).unwrap();
        assert_eq!(cmds[0], ScriptCmd::Stop(Target::Root));
        assert_eq!(cmds[1], ScriptCmd::Goto { target: Target::Root, frame: FrameRef::Label("intro".into()), play: true });
        assert_eq!(cmds[2], ScriptCmd::Goto { target: Target::Named("ball".into()), frame: FrameRef::Number(3), play: false });
        assert_eq!(cmds[3], ScriptCmd::Trace("3".into()));
    }

    #[test]
    fn handlers_and_persistent_variables() {
        let mut h = ScriptHost::new();
        h.run("let count = 0;\non_click(\"btn\", || { trace(\"clicked\"); nextFrame(); });", Target::Root).unwrap();
        let cmds = h.fire("click", "btn").unwrap();
        assert_eq!(cmds, vec![ScriptCmd::Trace("clicked".into()), ScriptCmd::NextFrame(Target::Root)]);
        let cmds = h.run("count += 1; trace(count);", Target::Root).unwrap();
        assert_eq!(cmds, vec![ScriptCmd::Trace("1".into())]);
    }

    #[test]
    fn actionscript_compat_and_sandbox() {
        let mut h = ScriptHost::new();
        let cmds = h.run("var speed:Number = 5;\ntrace(speed);\nthis.stop();", Target::Root).unwrap();
        assert_eq!(cmds, vec![ScriptCmd::Trace("5".into()), ScriptCmd::Stop(Target::Root)]);
        // Infinite loops are cut off by the operation limit.
        assert!(h.run("loop { }", Target::Root).is_err());
        assert!(check_syntax("stop(;").is_err());
        assert!(check_syntax("stop();").is_ok());
    }

    #[test]
    fn plugin_modifies_document() {
        let plugin = Plugin { name: "dots".into(), description: String::new(), path: PathBuf::new(), source: EXAMPLE_PLUGINS[0].1.into() };
        let doc = Document::new(640.0, 480.0, 24.0);
        let (out, log) = run_plugin(&plugin, &doc).unwrap();
        assert_eq!(out.scenes[0].timeline.layers[0].frames[0].elements.len(), 40);
        assert_eq!(log, vec!["added 40 dots".to_string()]);
    }
}
