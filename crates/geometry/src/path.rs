use crate::{Affine, Rect, Vec2};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Write as _;

/// Fill rule for path interiors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

/// A single path command. Coordinates are absolute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCmd {
    MoveTo(Vec2),
    LineTo(Vec2),
    QuadTo(Vec2, Vec2),
    CubicTo(Vec2, Vec2, Vec2),
    Close,
}

impl PathCmd {
    /// End point of the command, if any.
    pub fn end(&self) -> Option<Vec2> {
        match *self {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) | PathCmd::QuadTo(_, p) | PathCmd::CubicTo(_, _, p) => Some(p),
            PathCmd::Close => None,
        }
    }
    fn map(&self, f: impl Fn(Vec2) -> Vec2) -> PathCmd {
        match *self {
            PathCmd::MoveTo(p) => PathCmd::MoveTo(f(p)),
            PathCmd::LineTo(p) => PathCmd::LineTo(f(p)),
            PathCmd::QuadTo(a, p) => PathCmd::QuadTo(f(a), f(p)),
            PathCmd::CubicTo(a, b, p) => PathCmd::CubicTo(f(a), f(b), f(p)),
            PathCmd::Close => PathCmd::Close,
        }
    }
}

/// A drawable segment with explicit start point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    Line(Vec2, Vec2),
    Quad(Vec2, Vec2, Vec2),
    Cubic(Vec2, Vec2, Vec2, Vec2),
}

impl Segment {
    pub fn start(&self) -> Vec2 {
        match *self {
            Segment::Line(a, _) | Segment::Quad(a, _, _) | Segment::Cubic(a, _, _, _) => a,
        }
    }
    pub fn end(&self) -> Vec2 {
        match *self {
            Segment::Line(_, b) | Segment::Quad(_, _, b) | Segment::Cubic(_, _, _, b) => b,
        }
    }
    pub fn eval(&self, t: f64) -> Vec2 {
        match *self {
            Segment::Line(a, b) => a.lerp(b, t),
            Segment::Quad(a, b, c) => {
                let mt = 1.0 - t;
                a * (mt * mt) + b * (2.0 * mt * t) + c * (t * t)
            }
            Segment::Cubic(a, b, c, d) => {
                let mt = 1.0 - t;
                a * (mt * mt * mt) + b * (3.0 * mt * mt * t) + c * (3.0 * mt * t * t) + d * (t * t * t)
            }
        }
    }
    pub fn derivative(&self, t: f64) -> Vec2 {
        match *self {
            Segment::Line(a, b) => b - a,
            Segment::Quad(a, b, c) => ((b - a) * (1.0 - t) + (c - b) * t) * 2.0,
            Segment::Cubic(a, b, c, d) => {
                let mt = 1.0 - t;
                (b - a) * (3.0 * mt * mt) + (c - b) * (6.0 * mt * t) + (d - c) * (3.0 * t * t)
            }
        }
    }
    /// Elevate to a cubic Bézier `[p0, c1, c2, p3]`.
    pub fn to_cubic(&self) -> [Vec2; 4] {
        match *self {
            Segment::Line(a, b) => [a, a.lerp(b, 1.0 / 3.0), a.lerp(b, 2.0 / 3.0), b],
            Segment::Quad(a, q, b) => [a, a + (q - a) * (2.0 / 3.0), b + (q - b) * (2.0 / 3.0), b],
            Segment::Cubic(a, b, c, d) => [a, b, c, d],
        }
    }
    /// Number of subdivisions needed so flattening error stays below `tol`.
    fn flatten_steps(&self, tol: f64) -> usize {
        match *self {
            Segment::Line(..) => 1,
            Segment::Quad(a, b, c) => {
                let dd = (a - b * 2.0 + c).length();
                ((dd / (8.0 * tol)).sqrt().ceil() as usize).clamp(1, 256)
            }
            Segment::Cubic(a, b, c, d) => {
                let d1 = (a - b * 2.0 + c).length();
                let d2 = (b - c * 2.0 + d).length();
                let dd = d1.max(d2);
                ((3.0 * dd / (4.0 * tol)).sqrt().ceil() as usize).clamp(1, 512)
            }
        }
    }
    pub fn flatten_into(&self, tol: f64, out: &mut Vec<Vec2>) {
        let n = self.flatten_steps(tol);
        for i in 1..=n {
            out.push(self.eval(i as f64 / n as f64));
        }
    }
    pub fn approx_length(&self) -> f64 {
        let mut pts = vec![self.start()];
        self.flatten_into(0.05, &mut pts);
        pts.windows(2).map(|w| w[0].distance(w[1])).sum()
    }
    pub fn split(&self, t: f64) -> (Segment, Segment) {
        let [a, b, c, d] = self.to_cubic();
        let ab = a.lerp(b, t);
        let bc = b.lerp(c, t);
        let cd = c.lerp(d, t);
        let abc = ab.lerp(bc, t);
        let bcd = bc.lerp(cd, t);
        let m = abc.lerp(bcd, t);
        match self {
            Segment::Line(a, b) => {
                let m = a.lerp(*b, t);
                (Segment::Line(*a, m), Segment::Line(m, *b))
            }
            _ => (Segment::Cubic(a, ab, abc, m), Segment::Cubic(m, bcd, cd, d)),
        }
    }
}

/// A vector path made of one or more subpaths.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Path {
    pub cmds: Vec<PathCmd>,
}

/// One subpath decomposed into segments.
#[derive(Debug, Clone, PartialEq)]
pub struct SubPath {
    pub segments: Vec<Segment>,
    pub closed: bool,
    pub start: Vec2,
}

impl Path {
    pub fn new() -> Path {
        Path { cmds: Vec::new() }
    }
    pub fn is_empty(&self) -> bool {
        !self.cmds.iter().any(|c| !matches!(c, PathCmd::MoveTo(_) | PathCmd::Close))
    }
    pub fn move_to(&mut self, p: Vec2) -> &mut Self {
        self.cmds.push(PathCmd::MoveTo(p));
        self
    }
    pub fn line_to(&mut self, p: Vec2) -> &mut Self {
        self.cmds.push(PathCmd::LineTo(p));
        self
    }
    pub fn quad_to(&mut self, c: Vec2, p: Vec2) -> &mut Self {
        self.cmds.push(PathCmd::QuadTo(c, p));
        self
    }
    pub fn cubic_to(&mut self, c1: Vec2, c2: Vec2, p: Vec2) -> &mut Self {
        self.cmds.push(PathCmd::CubicTo(c1, c2, p));
        self
    }
    pub fn close(&mut self) -> &mut Self {
        self.cmds.push(PathCmd::Close);
        self
    }
    pub fn extend(&mut self, other: &Path) {
        self.cmds.extend_from_slice(&other.cmds);
    }

    // ---------- builders ----------

    pub fn line(a: Vec2, b: Vec2) -> Path {
        let mut p = Path::new();
        p.move_to(a).line_to(b);
        p
    }
    pub fn polyline(points: &[Vec2], closed: bool) -> Path {
        let mut p = Path::new();
        if let Some((first, rest)) = points.split_first() {
            p.move_to(*first);
            for q in rest {
                p.line_to(*q);
            }
            if closed {
                p.close();
            }
        }
        p
    }
    pub fn rect(r: Rect) -> Path {
        Path::polyline(&r.corners(), true)
    }
    pub fn rounded_rect(r: Rect, radius: f64) -> Path {
        let rad = radius.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
        if rad <= 0.0 {
            return Path::rect(r);
        }
        let k = rad * 0.552_284_749_830_793_6;
        let (x0, y0, x1, y1) = (r.min.x, r.min.y, r.max.x, r.max.y);
        let mut p = Path::new();
        p.move_to(Vec2::new(x0 + rad, y0))
            .line_to(Vec2::new(x1 - rad, y0))
            .cubic_to(Vec2::new(x1 - rad + k, y0), Vec2::new(x1, y0 + rad - k), Vec2::new(x1, y0 + rad))
            .line_to(Vec2::new(x1, y1 - rad))
            .cubic_to(Vec2::new(x1, y1 - rad + k), Vec2::new(x1 - rad + k, y1), Vec2::new(x1 - rad, y1))
            .line_to(Vec2::new(x0 + rad, y1))
            .cubic_to(Vec2::new(x0 + rad - k, y1), Vec2::new(x0, y1 - rad + k), Vec2::new(x0, y1 - rad))
            .line_to(Vec2::new(x0, y0 + rad))
            .cubic_to(Vec2::new(x0, y0 + rad - k), Vec2::new(x0 + rad - k, y0), Vec2::new(x0 + rad, y0))
            .close();
        p
    }
    pub fn ellipse(r: Rect) -> Path {
        let c = r.center();
        let rx = r.width() / 2.0;
        let ry = r.height() / 2.0;
        let k = 0.552_284_749_830_793_6;
        let mut p = Path::new();
        p.move_to(Vec2::new(c.x + rx, c.y))
            .cubic_to(Vec2::new(c.x + rx, c.y + ry * k), Vec2::new(c.x + rx * k, c.y + ry), Vec2::new(c.x, c.y + ry))
            .cubic_to(Vec2::new(c.x - rx * k, c.y + ry), Vec2::new(c.x - rx, c.y + ry * k), Vec2::new(c.x - rx, c.y))
            .cubic_to(Vec2::new(c.x - rx, c.y - ry * k), Vec2::new(c.x - rx * k, c.y - ry), Vec2::new(c.x, c.y - ry))
            .cubic_to(Vec2::new(c.x + rx * k, c.y - ry), Vec2::new(c.x + rx, c.y - ry * k), Vec2::new(c.x + rx, c.y))
            .close();
        p
    }
    /// Regular polygon with `sides` vertices inscribed in a circle.
    pub fn polygon(center: Vec2, radius: f64, sides: u32, rotation: f64) -> Path {
        let n = sides.max(3);
        let pts: Vec<Vec2> = (0..n)
            .map(|i| center + Vec2::from_angle(rotation + i as f64 * std::f64::consts::TAU / n as f64) * radius)
            .collect();
        Path::polyline(&pts, true)
    }
    /// Star with `points` tips; `inner_ratio` is inner radius / outer radius.
    pub fn star(center: Vec2, radius: f64, points: u32, inner_ratio: f64, rotation: f64) -> Path {
        let n = points.max(3) * 2;
        let pts: Vec<Vec2> = (0..n)
            .map(|i| {
                let r = if i % 2 == 0 { radius } else { radius * inner_ratio };
                center + Vec2::from_angle(rotation + i as f64 * std::f64::consts::TAU / n as f64) * r
            })
            .collect();
        Path::polyline(&pts, true)
    }

    // ---------- structure ----------

    pub fn subpaths(&self) -> Vec<SubPath> {
        let mut out = Vec::new();
        let mut cur: Option<SubPath> = None;
        let mut pos = Vec2::ZERO;
        for cmd in &self.cmds {
            match *cmd {
                PathCmd::MoveTo(p) => {
                    if let Some(sp) = cur.take()
                        && !sp.segments.is_empty()
                    {
                        out.push(sp);
                    }
                    cur = Some(SubPath { segments: Vec::new(), closed: false, start: p });
                    pos = p;
                }
                PathCmd::Close => {
                    if let Some(mut sp) = cur.take() {
                        if pos.distance(sp.start) > 1e-9 {
                            sp.segments.push(Segment::Line(pos, sp.start));
                        }
                        sp.closed = true;
                        pos = sp.start;
                        if !sp.segments.is_empty() {
                            out.push(sp);
                        }
                    }
                }
                _ => {
                    let sp = cur.get_or_insert_with(|| SubPath { segments: Vec::new(), closed: false, start: pos });
                    let seg = match *cmd {
                        PathCmd::LineTo(p) => Segment::Line(pos, p),
                        PathCmd::QuadTo(c, p) => Segment::Quad(pos, c, p),
                        PathCmd::CubicTo(c1, c2, p) => Segment::Cubic(pos, c1, c2, p),
                        _ => unreachable!(),
                    };
                    pos = seg.end();
                    sp.segments.push(seg);
                }
            }
        }
        if let Some(sp) = cur
            && !sp.segments.is_empty()
        {
            out.push(sp);
        }
        out
    }

    pub fn from_subpaths(subs: &[SubPath]) -> Path {
        let mut p = Path::new();
        for sp in subs {
            let Some(first) = sp.segments.first() else { continue };
            p.move_to(first.start());
            for s in &sp.segments {
                match *s {
                    Segment::Line(_, b) => p.line_to(b),
                    Segment::Quad(_, c, b) => p.quad_to(c, b),
                    Segment::Cubic(_, c1, c2, b) => p.cubic_to(c1, c2, b),
                };
            }
            if sp.closed {
                p.close();
            }
        }
        p
    }

    pub fn segments(&self) -> Vec<Segment> {
        self.subpaths().into_iter().flat_map(|s| s.segments).collect()
    }

    pub fn transform(&self, m: &Affine) -> Path {
        Path { cmds: self.cmds.iter().map(|c| c.map(|p| m.apply(p))).collect() }
    }
    pub fn translate(&self, d: Vec2) -> Path {
        Path { cmds: self.cmds.iter().map(|c| c.map(|p| p + d)).collect() }
    }

    /// Polylines approximating every subpath, with closed flags.
    pub fn flatten(&self, tol: f64) -> Vec<(Vec<Vec2>, bool)> {
        self.subpaths()
            .into_iter()
            .map(|sp| {
                let mut pts = vec![sp.start];
                for s in &sp.segments {
                    s.flatten_into(tol, &mut pts);
                }
                (pts, sp.closed)
            })
            .collect()
    }

    pub fn bounds(&self) -> Rect {
        let mut r = Rect::EMPTY;
        for (pts, _) in self.flatten(0.1) {
            for p in pts {
                r.include(p);
            }
        }
        r
    }

    /// Winding-number containment test. Open subpaths are implicitly closed (like SVG fills).
    pub fn contains(&self, p: Vec2, rule: FillRule) -> bool {
        let mut winding = 0i32;
        for (pts, _) in self.flatten(0.25) {
            if pts.len() < 2 {
                continue;
            }
            let n = pts.len();
            for i in 0..n {
                let a = pts[i];
                let b = pts[(i + 1) % n];
                if a.y <= p.y {
                    if b.y > p.y && (b - a).cross(p - a) > 0.0 {
                        winding += 1;
                    }
                } else if b.y <= p.y && (b - a).cross(p - a) < 0.0 {
                    winding -= 1;
                }
            }
        }
        match rule {
            FillRule::NonZero => winding != 0,
            FillRule::EvenOdd => winding % 2 != 0,
        }
    }

    /// Minimum distance from `p` to the path outline.
    pub fn distance_to(&self, p: Vec2) -> f64 {
        let mut best = f64::INFINITY;
        for (pts, closed) in self.flatten(0.25) {
            let n = pts.len();
            if n == 1 {
                best = best.min(p.distance(pts[0]));
            }
            let count = if closed { n } else { n.saturating_sub(1) };
            for i in 0..count {
                best = best.min(point_segment_distance(p, pts[i], pts[(i + 1) % n]));
            }
        }
        best
    }

    pub fn length(&self) -> f64 {
        self.flatten(0.1).iter().map(|(pts, closed)| polyline_length(pts, *closed)).sum()
    }

    /// Sample point and tangent angle at normalized arc length `t` in [0, 1]
    /// along the first subpath (used for motion guides).
    pub fn sample(&self, t: f64) -> Option<(Vec2, f64)> {
        let (pts, closed) = self.flatten(0.1).into_iter().next()?;
        let mut pts = pts;
        if closed && pts.len() > 1 {
            pts.push(pts[0]);
        }
        if pts.len() < 2 {
            return pts.first().map(|p| (*p, 0.0));
        }
        let total = polyline_length(&pts, false);
        if total <= 1e-12 {
            return Some((pts[0], 0.0));
        }
        let target = t.clamp(0.0, 1.0) * total;
        let mut acc = 0.0;
        for w in pts.windows(2) {
            let l = w[0].distance(w[1]);
            if acc + l >= target && l > 0.0 {
                let u = (target - acc) / l;
                return Some((w[0].lerp(w[1], u), (w[1] - w[0]).angle()));
            }
            acc += l;
        }
        let n = pts.len();
        Some((pts[n - 1], (pts[n - 1] - pts[n - 2]).angle()))
    }

    pub fn reverse(&self) -> Path {
        let subs: Vec<SubPath> = self
            .subpaths()
            .into_iter()
            .map(|sp| {
                let segments: Vec<Segment> = sp
                    .segments
                    .iter()
                    .rev()
                    .map(|s| match *s {
                        Segment::Line(a, b) => Segment::Line(b, a),
                        Segment::Quad(a, c, b) => Segment::Quad(b, c, a),
                        Segment::Cubic(a, c1, c2, b) => Segment::Cubic(b, c2, c1, a),
                    })
                    .collect();
                let start = segments.first().map(|s| s.start()).unwrap_or(sp.start);
                SubPath { segments, closed: sp.closed, start }
            })
            .collect();
        Path::from_subpaths(&subs)
    }

    /// Indices into `cmds` of every anchor point (commands with end points).
    pub fn anchor_indices(&self) -> Vec<usize> {
        self.cmds.iter().enumerate().filter(|(_, c)| c.end().is_some()).map(|(i, _)| i).collect()
    }

    /// Move the anchor at command `idx` by `delta`, dragging its adjacent control handles.
    pub fn move_anchor(&mut self, idx: usize, delta: Vec2) {
        if idx >= self.cmds.len() {
            return;
        }
        self.cmds[idx] = match self.cmds[idx] {
            PathCmd::MoveTo(p) => PathCmd::MoveTo(p + delta),
            PathCmd::LineTo(p) => PathCmd::LineTo(p + delta),
            PathCmd::QuadTo(c, p) => PathCmd::QuadTo(c, p + delta),
            PathCmd::CubicTo(c1, c2, p) => PathCmd::CubicTo(c1, c2 + delta, p + delta),
            PathCmd::Close => PathCmd::Close,
        };
        if let Some(next) = self.cmds.get(idx + 1).copied()
            && let PathCmd::CubicTo(c1, c2, p) = next
        {
            self.cmds[idx + 1] = PathCmd::CubicTo(c1 + delta, c2, p);
        }
        // A MoveTo of a closed subpath coincides with its last anchor: move that too.
        if let PathCmd::MoveTo(_) = self.cmds[idx]
            && let Some(close_idx) = self.cmds[idx + 1..].iter().position(|c| matches!(c, PathCmd::Close | PathCmd::MoveTo(_)))
        {
            let ci = idx + 1 + close_idx;
            if matches!(self.cmds[ci], PathCmd::Close) && ci >= 1 {
                let last = ci - 1;
                let start = self.cmds[idx].end().unwrap() - delta;
                if last != idx && self.cmds[last].end().is_some_and(|e| e.distance(start) < 1e-6) {
                    self.cmds[last] = match self.cmds[last] {
                        PathCmd::LineTo(p) => PathCmd::LineTo(p + delta),
                        PathCmd::QuadTo(c, p) => PathCmd::QuadTo(c, p + delta),
                        PathCmd::CubicTo(c1, c2, p) => PathCmd::CubicTo(c1, c2 + delta, p + delta),
                        o => o,
                    };
                }
            }
        }
    }

    /// Set a control point of command `idx` (`which` = 0 for first, 1 for second).
    pub fn set_control(&mut self, idx: usize, which: usize, pos: Vec2) {
        if let Some(cmd) = self.cmds.get_mut(idx) {
            *cmd = match (*cmd, which) {
                (PathCmd::QuadTo(_, p), _) => PathCmd::QuadTo(pos, p),
                (PathCmd::CubicTo(_, c2, p), 0) => PathCmd::CubicTo(pos, c2, p),
                (PathCmd::CubicTo(c1, _, p), _) => PathCmd::CubicTo(c1, pos, p),
                (o, _) => o,
            };
        }
    }

    /// Convert a straight segment ending at anchor `idx` into a curve (Convert Anchor Point).
    pub fn convert_to_curve(&mut self, idx: usize) {
        if idx == 0 || idx >= self.cmds.len() {
            return;
        }
        let start = self.cmds[..idx].iter().rev().find_map(|c| c.end()).unwrap_or(Vec2::ZERO);
        if let PathCmd::LineTo(p) = self.cmds[idx] {
            self.cmds[idx] = PathCmd::CubicTo(start.lerp(p, 1.0 / 3.0), start.lerp(p, 2.0 / 3.0), p);
        }
    }

    /// Remove the anchor at command `idx` (Delete Anchor Point tool).
    pub fn delete_anchor(&mut self, idx: usize) {
        if idx < self.cmds.len() && self.cmds[idx].end().is_some() && !matches!(self.cmds[idx], PathCmd::MoveTo(_)) {
            self.cmds.remove(idx);
        }
    }

    /// Insert an anchor by splitting the segment ending at `idx` at parameter `t`.
    pub fn insert_anchor(&mut self, idx: usize, t: f64) {
        if idx == 0 || idx >= self.cmds.len() {
            return;
        }
        let start = self.cmds[..idx].iter().rev().find_map(|c| c.end()).unwrap_or(Vec2::ZERO);
        let seg = match self.cmds[idx] {
            PathCmd::LineTo(p) => Segment::Line(start, p),
            PathCmd::QuadTo(c, p) => Segment::Quad(start, c, p),
            PathCmd::CubicTo(c1, c2, p) => Segment::Cubic(start, c1, c2, p),
            _ => return,
        };
        let (a, b) = seg.split(t);
        let to_cmd = |s: Segment| match s {
            Segment::Line(_, p) => PathCmd::LineTo(p),
            Segment::Quad(_, c, p) => PathCmd::QuadTo(c, p),
            Segment::Cubic(_, c1, c2, p) => PathCmd::CubicTo(c1, c2, p),
        };
        self.cmds[idx] = to_cmd(a);
        self.cmds.insert(idx + 1, to_cmd(b));
    }

    /// Find the command index and parameter of the closest point on the outline.
    pub fn nearest_segment(&self, p: Vec2) -> Option<(usize, f64, f64)> {
        let mut pos = Vec2::ZERO;
        let mut start = Vec2::ZERO;
        let mut best: Option<(usize, f64, f64)> = None;
        for (i, cmd) in self.cmds.iter().enumerate() {
            let seg = match *cmd {
                PathCmd::MoveTo(q) => {
                    pos = q;
                    start = q;
                    continue;
                }
                PathCmd::LineTo(q) => Segment::Line(pos, q),
                PathCmd::QuadTo(c, q) => Segment::Quad(pos, c, q),
                PathCmd::CubicTo(c1, c2, q) => Segment::Cubic(pos, c1, c2, q),
                PathCmd::Close => {
                    pos = start;
                    continue;
                }
            };
            for k in 0..=32 {
                let t = k as f64 / 32.0;
                let d = seg.eval(t).distance(p);
                if best.is_none_or(|b| d < b.2) {
                    best = Some((i, t, d));
                }
            }
            pos = seg.end();
        }
        best
    }

    // ---------- SVG path data ----------

    pub fn to_svg(&self) -> String {
        let mut s = String::new();
        for cmd in &self.cmds {
            if !s.is_empty() {
                s.push(' ');
            }
            let _ = match *cmd {
                PathCmd::MoveTo(p) => write!(s, "M{} {}", p.x, p.y),
                PathCmd::LineTo(p) => write!(s, "L{} {}", p.x, p.y),
                PathCmd::QuadTo(c, p) => write!(s, "Q{} {} {} {}", c.x, c.y, p.x, p.y),
                PathCmd::CubicTo(a, b, p) => write!(s, "C{} {} {} {} {} {}", a.x, a.y, b.x, b.y, p.x, p.y),
                PathCmd::Close => write!(s, "Z"),
            };
        }
        s
    }

    /// Parse SVG path data (supports M, L, H, V, C, S, Q, T, A, Z in absolute and relative forms).
    pub fn parse_svg(data: &str) -> Result<Path, String> {
        let mut tokens = SvgTokens { s: data.as_bytes(), i: 0 };
        let mut path = Path::new();
        let mut cur = Vec2::ZERO;
        let mut start = Vec2::ZERO;
        let mut last_ctrl: Option<Vec2> = None;
        let mut last_quad: Option<Vec2> = None;
        let mut cmd = b'M';
        loop {
            tokens.skip_ws();
            if tokens.done() {
                break;
            }
            if let Some(c) = tokens.peek_cmd() {
                cmd = c;
                tokens.i += 1;
                if c == b'Z' || c == b'z' {
                    path.close();
                    cur = start;
                    last_ctrl = None;
                    last_quad = None;
                    continue;
                }
            } else if cmd == b'Z' || cmd == b'z' {
                return Err(format!("unexpected number at {}", tokens.i));
            }
            let rel = cmd.is_ascii_lowercase();
            let base = if rel { cur } else { Vec2::ZERO };
            let pt = |t: &mut SvgTokens| -> Result<Vec2, String> { Ok(Vec2::new(t.num()?, t.num()?) + base) };
            match cmd.to_ascii_uppercase() {
                b'M' => {
                    cur = pt(&mut tokens)?;
                    start = cur;
                    path.move_to(cur);
                    cmd = if rel { b'l' } else { b'L' };
                    last_ctrl = None;
                    last_quad = None;
                }
                b'L' => {
                    cur = pt(&mut tokens)?;
                    path.line_to(cur);
                    last_ctrl = None;
                    last_quad = None;
                }
                b'H' => {
                    let x = tokens.num()? + if rel { cur.x } else { 0.0 };
                    cur = Vec2::new(x, cur.y);
                    path.line_to(cur);
                    last_ctrl = None;
                    last_quad = None;
                }
                b'V' => {
                    let y = tokens.num()? + if rel { cur.y } else { 0.0 };
                    cur = Vec2::new(cur.x, y);
                    path.line_to(cur);
                    last_ctrl = None;
                    last_quad = None;
                }
                b'C' => {
                    let c1 = pt(&mut tokens)?;
                    let c2 = pt(&mut tokens)?;
                    cur = pt(&mut tokens)?;
                    path.cubic_to(c1, c2, cur);
                    last_ctrl = Some(c2);
                    last_quad = None;
                }
                b'S' => {
                    let c1 = last_ctrl.map(|c| cur * 2.0 - c).unwrap_or(cur);
                    let c2 = pt(&mut tokens)?;
                    cur = pt(&mut tokens)?;
                    path.cubic_to(c1, c2, cur);
                    last_ctrl = Some(c2);
                    last_quad = None;
                }
                b'Q' => {
                    let c = pt(&mut tokens)?;
                    cur = pt(&mut tokens)?;
                    path.quad_to(c, cur);
                    last_quad = Some(c);
                    last_ctrl = None;
                }
                b'T' => {
                    let c = last_quad.map(|c| cur * 2.0 - c).unwrap_or(cur);
                    cur = pt(&mut tokens)?;
                    path.quad_to(c, cur);
                    last_quad = Some(c);
                    last_ctrl = None;
                }
                b'A' => {
                    let rx = tokens.num()?;
                    let ry = tokens.num()?;
                    let rot = tokens.num()?;
                    let large = tokens.flag()?;
                    let sweep = tokens.flag()?;
                    let end = pt(&mut tokens)?;
                    arc_to_cubics(&mut path, cur, end, rx, ry, rot.to_radians(), large, sweep);
                    cur = end;
                    last_ctrl = None;
                    last_quad = None;
                }
                other => return Err(format!("unsupported path command '{}'", other as char)),
            }
        }
        Ok(path)
    }
}

struct SvgTokens<'a> {
    s: &'a [u8],
    i: usize,
}

impl SvgTokens<'_> {
    fn done(&self) -> bool {
        self.i >= self.s.len()
    }
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == b',') {
            self.i += 1;
        }
    }
    fn peek_cmd(&self) -> Option<u8> {
        let c = *self.s.get(self.i)?;
        (c.is_ascii_alphabetic() && c != b'e' && c != b'E').then_some(c)
    }
    fn flag(&mut self) -> Result<bool, String> {
        self.skip_ws();
        match self.s.get(self.i) {
            Some(b'0') => {
                self.i += 1;
                Ok(false)
            }
            Some(b'1') => {
                self.i += 1;
                Ok(true)
            }
            _ => Err(format!("expected arc flag at {}", self.i)),
        }
    }
    fn num(&mut self) -> Result<f64, String> {
        self.skip_ws();
        let start = self.i;
        let s = self.s;
        if self.i < s.len() && (s[self.i] == b'-' || s[self.i] == b'+') {
            self.i += 1;
        }
        let mut seen_dot = false;
        while self.i < s.len() {
            let c = s[self.i];
            if c.is_ascii_digit() {
                self.i += 1;
            } else if c == b'.' && !seen_dot {
                seen_dot = true;
                self.i += 1;
            } else if (c == b'e' || c == b'E') && self.i > start {
                self.i += 1;
                if self.i < s.len() && (s[self.i] == b'-' || s[self.i] == b'+') {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
        std::str::from_utf8(&s[start..self.i])
            .ok()
            .and_then(|t| t.parse::<f64>().ok())
            .ok_or_else(|| format!("expected number at {start}"))
    }
}

#[allow(clippy::too_many_arguments)]
fn arc_to_cubics(path: &mut Path, p0: Vec2, p1: Vec2, rx: f64, ry: f64, phi: f64, large: bool, sweep: bool) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-9 || ry < 1e-9 || p0.distance(p1) < 1e-12 {
        path.line_to(p1);
        return;
    }
    let (sp, cp) = phi.sin_cos();
    let d = (p0 - p1) * 0.5;
    let x1p = cp * d.x + sp * d.y;
    let y1p = -sp * d.x + cp * d.y;
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut coef = (num / den).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = -coef * ry * x1p / rx;
    let mid = (p0 + p1) * 0.5;
    let c = Vec2::new(cp * cxp - sp * cyp + mid.x, sp * cxp + cp * cyp + mid.y);
    let ang = |u: Vec2, v: Vec2| {
        let a = (u.dot(v) / (u.length() * v.length())).clamp(-1.0, 1.0).acos();
        if u.cross(v) < 0.0 { -a } else { a }
    };
    let u = Vec2::new((x1p - cxp) / rx, (y1p - cyp) / ry);
    let v = Vec2::new((-x1p - cxp) / rx, (-y1p - cyp) / ry);
    let theta1 = ang(Vec2::new(1.0, 0.0), u);
    let mut dtheta = ang(u, v);
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f64::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f64::consts::TAU;
    }
    let n = (dtheta.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let step = dtheta / n as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let point = |t: f64| {
        let (st, ct) = t.sin_cos();
        Vec2::new(c.x + rx * ct * cp - ry * st * sp, c.y + rx * ct * sp + ry * st * cp)
    };
    let deriv = |t: f64| {
        let (st, ct) = t.sin_cos();
        Vec2::new(-rx * st * cp - ry * ct * sp, -rx * st * sp + ry * ct * cp)
    };
    for i in 0..n {
        let t0 = theta1 + step * i as f64;
        let t1 = t0 + step;
        let a = point(t0);
        let b = point(t1);
        let end = if i + 1 == n { p1 } else { b };
        path.cubic_to(a + deriv(t0) * k, b - deriv(t1) * k, end);
    }
}

pub(crate) fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let l2 = ab.length_sq();
    if l2 < 1e-18 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn polyline_length(pts: &[Vec2], closed: bool) -> f64 {
    let mut l: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
    if closed && pts.len() > 2 {
        l += pts[pts.len() - 1].distance(pts[0]);
    }
    l
}

// ---------- shape morphing (shape tweens) ----------

/// Cubic contour representation used for shape tweening.
#[derive(Debug, Clone)]
pub(crate) struct CubicContour {
    pub segs: Vec<[Vec2; 4]>,
    pub closed: bool,
}

fn contours(path: &Path) -> Vec<CubicContour> {
    path.subpaths()
        .into_iter()
        .map(|sp| CubicContour { segs: sp.segments.iter().map(|s| s.to_cubic()).collect(), closed: sp.closed })
        .collect()
}

fn split_cubic(c: [Vec2; 4]) -> ([Vec2; 4], [Vec2; 4]) {
    let (a, b) = Segment::Cubic(c[0], c[1], c[2], c[3]).split(0.5);
    (a.to_cubic(), b.to_cubic())
}

fn cubic_len(c: &[Vec2; 4]) -> f64 {
    c[0].distance(c[1]) + c[1].distance(c[2]) + c[2].distance(c[3])
}

fn subdivide_to(c: &mut CubicContour, n: usize) {
    while c.segs.len() < n {
        let (idx, _) = c
            .segs
            .iter()
            .enumerate()
            .max_by(|a, b| cubic_len(a.1).total_cmp(&cubic_len(b.1)))
            .expect("non-empty contour");
        let (a, b) = split_cubic(c.segs[idx]);
        c.segs[idx] = a;
        c.segs.insert(idx + 1, b);
    }
}

fn signed_area(c: &CubicContour) -> f64 {
    let pts: Vec<Vec2> = c.segs.iter().map(|s| s[0]).collect();
    let n = pts.len();
    (0..n).map(|i| pts[i].cross(pts[(i + 1) % n])).sum::<f64>() * 0.5
}

fn centroid(c: &CubicContour) -> Vec2 {
    let n = c.segs.len().max(1) as f64;
    c.segs.iter().fold(Vec2::ZERO, |acc, s| acc + s[0]) / n
}

fn reverse_contour(c: &mut CubicContour) {
    c.segs.reverse();
    for s in &mut c.segs {
        s.reverse();
    }
}

/// Precomputed correspondence between two paths for shape tweening.
#[derive(Debug, Clone)]
pub struct Morph {
    from: Vec<CubicContour>,
    to: Vec<CubicContour>,
}

impl Morph {
    pub fn new(a: &Path, b: &Path) -> Morph {
        let mut from = contours(a);
        let mut to = contours(b);
        // Equalize contour counts with degenerate contours collapsing to a centroid.
        let pad = |list: &mut Vec<CubicContour>, other: &[CubicContour]| {
            while list.len() < other.len() {
                let src = &other[list.len()];
                let c = list.last().map(centroid).unwrap_or_else(|| centroid(src));
                list.push(CubicContour { segs: vec![[c; 4]; src.segs.len().max(1)], closed: src.closed });
            }
        };
        pad(&mut from, &to.clone());
        pad(&mut to, &from.clone());
        for (fa, tb) in from.iter_mut().zip(to.iter_mut()) {
            if fa.segs.is_empty() || tb.segs.is_empty() {
                continue;
            }
            let n = fa.segs.len().max(tb.segs.len());
            subdivide_to(fa, n);
            subdivide_to(tb, n);
            if fa.closed && tb.closed {
                if signed_area(fa).signum() != signed_area(tb).signum() && signed_area(tb).abs() > 1e-9 {
                    reverse_contour(tb);
                }
                // Rotate `tb` so its start point best matches `fa`.
                let best = (0..n)
                    .min_by(|&i, &j| {
                        let cost = |k: usize| -> f64 { (0..n).map(|s| fa.segs[s][0].distance(tb.segs[(s + k) % n][0])).sum() };
                        cost(i).total_cmp(&cost(j))
                    })
                    .unwrap_or(0);
                tb.segs.rotate_left(best);
            }
        }
        Morph { from, to }
    }

    pub fn at(&self, t: f64) -> Path {
        let mut p = Path::new();
        for (a, b) in self.from.iter().zip(&self.to) {
            if a.segs.is_empty() || a.segs.len() != b.segs.len() {
                continue;
            }
            p.move_to(a.segs[0][0].lerp(b.segs[0][0], t));
            for (sa, sb) in a.segs.iter().zip(&b.segs) {
                p.cubic_to(sa[1].lerp(sb[1], t), sa[2].lerp(sb[2], t), sa[3].lerp(sb[3], t));
            }
            if a.closed || b.closed {
                p.close();
            }
        }
        p
    }
}

impl Serialize for Path {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_svg())
    }
}

impl<'de> Deserialize<'de> for Path {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Path, D::Error> {
        let s = String::deserialize(d)?;
        Path::parse_svg(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_roundtrip_is_lossless() {
        let mut p = Path::ellipse(Rect::new(0.1, 0.2, 100.3, 50.7));
        p.extend(&Path::star(Vec2::new(10.0, 10.0), 5.0, 5, 0.4, 0.1));
        let s = p.to_svg();
        let q = Path::parse_svg(&s).unwrap();
        assert_eq!(p, q);
    }

    #[test]
    fn parse_relative_and_arcs() {
        let p = Path::parse_svg("m10 10 h20 v20 h-20 z M50 50 a10 10 0 1 0 20 0").unwrap();
        let b = p.bounds();
        assert!((b.min.x - 10.0).abs() < 1e-6);
        assert!((b.max.x - 70.0).abs() < 1e-6);
        assert!(b.max.y > 59.0);
    }

    #[test]
    fn containment_and_distance() {
        let p = Path::rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert!(p.contains(Vec2::new(5.0, 5.0), FillRule::NonZero));
        assert!(!p.contains(Vec2::new(15.0, 5.0), FillRule::NonZero));
        assert!((p.distance_to(Vec2::new(5.0, -3.0)) - 3.0).abs() < 1e-9);
        let e = Path::ellipse(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert!(e.contains(Vec2::new(5.0, 5.0), FillRule::EvenOdd));
        assert!(!e.contains(Vec2::new(0.5, 0.5), FillRule::EvenOdd));
    }

    #[test]
    fn sampling_along_line() {
        let p = Path::line(Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0));
        let (pt, ang) = p.sample(0.25).unwrap();
        assert!((pt.x - 25.0).abs() < 1e-9 && ang.abs() < 1e-9);
    }

    #[test]
    fn morph_endpoints_match_inputs() {
        let a = Path::rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        let b = Path::ellipse(Rect::new(20.0, 20.0, 60.0, 40.0));
        let m = Morph::new(&a, &b);
        let start = m.at(0.0).bounds();
        let end = m.at(1.0).bounds();
        assert!((start.min.x - 0.0).abs() < 1e-6 && (start.max.x - 10.0).abs() < 1e-6);
        assert!((end.min.x - 20.0).abs() < 0.2 && (end.max.x - 60.0).abs() < 0.2);
        let mid = m.at(0.5).bounds();
        assert!(mid.min.x > 0.0 && mid.max.x < 60.0);
    }

    #[test]
    fn anchor_editing() {
        let mut p = Path::rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        p.move_anchor(2, Vec2::new(5.0, 5.0));
        assert!((p.bounds().max.x - 15.0).abs() < 1e-9);
        p.insert_anchor(1, 0.5);
        assert_eq!(p.anchor_indices().len(), 5);
        p.convert_to_curve(1);
        assert!(matches!(p.cmds[1], PathCmd::CubicTo(..)));
    }
}
