use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Named easing presets (mirrors the preset list of the classic-tween ease menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EasePreset {
    QuadIn,
    QuadOut,
    QuadInOut,
    CubicIn,
    CubicOut,
    CubicInOut,
    QuartIn,
    QuartOut,
    QuartInOut,
    QuintIn,
    QuintOut,
    QuintInOut,
    SineIn,
    SineOut,
    SineInOut,
    CircIn,
    CircOut,
    CircInOut,
    BackIn,
    BackOut,
    BackInOut,
    BounceIn,
    BounceOut,
    BounceInOut,
    ElasticIn,
    ElasticOut,
    ElasticInOut,
}

impl EasePreset {
    pub const ALL: [EasePreset; 27] = [
        EasePreset::QuadIn,
        EasePreset::QuadOut,
        EasePreset::QuadInOut,
        EasePreset::CubicIn,
        EasePreset::CubicOut,
        EasePreset::CubicInOut,
        EasePreset::QuartIn,
        EasePreset::QuartOut,
        EasePreset::QuartInOut,
        EasePreset::QuintIn,
        EasePreset::QuintOut,
        EasePreset::QuintInOut,
        EasePreset::SineIn,
        EasePreset::SineOut,
        EasePreset::SineInOut,
        EasePreset::CircIn,
        EasePreset::CircOut,
        EasePreset::CircInOut,
        EasePreset::BackIn,
        EasePreset::BackOut,
        EasePreset::BackInOut,
        EasePreset::BounceIn,
        EasePreset::BounceOut,
        EasePreset::BounceInOut,
        EasePreset::ElasticIn,
        EasePreset::ElasticOut,
        EasePreset::ElasticInOut,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EasePreset::QuadIn => "Quad In",
            EasePreset::QuadOut => "Quad Out",
            EasePreset::QuadInOut => "Quad In Out",
            EasePreset::CubicIn => "Cubic In",
            EasePreset::CubicOut => "Cubic Out",
            EasePreset::CubicInOut => "Cubic In Out",
            EasePreset::QuartIn => "Quart In",
            EasePreset::QuartOut => "Quart Out",
            EasePreset::QuartInOut => "Quart In Out",
            EasePreset::QuintIn => "Quint In",
            EasePreset::QuintOut => "Quint Out",
            EasePreset::QuintInOut => "Quint In Out",
            EasePreset::SineIn => "Sine In",
            EasePreset::SineOut => "Sine Out",
            EasePreset::SineInOut => "Sine In Out",
            EasePreset::CircIn => "Circ In",
            EasePreset::CircOut => "Circ Out",
            EasePreset::CircInOut => "Circ In Out",
            EasePreset::BackIn => "Back In",
            EasePreset::BackOut => "Back Out",
            EasePreset::BackInOut => "Back In Out",
            EasePreset::BounceIn => "Bounce In",
            EasePreset::BounceOut => "Bounce Out",
            EasePreset::BounceInOut => "Bounce In Out",
            EasePreset::ElasticIn => "Elastic In",
            EasePreset::ElasticOut => "Elastic Out",
            EasePreset::ElasticInOut => "Elastic In Out",
        }
    }

    pub fn eval(self, t: f64) -> f64 {
        fn out_from_in(f: fn(f64) -> f64, t: f64) -> f64 {
            1.0 - f(1.0 - t)
        }
        fn in_out(f: fn(f64) -> f64, t: f64) -> f64 {
            if t < 0.5 { f(t * 2.0) / 2.0 } else { 1.0 - f((1.0 - t) * 2.0) / 2.0 }
        }
        fn quad(t: f64) -> f64 {
            t * t
        }
        fn cubic(t: f64) -> f64 {
            t * t * t
        }
        fn quart(t: f64) -> f64 {
            t * t * t * t
        }
        fn quint(t: f64) -> f64 {
            t * t * t * t * t
        }
        fn sine(t: f64) -> f64 {
            1.0 - (t * PI / 2.0).cos()
        }
        fn circ(t: f64) -> f64 {
            1.0 - (1.0 - t * t).max(0.0).sqrt()
        }
        fn back(t: f64) -> f64 {
            let s = 1.70158;
            t * t * ((s + 1.0) * t - s)
        }
        fn bounce_out(t: f64) -> f64 {
            let (n1, d1) = (7.5625, 2.75);
            if t < 1.0 / d1 {
                n1 * t * t
            } else if t < 2.0 / d1 {
                let t = t - 1.5 / d1;
                n1 * t * t + 0.75
            } else if t < 2.5 / d1 {
                let t = t - 2.25 / d1;
                n1 * t * t + 0.9375
            } else {
                let t = t - 2.625 / d1;
                n1 * t * t + 0.984375
            }
        }
        fn bounce_in(t: f64) -> f64 {
            1.0 - bounce_out(1.0 - t)
        }
        fn elastic(t: f64) -> f64 {
            if t <= 0.0 || t >= 1.0 {
                return t;
            }
            -(2f64.powf(10.0 * t - 10.0)) * ((t * 10.0 - 10.75) * (2.0 * PI / 3.0)).sin()
        }
        match self {
            EasePreset::QuadIn => quad(t),
            EasePreset::QuadOut => out_from_in(quad, t),
            EasePreset::QuadInOut => in_out(quad, t),
            EasePreset::CubicIn => cubic(t),
            EasePreset::CubicOut => out_from_in(cubic, t),
            EasePreset::CubicInOut => in_out(cubic, t),
            EasePreset::QuartIn => quart(t),
            EasePreset::QuartOut => out_from_in(quart, t),
            EasePreset::QuartInOut => in_out(quart, t),
            EasePreset::QuintIn => quint(t),
            EasePreset::QuintOut => out_from_in(quint, t),
            EasePreset::QuintInOut => in_out(quint, t),
            EasePreset::SineIn => sine(t),
            EasePreset::SineOut => out_from_in(sine, t),
            EasePreset::SineInOut => in_out(sine, t),
            EasePreset::CircIn => circ(t),
            EasePreset::CircOut => out_from_in(circ, t),
            EasePreset::CircInOut => in_out(circ, t),
            EasePreset::BackIn => back(t),
            EasePreset::BackOut => out_from_in(back, t),
            EasePreset::BackInOut => in_out(back, t),
            EasePreset::BounceIn => bounce_in(t),
            EasePreset::BounceOut => bounce_out(t),
            EasePreset::BounceInOut => in_out(bounce_in, t),
            EasePreset::ElasticIn => elastic(t),
            EasePreset::ElasticOut => out_from_in(elastic, t),
            EasePreset::ElasticInOut => in_out(elastic, t),
        }
    }
}

/// Point on a custom ease curve with Bézier handles (relative to the point).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub x: f64,
    pub y: f64,
    pub in_handle: (f64, f64),
    pub out_handle: (f64, f64),
}

/// Easing function applied to tween progress.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ease {
    #[default]
    Linear,
    /// Classic-tween ease strength: -100 (ease in) .. 100 (ease out).
    Classic {
        strength: i32,
    },
    Preset {
        preset: EasePreset,
    },
    /// Custom curve from (0,0) to (1,1) through the given points (sorted by x).
    Custom {
        points: Vec<CurvePoint>,
    },
}

impl Ease {
    /// Map linear progress `t` in [0, 1] to eased progress.
    pub fn eval(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::Classic { strength } => {
                // Quadratic blend: positive = ease out (decelerate), negative = ease in.
                let s = (*strength).clamp(-100, 100) as f64 / 100.0;
                t + s * (t - t * t)
            }
            Ease::Preset { preset } => preset.eval(t),
            Ease::Custom { points } => eval_custom(points, t),
        }
    }
}

fn eval_custom(points: &[CurvePoint], t: f64) -> f64 {
    let mut pts = vec![CurvePoint { x: 0.0, y: 0.0, in_handle: (0.0, 0.0), out_handle: (1.0 / 3.0, 1.0 / 3.0) }];
    pts.extend(points.iter().copied().filter(|p| p.x > 0.0 && p.x < 1.0));
    pts.push(CurvePoint { x: 1.0, y: 1.0, in_handle: (-1.0 / 3.0, -1.0 / 3.0), out_handle: (0.0, 0.0) });
    pts.sort_by(|a, b| a.x.total_cmp(&b.x));
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t >= a.x && t <= b.x {
            let p0 = (a.x, a.y);
            let p1 = (a.x + a.out_handle.0, a.y + a.out_handle.1);
            let p2 = (b.x + b.in_handle.0, b.y + b.in_handle.1);
            let p3 = (b.x, b.y);
            let bez = |u: f64, i: usize| {
                let c = |p: (f64, f64)| if i == 0 { p.0 } else { p.1 };
                let mu = 1.0 - u;
                c(p0) * mu * mu * mu + 3.0 * c(p1) * mu * mu * u + 3.0 * c(p2) * mu * u * u + c(p3) * u * u * u
            };
            // Solve x(u) = t by bisection (x is monotonic for sane handles).
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..50 {
                let mid = (lo + hi) / 2.0;
                if bez(mid, 0) < t {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return bez((lo + hi) / 2.0, 1);
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_hit_endpoints() {
        for p in EasePreset::ALL {
            assert!((p.eval(0.0)).abs() < 1e-9, "{p:?}");
            assert!((p.eval(1.0) - 1.0).abs() < 1e-9, "{p:?}");
        }
    }

    #[test]
    fn classic_ease_direction() {
        let out = Ease::Classic { strength: 100 };
        let inn = Ease::Classic { strength: -100 };
        assert!(out.eval(0.5) > 0.5);
        assert!(inn.eval(0.5) < 0.5);
    }

    #[test]
    fn custom_linear_curve_is_identity() {
        let e = Ease::Custom { points: vec![] };
        for i in 0..=10 {
            let t = i as f64 / 10.0;
            assert!((e.eval(t) - t).abs() < 1e-6);
        }
    }
}
