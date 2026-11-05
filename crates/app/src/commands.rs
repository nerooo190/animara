//! Every user-invocable action. Menus, keyboard shortcuts and the command
//! palette all dispatch through [`Command`].

use egui::{Key, KeyboardShortcut, Modifiers};
use serde::{Deserialize, Serialize};

macro_rules! commands {
    ($( $name:ident => ($label:expr, $cat:expr, $sc:expr) ),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum Command { $( $name ),* }
        impl Command {
            pub const ALL: &'static [Command] = &[ $( Command::$name ),* ];
            pub fn label(self) -> &'static str { match self { $( Command::$name => $label ),* } }
            pub fn category(self) -> &'static str { match self { $( Command::$name => $cat ),* } }
            pub fn default_shortcut(self) -> Option<KeyboardShortcut> { match self { $( Command::$name => $sc ),* } }
            pub fn id(self) -> &'static str { match self { $( Command::$name => stringify!($name) ),* } }
            pub fn from_id(s: &str) -> Option<Command> { match s { $( stringify!($name) => Some(Command::$name), )* _ => None } }
        }
    };
}

const fn ks(m: Modifiers, k: Key) -> Option<KeyboardShortcut> {
    Some(KeyboardShortcut::new(m, k))
}
const NONE: Modifiers = Modifiers::NONE;
const CMD: Modifiers = Modifiers::COMMAND;
const SHIFT: Modifiers = Modifiers::SHIFT;
const ALT: Modifiers = Modifiers::ALT;
const CMD_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::COMMAND };
const CMD_ALT: Modifiers = Modifiers { alt: true, ..Modifiers::COMMAND };

commands! {
    // File
    NewDocument => ("New…", "File", ks(CMD, Key::N)),
    NewFromTemplate => ("New from Template…", "File", ks(CMD_SHIFT, Key::N)),
    Open => ("Open…", "File", ks(CMD, Key::O)),
    Close => ("Close", "File", ks(CMD, Key::W)),
    CloseAll => ("Close All", "File", ks(CMD_ALT, Key::W)),
    Save => ("Save", "File", ks(CMD, Key::S)),
    SaveAs => ("Save As…", "File", ks(CMD_SHIFT, Key::S)),
    SaveAll => ("Save All", "File", None),
    Revert => ("Revert", "File", None),
    ImportToStage => ("Import to Stage…", "File", ks(CMD, Key::R)),
    ImportToLibrary => ("Import to Library…", "File", None),
    OpenExternalLibrary => ("Open External Library…", "File", ks(CMD_SHIFT, Key::O)),
    ExportImage => ("Export Image…", "File", None),
    ExportAnimatedGif => ("Export Animated GIF…", "File", None),
    ExportVideo => ("Export Video / Media…", "File", ks(CMD_ALT, Key::Enter)),
    ExportMovie => ("Export Movie (sequence / formats)…", "File", None),
    ExportSvg => ("Export SVG…", "File", None),
    ExportSpriteSheet => ("Generate Sprite Sheet…", "File", None),
    ExportLottie => ("Export Lottie JSON…", "File", None),
    ExportHtml5 => ("Export HTML5 Canvas…", "File", None),
    ExportSwf => ("Export SWF…", "File", None),
    ExportXfl => ("Export XFL…", "File", None),
    PublishSettings => ("Publish Settings…", "File", ks(CMD_SHIFT, Key::F12)),
    Publish => ("Publish", "File", ks(SHIFT, Key::F12)),
    Exit => ("Exit", "File", ks(CMD, Key::Q)),
    // Edit
    Undo => ("Undo", "Edit", ks(CMD, Key::Z)),
    Redo => ("Redo", "Edit", ks(CMD, Key::Y)),
    Cut => ("Cut", "Edit", ks(CMD, Key::X)),
    Copy => ("Copy", "Edit", ks(CMD, Key::C)),
    Paste => ("Paste in Center", "Edit", ks(CMD, Key::V)),
    PasteInPlace => ("Paste in Place", "Edit", ks(CMD_SHIFT, Key::V)),
    Clear => ("Clear", "Edit", ks(NONE, Key::Delete)),
    Duplicate => ("Duplicate", "Edit", ks(CMD, Key::D)),
    SelectAll => ("Select All", "Edit", ks(CMD, Key::A)),
    DeselectAll => ("Deselect All", "Edit", ks(CMD_SHIFT, Key::A)),
    EditSymbols => ("Edit Symbols", "Edit", ks(CMD, Key::E)),
    EditSelected => ("Edit Selected", "Edit", None),
    EditInPlace => ("Edit in Place", "Edit", None),
    ExitSymbol => ("Exit Symbol Editing", "Edit", None),
    Preferences => ("Preferences…", "Edit", ks(CMD, Key::U)),
    KeyboardShortcuts => ("Keyboard Shortcuts…", "Edit", None),
    CommandPalette => ("Command Palette…", "Edit", ks(CMD_SHIFT, Key::P)),
    // View
    ZoomIn => ("Zoom In", "View", ks(CMD, Key::Equals)),
    ZoomOut => ("Zoom Out", "View", ks(CMD, Key::Minus)),
    Zoom100 => ("Magnification 100%", "View", ks(CMD, Key::Num1)),
    FitInWindow => ("Fit in Window", "View", ks(CMD, Key::Num2)),
    ShowFrame => ("Center Stage", "View", ks(CMD, Key::Num0)),
    ToggleRulers => ("Rulers", "View", ks(CMD_SHIFT, Key::R)),
    ToggleGrid => ("Show Grid", "View", ks(CMD, Key::Quote)),
    ToggleSnapGrid => ("Snap to Grid", "View", None),
    ToggleGuides => ("Show Guides", "View", ks(CMD, Key::Semicolon)),
    ToggleSnapGuides => ("Snap to Guides", "View", None),
    ToggleSnapObjects => ("Snap to Objects", "View", None),
    ClearGuides => ("Clear Guides", "View", None),
    TogglePreviewOutlines => ("Preview: Outlines", "View", None),
    TogglePasteboard => ("Show Pasteboard Content", "View", None),
    // Insert
    NewSymbol => ("New Symbol…", "Insert", ks(CMD, Key::F8)),
    InsertLayer => ("Layer", "Insert", None),
    InsertFolder => ("Layer Folder", "Insert", None),
    InsertCameraLayer => ("Camera Layer", "Insert", None),
    InsertFrame => ("Frame", "Insert", ks(NONE, Key::F5)),
    RemoveFrames => ("Remove Frames", "Insert", ks(SHIFT, Key::F5)),
    InsertKeyframe => ("Keyframe", "Insert", ks(NONE, Key::F6)),
    ClearKeyframe => ("Clear Keyframe", "Insert", ks(SHIFT, Key::F6)),
    InsertBlankKeyframe => ("Blank Keyframe", "Insert", ks(NONE, Key::F7)),
    CreateClassicTween => ("Create Classic Tween", "Insert", None),
    CreateMotionTween => ("Create Motion Tween", "Insert", None),
    CreateShapeTween => ("Create Shape Tween", "Insert", None),
    RemoveTween => ("Remove Tween", "Insert", None),
    InsertScene => ("Scene", "Insert", None),
    // Modify
    DocumentSettings => ("Document…", "Modify", ks(CMD, Key::J)),
    ConvertToSymbol => ("Convert to Symbol…", "Modify", ks(NONE, Key::F8)),
    BreakApart => ("Break Apart", "Modify", ks(CMD, Key::B)),
    Group => ("Group", "Modify", ks(CMD, Key::G)),
    Ungroup => ("Ungroup", "Modify", ks(CMD_SHIFT, Key::G)),
    SwapSymbol => ("Swap Symbol…", "Modify", None),
    DuplicateSymbol => ("Duplicate Symbol", "Modify", None),
    Smooth => ("Smooth", "Modify", None),
    Straighten => ("Straighten", "Modify", None),
    Optimize => ("Optimize Curves", "Modify", None),
    LinesToFills => ("Convert Lines to Fills", "Modify", None),
    Union => ("Union", "Modify", None),
    Intersect => ("Intersect", "Modify", None),
    Punch => ("Punch", "Modify", None),
    Crop => ("Crop", "Modify", None),
    ReverseFrames => ("Reverse Frames", "Modify", None),
    ConvertToKeyframes => ("Convert to Keyframes", "Modify", None),
    DistributeToLayers => ("Distribute to Layers", "Modify", ks(CMD_SHIFT, Key::D)),
    FreeTransform => ("Free Transform", "Modify", None),
    Rotate90CW => ("Rotate 90° CW", "Modify", ks(CMD_SHIFT, Key::Num9)),
    Rotate90CCW => ("Rotate 90° CCW", "Modify", ks(CMD_SHIFT, Key::Num7)),
    FlipHorizontal => ("Flip Horizontal", "Modify", None),
    FlipVertical => ("Flip Vertical", "Modify", None),
    RemoveTransform => ("Remove Transform", "Modify", ks(CMD_SHIFT, Key::Z)),
    BringToFront => ("Bring to Front", "Modify", ks(CMD_SHIFT, Key::ArrowUp)),
    BringForward => ("Bring Forward", "Modify", ks(CMD, Key::ArrowUp)),
    SendBackward => ("Send Backward", "Modify", ks(CMD, Key::ArrowDown)),
    SendToBack => ("Send to Back", "Modify", ks(CMD_SHIFT, Key::ArrowDown)),
    LockSelection => ("Lock Selection", "Modify", ks(CMD_ALT, Key::L)),
    UnlockAll => ("Unlock All", "Modify", None),
    AlignLeft => ("Align Left", "Modify", ks(CMD_ALT, Key::Num1)),
    AlignHCenter => ("Align Horizontal Center", "Modify", ks(CMD_ALT, Key::Num2)),
    AlignRight => ("Align Right", "Modify", ks(CMD_ALT, Key::Num3)),
    AlignTop => ("Align Top", "Modify", ks(CMD_ALT, Key::Num4)),
    AlignVCenter => ("Align Vertical Center", "Modify", ks(CMD_ALT, Key::Num5)),
    AlignBottom => ("Align Bottom", "Modify", ks(CMD_ALT, Key::Num6)),
    DistributeH => ("Distribute Horizontal Centers", "Modify", ks(CMD_ALT, Key::Num7)),
    DistributeV => ("Distribute Vertical Centers", "Modify", ks(CMD_ALT, Key::Num8)),
    MatchWidth => ("Match Width", "Modify", None),
    MatchHeight => ("Match Height", "Modify", None),
    ToggleAlignToStage => ("Align to Stage", "Modify", None),
    // Text
    TextBold => ("Bold", "Text", ks(CMD_SHIFT, Key::B)),
    TextItalic => ("Italic", "Text", ks(CMD_SHIFT, Key::I)),
    TextAlignLeft => ("Align Text Left", "Text", ks(CMD_SHIFT, Key::L)),
    TextAlignCenter => ("Align Text Center", "Text", ks(CMD_SHIFT, Key::C)),
    TextAlignRight => ("Align Text Right", "Text", None),
    TextToOutlines => ("Convert Text to Outlines", "Text", None),
    // Commands
    RunScriptCommand => ("Run Command Script…", "Commands", None),
    ManagePlugins => ("Manage Plugins…", "Commands", None),
    // Control
    PlayStop => ("Play / Stop", "Control", ks(NONE, Key::Enter)),
    Rewind => ("Rewind", "Control", ks(SHIFT, Key::Comma)),
    GoToEnd => ("Go to End", "Control", ks(SHIFT, Key::Period)),
    StepForward => ("Step Forward One Frame", "Control", ks(NONE, Key::Period)),
    StepBackward => ("Step Backward One Frame", "Control", ks(NONE, Key::Comma)),
    NextKeyframe => ("Next Keyframe", "Control", ks(CMD, Key::Period)),
    PrevKeyframe => ("Previous Keyframe", "Control", ks(CMD, Key::Comma)),
    TestMovie => ("Test Movie", "Control", ks(CMD, Key::Enter)),
    ToggleLoop => ("Loop Playback", "Control", ks(SHIFT, Key::L)),
    ToggleOnionSkin => ("Onion Skin", "Control", ks(ALT, Key::O)),
    ToggleOnionOutline => ("Onion Skin Outlines", "Control", None),
    MuteSounds => ("Mute Sounds", "Control", None),
    // Debug
    DebugMovie => ("Debug Movie (Script Check)", "Debug", ks(CMD_SHIFT, Key::Enter)),
    ValidateDocument => ("Validate Document", "Debug", None),
    // Window
    ShowTimeline => ("Timeline", "Window", ks(CMD_ALT, Key::T)),
    ShowProperties => ("Properties", "Window", ks(CMD, Key::F3)),
    ShowLibrary => ("Library", "Window", ks(CMD, Key::L)),
    ShowColor => ("Color", "Window", ks(CMD_SHIFT, Key::F9)),
    ShowSwatches => ("Swatches", "Window", ks(CMD, Key::F9)),
    ShowAlign => ("Align", "Window", ks(CMD, Key::K)),
    ShowTransform => ("Transform", "Window", ks(CMD, Key::T)),
    ShowHistory => ("History", "Window", ks(CMD, Key::F10)),
    ShowActions => ("Actions", "Window", ks(NONE, Key::F9)),
    ShowOutput => ("Output", "Window", ks(NONE, Key::F2)),
    ShowMotionEditor => ("Motion Editor", "Window", None),
    ShowScenes => ("Scene", "Window", ks(SHIFT, Key::F2)),
    ShowInfo => ("Info", "Window", ks(CMD, Key::I)),
    ShowComponents => ("Components", "Window", ks(CMD, Key::F7)),
    ShowAssets => ("Assets", "Window", None),
    ResetWorkspace => ("Reset Workspace", "Window", None),
    SaveWorkspace => ("New Workspace…", "Window", None),
    ToggleTheme => ("Toggle Dark / Light Theme", "Window", None),
    HidePanels => ("Hide Panels", "Window", ks(NONE, Key::F4)),
    // Help
    About => ("About Animara", "Help", None),
    ShowHelp => ("Animara Help", "Help", ks(NONE, Key::F1)),
}

/// Human-readable shortcut text.
pub fn shortcut_text(ctx: &egui::Context, s: &KeyboardShortcut) -> String {
    ctx.format_shortcut(s)
}

/// Parse a shortcut from text such as `Ctrl+Shift+S` (used by the shortcut editor).
pub fn parse_shortcut(text: &str) -> Option<KeyboardShortcut> {
    let mut mods = Modifiers::NONE;
    let mut key = None;
    for part in text.split('+').map(|s| s.trim()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "cmd" | "command" | "⌘" => mods = mods | Modifiers::COMMAND,
            "shift" | "⇧" => mods = mods | Modifiers::SHIFT,
            "alt" | "option" | "⌥" => mods = mods | Modifiers::ALT,
            other => key = Key::from_name(other).or_else(|| Key::from_name(&other.to_uppercase())).or_else(|| Key::from_name(part)),
        }
    }
    key.map(|k| KeyboardShortcut::new(mods, k))
}

/// Single-key tool shortcuts (not routed through commands so they never conflict with text input).
pub fn tool_key(key: Key, shift: bool) -> Option<crate::tools::Tool> {
    use crate::tools::Tool;
    Some(match (key, shift) {
        (Key::V, false) => Tool::Select,
        (Key::A, false) => Tool::Subselect,
        (Key::Q, false) => Tool::FreeTransform,
        (Key::F, false) => Tool::GradientTransform,
        (Key::L, false) => Tool::Lasso,
        (Key::P, false) => Tool::Pen,
        (Key::Y, true) => Tool::Pencil,
        (Key::B, false) => Tool::Brush,
        (Key::B, true) => Tool::ClassicBrush,
        (Key::E, false) => Tool::Eraser,
        (Key::N, false) => Tool::Line,
        (Key::R, false) => Tool::Rectangle,
        (Key::O, false) => Tool::Oval,
        (Key::K, false) => Tool::PaintBucket,
        (Key::S, false) => Tool::InkBottle,
        (Key::I, false) => Tool::Eyedropper,
        (Key::T, false) => Tool::Text,
        (Key::M, false) => Tool::Bone,
        (Key::C, false) => Tool::Camera,
        (Key::H, false) => Tool::Hand,
        (Key::Z, false) => Tool::Zoom,
        (Key::W, false) => Tool::AssetWarp,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_roundtrip_and_shortcuts_unique() {
        let mut seen = std::collections::HashSet::new();
        for c in Command::ALL {
            assert_eq!(Command::from_id(c.id()), Some(*c));
            if let Some(s) = c.default_shortcut() {
                assert!(seen.insert((s.modifiers.command, s.modifiers.shift, s.modifiers.alt, s.logical_key)), "duplicate shortcut for {c:?}");
            }
        }
    }

    #[test]
    fn parse_shortcuts() {
        let s = parse_shortcut("Ctrl+Shift+S").unwrap();
        assert!(s.modifiers.command && s.modifiers.shift);
        assert_eq!(s.logical_key, Key::S);
        assert_eq!(parse_shortcut("F5").unwrap().logical_key, Key::F5);
    }
}
