use crate::Vec2;
use serde::{Deserialize, Serialize};

/// Axis-aligned rectangle (min inclusive, max exclusive semantics are not enforced).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub const EMPTY: Rect = Rect {
        min: Vec2 { x: f64::INFINITY, y: f64::INFINITY },
        max: Vec2 { x: f64::NEG_INFINITY, y: f64::NEG_INFINITY },
    };

    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect { min: Vec2::new(x0.min(x1), y0.min(y1)), max: Vec2::new(x0.max(x1), y0.max(y1)) }
    }
    pub fn from_points(a: Vec2, b: Vec2) -> Rect {
        Rect::new(a.x, a.y, b.x, b.y)
    }
    pub fn from_xywh(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, x + w, y + h)
    }
    pub fn is_empty(&self) -> bool {
        !(self.min.x <= self.max.x && self.min.y <= self.max.y)
    }
    pub fn width(&self) -> f64 {
        (self.max.x - self.min.x).max(0.0)
    }
    pub fn height(&self) -> f64 {
        (self.max.y - self.min.y).max(0.0)
    }
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
    pub fn include(&mut self, p: Vec2) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    pub fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        Rect { min: self.min.min(o.min), max: self.max.max(o.max) }
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        !self.is_empty()
            && !o.is_empty()
            && self.min.x <= o.max.x
            && o.min.x <= self.max.x
            && self.min.y <= o.max.y
            && o.min.y <= self.max.y
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
    pub fn contains_rect(&self, o: &Rect) -> bool {
        self.contains(o.min) && self.contains(o.max)
    }
    pub fn inflate(&self, d: f64) -> Rect {
        Rect { min: self.min - Vec2::new(d, d), max: self.max + Vec2::new(d, d) }
    }
    pub fn corners(&self) -> [Vec2; 4] {
        [self.min, Vec2::new(self.max.x, self.min.y), self.max, Vec2::new(self.min.x, self.max.y)]
    }
}

impl Default for Rect {
    fn default() -> Self {
        Rect::EMPTY
    }
}
