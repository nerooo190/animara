use crate::{Color, ColorEffect, Fill, Id, Stroke, fresh_id};
use animara_geometry::{Affine, FillRule, Path, Rect, Vec2};
use serde::{Deserialize, Serialize};

/// Parametric primitive shapes (Rectangle/Oval Primitive tools) that stay editable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Primitive {
    Rect { rect: Rect, corner_radius: f64 },
    Oval { rect: Rect, start_angle: f64, end_angle: f64, inner_radius: f64 },
    Polygon { center: Vec2, radius: f64, sides: u32, star: bool, inner_ratio: f64, rotation: f64 },
}

impl Primitive {
    pub fn to_path(&self) -> Path {
        match *self {
            Primitive::Rect { rect, corner_radius } => Path::rounded_rect(rect, corner_radius),
            Primitive::Oval { rect, start_angle, end_angle, inner_radius } => {
                let full = (end_angle - start_angle).abs() < 1e-9 || (end_angle - start_angle).abs() >= 360.0 - 1e-9;
                if full && inner_radius <= 0.0 {
                    return Path::ellipse(rect);
                }
                let c = rect.center();
                let (rx, ry) = (rect.width() / 2.0, rect.height() / 2.0);
                let (a0, mut a1) = (start_angle.to_radians(), end_angle.to_radians());
                if a1 <= a0 {
                    a1 += std::f64::consts::TAU;
                }
                let steps = 64;
                let pt = |a: f64, k: f64| Vec2::new(c.x + rx * k * a.cos(), c.y + ry * k * a.sin());
                let mut pts: Vec<Vec2> = (0..=steps).map(|i| pt(a0 + (a1 - a0) * i as f64 / steps as f64, 1.0)).collect();
                if inner_radius > 0.0 {
                    let k = inner_radius.clamp(0.0, 0.99);
                    pts.extend((0..=steps).rev().map(|i| pt(a0 + (a1 - a0) * i as f64 / steps as f64, k)));
                } else if !full {
                    pts.push(c);
                }
                Path::polyline(&pts, true)
            }
            Primitive::Polygon { center, radius, sides, star, inner_ratio, rotation } => {
                if star {
                    Path::star(center, radius, sides, inner_ratio, rotation)
                } else {
                    Path::polygon(center, radius, sides, rotation)
                }
            }
        }
    }
}

/// One filled and/or stroked contour group inside a shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapePath {
    pub path: Path,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    #[serde(default)]
    pub fill_rule: FillRule,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive: Option<Primitive>,
}

impl ShapePath {
    pub fn filled(path: Path, fill: Fill) -> ShapePath {
        ShapePath { path, fill: Some(fill), stroke: None, fill_rule: FillRule::NonZero, primitive: None }
    }
    pub fn stroked(path: Path, stroke: Stroke) -> ShapePath {
        ShapePath { path, fill: None, stroke: Some(stroke), fill_rule: FillRule::NonZero, primitive: None }
    }
    pub fn bounds(&self) -> Rect {
        let b = self.path.bounds();
        match &self.stroke {
            Some(s) => b.inflate(s.width / 2.0),
            None => b,
        }
    }
    /// Hit test in local coordinates with a pick tolerance.
    pub fn hit(&self, p: Vec2, tol: f64) -> bool {
        if self.fill.is_some() && self.path.contains(p, self.fill_rule) {
            return true;
        }
        let w = self.stroke.as_ref().map(|s| s.width / 2.0).unwrap_or(0.0);
        (self.stroke.is_some() || self.fill.is_none()) && self.path.distance_to(p) <= w + tol
    }
}

/// Vector artwork: an ordered list of paths drawn bottom to top.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Shape {
    pub paths: Vec<ShapePath>,
}

impl Shape {
    pub fn bounds(&self) -> Rect {
        self.paths.iter().fold(Rect::EMPTY, |r, p| r.union(&p.bounds()))
    }
    pub fn transformed(&self, m: &Affine) -> Shape {
        let s = m.mean_scale();
        Shape {
            paths: self
                .paths
                .iter()
                .map(|p| ShapePath {
                    path: p.path.transform(m),
                    fill: p.fill.as_ref().map(|f| f.transformed(m)),
                    stroke: p.stroke.as_ref().map(|st| {
                        let mut st = st.clone();
                        if st.scale_with_object {
                            st.width *= s;
                        }
                        st.fill = st.fill.transformed(m);
                        st
                    }),
                    fill_rule: p.fill_rule,
                    primitive: None,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    Normal,
    Layer,
    Multiply,
    Screen,
    Overlay,
    HardLight,
    Lighten,
    Darken,
    Difference,
    Add,
    Subtract,
    Invert,
    Alpha,
    Erase,
}

impl BlendMode {
    pub const ALL: [BlendMode; 14] = [
        BlendMode::Normal,
        BlendMode::Layer,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::HardLight,
        BlendMode::Lighten,
        BlendMode::Darken,
        BlendMode::Difference,
        BlendMode::Add,
        BlendMode::Subtract,
        BlendMode::Invert,
        BlendMode::Alpha,
        BlendMode::Erase,
    ];
}

/// Instance filters (Properties ▸ Filters).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Filter {
    Blur {
        blur_x: f64,
        blur_y: f64,
        quality: u32,
    },
    DropShadow {
        blur_x: f64,
        blur_y: f64,
        strength: f32,
        quality: u32,
        angle: f64,
        distance: f64,
        color: Color,
        knockout: bool,
        inner: bool,
        hide_object: bool,
    },
    Glow {
        blur_x: f64,
        blur_y: f64,
        strength: f32,
        quality: u32,
        color: Color,
        knockout: bool,
        inner: bool,
    },
    /// brightness/contrast/saturation in -100..100, hue in -180..180.
    AdjustColor {
        brightness: f32,
        contrast: f32,
        saturation: f32,
        hue: f32,
    },
}

impl Filter {
    pub fn default_blur() -> Filter {
        Filter::Blur { blur_x: 5.0, blur_y: 5.0, quality: 1 }
    }
    pub fn default_drop_shadow() -> Filter {
        Filter::DropShadow {
            blur_x: 5.0,
            blur_y: 5.0,
            strength: 1.0,
            quality: 1,
            angle: 45.0,
            distance: 5.0,
            color: Color::BLACK,
            knockout: false,
            inner: false,
            hide_object: false,
        }
    }
    pub fn default_glow() -> Filter {
        Filter::Glow {
            blur_x: 5.0,
            blur_y: 5.0,
            strength: 1.0,
            quality: 1,
            color: Color::rgb(1.0, 0.0, 0.0),
            knockout: false,
            inner: false,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Filter::Blur { .. } => "Blur",
            Filter::DropShadow { .. } => "Drop Shadow",
            Filter::Glow { .. } => "Glow",
            Filter::AdjustColor { .. } => "Adjust Color",
        }
    }
    /// How far the filter can extend content beyond its bounds.
    pub fn padding(&self) -> f64 {
        match *self {
            Filter::Blur { blur_x, blur_y, quality } => blur_x.max(blur_y) * quality.max(1) as f64,
            Filter::DropShadow { blur_x, blur_y, quality, distance, .. } => blur_x.max(blur_y) * quality.max(1) as f64 + distance,
            Filter::Glow { blur_x, blur_y, quality, .. } => blur_x.max(blur_y) * quality.max(1) as f64,
            Filter::AdjustColor { .. } => 0.0,
        }
    }
    /// Interpolate numeric filter parameters (tweened filters).
    pub fn lerp(&self, o: &Filter, t: f64) -> Filter {
        use animara_geometry::lerp as l;
        let lf = |a: f32, b: f32| a + (b - a) * t as f32;
        match (self, o) {
            (Filter::Blur { blur_x: ax, blur_y: ay, quality }, Filter::Blur { blur_x: bx, blur_y: by, .. }) => {
                Filter::Blur { blur_x: l(*ax, *bx, t), blur_y: l(*ay, *by, t), quality: *quality }
            }
            (
                Filter::DropShadow {
                    blur_x: ax,
                    blur_y: ay,
                    strength: asg,
                    angle: aa,
                    distance: ad,
                    color: ac,
                    quality,
                    knockout,
                    inner,
                    hide_object,
                },
                Filter::DropShadow { blur_x: bx, blur_y: by, strength: bs, angle: ba, distance: bd, color: bc, .. },
            ) => Filter::DropShadow {
                blur_x: l(*ax, *bx, t),
                blur_y: l(*ay, *by, t),
                strength: lf(*asg, *bs),
                quality: *quality,
                angle: l(*aa, *ba, t),
                distance: l(*ad, *bd, t),
                color: ac.lerp(*bc, t),
                knockout: *knockout,
                inner: *inner,
                hide_object: *hide_object,
            },
            (
                Filter::Glow { blur_x: ax, blur_y: ay, strength: asg, color: ac, quality, knockout, inner },
                Filter::Glow { blur_x: bx, blur_y: by, strength: bs, color: bc, .. },
            ) => Filter::Glow {
                blur_x: l(*ax, *bx, t),
                blur_y: l(*ay, *by, t),
                strength: lf(*asg, *bs),
                quality: *quality,
                color: ac.lerp(*bc, t),
                knockout: *knockout,
                inner: *inner,
            },
            (
                Filter::AdjustColor { brightness: ab, contrast: ac, saturation: as_, hue: ah },
                Filter::AdjustColor { brightness: bb, contrast: bc, saturation: bs, hue: bh },
            ) => Filter::AdjustColor { brightness: lf(*ab, *bb), contrast: lf(*ac, *bc), saturation: lf(*as_, *bs), hue: lf(*ah, *bh) },
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

/// Graphic-symbol playback options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LoopMode {
    #[default]
    Loop,
    PlayOnce,
    SingleFrame,
    LoopReverse,
    PlayOnceReverse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolInstance {
    pub symbol: Id,
    #[serde(default)]
    pub color_effect: ColorEffect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub blend: BlendMode,
    #[serde(default)]
    pub loop_mode: LoopMode,
    /// First frame (0-based) for graphic symbols.
    #[serde(default)]
    pub first_frame: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instance_name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub cache_as_bitmap: bool,
}

fn yes() -> bool {
    true
}

impl SymbolInstance {
    pub fn new(symbol: Id) -> SymbolInstance {
        SymbolInstance {
            symbol,
            color_effect: ColorEffect::None,
            filters: Vec::new(),
            blend: BlendMode::Normal,
            loop_mode: LoopMode::Loop,
            first_frame: 0,
            instance_name: String::new(),
            visible: true,
            cache_as_bitmap: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextKind {
    #[default]
    Static,
    Dynamic,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_family: String,
    pub size: f64,
    pub color: Color,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    /// Extra spacing between characters in px (tracking).
    #[serde(default)]
    pub letter_spacing: f64,
    #[serde(default = "yes")]
    pub kerning: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            font_family: "sans-serif".into(),
            size: 24.0,
            color: Color::BLACK,
            bold: false,
            italic: false,
            letter_spacing: 0.0,
            kerning: true,
            url: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    pub style: TextStyle,
}

/// Editable text block. Text is laid out from the local origin (top-left of the box).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextElement {
    pub runs: Vec<TextRun>,
    #[serde(default)]
    pub kind: TextKind,
    #[serde(default)]
    pub align: TextAlign,
    /// Fixed width for wrapping; `None` = auto-width (single line per paragraph).
    #[serde(default)]
    pub width: Option<f64>,
    /// Extra spacing between lines in px (leading).
    #[serde(default)]
    pub line_spacing: f64,
    #[serde(default)]
    pub rtl: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instance_name: String,
    #[serde(default)]
    pub selectable: bool,
}

impl TextElement {
    pub fn new(text: &str, style: TextStyle) -> TextElement {
        TextElement {
            runs: vec![TextRun { text: text.to_string(), style }],
            kind: TextKind::Static,
            align: TextAlign::Left,
            width: None,
            line_spacing: 2.0,
            rtl: false,
            instance_name: String::new(),
            selectable: false,
        }
    }
    pub fn plain_text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
    /// Replace all text, keeping the first run's style.
    pub fn set_plain_text(&mut self, text: &str) {
        let style = self.runs.first().map(|r| r.style.clone()).unwrap_or_default();
        self.runs = vec![TextRun { text: text.to_string(), style }];
    }
    pub fn style(&self) -> TextStyle {
        self.runs.first().map(|r| r.style.clone()).unwrap_or_default()
    }
    pub fn set_style_all(&mut self, f: impl Fn(&mut TextStyle)) {
        for r in &mut self.runs {
            f(&mut r.style);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BitmapInstance {
    pub asset: Id,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ElementKind {
    Shape(Shape),
    Instance(SymbolInstance),
    Text(TextElement),
    Bitmap(BitmapInstance),
    Group { children: Vec<Element> },
}

/// Anything placed on a keyframe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: Id,
    #[serde(default)]
    pub transform: Affine,
    /// Transformation point in local coordinates.
    #[serde(default)]
    pub pivot: Vec2,
    pub kind: ElementKind,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
}

impl Element {
    pub fn new(kind: ElementKind) -> Element {
        Element { id: fresh_id(), transform: Affine::IDENTITY, pivot: Vec2::ZERO, kind, locked: false }
    }
    pub fn shape(paths: Vec<ShapePath>) -> Element {
        Element::new(ElementKind::Shape(Shape { paths }))
    }
    pub fn with_transform(mut self, m: Affine) -> Element {
        self.transform = m;
        self
    }
    /// Deep copy with fresh ids (paste / duplicate / new keyframe content).
    pub fn duplicate(&self) -> Element {
        let mut e = self.clone();
        e.reassign_ids();
        e
    }
    pub fn reassign_ids(&mut self) {
        self.id = fresh_id();
        if let ElementKind::Group { children } = &mut self.kind {
            children.iter_mut().for_each(Element::reassign_ids);
        }
    }
    pub fn as_instance(&self) -> Option<&SymbolInstance> {
        match &self.kind {
            ElementKind::Instance(i) => Some(i),
            _ => None,
        }
    }
    pub fn as_instance_mut(&mut self) -> Option<&mut SymbolInstance> {
        match &mut self.kind {
            ElementKind::Instance(i) => Some(i),
            _ => None,
        }
    }
    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            ElementKind::Shape(_) => "Shape",
            ElementKind::Instance(_) => "Symbol instance",
            ElementKind::Text(_) => "Text",
            ElementKind::Bitmap(_) => "Bitmap",
            ElementKind::Group { .. } => "Group",
        }
    }
    /// Transformation point in parent coordinates.
    pub fn pivot_world(&self) -> Vec2 {
        self.transform.apply(self.pivot)
    }
}
