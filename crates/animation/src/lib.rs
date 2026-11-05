//! Animation engine: resolves a timeline at a frame into concrete elements
//! (tween interpolation, motion guides, graphic-symbol synchronisation,
//! armatures, layer parenting, cameras) and flattens nested symbols into a
//! renderer-ready [`DisplayItem`] tree.

mod display;
mod tween;

pub use display::{
    DisplayItem, DisplayList, EvalOptions, evaluate_document_frame, evaluate_timeline, recolor_item, scene_at_global_frame,
    transform_item,
};
pub use tween::{camera_at, layer_elements_at, pose_at};
