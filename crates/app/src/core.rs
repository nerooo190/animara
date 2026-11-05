//! Application-wide state shared by all panels.

use crate::commands::Command;
use crate::ops::Clipboard;
use crate::state::DocState;
use crate::tools::{Tool, ToolSettings, ToolState};
use animara_document::{Id, SymbolKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dark: bool,
    pub recent: Vec<PathBuf>,
    /// Command id → shortcut text overrides.
    pub shortcuts: BTreeMap<String, String>,
    pub autosave_secs: u64,
    pub workspaces: BTreeMap<String, String>,
    pub current_workspace: String,
    pub rulers: bool,
    pub ui_scale: f32,
    pub tools: ToolSettings,
    pub pasteboard_content: bool,
    pub preview_outlines: bool,
    pub show_welcome: bool,
    pub frame_width: f32,
    pub mute: bool,
    pub high_quality_stage: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            dark: true,
            recent: Vec::new(),
            shortcuts: BTreeMap::new(),
            autosave_secs: 120,
            workspaces: BTreeMap::new(),
            current_workspace: "Essentials".into(),
            rulers: false,
            ui_scale: 1.0,
            tools: ToolSettings::default(),
            pasteboard_content: true,
            preview_outlines: false,
            show_welcome: true,
            frame_width: 12.0,
            mute: false,
            high_quality_stage: true,
        }
    }
}

impl Settings {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("animara")
    }
    pub fn load() -> Settings {
        std::fs::read_to_string(Self::config_dir().join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) {
        let dir = Self::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(dir.join("settings.json"), s);
        }
    }
    pub fn add_recent(&mut self, p: PathBuf) {
        self.recent.retain(|q| q != &p);
        self.recent.insert(0, p);
        self.recent.truncate(12);
    }
    pub fn shortcut(&self, c: Command) -> Option<egui::KeyboardShortcut> {
        match self.shortcuts.get(c.id()) {
            Some(s) if s.is_empty() => None,
            Some(s) => crate::commands::parse_shortcut(s),
            None => c.default_shortcut(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Export targets offered by File ▸ Export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Png,
    PngSequence,
    Gif,
    Apng,
    WebP,
    Mp4,
    WebM,
    Mov,
    Svg,
    SvgSequence,
    SpriteSheet,
    Lottie,
    Html5,
    Swf,
    Xfl,
}

impl ExportKind {
    pub fn name(self) -> &'static str {
        match self {
            ExportKind::Png => "PNG image (current frame)",
            ExportKind::PngSequence => "PNG sequence",
            ExportKind::Gif => "Animated GIF",
            ExportKind::Apng => "Animated PNG (APNG)",
            ExportKind::WebP => "Animated WebP",
            ExportKind::Mp4 => "MP4 video (H.264)",
            ExportKind::WebM => "WebM video (VP9, alpha capable)",
            ExportKind::Mov => "QuickTime MOV (ProRes 4444, alpha)",
            ExportKind::Svg => "SVG image (current frame)",
            ExportKind::SvgSequence => "SVG sequence",
            ExportKind::SpriteSheet => "Sprite sheet (PNG + JSON)",
            ExportKind::Lottie => "Lottie JSON",
            ExportKind::Html5 => "HTML5 Canvas (HTML + JS)",
            ExportKind::Swf => "SWF (Flash Player)",
            ExportKind::Xfl => "XFL (uncompressed document)",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            ExportKind::Png | ExportKind::PngSequence | ExportKind::Apng | ExportKind::SpriteSheet => "png",
            ExportKind::Gif => "gif",
            ExportKind::WebP => "webp",
            ExportKind::Mp4 => "mp4",
            ExportKind::WebM => "webm",
            ExportKind::Mov => "mov",
            ExportKind::Svg | ExportKind::SvgSequence => "svg",
            ExportKind::Lottie => "json",
            ExportKind::Html5 => "html",
            ExportKind::Swf => "swf",
            ExportKind::Xfl => "xfl",
        }
    }
    pub const ALL: [ExportKind; 15] = [
        ExportKind::Png,
        ExportKind::PngSequence,
        ExportKind::Gif,
        ExportKind::Apng,
        ExportKind::WebP,
        ExportKind::Mp4,
        ExportKind::WebM,
        ExportKind::Mov,
        ExportKind::Svg,
        ExportKind::SvgSequence,
        ExportKind::SpriteSheet,
        ExportKind::Lottie,
        ExportKind::Html5,
        ExportKind::Swf,
        ExportKind::Xfl,
    ];
}

#[derive(Debug, Clone)]
pub struct ExportSettings {
    pub kind: ExportKind,
    pub scale: f64,
    pub fps: f64,
    pub first: u32,
    pub last: u32,
    pub transparent: bool,
    pub quality: u8,
    pub loop_count: u16,
    pub include_audio: bool,
    pub all_scenes: bool,
}

pub enum Dialog {
    Welcome,
    NewDocument { category: usize, template: usize, width: f64, height: f64, fps: f64 },
    DocumentSettings { width: f64, height: f64, fps: f64, background: animara_document::Color, scale_content: bool },
    ConvertToSymbol { name: String, kind: SymbolKind, center: bool },
    NewSymbol { name: String, kind: SymbolKind },
    Export(ExportSettings),
    Preferences,
    Shortcuts { filter: String, capturing: Option<Command> },
    About,
    SwapSymbol { selected: Option<Id> },
    SaveWorkspace { name: String },
    ConfirmClose { uid: u64, then_exit: bool },
    Recovery { files: Vec<PathBuf> },
    Plugins,
    Help,
}

pub struct AppCore {
    pub docs: Vec<DocState>,
    pub active: usize,
    pub tool: Tool,
    pub tstate: ToolState,
    pub settings: Settings,
    pub clipboard: Clipboard,
    pub queue: Vec<Command>,
    pub log: Vec<(LogLevel, String)>,
    pub dialog: Option<Dialog>,
    pub status: String,
    pub palette: Option<String>,
    pub font_families: Vec<String>,
    pub thumbnails: HashMap<(u64, Id, u64), egui::TextureHandle>,
    pub library_selected: Option<Id>,
    pub library_filter: String,
    pub hide_panels: bool,
    pub exit_requested: bool,
    pub player: Option<crate::player::Player>,
    pub audio: crate::audio::AudioPlayback,
    pub plugins: Vec<animara_scripting::Plugin>,
    pub last_autosave: f64,
}

impl AppCore {
    pub fn new(settings: Settings) -> AppCore {
        AppCore {
            docs: Vec::new(),
            active: 0,
            tool: Tool::Select,
            tstate: ToolState::default(),
            settings,
            clipboard: Clipboard::default(),
            queue: Vec::new(),
            log: Vec::new(),
            dialog: None,
            status: String::new(),
            palette: None,
            font_families: Vec::new(),
            thumbnails: HashMap::new(),
            library_selected: None,
            library_filter: String::new(),
            hide_panels: false,
            exit_requested: false,
            player: None,
            audio: crate::audio::AudioPlayback::default(),
            plugins: Vec::new(),
            last_autosave: 0.0,
        }
    }

    pub fn doc(&self) -> Option<&DocState> {
        self.docs.get(self.active)
    }
    pub fn doc_mut(&mut self) -> Option<&mut DocState> {
        self.docs.get_mut(self.active)
    }
    pub fn info(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        log::info!("{m}");
        self.status = m.clone();
        self.log.push((LogLevel::Info, m));
    }
    pub fn warn(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        log::warn!("{m}");
        self.status = m.clone();
        self.log.push((LogLevel::Warn, m));
    }
    pub fn error(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        log::error!("{m}");
        self.status = m.clone();
        self.log.push((LogLevel::Error, m));
    }
    pub fn run(&mut self, c: Command) {
        self.queue.push(c);
    }
}
