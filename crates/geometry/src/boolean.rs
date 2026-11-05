use crate::{FillRule, Path, Vec2};
use i_overlay::core::fill_rule::FillRule as OvFill;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;

/// Boolean path operation (Modify ▸ Combine Objects).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Intersect,
    /// `a - b` (Punch).
    Difference,
    Xor,
}

const TOL: f64 = 0.2;

fn to_contours(p: &Path) -> Vec<Vec<[f64; 2]>> {
    p.flatten(TOL)
        .into_iter()
        .filter(|(pts, _)| pts.len() >= 3)
        .map(|(pts, _)| pts.into_iter().map(|v| [v.x, v.y]).collect())
        .collect()
}

fn fill(rule: FillRule) -> OvFill {
    match rule {
        FillRule::NonZero => OvFill::NonZero,
        FillRule::EvenOdd => OvFill::EvenOdd,
    }
}

/// Convert overlay output (shapes → contours → points) into a path.
pub fn polygons_to_path(shapes: &[Vec<Vec<[f64; 2]>>]) -> Path {
    let mut p = Path::new();
    for shape in shapes {
        for contour in shape {
            let pts: Vec<Vec2> = contour.iter().map(|q| Vec2::new(q[0], q[1])).collect();
            if pts.len() >= 3 {
                p.extend(&Path::polyline(&pts, true));
            }
        }
    }
    p
}

/// Perform a boolean operation between two filled paths. Curves are flattened
/// to polygons (0.2 px tolerance); the result consists of straight segments.
pub fn boolean_op(a: &Path, b: &Path, op: BoolOp, rule: FillRule) -> Path {
    let sa = to_contours(a);
    let sb = to_contours(b);
    let rule_ov = match op {
        BoolOp::Union => OverlayRule::Union,
        BoolOp::Intersect => OverlayRule::Intersect,
        BoolOp::Difference => OverlayRule::Difference,
        BoolOp::Xor => OverlayRule::Xor,
    };
    if sa.is_empty() && sb.is_empty() {
        return Path::new();
    }
    let shapes = sa.overlay(&sb, rule_ov, fill(rule));
    polygons_to_path(&shapes)
}

/// Remove self-intersections and overlaps, producing a clean outline.
pub fn simplify_fill(p: &Path, rule: FillRule) -> Path {
    let s = to_contours(p);
    if s.is_empty() {
        return Path::new();
    }
    polygons_to_path(&s.simplify_shape(fill(rule)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    fn area(p: &Path) -> f64 {
        p.flatten(0.1)
            .iter()
            .map(|(pts, _)| {
                let n = pts.len();
                (0..n).map(|i| pts[i].cross(pts[(i + 1) % n])).sum::<f64>() * 0.5
            })
            .sum::<f64>()
            .abs()
    }

    #[test]
    fn union_and_difference_areas() {
        let a = Path::rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        let b = Path::rect(Rect::new(5.0, 0.0, 15.0, 10.0));
        assert!((area(&boolean_op(&a, &b, BoolOp::Union, FillRule::NonZero)) - 150.0).abs() < 1e-6);
        assert!((area(&boolean_op(&a, &b, BoolOp::Intersect, FillRule::NonZero)) - 50.0).abs() < 1e-6);
        assert!((area(&boolean_op(&a, &b, BoolOp::Difference, FillRule::NonZero)) - 50.0).abs() < 1e-6);
    }
}
