//! Pixel effects computed from the original capture: redaction, erase, lens.

use crate::doc::{Jitter, PixelOp, R};
use crate::tiled::{PxRect, TiledImage};

/// Tightly packed premultiplied BGRA, `w`×`h`, drawn into `at` (image pixels).
pub struct Patch {
    pub at: R,
    pub w: u32,
    pub h: u32,
    pub bgra: Vec<u8>,
}

impl Patch {
    fn at_rect(rect: PxRect, bgra: Vec<u8>) -> Self {
        Self {
            at: R::new(rect.x as f32, rect.y as f32, rect.w as f32, rect.h as f32),
            w: rect.w,
            h: rect.h,
            bgra,
        }
    }
}

pub fn patch(image: &TiledImage, op: PixelOp, rect: R) -> Option<Patch> {
    match op {
        PixelOp::Pixelate { cell, seed } => {
            let rect = clamp(image, rect)?;
            let mut bgra = image.read(rect);
            pixelate(&mut bgra, rect.w, rect.h, cell, seed);
            Some(Patch::at_rect(rect, bgra))
        }
        PixelOp::Erase => {
            let rect = clamp(image, rect)?;
            Some(Patch::at_rect(rect, erase(image, rect)))
        }
        PixelOp::Lens { src, round } => lens(image, src, rect, round),
    }
}

fn clamp(image: &TiledImage, r: R) -> Option<PxRect> {
    let r = r.round_out().intersect(&R::new(
        0.,
        0.,
        image.width as f32,
        image.height as f32,
    ))?;
    (r.w >= 1. && r.h >= 1.).then(|| PxRect {
        x: r.x as u32,
        y: r.y as u32,
        w: r.w as u32,
        h: r.h as u32,
    })
}

/// Cell averages, so flat areas stay flat. Cells vary in size, as in the alternative:
/// depixelation tools that match blocks against candidate text assume a regular grid.
pub fn pixelate(bgra: &mut [u8], w: u32, h: u32, cell: u32, seed: u64) {
    let mut rng = Jitter(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    // A sliver left at the far edge joins the cell before it.
    let span = |rng: &mut Jitter, left: u32| {
        let s = (cell as f32 * rng.next(0.5, 2.)).round().max(2.) as u32;
        if left < s + cell / 2 { left } else { s }
    };
    let mut by = 0;
    while by < h {
        let bh = span(&mut rng, h - by);
        let mut bx = 0;
        while bx < w {
            let bw = span(&mut rng, w - bx);
            let mut sum = [0u32; 4];
            for y in by..by + bh {
                for x in bx..bx + bw {
                    let i = ((y * w + x) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += bgra[i + c] as u32;
                    }
                }
            }
            let n = bw * bh;
            let avg = sum.map(|s| (s / n) as u8);
            for y in by..by + bh {
                for x in bx..bx + bw {
                    let i = ((y * w + x) * 4) as usize;
                    bgra[i..i + 4].copy_from_slice(&avg);
                }
            }
            bx += bw;
        }
        by += bh;
    }
}

/// Fills `rect` by blending the pixels just outside it: horizontally from the
/// left and right borders, vertically from the top and bottom, weighted toward
/// whichever pair is closer.
fn erase(image: &TiledImage, rect: PxRect) -> Vec<u8> {
    let (w, h) = (rect.w as usize, rect.h as usize);
    let px = |x: i64, y: i64| -> Option<[f32; 4]> {
        (x >= 0 && y >= 0)
            .then(|| image.pixel(x as u32, y as u32))
            .flatten()
            .map(|p| p.map(|c| c as f32))
    };
    let (x0, y0) = (rect.x as i64, rect.y as i64);
    let (x1, y1) = (x0 + w as i64, y0 + h as i64);
    let left: Vec<_> = (0..h).map(|j| px(x0 - 1, y0 + j as i64)).collect();
    let right: Vec<_> = (0..h).map(|j| px(x1, y0 + j as i64)).collect();
    let top: Vec<_> = (0..w).map(|i| px(x0 + i as i64, y0 - 1)).collect();
    let bottom: Vec<_> = (0..w).map(|i| px(x0 + i as i64, y1)).collect();
    let lerp = |a: Option<[f32; 4]>, b: Option<[f32; 4]>, t: f32| match (a, b) {
        (Some(a), Some(b)) => Some([0, 1, 2, 3].map(|c| a[c] + (b[c] - a[c]) * t)),
        (a, b) => a.or(b),
    };
    let mut out = vec![0u8; w * h * 4];
    for j in 0..h {
        let ty = (j as f32 + 0.5) / h as f32;
        for i in 0..w {
            let tx = (i as f32 + 0.5) / w as f32;
            let horiz = lerp(left[j], right[j], tx);
            let vert = lerp(top[i], bottom[i], ty);
            let dh = tx.min(1. - tx) * w as f32;
            let dv = ty.min(1. - ty) * h as f32;
            let c = match (horiz, vert) {
                (Some(a), Some(b)) => {
                    let wa = dv / (dh + dv).max(1e-3);
                    [0, 1, 2, 3].map(|c| a[c] * wa + b[c] * (1. - wa))
                }
                (a, b) => a.or(b).unwrap_or([0., 0., 0., 255.]),
            };
            let k = (j * w + i) * 4;
            for c2 in 0..4 {
                out[k + c2] = c[c2].round().clamp(0., 255.) as u8;
            }
        }
    }
    out
}

/// Nearest-neighbor magnification, so pixels stay crisp in the lens.
fn lens(image: &TiledImage, src: R, dst: R, round: bool) -> Option<Patch> {
    let dst = dst.round_out();
    let (w, h) = (dst.w.max(1.) as u32, dst.h.max(1.) as u32);
    let read = R::new(0., 0., image.width as f32, image.height as f32).intersect(&src.round_out())?;
    let read = PxRect {
        x: read.x as u32,
        y: read.y as u32,
        w: read.w as u32,
        h: read.h as u32,
    };
    let source = image.read(read);
    let mut out = vec![0u8; (w * h * 4) as usize];
    let (cx, cy, r) = (w as f32 / 2., h as f32 / 2., w.min(h) as f32 / 2.);
    let corner = w.min(h) as f32 * 0.12;
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let coverage = if round {
                (r - (fx - cx).hypot(fy - cy) + 0.5).clamp(0., 1.)
            } else {
                rounded_coverage(fx, fy, w as f32, h as f32, corner)
            };
            if coverage <= 0. {
                continue;
            }
            let sx = src.x + fx / w as f32 * src.w;
            let sy = src.y + fy / h as f32 * src.h;
            let (ix, iy) = (sx.floor() as i64 - read.x as i64, sy.floor() as i64 - read.y as i64);
            if ix < 0 || iy < 0 || ix >= read.w as i64 || iy >= read.h as i64 {
                continue;
            }
            let si = ((iy as u32 * read.w + ix as u32) * 4) as usize;
            let di = ((y * w + x) * 4) as usize;
            for c in 0..4 {
                out[di + c] = (source[si + c] as f32 * coverage).round() as u8;
            }
        }
    }
    Some(Patch {
        at: R::new(dst.x, dst.y, w as f32, h as f32),
        w,
        h,
        bgra: out,
    })
}

fn rounded_coverage(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let qx = (x - w / 2.).abs() - (w / 2. - r);
    let qy = (y - h / 2.).abs() - (h / 2. - r);
    let outside = qx.max(0.).hypot(qy.max(0.)) + qx.max(qy).min(0.) - r;
    (0.5 - outside).clamp(0., 1.)
}

/// The most common color along the crop's border, which is almost always the
/// app or page background. Returned as 0xRRGGBB.
/// `None` when the edge is mostly transparent, as around a window capture, so grown canvas stays clear.
pub fn background_color(image: &TiledImage, crop: PxRect) -> Option<u32> {
    let mut counts = std::collections::HashMap::<u32, (u32, [u32; 3])>::new();
    let (mut clear, mut total) = (0u32, 0u32);
    let mut add = |x: u32, y: u32| {
        if let Some([b, g, r, a]) = image.pixel(x, y) {
            total += 1;
            if a < 128 {
                clear += 1;
                return;
            }
            let key = ((r as u32 >> 3) << 10) | ((g as u32 >> 3) << 5) | (b as u32 >> 3);
            let e = counts.entry(key).or_default();
            e.0 += 1;
            e.1[0] += r as u32;
            e.1[1] += g as u32;
            e.1[2] += b as u32;
        }
    };
    let (x1, y1) = (crop.x + crop.w - 1, crop.y + crop.h - 1);
    let step = ((crop.w + crop.h) / 400).max(1);
    for x in (crop.x..=x1).step_by(step as usize) {
        add(x, crop.y);
        add(x, y1);
    }
    for y in (crop.y..=y1).step_by(step as usize) {
        add(crop.x, y);
        add(x1, y);
    }
    if clear * 2 > total {
        return None;
    }
    let fill = counts
        .values()
        .max_by_key(|(n, _)| *n)
        .map(|(n, [r, g, b])| ((r / n) << 16) | ((g / n) << 8) | (b / n));
    Some(fill.unwrap_or(0xFFFFFF))
}

/// Shrinks `rect` to the content inside it: the edges move in while each row or
/// column still matches the border's uniform color.
pub fn autoadjust(image: &TiledImage, rect: PxRect) -> PxRect {
    // Reads pixels in place: the scan usually stops a few rows in, and the drag repeats it per move.
    let (w, h) = (rect.w as usize, rect.h as usize);
    let at = |x: usize, y: usize| -> [i32; 3] {
        let p = image.pixel(rect.x + x as u32, rect.y + y as u32).unwrap_or_default();
        [p[0] as i32, p[1] as i32, p[2] as i32]
    };
    // Tight, as in the alternative: a white card on #F7F7F7 is an edge.
    let close = |a: [i32; 3], b: [i32; 3]| (0..3).all(|c| (a[c] - b[c]).abs() <= 4);
    let row_uniform = |y: usize, x0: usize, x1: usize, c: [i32; 3]| (x0..x1).all(|x| close(at(x, y), c));
    let col_uniform = |x: usize, y0: usize, y1: usize, c: [i32; 3]| (y0..y1).all(|y| close(at(x, y), c));
    let (mut l, mut t, mut r, mut b) = (0usize, 0usize, w, h);
    let bg = at(0, 0);
    while t + 1 < b && row_uniform(t, l, r, bg) {
        t += 1;
    }
    while b > t + 1 && row_uniform(b - 1, l, r, bg) {
        b -= 1;
    }
    while l + 1 < r && col_uniform(l, t, b, bg) {
        l += 1;
    }
    while r > l + 1 && col_uniform(r - 1, t, b, bg) {
        r -= 1;
    }
    if r - l < 2 || b - t < 2 || (l == 0 && t == 0 && r == w && b == h) {
        return rect;
    }
    PxRect {
        x: rect.x + l as u32,
        y: rect.y + t as u32,
        w: (r - l) as u32,
        h: (b - t) as u32,
    }
}

/// Premultiplied RGBA or BGRA at a new size: area-averaged down, bicubic up.
pub fn resample(px: &[u8], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u8> {
    if nw <= w && nh <= h {
        return shrink(px, w, h, nw, nh);
    }
    let src = tiny_skia::PixmapRef::from_bytes(px, w, h).expect("pixmap size");
    let mut out = tiny_skia::Pixmap::new(nw, nh).expect("resize to a non-empty size");
    let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
    let k = tiny_skia::Transform::from_scale(nw as f32 / w as f32, nh as f32 / h as f32);
    out.draw_pixmap(0, 0, src, &paint, k, None);
    out.take()
}

/// Area-averaged downscale; the GPU's linear sampling aliases badly past 2×.
pub fn shrink(bgra: &[u8], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u8> {
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for ny in 0..nh {
        let (y0, y1) = (ny * h / nh, ((ny + 1) * h / nh).max(ny * h / nh + 1));
        for nx in 0..nw {
            let (x0, x1) = (nx * w / nw, ((nx + 1) * w / nw).max(nx * w / nw + 1));
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                let row = &bgra[((y * w + x0) * 4) as usize..((y * w + x1) * 4) as usize];
                for p in row.chunks_exact(4) {
                    for c in 0..4 {
                        sum[c] += p[c] as u32;
                    }
                }
            }
            let n = (x1 - x0) * (y1 - y0);
            let i = ((ny * nw + nx) * 4) as usize;
            for c in 0..4 {
                out[i + c] = (sum[c] / n) as u8;
            }
        }
    }
    out
}

/// Up to ~64k evenly spaced pixels of `rect` as 0xRRGGBB, so color stats stay cheap on huge selections.
pub fn sample(image: &TiledImage, rect: PxRect) -> impl Iterator<Item = u32> + '_ {
    let step = ((rect.w as f32 * rect.h as f32 / 65_536.).sqrt().ceil() as u32).max(1);
    (rect.y..rect.y + rect.h).step_by(step as usize).flat_map(move |y| {
        (rect.x..rect.x + rect.w)
            .step_by(step as usize)
            .filter_map(move |x| image.pixel(x, y))
            .map(|[b, g, r, _]| (r as u32) << 16 | (g as u32) << 8 | b as u32)
    })
}

fn mean(colors: impl Iterator<Item = u32>) -> Option<u32> {
    let (mut sum, mut n) = ([0u64; 3], 0u64);
    for c in colors {
        for (i, s) in sum.iter_mut().enumerate() {
            *s += (c >> (16 - 8 * i) & 255) as u64;
        }
        n += 1;
    }
    (n > 0).then(|| sum.iter().fold(0, |acc, s| acc << 8 | (s / n) as u32))
}

pub fn average_color(image: &TiledImage, rect: PxRect) -> Option<u32> {
    mean(sample(image, rect))
}

/// The two most common clearly different colors, e.g. text and its background.
pub fn dominant_pair(image: &TiledImage, rect: PxRect) -> Option<(u32, u32)> {
    let bucket = |c: u32| (c >> 12 & 0xF00 | c >> 8 & 0xF0 | c >> 4 & 0xF) as usize;
    let mut counts = vec![0u32; 4096];
    for c in sample(image, rect) {
        counts[bucket(c)] += 1;
    }
    let center = |b: usize| (b as u32 & 0xF00) << 12 | (b as u32 & 0xF0) << 8 | (b as u32 & 0xF) << 4;
    let first = (0..4096).max_by_key(|&b| counts[b])?;
    let far = |b: usize| (luminance(center(b)) - luminance(center(first))).abs() > 0.15;
    let second = (0..4096).filter(|&b| counts[b] > 0 && far(b)).max_by_key(|&b| counts[b])?;
    let mode = |b: usize| {
        let mut exact = std::collections::HashMap::new();
        for c in sample(image, rect).filter(|&c| bucket(c) == b) {
            *exact.entry(c).or_insert(0u32) += 1;
        }
        exact.into_iter().max_by_key(|&(_, n)| n).map(|(c, _)| c)
    };
    Some((mode(first)?, mode(second)?))
}

/// WCAG 2 relative luminance.
fn luminance(c: u32) -> f32 {
    let lin = |v: u32| {
        let v = v as f32 / 255.;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c >> 16 & 255) + 0.7152 * lin(c >> 8 & 255) + 0.0722 * lin(c & 255)
}

pub fn contrast_ratio(a: u32, b: u32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_edges_have_no_background() {
        let (w, h) = (8u32, 8u32);
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let clear = TiledImage::from_bgra(w, h, &bgra, w as usize * 4);
        assert_eq!(background_color(&clear, clear.full()), None);
        bgra.chunks_mut(4).for_each(|p| p.copy_from_slice(&[0x30, 0x20, 0x10, 0xFF]));
        let opaque = TiledImage::from_bgra(w, h, &bgra, w as usize * 4);
        assert_eq!(background_color(&opaque, opaque.full()), Some(0x102030));
    }
}
