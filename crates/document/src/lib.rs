//! Animara document model.
//!
//! A [`Document`] contains scenes, each with a [`Timeline`] of [`Layer`]s.
//! Layers hold [`Keyframe`] spans whose [`Element`]s are drawn on the stage.
//! Reusable content lives in the [`Library`] as [`Symbol`]s, bitmaps and sounds.

mod color;
mod document;
mod ease;
mod element;
mod history;
mod id;
mod io;
mod library;
mod timeline;

pub use animara_geometry as geom;
pub use color::{Color, ColorEffect, ColorTransform, Fill, GradientStop, LineCap, LineJoin, SpreadMode, Stroke, StrokeStyle};
pub use document::{Document, FORMAT_VERSION, GridSettings, Guides, Metadata, PublishSettings, Scene, TEMPLATES, Template};
pub use ease::{CurvePoint, Ease, EasePreset};
pub use element::{
    BitmapInstance, BlendMode, Element, ElementKind, Filter, LoopMode, Primitive, Shape, ShapePath, SymbolInstance, TextAlign, TextElement,
    TextKind, TextRun, TextStyle,
};
pub use history::History;
pub use id::{Id, bump_ids_above, fresh_id};
pub use io::{DocumentError, load_document, load_from_bytes, save_document, save_to_bytes};
pub use library::{BitmapAsset, Library, SoundAsset, Symbol, SymbolKind};
pub use timeline::{
    Armature, Bone, CameraState, FrameClipboard, FrameLabel, Keyframe, LAYER_COLORS, LabelKind, Layer, LayerKind, Pose, RotateMode,
    SoundRef, SoundSync, Timeline, Tween, TweenKind,
};
