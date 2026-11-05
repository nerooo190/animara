use crate::{Color, Element, ElementKind, Id, Library, SymbolInstance, Timeline, fresh_id};
use serde::{Deserialize, Serialize};

/// Current native format version. Bump on incompatible changes and add a migration in `io.rs`.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub id: Id,
    pub name: String,
    pub timeline: Timeline,
}

impl Scene {
    pub fn new(name: &str) -> Scene {
        Scene { id: fresh_id(), name: name.to_string(), timeline: Timeline::with_layer("Layer 1") }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Guides {
    pub horizontal: Vec<f64>,
    pub vertical: Vec<f64>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub snap: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridSettings {
    pub visible: bool,
    pub snap: bool,
    pub spacing: f64,
    pub color: Color,
    #[serde(default)]
    pub show_over_objects: bool,
}

impl Default for GridSettings {
    fn default() -> Self {
        GridSettings { visible: false, snap: false, spacing: 20.0, color: Color::rgba(0.5, 0.5, 0.5, 0.4), show_over_objects: false }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublishSettings {
    pub output_name: String,
    #[serde(default = "yes")]
    pub loop_playback: bool,
    #[serde(default)]
    pub transparent: bool,
    #[serde(default = "one")]
    pub scale: f64,
    #[serde(default = "q")]
    pub quality: u8,
    #[serde(default)]
    pub include_audio: bool,
}

fn one() -> f64 {
    1.0
}
fn q() -> u8 {
    90
}

impl Default for PublishSettings {
    fn default() -> Self {
        PublishSettings {
            output_name: "untitled".into(),
            loop_playback: true,
            transparent: false,
            scale: 1.0,
            quality: 90,
            include_audio: true,
        }
    }
}

/// A complete animation project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub format_version: u32,
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub fps: f64,
    pub background: Color,
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub library: Library,
    #[serde(default)]
    pub guides: Guides,
    #[serde(default)]
    pub grid: GridSettings,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(default)]
    pub publish: PublishSettings,
    #[serde(default = "default_swatches")]
    pub swatches: Vec<Color>,
    /// Document-level script executed before frame 1 (frame scripts live on keyframes).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub document_script: String,
}

pub fn default_swatches() -> Vec<Color> {
    let mut v = Vec::new();
    for i in 0..12 {
        let g = i as f32 / 11.0;
        v.push(Color::rgb(g, g, g));
    }
    for h in (0..360).step_by(30) {
        v.push(Color::from_hsv(h as f32, 1.0, 1.0, 1.0));
    }
    for h in (0..360).step_by(30) {
        v.push(Color::from_hsv(h as f32, 0.5, 0.8, 1.0));
    }
    v
}

impl Default for Document {
    fn default() -> Self {
        Document::new(1280.0, 720.0, 30.0)
    }
}

impl Document {
    pub fn new(width: f64, height: f64, fps: f64) -> Document {
        Document {
            format_version: FORMAT_VERSION,
            name: "Untitled-1".into(),
            width,
            height,
            fps,
            background: Color::WHITE,
            scenes: vec![Scene::new("Scene 1")],
            library: Library::default(),
            guides: Guides { visible: true, ..Guides::default() },
            grid: GridSettings::default(),
            metadata: Metadata::default(),
            publish: PublishSettings::default(),
            swatches: default_swatches(),
            document_script: String::new(),
        }
    }

    /// Resolve a timeline: the scene's own when `symbol` is `None`, otherwise a symbol's.
    pub fn timeline(&self, scene: usize, symbol: Option<Id>) -> Option<&Timeline> {
        match symbol {
            None => self.scenes.get(scene).map(|s| &s.timeline),
            Some(id) => self.library.symbol(id).map(|s| &s.timeline),
        }
    }
    pub fn timeline_mut(&mut self, scene: usize, symbol: Option<Id>) -> Option<&mut Timeline> {
        match symbol {
            None => self.scenes.get_mut(scene).map(|s| &mut s.timeline),
            Some(id) => self.library.symbol_mut(id).map(|s| &mut s.timeline),
        }
    }

    /// Total frames across all scenes (scenes play back to back).
    pub fn total_frames(&self) -> u32 {
        self.scenes.iter().map(|s| s.timeline.length()).sum()
    }

    /// Largest id used anywhere in the document.
    pub fn max_id(&self) -> u64 {
        fn elements(e: &[Element], m: &mut u64) {
            for el in e {
                *m = (*m).max(el.id.0);
                if let ElementKind::Group { children } = &el.kind {
                    elements(children, m);
                }
            }
        }
        fn tl(t: &Timeline, m: &mut u64) {
            for l in &t.layers {
                *m = (*m).max(l.id.0);
                for k in &l.frames {
                    elements(&k.elements, m);
                }
                if let Some(a) = &l.armature {
                    for b in &a.bones {
                        *m = (*m).max(b.id.0);
                    }
                }
            }
        }
        let mut m = 0;
        for s in &self.scenes {
            m = m.max(s.id.0);
            tl(&s.timeline, &mut m);
        }
        for s in &self.library.symbols {
            m = m.max(s.id.0);
            tl(&s.timeline, &mut m);
        }
        for b in &self.library.bitmaps {
            m = m.max(b.id.0);
        }
        for s in &self.library.sounds {
            m = m.max(s.id.0);
        }
        m
    }

    /// Validate structural invariants and references. Returns a list of problems.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let check_tl = |t: &Timeline, ctx: &str, problems: &mut Vec<String>| {
            if let Err(e) = t.validate() {
                problems.push(format!("{ctx}: {e}"));
            }
            for l in &t.layers {
                for k in &l.frames {
                    check_refs(&k.elements, self, ctx, problems);
                }
            }
        };
        for s in &self.scenes {
            check_tl(&s.timeline, &format!("scene '{}'", s.name), &mut problems);
        }
        for s in &self.library.symbols {
            check_tl(&s.timeline, &format!("symbol '{}'", s.name), &mut problems);
            if self.library.symbols.iter().any(|o| o.id != s.id && self.library.symbol_contains(s.id, s.id) && o.id == s.id) {
                problems.push(format!("symbol '{}' contains itself", s.name));
            }
        }
        problems
    }

    /// Instance a symbol as a new element positioned at `(x, y)`.
    pub fn instantiate(&self, symbol: Id, x: f64, y: f64) -> Element {
        Element::new(ElementKind::Instance(SymbolInstance::new(symbol))).with_transform(animara_geometry::Affine::translate(x, y))
    }
}

fn check_refs(elements: &[Element], doc: &Document, ctx: &str, problems: &mut Vec<String>) {
    for e in elements {
        match &e.kind {
            ElementKind::Instance(i) if doc.library.symbol(i.symbol).is_none() => {
                problems.push(format!("{ctx}: instance {} references missing symbol {}", e.id, i.symbol))
            }
            ElementKind::Bitmap(b) if doc.library.bitmap(b.asset).is_none() => {
                problems.push(format!("{ctx}: bitmap {} references missing asset {}", e.id, b.asset))
            }
            ElementKind::Group { children } => check_refs(children, doc, ctx, problems),
            _ => {}
        }
    }
}

/// New-document preset.
#[derive(Debug, Clone, Copy)]
pub struct Template {
    pub category: &'static str,
    pub name: &'static str,
    pub width: f64,
    pub height: f64,
    pub fps: f64,
}

/// Presets offered by the New Document dialog, grouped by intent category.
pub const TEMPLATES: &[Template] = &[
    Template { category: "Character Animation", name: "HD", width: 1280.0, height: 720.0, fps: 30.0 },
    Template { category: "Character Animation", name: "Full HD", width: 1920.0, height: 1080.0, fps: 30.0 },
    Template { category: "Character Animation", name: "4K", width: 3840.0, height: 2160.0, fps: 30.0 },
    Template { category: "Character Animation", name: "Standard", width: 640.0, height: 480.0, fps: 30.0 },
    Template { category: "Social", name: "Square post", width: 1080.0, height: 1080.0, fps: 30.0 },
    Template { category: "Social", name: "Portrait story", width: 1080.0, height: 1920.0, fps: 30.0 },
    Template { category: "Social", name: "Landscape video", width: 1920.0, height: 1080.0, fps: 30.0 },
    Template { category: "Social", name: "Cover banner", width: 1500.0, height: 500.0, fps: 24.0 },
    Template { category: "Game", name: "Mobile landscape", width: 1334.0, height: 750.0, fps: 60.0 },
    Template { category: "Game", name: "Mobile portrait", width: 750.0, height: 1334.0, fps: 60.0 },
    Template { category: "Game", name: "Desktop", width: 1280.0, height: 720.0, fps: 60.0 },
    Template { category: "Education", name: "Slide 16:9", width: 1280.0, height: 720.0, fps: 24.0 },
    Template { category: "Education", name: "Slide 4:3", width: 1024.0, height: 768.0, fps: 24.0 },
    Template { category: "Ads", name: "Medium rectangle", width: 300.0, height: 250.0, fps: 24.0 },
    Template { category: "Ads", name: "Leaderboard", width: 728.0, height: 90.0, fps: 24.0 },
    Template { category: "Ads", name: "Skyscraper", width: 160.0, height: 600.0, fps: 24.0 },
    Template { category: "Ads", name: "Half page", width: 300.0, height: 600.0, fps: 24.0 },
    Template { category: "Web", name: "Web banner", width: 970.0, height: 250.0, fps: 24.0 },
    Template { category: "Web", name: "Hero", width: 1440.0, height: 600.0, fps: 30.0 },
    Template { category: "Advanced", name: "Custom", width: 550.0, height: 400.0, fps: 24.0 },
];
