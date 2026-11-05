//! Drawing and editing tools, their options and in-progress interaction state.

use animara_document::{Color, Fill, GradientStop, LineCap, LineJoin, SpreadMode, Stroke, StrokeStyle, TextStyle};
use animara_geometry::{Affine, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tool {
    Select,
    Subselect,
    FreeTransform,
    GradientTransform,
    Lasso,
    Pen,
    Pencil,
    Brush,
    ClassicBrush,
    Eraser,
    Line,
    Rectangle,
    Oval,
    PolyStar,
    PaintBucket,
    InkBottle,
    Eyedropper,
    Text,
    Bone,
    AssetWarp,
    Camera,
    Hand,
    Zoom,
}

impl Tool {
    /// Toolbar order (groups separated by `None`).
    pub const TOOLBAR: &'static [Option<Tool>] = &[
        Some(Tool::Select),
        Some(Tool::Subselect),
        Some(Tool::FreeTransform),
        Some(Tool::GradientTransform),
        Some(Tool::Lasso),
        None,
        Some(Tool::Brush),
        Some(Tool::ClassicBrush),
        Some(Tool::Pencil),
        Some(Tool::Eraser),
        Some(Tool::Rectangle),
        Some(Tool::Oval),
        Some(Tool::PolyStar),
        Some(Tool::Line),
        None,
        Some(Tool::Pen),
        Some(Tool::Text),
        Some(Tool::PaintBucket),
        Some(Tool::InkBottle),
        Some(Tool::Eyedropper),
        Some(Tool::AssetWarp),
        Some(Tool::Bone),
        None,
        Some(Tool::Camera),
        Some(Tool::Hand),
        Some(Tool::Zoom),
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Selection Tool (V)",
            Tool::Subselect => "Subselection Tool (A)",
            Tool::FreeTransform => "Free Transform Tool (Q)",
            Tool::GradientTransform => "Gradient Transform Tool (F)",
            Tool::Lasso => "Lasso Tool (L)",
            Tool::Pen => "Pen Tool (P)",
            Tool::Pencil => "Pencil Tool (Shift+Y)",
            Tool::Brush => "Paint Brush Tool (B) — pressure & velocity sensitive",
            Tool::ClassicBrush => "Classic Brush Tool (Shift+B)",
            Tool::Eraser => "Eraser Tool (E)",
            Tool::Line => "Line Tool (N)",
            Tool::Rectangle => "Rectangle Tool (R)",
            Tool::Oval => "Oval Tool (O)",
            Tool::PolyStar => "PolyStar Tool",
            Tool::PaintBucket => "Paint Bucket Tool (K)",
            Tool::InkBottle => "Ink Bottle Tool (S)",
            Tool::Eyedropper => "Eyedropper Tool (I)",
            Tool::Text => "Text Tool (T)",
            Tool::Bone => "Bone Tool (M)",
            Tool::AssetWarp => "Asset Warp Tool (W)",
            Tool::Camera => "Camera Tool (C)",
            Tool::Hand => "Hand Tool (H)",
            Tool::Zoom => "Zoom Tool (Z)",
        }
    }

    pub fn short(self) -> &'static str {
        let n = self.name();
        n.split(" Tool").next().unwrap_or(n)
    }

    pub fn draws(self) -> bool {
        matches!(
            self,
            Tool::Pen | Tool::Pencil | Tool::Brush | Tool::ClassicBrush | Tool::Line | Tool::Rectangle | Tool::Oval | Tool::PolyStar | Tool::Text
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PencilMode {
    Straighten,
    Smooth,
    Ink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EraserMode {
    Normal,
    FillsOnly,
    LinesOnly,
    Faucet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillKind {
    Solid,
    Linear,
    Radial,
}

/// Persistent tool options (shown in Properties ▸ Tool).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSettings {
    pub stroke_color: Color,
    pub fill_color: Color,
    pub fill_kind: FillKind,
    pub gradient: Vec<GradientStop>,
    pub no_stroke: bool,
    pub no_fill: bool,
    pub stroke_width: f64,
    pub stroke_style: StrokeStyle,
    pub cap: LineCap,
    pub join: LineJoin,
    pub brush_size: f64,
    pub brush_smoothing: f64,
    pub pressure: bool,
    pub velocity: bool,
    pub pencil_mode: PencilMode,
    pub eraser_size: f64,
    pub eraser_mode: EraserMode,
    pub rect_radius: f64,
    pub poly_sides: u32,
    pub poly_star: bool,
    pub star_ratio: f64,
    pub text_style: TextStyle,
    pub snap_objects: bool,
    pub align_to_stage: bool,
    pub auto_keyframe: bool,
}

impl Default for ToolSettings {
    fn default() -> Self {
        ToolSettings {
            stroke_color: Color::BLACK,
            fill_color: Color::from_rgba8(0x26, 0x80, 0xEB, 255),
            fill_kind: FillKind::Solid,
            gradient: vec![GradientStop { offset: 0.0, color: Color::WHITE }, GradientStop { offset: 1.0, color: Color::BLACK }],
            no_stroke: false,
            no_fill: false,
            stroke_width: 2.0,
            stroke_style: StrokeStyle::Solid,
            cap: LineCap::Round,
            join: LineJoin::Round,
            brush_size: 8.0,
            brush_smoothing: 1.0,
            pressure: true,
            velocity: true,
            pencil_mode: PencilMode::Smooth,
            eraser_size: 16.0,
            eraser_mode: EraserMode::Normal,
            rect_radius: 0.0,
            poly_sides: 5,
            poly_star: false,
            star_ratio: 0.5,
            text_style: TextStyle::default(),
            snap_objects: true,
            align_to_stage: false,
            auto_keyframe: true,
        }
    }
}

impl ToolSettings {
    pub fn stroke(&self) -> Option<Stroke> {
        (!self.no_stroke).then(|| Stroke {
            fill: Fill::solid(self.stroke_color),
            width: self.stroke_width,
            cap: self.cap,
            join: self.join,
            miter_limit: 3.0,
            style: self.stroke_style.clone(),
            scale_with_object: true,
        })
    }

    /// Current fill, with gradients mapped onto `bounds` (local coordinates).
    pub fn fill_for(&self, bounds: animara_geometry::Rect) -> Option<Fill> {
        if self.no_fill {
            return None;
        }
        Some(match self.fill_kind {
            FillKind::Solid => Fill::solid(self.fill_color),
            FillKind::Linear => Fill::Linear {
                stops: self.gradient.clone(),
                matrix: Affine::translate(bounds.center().x, bounds.center().y) * Affine::scale(bounds.width().max(1.0) / 2.0, bounds.width().max(1.0) / 2.0),
                spread: SpreadMode::Pad,
            },
            FillKind::Radial => {
                let r = bounds.width().max(bounds.height()).max(1.0) / 2.0;
                Fill::Radial {
                    stops: self.gradient.clone(),
                    matrix: Affine::translate(bounds.center().x, bounds.center().y) * Affine::scale(r, r),
                    spread: SpreadMode::Pad,
                    focal: 0.0,
                }
            }
        })
    }
}

/// Free-transform handle under the pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Handle {
    /// Scale handle at box-local unit position (0, 0.5, 1 per axis).
    Scale(f64, f64),
    Rotate,
    /// Skew along an edge: true = horizontal edge.
    Skew(bool, f64),
    Pivot,
}

/// In-progress pointer interaction on the stage.
#[derive(Debug, Clone, Default)]
pub enum Drag {
    #[default]
    None,
    Marquee {
        start: Vec2,
        add: bool,
    },
    Move {
        last: Vec2,
        started: bool,
    },
    Transform {
        handle: Handle,
        start: Vec2,
        /// Box frame: maps box-local coordinates to world.
        frame: Affine,
        local: animara_geometry::Rect,
        pivot: Vec2,
        originals: Vec<(animara_document::Id, Affine, Vec2)>,
    },
    Points {
        points: Vec<(Vec2, f64, f64)>,
    },
    Rect {
        start: Vec2,
    },
    Pan {
        last_screen: egui::Pos2,
    },
    Lasso {
        points: Vec<Vec2>,
    },
    Anchor {
        element: animara_document::Id,
        path: usize,
        cmd: usize,
        control: Option<usize>,
        last: Vec2,
    },
    Gradient {
        element: animara_document::Id,
        path: usize,
        kind: u8,
        start: Vec2,
        original: Affine,
    },
    Bone {
        start: Vec2,
        parent: Option<usize>,
    },
    Ik {
        bone: usize,
    },
    Camera {
        last: Vec2,
    },
    Warp {
        element: animara_document::Id,
        pin: usize,
        last: Vec2,
    },
    TextBox {
        start: Vec2,
    },
}

/// Pen tool path under construction (anchor, out-handle).
#[derive(Debug, Clone, Default)]
pub struct PenState {
    pub points: Vec<(Vec2, Vec2)>,
    pub dragging: bool,
}

#[derive(Default)]
pub struct ToolState {
    pub drag: Drag,
    pub pen: PenState,
    /// Asset-warp pins for the element being warped (element id, local pin positions).
    pub warp_pins: Option<(animara_document::Id, Vec<Vec2>)>,
    pub last_pointer_doc: Vec2,
    pub hover_handle: Option<Handle>,
}
