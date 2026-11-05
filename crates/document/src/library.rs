use crate::{Id, Timeline, fresh_id};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SymbolKind {
    #[default]
    Graphic,
    MovieClip,
    Button,
}

impl SymbolKind {
    pub fn name(self) -> &'static str {
        match self {
            SymbolKind::Graphic => "Graphic",
            SymbolKind::MovieClip => "Movie Clip",
            SymbolKind::Button => "Button",
        }
    }
}

/// Reusable timeline stored in the library. Buttons use frames 0..4 as Up/Over/Down/Hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: Id,
    pub name: String,
    pub kind: SymbolKind,
    pub timeline: Timeline,
    /// Library folder path, `/`-separated ("" = root).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    /// Export-for-runtime class name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub linkage: String,
}

impl Symbol {
    pub fn new(name: &str, kind: SymbolKind) -> Symbol {
        Symbol { id: fresh_id(), name: name.to_string(), kind, timeline: Timeline::with_layer("Layer 1"), folder: String::new(), linkage: String::new() }
    }
}

/// Embedded bitmap. `data` holds the encoded file bytes (PNG/JPEG/GIF/WebP).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BitmapAsset {
    pub id: Id,
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// File extension of `data` ("png", "jpg", ...).
    pub format: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    #[serde(default = "yes")]
    pub smoothing: bool,
    #[serde(skip)]
    pub data: Arc<Vec<u8>>,
}

fn yes() -> bool {
    true
}

/// Embedded sound. `data` holds the encoded file bytes (normally WAV).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundAsset {
    pub id: Id,
    pub name: String,
    pub format: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Duration in seconds.
    pub duration: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    #[serde(skip)]
    pub data: Arc<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Library {
    #[serde(default)]
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub bitmaps: Vec<BitmapAsset>,
    #[serde(default)]
    pub sounds: Vec<SoundAsset>,
    /// Explicitly created (possibly empty) folders.
    #[serde(default)]
    pub folders: Vec<String>,
}

impl Library {
    pub fn symbol(&self, id: Id) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.id == id)
    }
    pub fn symbol_mut(&mut self, id: Id) -> Option<&mut Symbol> {
        self.symbols.iter_mut().find(|s| s.id == id)
    }
    pub fn bitmap(&self, id: Id) -> Option<&BitmapAsset> {
        self.bitmaps.iter().find(|b| b.id == id)
    }
    pub fn sound(&self, id: Id) -> Option<&SoundAsset> {
        self.sounds.iter().find(|s| s.id == id)
    }
    pub fn symbol_by_name(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.name == name)
    }
    pub fn unique_name(&self, base: &str) -> String {
        let taken = |n: &str| {
            self.symbols.iter().any(|s| s.name == n) || self.bitmaps.iter().any(|b| b.name == n) || self.sounds.iter().any(|s| s.name == n)
        };
        if !taken(base) {
            return base.to_string();
        }
        (1..).map(|i| format!("{base} {i}")).find(|n| !taken(n)).expect("infinite")
    }
    pub fn add_symbol(&mut self, s: Symbol) -> Id {
        let id = s.id;
        self.symbols.push(s);
        id
    }
    /// Does symbol `outer` (transitively) contain an instance of `inner`?
    pub fn symbol_contains(&self, outer: Id, inner: Id) -> bool {
        let mut stack = vec![outer];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if id == inner {
                return true;
            }
            if !seen.insert(id) {
                continue;
            }
            if let Some(s) = self.symbol(id) {
                for l in &s.timeline.layers {
                    for k in &l.frames {
                        collect_instances(&k.elements, &mut stack);
                    }
                }
            }
        }
        false
    }
    /// Count instances of a symbol across the given timelines and all symbols.
    pub fn use_count(&self, id: Id, scenes: &[&Timeline]) -> usize {
        let mut ids = Vec::new();
        for t in scenes.iter().copied().chain(self.symbols.iter().map(|s| &s.timeline)) {
            for l in &t.layers {
                for k in &l.frames {
                    collect_instances(&k.elements, &mut ids);
                }
            }
        }
        ids.iter().filter(|&&i| i == id).count()
    }
    pub fn all_folders(&self) -> Vec<String> {
        let mut f: Vec<String> = self.folders.clone();
        f.extend(self.symbols.iter().map(|s| s.folder.clone()));
        f.extend(self.bitmaps.iter().map(|s| s.folder.clone()));
        f.extend(self.sounds.iter().map(|s| s.folder.clone()));
        f.retain(|s| !s.is_empty());
        f.sort();
        f.dedup();
        f
    }
}

fn collect_instances(elements: &[crate::Element], out: &mut Vec<Id>) {
    for e in elements {
        match &e.kind {
            crate::ElementKind::Instance(i) => out.push(i.symbol),
            crate::ElementKind::Group { children } => collect_instances(children, out),
            _ => {}
        }
    }
}
