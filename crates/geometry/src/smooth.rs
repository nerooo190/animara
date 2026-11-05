//! Freehand input processing: polyline simplification, curve fitting and
//! pressure-sensitive brush outlines.

use crate::boolean::{polygons_to_path, simplify_fill};
use crate::path::point_segment_distance;
use crate::{FillRule, Path, Vec2};

/// A brush sample with already pressure-scaled diameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeSample {
    pub pos: Vec2,
    pub width: f64,
}

/// Ramer–Douglas–Peucker simplification.
pub fn simplify_polyline(points: &[Vec2], tolerance: f64) -> Vec<Vec2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((s, e)) = stack.pop() {
        let mut best = (0usize, -1.0f64);
        for i in s + 1..e {
            let d = point_segment_distance(points[i], points[s], points[e]);
            if d > best.1 {
                best = (i, d);
            }
        }
        if best.1 > tolerance {
            keep[best.0] = true;
            stack.push((s, best.0));
            stack.push((best.0, e));
        }
    }
    points.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

/// Fit a smooth cubic spline through freehand points (Pencil "Smooth" mode).
/// `tolerance` controls how aggressively points are simplified first.
pub fn fit_smooth_path(points: &[Vec2], tolerance: f64) -> Path {
    let mut pts: Vec<Vec2> = Vec::with_capacity(points.len());
    for p in points {
        if pts.last().is_none_or(|q: &Vec2| q.distance(*p) > 0.01) {
            pts.push(*p);
        }
    }
    if pts.len() < 2 {
        let mut p = Path::new();
        if let Some(q) = pts.first() {
            p.move_to(*q).line_to(*q + Vec2::new(0.01, 0.0));
        }
        return p;
    }
    let closed = pts.len() > 3 && pts[0].distance(*pts.last().unwrap()) < tolerance.max(2.0) * 2.0;
    let s = simplify_polyline(&pts, tolerance);
    if s.len() == 2 {
        return Path::line(s[0], s[1]);
    }
    let n = s.len();
    let get = |i: isize| -> Vec2 {
        if closed {
            s[(i.rem_euclid(n as isize - 1)) as usize]
        } else {
            s[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let mut path = Path::new();
    path.move_to(s[0]);
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (get(i as isize - 1), get(i as isize), get(i as isize + 1), get(i as isize + 2));
        let c1 = p1 + (p2 - p0) / 6.0;
        let c2 = p2 - (p3 - p1) / 6.0;
        path.cubic_to(c1, c2, p2);
    }
    if closed {
        path.close();
    }
    path
}

fn circle_poly(c: Vec2, r: f64) -> Vec<[f64; 2]> {
    let n = ((r * 1.5).ceil() as usize).clamp(8, 32);
    (0..n)
        .map(|i| {
            let a = i as f64 * std::f64::consts::TAU / n as f64;
            [c.x + r * a.cos(), c.y + r * a.sin()]
        })
        .collect()
}

/// Build a filled outline for a variable-width stroke (Paint Brush / Fluid Brush).
/// The result is a clean, non-self-intersecting fill path.
pub fn variable_width_outline(samples: &[StrokeSample]) -> Path {
    let pts: Vec<StrokeSample> = samples.iter().copied().filter(|s| s.pos.is_finite() && s.width > 0.0).collect();
    if pts.is_empty() {
        return Path::new();
    }
    if pts.len() == 1 {
        return polygons_to_path(&[vec![circle_poly(pts[0].pos, pts[0].width / 2.0)]]);
    }
    let mut contours: Vec<Vec<[f64; 2]>> = Vec::with_capacity(pts.len() * 2);
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let dir = (b.pos - a.pos).normalize();
        if dir == Vec2::ZERO {
            continue;
        }
        let n = dir.perp();
        let (ra, rb) = (a.width / 2.0, b.width / 2.0);
        let quad = [a.pos + n * ra, b.pos + n * rb, b.pos - n * rb, a.pos - n * ra];
        contours.push(quad.iter().map(|p| [p.x, p.y]).collect());
    }
    for s in &pts {
        contours.push(circle_poly(s.pos, s.width / 2.0));
    }
    // Give every contour the same orientation so NonZero unions them instead of cancelling.
    for c in &mut contours {
        let n = c.len();
        let area: f64 = (0..n).map(|i| c[i][0] * c[(i + 1) % n][1] - c[(i + 1) % n][0] * c[i][1]).sum();
        if area < 0.0 {
            c.reverse();
        }
    }
    let raw = polygons_to_path(&[contours]);
    simplify_fill(&raw, FillRule::NonZero)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rdp_removes_collinear_points() {
        let pts: Vec<Vec2> = (0..10).map(|i| Vec2::new(i as f64, 0.0)).collect();
        assert_eq!(simplify_polyline(&pts, 0.1).len(), 2);
    }

    #[test]
    fn smooth_fit_passes_through_ends() {
        let pts: Vec<Vec2> = (0..50).map(|i| Vec2::new(i as f64, (i as f64 * 0.2).sin() * 10.0)).collect();
        let p = fit_smooth_path(&pts, 0.5);
        let b = p.bounds();
        assert!(b.min.x.abs() < 1e-6 && (b.max.x - 49.0).abs() < 1e-6);
    }

    #[test]
    fn brush_outline_has_area() {
        let s: Vec<StrokeSample> =
            (0..20).map(|i| StrokeSample { pos: Vec2::new(i as f64 * 5.0, 0.0), width: 4.0 + i as f64 * 0.2 }).collect();
        let p = variable_width_outline(&s);
        let b = p.bounds();
        assert!(b.width() > 95.0 && b.height() > 4.0);
        assert!(p.contains(Vec2::new(50.0, 0.0), FillRule::NonZero));
    }
}
