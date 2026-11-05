//! Raster filters (blur, drop shadow, glow, adjust color) and custom composites.
//! All operations work on premultiplied RGBA8 pixmaps.

use animara_document::{Color, Filter};
use tiny_skia::Pixmap;

/// Separable box blur of all four channels, repeated `passes` times (≈ Gaussian for 3).
pub fn box_blur(data: &mut [u8], w: usize, h: usize, rx: usize, ry: usize, passes: u32) {
    if w == 0 || h == 0 {
        return;
    }
    let mut tmp = vec![0u8; data.len()];
    for _ in 0..passes.max(1) {
        if rx > 0 {
            blur_lines(data, &mut tmp, w, h, rx, true);
            data.copy_from_slice(&tmp);
        }
        if ry > 0 {
            blur_lines(data, &mut tmp, w, h, ry, false);
            data.copy_from_slice(&tmp);
        }
    }
}

fn blur_lines(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize, horizontal: bool) {
    let (lines, len) = if horizontal { (h, w) } else { (w, h) };
    let idx = |line: usize, i: usize| if horizontal { (line * w + i) * 4 } else { (i * w + line) * 4 };
    let win = (2 * r + 1) as u32;
    for line in 0..lines {
        let mut sum = [0u32; 4];
        // Prime the window with clamped (transparent outside) samples.
        for i in 0..=r.min(len - 1) {
            let p = idx(line, i);
            for c in 0..4 {
                sum[c] += src[p + c] as u32;
            }
        }
        for i in 0..len {
            let p = idx(line, i);
            for c in 0..4 {
                dst[p + c] = (sum[c] / win) as u8;
            }
            let add = i + r + 1;
            if add < len {
                let q = idx(line, add);
                for c in 0..4 {
                    sum[c] += src[q + c] as u32;
                }
            }
            if i >= r {
                let q = idx(line, i - r);
                for c in 0..4 {
                    sum[c] -= src[q + c] as u32;
                }
            }
        }
    }
}

fn alpha_tinted(src: &Pixmap, color: Color, strength: f32, invert: bool) -> Vec<u8> {
    let [r, g, b, a] = color.to_rgba8();
    let mut out = vec![0u8; src.data().len()];
    for (i, px) in src.data().as_chunks::<4>().0.iter().enumerate() {
        let mut al = px[3] as f32 / 255.0;
        if invert {
            al = 1.0 - al;
        }
        let k = (al * strength * a as f32 / 255.0).clamp(0.0, 1.0);
        out[i * 4] = (r as f32 * k) as u8;
        out[i * 4 + 1] = (g as f32 * k) as u8;
        out[i * 4 + 2] = (b as f32 * k) as u8;
        out[i * 4 + 3] = (255.0 * k) as u8;
    }
    out
}

fn shift(data: &[u8], w: usize, h: usize, dx: i32, dy: i32) -> Vec<u8> {
    let mut out = vec![0u8; data.len()];
    for y in 0..h as i32 {
        let sy = y - dy;
        if sy < 0 || sy >= h as i32 {
            continue;
        }
        for x in 0..w as i32 {
            let sx = x - dx;
            if sx < 0 || sx >= w as i32 {
                continue;
            }
            let d = (y as usize * w + x as usize) * 4;
            let s = (sy as usize * w + sx as usize) * 4;
            out[d..d + 4].copy_from_slice(&data[s..s + 4]);
        }
    }
    out
}

/// Porter-Duff "src over dst" for premultiplied bytes.
fn over(src: &[u8], dst: &[u8]) -> [u8; 4] {
    let sa = src[3] as u32;
    let mut o = [0u8; 4];
    for c in 0..4 {
        o[c] = (src[c] as u32 + dst[c] as u32 * (255 - sa) / 255).min(255) as u8;
    }
    o
}

#[allow(clippy::too_many_arguments)]
fn shadow_like(
    pm: &mut Pixmap,
    color: Color,
    strength: f32,
    blur: (f64, f64),
    passes: u32,
    offset: (i32, i32),
    inner: bool,
    knockout: bool,
    hide: bool,
) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let mut sh = alpha_tinted(pm, color, strength, inner);
    sh = shift(&sh, w, h, offset.0, offset.1);
    box_blur(&mut sh, w, h, (blur.0 / 2.0).round() as usize, (blur.1 / 2.0).round() as usize, passes);
    let src = pm.data().to_vec();
    let data = pm.data_mut();
    for i in 0..w * h {
        let s = &src[i * 4..i * 4 + 4];
        let d = &sh[i * 4..i * 4 + 4];
        let sa = s[3] as u32;
        let px: [u8; 4] = if inner {
            // Shadow confined to the object's interior.
            let mut m = [0u8; 4];
            for c in 0..4 {
                m[c] = (d[c] as u32 * sa / 255) as u8;
            }
            if knockout || hide { m } else { over(&m, s) }
        } else if knockout {
            let mut m = [0u8; 4];
            for c in 0..4 {
                m[c] = (d[c] as u32 * (255 - sa) / 255) as u8;
            }
            m
        } else if hide {
            [d[0], d[1], d[2], d[3]]
        } else {
            over(s, d)
        };
        data[i * 4..i * 4 + 4].copy_from_slice(&px);
    }
}

fn adjust_color(pm: &mut Pixmap, brightness: f32, contrast: f32, saturation: f32, hue: f32) {
    let b = brightness / 100.0;
    let c = 1.0 + contrast / 100.0;
    let s = 1.0 + saturation / 100.0;
    let (sin, cos) = hue.to_radians().sin_cos();
    // Hue rotation matrix in RGB space (luminance-preserving approximation).
    let (lr, lg, lb) = (0.213, 0.715, 0.072);
    let hm = [
        [lr + cos * (1.0 - lr) + sin * (-lr), lg + cos * (-lg) + sin * (-lg), lb + cos * (-lb) + sin * (1.0 - lb)],
        [lr + cos * (-lr) + sin * 0.143, lg + cos * (1.0 - lg) + sin * 0.140, lb + cos * (-lb) + sin * (-0.283)],
        [lr + cos * (-lr) + sin * (-(1.0 - lr)), lg + cos * (-lg) + sin * lg, lb + cos * (1.0 - lb) + sin * lb],
    ];
    for px in pm.pixels_mut() {
        let p = px.demultiply();
        if p.alpha() == 0 {
            continue;
        }
        let mut rgb = [p.red() as f32 / 255.0, p.green() as f32 / 255.0, p.blue() as f32 / 255.0];
        rgb = [
            hm[0][0] * rgb[0] + hm[0][1] * rgb[1] + hm[0][2] * rgb[2],
            hm[1][0] * rgb[0] + hm[1][1] * rgb[1] + hm[1][2] * rgb[2],
            hm[2][0] * rgb[0] + hm[2][1] * rgb[1] + hm[2][2] * rgb[2],
        ];
        let l = lr * rgb[0] + lg * rgb[1] + lb * rgb[2];
        for v in &mut rgb {
            *v = l + (*v - l) * s;
            *v = (*v - 0.5) * c + 0.5 + b;
        }
        let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        *px = tiny_skia::ColorU8::from_rgba(to(rgb[0]), to(rgb[1]), to(rgb[2]), p.alpha()).premultiply();
    }
}

/// Apply one filter in place. `scale` converts document pixels to output pixels.
pub fn apply(pm: &mut Pixmap, f: &Filter, scale: f64) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    match *f {
        Filter::Blur { blur_x, blur_y, quality } => {
            let rx = (blur_x * scale / 2.0).round() as usize;
            let ry = (blur_y * scale / 2.0).round() as usize;
            box_blur(pm.data_mut(), w, h, rx, ry, quality.clamp(1, 3));
        }
        Filter::DropShadow { blur_x, blur_y, strength, quality, angle, distance, color, knockout, inner, hide_object } => {
            let a = angle.to_radians();
            let off = ((a.cos() * distance * scale).round() as i32, (a.sin() * distance * scale).round() as i32);
            shadow_like(pm, color, strength, (blur_x * scale, blur_y * scale), quality.clamp(1, 3), off, inner, knockout, hide_object);
        }
        Filter::Glow { blur_x, blur_y, strength, quality, color, knockout, inner } => {
            shadow_like(pm, color, strength, (blur_x * scale, blur_y * scale), quality.clamp(1, 3), (0, 0), inner, knockout, false);
        }
        Filter::AdjustColor { brightness, contrast, saturation, hue } => adjust_color(pm, brightness, contrast, saturation, hue),
    }
}

/// Invert blend: inverts the destination where the source has coverage.
pub fn composite_invert(dst: &mut Pixmap, src: &Pixmap) {
    let s = src.data().to_vec();
    for (i, d) in dst.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let a = s[i * 4 + 3] as u32;
        if a == 0 {
            continue;
        }
        for c in 0..3 {
            let inv = d[3] as u32 - d[c] as u32;
            d[c] = ((d[c] as u32 * (255 - a) + inv * a) / 255) as u8;
        }
    }
}

/// Subtract blend: destination minus source color.
pub fn composite_subtract(dst: &mut Pixmap, src: &Pixmap) {
    let s = src.data().to_vec();
    for (i, d) in dst.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
        for c in 0..3 {
            d[c] = d[c].saturating_sub(s[i * 4 + c]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_preserves_total_alpha_roughly() {
        let mut pm = Pixmap::new(40, 40).unwrap();
        for y in 15..25 {
            for x in 15..25 {
                let i = (y * 40 + x) * 4;
                pm.data_mut()[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let before: u32 = pm.data().chunks(4).map(|p| p[3] as u32).sum();
        apply(&mut pm, &Filter::Blur { blur_x: 6.0, blur_y: 6.0, quality: 3 }, 1.0);
        let after: u32 = pm.data().chunks(4).map(|p| p[3] as u32).sum();
        assert!((after as f64 / before as f64 - 1.0).abs() < 0.1, "{before} {after}");
        assert!(pm.data()[(20 * 40 + 12) * 4 + 3] > 0);
    }

    #[test]
    fn drop_shadow_offsets() {
        let mut pm = Pixmap::new(40, 40).unwrap();
        for y in 10..20 {
            for x in 10..20 {
                let i = (y * 40 + x) * 4;
                pm.data_mut()[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        apply(
            &mut pm,
            &Filter::DropShadow {
                blur_x: 0.0,
                blur_y: 0.0,
                strength: 1.0,
                quality: 1,
                angle: 0.0,
                distance: 10.0,
                color: Color::BLACK,
                knockout: false,
                inner: false,
                hide_object: false,
            },
            1.0,
        );
        let i = (15 * 40 + 25) * 4;
        assert_eq!(&pm.data()[i..i + 4], &[0, 0, 0, 255]);
        let j = (15 * 40 + 15) * 4;
        assert_eq!(&pm.data()[j..j + 4], &[255, 0, 0, 255]);
    }
}
