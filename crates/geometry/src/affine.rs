use crate::{Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::ops::Mul;

/// 2D affine matrix in the Flash/XFL convention:
/// `x' = a*x + c*y + tx`, `y' = b*x + d*y + ty`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

/// Decomposed transform used for tween interpolation and the Transform panel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TransformParts {
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// Rotation of the x axis, radians.
    pub skew_x: f64,
    /// Rotation of the y axis, radians (equals skew_x when there is no skew).
    pub skew_y: f64,
}

impl Default for Affine {
    fn default() -> Self {
        Affine::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub const fn new(a: f64, b: f64, c: f64, d: f64, tx: f64, ty: f64) -> Self {
        Affine { a, b, c, d, tx, ty }
    }
    pub fn translate(x: f64, y: f64) -> Affine {
        Affine { tx: x, ty: y, ..Affine::IDENTITY }
    }
    pub fn scale(sx: f64, sy: f64) -> Affine {
        Affine { a: sx, d: sy, ..Affine::IDENTITY }
    }
    pub fn rotate(angle: f64) -> Affine {
        let (s, c) = angle.sin_cos();
        Affine { a: c, b: s, c: -s, d: c, tx: 0.0, ty: 0.0 }
    }
    pub fn skew(kx: f64, ky: f64) -> Affine {
        Affine { a: 1.0, b: ky.tan(), c: kx.tan(), d: 1.0, tx: 0.0, ty: 0.0 }
    }
    /// Transform about a pivot point: `T(p) * m * T(-p)`.
    pub fn around(m: Affine, pivot: Vec2) -> Affine {
        Affine::translate(pivot.x, pivot.y) * m * Affine::translate(-pivot.x, -pivot.y)
    }
    pub fn apply(&self, p: Vec2) -> Vec2 {
        Vec2::new(self.a * p.x + self.c * p.y + self.tx, self.b * p.x + self.d * p.y + self.ty)
    }
    pub fn apply_vector(&self, v: Vec2) -> Vec2 {
        Vec2::new(self.a * v.x + self.c * v.y, self.b * v.x + self.d * v.y)
    }
    pub fn determinant(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }
    pub fn invert(&self) -> Option<Affine> {
        let det = self.determinant();
        if det.abs() < 1e-14 || !det.is_finite() {
            return None;
        }
        let inv = 1.0 / det;
        let a = self.d * inv;
        let b = -self.b * inv;
        let c = -self.c * inv;
        let d = self.a * inv;
        Some(Affine { a, b, c, d, tx: -(a * self.tx + c * self.ty), ty: -(b * self.tx + d * self.ty) })
    }
    pub fn translation(&self) -> Vec2 {
        Vec2::new(self.tx, self.ty)
    }
    pub fn with_translation(mut self, p: Vec2) -> Affine {
        self.tx = p.x;
        self.ty = p.y;
        self
    }
    /// Average linear scale factor (useful for stroke widths).
    pub fn mean_scale(&self) -> f64 {
        self.determinant().abs().sqrt()
    }
    pub fn transform_rect(&self, r: &Rect) -> Rect {
        let mut out = Rect::EMPTY;
        if r.is_empty() {
            return out;
        }
        for c in r.corners() {
            out.include(self.apply(c));
        }
        out
    }
    pub fn decompose(&self) -> TransformParts {
        let scale_x = self.a.hypot(self.b);
        let scale_y = self.c.hypot(self.d);
        let skew_x = self.b.atan2(self.a);
        let mut skew_y = (-self.c).atan2(self.d);
        let mut sy = scale_y;
        // Detect mirrored transforms: keep scale_x positive and flip y.
        if self.determinant() < 0.0 {
            sy = -scale_y;
            skew_y = self.c.atan2(-self.d);
        }
        TransformParts { x: self.tx, y: self.ty, scale_x, scale_y: sy, skew_x, skew_y }
    }
    pub fn compose(p: &TransformParts) -> Affine {
        Affine {
            a: p.scale_x * p.skew_x.cos(),
            b: p.scale_x * p.skew_x.sin(),
            c: -p.scale_y * p.skew_y.sin(),
            d: p.scale_y * p.skew_y.cos(),
            tx: p.x,
            ty: p.y,
        }
    }
    pub fn is_finite(&self) -> bool {
        [self.a, self.b, self.c, self.d, self.tx, self.ty].iter().all(|v| v.is_finite())
    }
    pub fn approx_eq(&self, o: &Affine, eps: f64) -> bool {
        (self.a - o.a).abs() < eps
            && (self.b - o.b).abs() < eps
            && (self.c - o.c).abs() < eps
            && (self.d - o.d).abs() < eps
            && (self.tx - o.tx).abs() < eps
            && (self.ty - o.ty).abs() < eps
    }
}

impl TransformParts {
    pub fn rotation(&self) -> f64 {
        self.skew_x
    }
}

impl Mul for Affine {
    type Output = Affine;
    /// `self * o` applies `o` first, then `self`.
    fn mul(self, o: Affine) -> Affine {
        Affine {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            tx: self.a * o.tx + self.c * o.ty + self.tx,
            ty: self.b * o.tx + self.d * o.ty + self.ty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_roundtrip() {
        let m = Affine::translate(10.0, 5.0) * Affine::rotate(0.7) * Affine::scale(2.0, 3.0);
        let inv = m.invert().unwrap();
        assert!((m * inv).approx_eq(&Affine::IDENTITY, 1e-9));
        let p = Vec2::new(3.0, -4.0);
        let q = inv.apply(m.apply(p));
        assert!((p - q).length() < 1e-9);
    }

    #[test]
    fn decompose_compose_roundtrip() {
        for m in [
            Affine::translate(4.0, 2.0) * Affine::rotate(1.2) * Affine::scale(2.0, 0.5),
            Affine::scale(-1.0, 1.0),
            Affine::skew(0.3, 0.1) * Affine::scale(1.5, 1.5),
        ] {
            let p = m.decompose();
            let back = Affine::compose(&p);
            assert!(back.approx_eq(&m, 1e-9), "{m:?} -> {p:?} -> {back:?}");
        }
    }
}
