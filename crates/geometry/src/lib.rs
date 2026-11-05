//! Core 2D geometry for Animara: vectors, affine transforms, Bézier paths,
//! hit-testing, flattening, boolean operations and stroke outlining.
//!
//! All coordinates are `f64` document pixels with the y axis pointing down,
//! matching the stage coordinate system of classic Flash-style authoring tools.

mod affine;
mod boolean;
mod path;
mod rect;
mod smooth;
mod vec2;

pub use affine::{Affine, TransformParts};
pub use boolean::{BoolOp, boolean_op, polygons_to_path, simplify_fill};
pub use path::{FillRule, Morph, Path, PathCmd, Segment, SubPath};
pub use rect::Rect;
pub use smooth::{StrokeSample, fit_smooth_path, simplify_polyline, variable_width_outline};
pub use vec2::Vec2;

/// Linear interpolation helper.
#[inline]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Interpolate angles (radians) along the shortest arc.
pub fn lerp_angle(a: f64, b: f64, t: f64) -> f64 {
    let mut d = (b - a) % std::f64::consts::TAU;
    if d > std::f64::consts::PI {
        d -= std::f64::consts::TAU;
    } else if d < -std::f64::consts::PI {
        d += std::f64::consts::TAU;
    }
    a + d * t
}
