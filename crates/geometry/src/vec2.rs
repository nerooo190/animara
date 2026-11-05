use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// A 2D point or vector.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    #[inline]
    pub fn dot(self, o: Vec2) -> f64 {
        self.x * o.x + self.y * o.y
    }
    #[inline]
    pub fn cross(self, o: Vec2) -> f64 {
        self.x * o.y - self.y * o.x
    }
    #[inline]
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }
    #[inline]
    pub fn length_sq(self) -> f64 {
        self.x * self.x + self.y * self.y
    }
    #[inline]
    pub fn distance(self, o: Vec2) -> f64 {
        (self - o).length()
    }
    pub fn normalize(self) -> Vec2 {
        let l = self.length();
        if l > 1e-12 { self / l } else { Vec2::ZERO }
    }
    /// Perpendicular vector (rotated 90° counter-clockwise in y-down space).
    #[inline]
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }
    #[inline]
    pub fn lerp(self, o: Vec2, t: f64) -> Vec2 {
        self + (o - self) * t
    }
    #[inline]
    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }
    pub fn rotate(self, angle: f64) -> Vec2 {
        let (s, c) = angle.sin_cos();
        Vec2::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }
    pub fn from_angle(angle: f64) -> Vec2 {
        Vec2::new(angle.cos(), angle.sin())
    }
    pub fn min(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.min(o.x), self.y.min(o.y))
    }
    pub fn max(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.max(o.x), self.y.max(o.y))
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}
impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f64) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}
impl Div<f64> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f64) -> Vec2 {
        Vec2::new(self.x / s, self.y / s)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}
