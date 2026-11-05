use animara_geometry::{Affine, lerp};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Straight-alpha RGBA color with components in `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub const fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color { r, g, b, a: 1.0 }
    }
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 }
    }
    pub fn to_rgba8(self) -> [u8; 4] {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [c(self.r), c(self.g), c(self.b), c(self.a)]
    }
    pub fn with_alpha(self, a: f32) -> Color {
        Color { a, ..self }
    }
    pub fn lerp(self, o: Color, t: f64) -> Color {
        let t = t as f32;
        Color {
            r: self.r + (o.r - self.r) * t,
            g: self.g + (o.g - self.g) * t,
            b: self.b + (o.b - self.b) * t,
            a: self.a + (o.a - self.a) * t,
        }
    }
    /// `#RRGGBB` or `#RRGGBBAA`.
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.to_rgba8();
        if a == 255 { format!("#{r:02X}{g:02X}{b:02X}") } else { format!("#{r:02X}{g:02X}{b:02X}{a:02X}") }
    }
    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            6 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, 255)),
            8 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            3 => {
                let n = |i: usize| u8::from_str_radix(s.get(i..i + 1)?, 16).ok().map(|v| v * 17);
                Some(Color::from_rgba8(n(0)?, n(1)?, n(2)?, 255))
            }
            _ => None,
        }
    }
    /// HSV → RGB, `h` in degrees, `s`/`v` in `0..=1`.
    pub fn from_hsv(h: f32, s: f32, v: f32, a: f32) -> Color {
        let h = h.rem_euclid(360.0) / 60.0;
        let c = v * s;
        let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        Color { r: r + m, g: g + m, b: b + m, a }
    }
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        let d = max - min;
        let h = if d == 0.0 {
            0.0
        } else if max == self.r {
            60.0 * ((self.g - self.b) / d).rem_euclid(6.0)
        } else if max == self.g {
            60.0 * ((self.b - self.r) / d + 2.0)
        } else {
            60.0 * ((self.r - self.g) / d + 4.0)
        };
        let s = if max == 0.0 { 0.0 } else { d / max };
        (h, s, max)
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Full precision is kept when the color is not representable in 8 bits.
        let [r, g, b, a] = self.to_rgba8();
        if Color::from_rgba8(r, g, b, a) == *self { s.serialize_str(&self.to_hex()) } else { [self.r, self.g, self.b, self.a].serialize(s) }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Hex(String),
            Arr([f32; 4]),
        }
        match Repr::deserialize(d)? {
            Repr::Hex(h) => Color::from_hex(&h).ok_or_else(|| serde::de::Error::custom(format!("bad color {h}"))),
            Repr::Arr([r, g, b, a]) => Ok(Color { r, g, b, a }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpreadMode {
    #[default]
    Pad,
    Reflect,
    Repeat,
}

/// Paint used for shape fills and strokes.
///
/// Gradients are defined in a canonical space: linear gradients run along x
/// from -1 to 1; radial gradients are a unit circle at the origin. `matrix`
/// maps that space into the shape's local coordinates (the Gradient Transform tool edits it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Fill {
    Solid {
        color: Color,
    },
    Linear {
        stops: Vec<GradientStop>,
        matrix: Affine,
        #[serde(default)]
        spread: SpreadMode,
    },
    Radial {
        stops: Vec<GradientStop>,
        matrix: Affine,
        #[serde(default)]
        spread: SpreadMode,
        #[serde(default)]
        focal: f64,
    },
    Bitmap {
        asset: crate::Id,
        matrix: Affine,
        #[serde(default = "yes")]
        smooth: bool,
        #[serde(default = "yes")]
        repeat: bool,
    },
}

fn yes() -> bool {
    true
}

impl Fill {
    pub fn solid(c: Color) -> Fill {
        Fill::Solid { color: c }
    }
    /// Representative color (first stop for gradients), used by swatches and eyedropper.
    pub fn primary_color(&self) -> Color {
        match self {
            Fill::Solid { color } => *color,
            Fill::Linear { stops, .. } | Fill::Radial { stops, .. } => stops.first().map(|s| s.color).unwrap_or(Color::BLACK),
            Fill::Bitmap { .. } => Color::rgb(0.5, 0.5, 0.5),
        }
    }
    pub fn stops(&self) -> Option<&[GradientStop]> {
        match self {
            Fill::Linear { stops, .. } | Fill::Radial { stops, .. } => Some(stops),
            _ => None,
        }
    }
    pub fn matrix(&self) -> Option<Affine> {
        match self {
            Fill::Linear { matrix, .. } | Fill::Radial { matrix, .. } | Fill::Bitmap { matrix, .. } => Some(*matrix),
            _ => None,
        }
    }
    pub fn set_matrix(&mut self, m: Affine) {
        if let Fill::Linear { matrix, .. } | Fill::Radial { matrix, .. } | Fill::Bitmap { matrix, .. } = self {
            *matrix = m;
        }
    }
    pub fn transformed(&self, m: &Affine) -> Fill {
        let mut f = self.clone();
        if let Some(old) = f.matrix() {
            f.set_matrix(*m * old);
        }
        f
    }
    /// Interpolate two fills (shape tweens). Mismatched kinds snap at t = 0.5.
    pub fn lerp(&self, o: &Fill, t: f64) -> Fill {
        let lerp_stops = |a: &[GradientStop], b: &[GradientStop]| -> Vec<GradientStop> {
            let n = a.len().max(b.len());
            (0..n)
                .map(|i| {
                    let sa = a[i.min(a.len() - 1)];
                    let sb = b[i.min(b.len() - 1)];
                    GradientStop { offset: lerp(sa.offset as f64, sb.offset as f64, t) as f32, color: sa.color.lerp(sb.color, t) }
                })
                .collect()
        };
        let lerp_m = |a: &Affine, b: &Affine| Affine {
            a: lerp(a.a, b.a, t),
            b: lerp(a.b, b.b, t),
            c: lerp(a.c, b.c, t),
            d: lerp(a.d, b.d, t),
            tx: lerp(a.tx, b.tx, t),
            ty: lerp(a.ty, b.ty, t),
        };
        match (self, o) {
            (Fill::Solid { color: a }, Fill::Solid { color: b }) => Fill::Solid { color: a.lerp(*b, t) },
            (Fill::Linear { stops: sa, matrix: ma, spread }, Fill::Linear { stops: sb, matrix: mb, .. })
                if !sa.is_empty() && !sb.is_empty() =>
            {
                Fill::Linear { stops: lerp_stops(sa, sb), matrix: lerp_m(ma, mb), spread: *spread }
            }
            (Fill::Radial { stops: sa, matrix: ma, spread, focal: fa }, Fill::Radial { stops: sb, matrix: mb, focal: fb, .. })
                if !sa.is_empty() && !sb.is_empty() =>
            {
                Fill::Radial { stops: lerp_stops(sa, sb), matrix: lerp_m(ma, mb), spread: *spread, focal: lerp(*fa, *fb, t) }
            }
            (Fill::Solid { color }, other @ (Fill::Linear { .. } | Fill::Radial { .. })) => {
                let mut as_grad = other.clone();
                if let Fill::Linear { stops, .. } | Fill::Radial { stops, .. } = &mut as_grad {
                    stops.iter_mut().for_each(|s| s.color = *color);
                }
                as_grad.lerp(other, t)
            }
            (a @ (Fill::Linear { .. } | Fill::Radial { .. }), Fill::Solid { color }) => {
                let mut as_grad = a.clone();
                if let Fill::Linear { stops, .. } | Fill::Radial { stops, .. } = &mut as_grad {
                    stops.iter_mut().for_each(|s| s.color = *color);
                }
                a.lerp(&as_grad, t)
            }
            _ => {
                if t < 0.5 {
                    self.clone()
                } else {
                    o.clone()
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineCap {
    Butt,
    #[default]
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineJoin {
    Miter,
    #[default]
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StrokeStyle {
    #[default]
    Solid,
    Dashed {
        dash: f64,
        gap: f64,
    },
    Dotted {
        spacing: f64,
    },
    Hairline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub fill: Fill,
    pub width: f64,
    #[serde(default)]
    pub cap: LineCap,
    #[serde(default)]
    pub join: LineJoin,
    #[serde(default = "default_miter")]
    pub miter_limit: f64,
    #[serde(default)]
    pub style: StrokeStyle,
    /// Scale stroke width with the object (`false` = "Scale: None").
    #[serde(default = "yes")]
    pub scale_with_object: bool,
}

fn default_miter() -> f64 {
    3.0
}

impl Stroke {
    pub fn solid(color: Color, width: f64) -> Stroke {
        Stroke {
            fill: Fill::solid(color),
            width,
            cap: LineCap::Round,
            join: LineJoin::Round,
            miter_limit: 3.0,
            style: StrokeStyle::Solid,
            scale_with_object: true,
        }
    }
    pub fn lerp(&self, o: &Stroke, t: f64) -> Stroke {
        Stroke { fill: self.fill.lerp(&o.fill, t), width: lerp(self.width, o.width, t), ..self.clone() }
    }
}

/// Linear per-channel color transform: `out = in * mul + add` (add in 0..1 units).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorTransform {
    pub mul: [f32; 4],
    pub add: [f32; 4],
}

impl Default for ColorTransform {
    fn default() -> Self {
        ColorTransform::IDENTITY
    }
}

impl ColorTransform {
    pub const IDENTITY: ColorTransform = ColorTransform { mul: [1.0; 4], add: [0.0; 4] };

    pub fn is_identity(&self) -> bool {
        *self == ColorTransform::IDENTITY
    }
    pub fn apply(&self, c: Color) -> Color {
        Color {
            r: (c.r * self.mul[0] + self.add[0]).clamp(0.0, 1.0),
            g: (c.g * self.mul[1] + self.add[1]).clamp(0.0, 1.0),
            b: (c.b * self.mul[2] + self.add[2]).clamp(0.0, 1.0),
            a: (c.a * self.mul[3] + self.add[3]).clamp(0.0, 1.0),
        }
    }
    /// Compose: apply `inner` first, then `self`.
    pub fn concat(&self, inner: &ColorTransform) -> ColorTransform {
        let mut out = ColorTransform::IDENTITY;
        for i in 0..4 {
            out.mul[i] = inner.mul[i] * self.mul[i];
            out.add[i] = inner.add[i] * self.mul[i] + self.add[i];
        }
        out
    }
    pub fn lerp(&self, o: &ColorTransform, t: f64) -> ColorTransform {
        let t = t as f32;
        let mut out = *self;
        for i in 0..4 {
            out.mul[i] += (o.mul[i] - self.mul[i]) * t;
            out.add[i] += (o.add[i] - self.add[i]) * t;
        }
        out
    }
}

/// Instance "Color Effect" style, as presented in the Properties panel.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ColorEffect {
    #[default]
    None,
    /// -1..=1
    Brightness {
        amount: f32,
    },
    /// amount 0..=1
    Tint {
        color: Color,
        amount: f32,
    },
    /// 0..=1
    Alpha {
        alpha: f32,
    },
    Advanced {
        transform: ColorTransform,
    },
}

impl ColorEffect {
    pub fn to_transform(&self) -> ColorTransform {
        match *self {
            ColorEffect::None => ColorTransform::IDENTITY,
            ColorEffect::Brightness { amount } => {
                if amount >= 0.0 {
                    ColorTransform { mul: [1.0 - amount, 1.0 - amount, 1.0 - amount, 1.0], add: [amount, amount, amount, 0.0] }
                } else {
                    let m = 1.0 + amount;
                    ColorTransform { mul: [m, m, m, 1.0], add: [0.0; 4] }
                }
            }
            ColorEffect::Tint { color, amount } => {
                let m = 1.0 - amount;
                ColorTransform { mul: [m, m, m, 1.0], add: [color.r * amount, color.g * amount, color.b * amount, 0.0] }
            }
            ColorEffect::Alpha { alpha } => ColorTransform { mul: [1.0, 1.0, 1.0, alpha], add: [0.0; 4] },
            ColorEffect::Advanced { transform } => transform,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let c = Color::from_rgba8(0x12, 0xAB, 0xFF, 0x80);
        assert_eq!(Color::from_hex(&c.to_hex()), Some(c));
        assert_eq!(Color::from_hex("#fff"), Some(Color::WHITE));
    }

    #[test]
    fn hsv_roundtrip() {
        let c = Color::rgb(0.2, 0.6, 0.9);
        let (h, s, v) = c.to_hsv();
        let d = Color::from_hsv(h, s, v, 1.0);
        assert!((c.r - d.r).abs() < 1e-5 && (c.g - d.g).abs() < 1e-5 && (c.b - d.b).abs() < 1e-5);
    }

    #[test]
    fn tint_and_brightness() {
        let t = ColorEffect::Tint { color: Color::rgb(1.0, 0.0, 0.0), amount: 1.0 }.to_transform();
        assert_eq!(t.apply(Color::rgb(0.3, 0.3, 0.3)), Color::rgb(1.0, 0.0, 0.0));
        let b = ColorEffect::Brightness { amount: -1.0 }.to_transform();
        assert_eq!(b.apply(Color::WHITE), Color::BLACK);
    }
}
