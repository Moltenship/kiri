//! Renders the document to pixels with tiny-skia, from the same primitives the
//! screen painter uses.

use std::path::PathBuf;

use tiny_skia::{
    BlendMode, FillRule, FilterQuality, IntSize, LineCap, LineJoin, Mask, Paint, PathBuilder, Pattern, Pixmap,
    PixmapPaint, Rect, SpreadMode, Stroke, StrokeDash, Transform,
};

use crate::doc::{Ctx, LINE_HEIGHT, Object, Prim, R, Seg, scene};
use crate::pixels;
use crate::tiled::{PxRect, TiledImage};

pub struct Scene<'a> {
    pub image: &'a TiledImage,
    pub crop: PxRect,
    pub canvas: R,
    /// Fill for canvas area outside the image; `None` is transparent.
    pub fill: Option<u32>,
    pub objects: &'a [Object],
    pub cx: &'a Ctx<'a>,
}

pub fn render(s: &Scene) -> Pixmap {
    let c = s.canvas.round_out();
    let (w, h) = (c.w.max(1.) as u32, c.h.max(1.) as u32);
    let base = bgra_pixmap(s.crop.w, s.crop.h, s.image.read(s.crop)).expect("crop size");
    let (bx, by) = (s.crop.x as f32 - c.x, s.crop.y as f32 - c.y);
    let mut pixmap = if bx == 0. && by == 0. && w == s.crop.w && h == s.crop.h {
        base
    } else {
        let mut canvas = Pixmap::new(w, h).expect("canvas size");
        if let Some(fill) = s.fill {
            canvas.fill(color(fill, 1.));
        }
        canvas.draw_pixmap(
            bx as i32,
            by as i32,
            base.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        canvas
    };
    let t = Transform::from_translate(-c.x, -c.y);
    for prim in scene(s.objects, s.cx, c) {
        draw(&mut pixmap, &prim, s.image, t, c);
    }
    pixmap
}

fn color(rgb: u32, alpha: f32) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(
        (rgb >> 16) as u8,
        (rgb >> 8) as u8,
        rgb as u8,
        (alpha * 255.).round() as u8,
    )
}

pub fn path(segs: &[Seg]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for s in segs {
        match *s {
            Seg::M(p) => pb.move_to(p.0, p.1),
            Seg::L(p) => pb.line_to(p.0, p.1),
            Seg::Q(c, p) => pb.quad_to(c.0, c.1, p.0, p.1),
            Seg::C(a, b, p) => pb.cubic_to(a.0, a.1, b.0, b.1, p.0, p.1),
            Seg::Z => pb.close(),
        }
    }
    pb.finish()
}

fn marker(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Round,
        ..Stroke::default()
    }
}

/// What a highlighter shows on screen: the pixels under its stroke multiplied by its
/// color, transparent elsewhere. Export multiplies in place instead; both draw
/// highlighters first, so the results match.
pub fn highlight_patch(image: &TiledImage, fill: Option<u32>, segs: &[Seg], width: f32, c: u32, rect: R) -> Option<pixels::Patch> {
    let r = rect.round_out();
    let (w, h) = (r.w as u32, r.h as u32);
    let mut pm = Pixmap::new(w, h)?;
    if let Some(f) = fill {
        pm.fill(color(f, 1.));
    }
    if let Some(src) = pixels::clamp(image, r)
        && let Some(base) = bgra_pixmap(src.w, src.h, image.read(src))
    {
        let at = (src.x as i32 - r.x as i32, src.y as i32 - r.y as i32);
        pm.draw_pixmap(at.0, at.1, base.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }
    let mut mask = Mask::new(w, h)?;
    let outline = path(segs)?.stroke(&marker(width), 1.)?;
    mask.fill_path(&outline, FillRule::Winding, true, Transform::from_translate(-r.x, -r.y));
    let mut paint = Paint::default();
    paint.set_color(color(c, 1.));
    paint.blend_mode = BlendMode::Multiply;
    pm.fill_rect(Rect::from_xywh(0., 0., w as f32, h as f32)?, &paint, Transform::identity(), None);
    let mut bgra = pm.take();
    for (px, &a) in bgra.chunks_exact_mut(4).zip(mask.data()) {
        px.swap(0, 2);
        for ch in px.iter_mut() {
            *ch = ((*ch as u32 * a as u32 + 127) / 255) as u8;
        }
    }
    Some(pixels::Patch { at: r, w, h, bgra })
}

fn draw(pixmap: &mut Pixmap, prim: &Prim, image: &TiledImage, t: Transform, canvas: R) {
    match prim {
        Prim::Highlight { path: segs, width, color: c, .. } => {
            if let Some(p) = path(segs) {
                let mut paint = Paint::default();
                paint.set_color(color(*c, 1.));
                paint.anti_alias = true;
                paint.blend_mode = BlendMode::Multiply;
                pixmap.stroke_path(&p, &paint, &marker(*width), t, None);
            }
        }
        Prim::Fill {
            path: segs,
            color: c,
            alpha,
        } => {
            if let Some(p) = path(segs) {
                let mut paint = Paint::default();
                paint.set_color(color(*c, *alpha));
                paint.anti_alias = true;
                pixmap.fill_path(&p, &paint, FillRule::EvenOdd, t, None);
            }
        }
        Prim::Stroke {
            path: segs,
            width,
            color: c,
            dash,
        } => {
            if let Some(p) = path(segs) {
                let mut paint = Paint::default();
                paint.set_color(color(*c, 1.));
                paint.anti_alias = true;
                let stroke = Stroke {
                    width: *width,
                    line_cap: if dash.is_some() {
                        LineCap::Butt
                    } else {
                        LineCap::Round
                    },
                    line_join: LineJoin::Round,
                    dash: dash.and_then(|d| StrokeDash::new(vec![d, d * 0.75], 0.)),
                    ..Stroke::default()
                };
                pixmap.stroke_path(&p, &paint, &stroke, t, None);
            }
        }
        Prim::Text {
            at,
            text,
            size,
            color: c,
            bold,
        } => {
            if let Some((w, h, rgba)) = crate::mac::text_image(text, *size, LINE_HEIGHT, *c, *bold)
                && let Some(img) = Pixmap::from_vec(rgba, IntSize::from_wh(w, h).expect("size"))
            {
                pixmap.draw_pixmap(
                    0,
                    0,
                    img.as_ref(),
                    &PixmapPaint::default(),
                    t.pre_translate(at.0, at.1),
                    None,
                );
            }
        }
        Prim::Image { src, rect, radius, alpha, .. } => {
            let (Some(img), Some(p)) = (bgra_pixmap(src.0.w, src.0.h, src.0.bgra.clone()), path(&crate::doc::rect_path(*rect, *radius))) else {
                return;
            };
            let fit = Transform::from_row(rect.w / src.0.w as f32, 0., 0., rect.h / src.0.h as f32, rect.x, rect.y);
            let mut paint = Paint::default();
            paint.anti_alias = true;
            paint.shader = Pattern::new(img.as_ref(), SpreadMode::Pad, FilterQuality::Bicubic, *alpha, fit);
            pixmap.fill_path(&p, &paint, FillRule::Winding, t, None);
        }
        Prim::Pixels { op, rect, .. } => {
            if let Some(patch) = pixels::patch(image, *op, *rect)
                && let Some(img) = bgra_pixmap(patch.w, patch.h, patch.bgra)
            {
                pixmap.draw_pixmap(
                    (patch.at.x - canvas.x).round() as i32,
                    (patch.at.y - canvas.y).round() as i32,
                    img.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        }
    }
}

/// Fast PNG: the capture-to-clipboard loop is dominated by encoding. The density
/// records pixels per point (72 dpi each), as macOS screenshots do.
pub fn encode_png(pixmap: &Pixmap, scale: f32) -> Vec<u8> {
    encode_png_with(pixmap, scale, png::Compression::Fast)
}

fn encode_png_with(pixmap: &Pixmap, scale: f32, compression: png::Compression) -> Vec<u8> {
    let mut data = pixmap.data().to_vec();
    if data.chunks_exact(4).any(|p| p[3] != 255) {
        for p in data.chunks_exact_mut(4) {
            let a = p[3] as u32;
            if a > 0 && a < 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(compression);
    let ppm = (2834.6 * scale).round() as u32;
    encoder.set_pixel_dims(Some(png::PixelDimensions { xppu: ppm, yppu: ppm, unit: png::Unit::Meter }));
    if let Ok(mut writer) = encoder.write_header() {
        let _ = writer.write_image_data(&data);
    }
    out
}

/// What Save writes, and its extension. Auto keeps a JPEG only when it is smaller than
/// the PNG; transparency and sides past JPEG's 65535 limit stay PNG.
pub fn for_save(pixmap: &Pixmap, scale: f32, auto: bool) -> (Vec<u8>, &'static str) {
    if !auto {
        return (encode_png(pixmap, scale), "png");
    }
    // A fast PNG loses to JPEG even on UI screenshots, so Auto compares against a compact one.
    let png = encode_png_with(pixmap, scale, png::Compression::Balanced);
    let fits = pixmap.width() < 65535 && pixmap.height() < 65535;
    if fits && pixmap.pixels().iter().all(|p| p.alpha() == 255) {
        let rgb: Vec<u8> = pixmap.data().chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let mut jpeg = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 92);
        encoder.set_pixel_density(image::codecs::jpeg::PixelDensity::dpi((72. * scale).round() as u16));
        if encoder.encode(&rgb, pixmap.width(), pixmap.height(), image::ExtendedColorType::Rgb8).is_ok() && jpeg.len() < png.len() {
            return (jpeg, "jpeg");
        }
    }
    (png, "png")
}

/// Saves an unedited capture, returning the path: Auto may change its extension.
pub fn save_capture(image: &TiledImage, scale: f32, png: Vec<u8>, path: PathBuf, auto: bool) -> std::io::Result<PathBuf> {
    let (bytes, ext) = match auto.then(|| bgra_pixmap(image.width, image.height, image.read(image.full()))).flatten() {
        Some(pixmap) => for_save(&pixmap, scale, true),
        None => (png, "png"),
    };
    let path = path.with_extension(ext);
    std::fs::write(&path, bytes)?;
    crate::mac::mark_screenshot(&path);
    Ok(path)
}

/// "Downscale Retina" output: one pixel per point.
pub fn downscale(pixmap: &Pixmap, scale: f32) -> Pixmap {
    let (w, h) = (
        ((pixmap.width() as f32 / scale).round() as u32).max(1),
        ((pixmap.height() as f32 / scale).round() as u32).max(1),
    );
    let Some(mut out) = Pixmap::new(w, h) else {
        return pixmap.clone();
    };
    let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() };
    out.draw_pixmap(0, 0, pixmap.as_ref(), &paint, Transform::from_scale(1. / scale, 1. / scale), None);
    out
}

/// Tightly packed BGRA as a PNG; `Pixmap` wants RGBA, so channels swap on the way.
pub fn encode_bgra(w: u32, h: u32, bgra: Vec<u8>, scale: f32) -> Vec<u8> {
    bgra_pixmap(w, h, bgra).map(|pixmap| encode_png(&pixmap, scale)).unwrap_or_default()
}

pub fn bgra_pixmap(w: u32, h: u32, mut bgra: Vec<u8>) -> Option<Pixmap> {
    for p in bgra.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    IntSize::from_wh(w, h).and_then(|size| Pixmap::from_vec(bgra, size))
}

/// Decodes a PNG or JPEG into tiles plus its pixels-per-point, read from the PNG's
/// density so Retina screenshots reopen at their real size.
pub fn decode(bytes: &[u8]) -> Option<(TiledImage, f32)> {
    let (w, h, bgra, scale) = decode_bgra(bytes)?;
    Some((TiledImage::from_bgra(w, h, &bgra), scale))
}

/// Premultiplied BGRA, as the document keeps its pixels.
pub fn decode_bgra(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>, f32)> {
    let (w, h, mut px, scale) = decode_rgba(bytes)?;
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
        let a = p[3] as u32;
        if a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * a + 127) / 255) as u8;
            }
        }
    }
    Some((w, h, px, scale))
}

/// Straight-alpha RGBA.
pub fn decode_rgba(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>, f32)> {
    decode_png(bytes).or_else(|| {
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        Some((w, h, img.into_raw(), 1.))
    })
}

fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>, f32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let scale = match reader.info().pixel_dims {
        Some(d) if d.unit == png::Unit::Meter && d.xppu > 0 => (d.xppu as f32 / 2834.6).round().max(1.),
        _ => 1.,
    };
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (frame.width, frame.height);
    buf.truncate(frame.buffer_size());
    let rgba = match frame.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some((w, h, rgba, scale))
}

/// An endless GIF cycling through `frames`, each shown for `delay_ms`.
pub fn encode_gif(frames: &[Pixmap], delay_ms: u32) -> Result<Vec<u8>, String> {
    use image::codecs::gif::{GifEncoder, Repeat};
    let mut out = Vec::new();
    {
        // Speed 10 of 30: NeuQuant at full quality takes seconds on a Retina screenshot.
        let mut encoder = GifEncoder::new_with_speed(&mut out, 10);
        encoder.set_repeat(Repeat::Infinite).map_err(|e| e.to_string())?;
        for pixmap in frames {
            let rgba = pixmap.pixels().iter().flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            });
            let image = image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), rgba.collect()).ok_or("bad frame")?;
            let delay = image::Delay::from_numer_denom_ms(delay_ms, 1);
            encoder.encode_frame(image::Frame::from_parts(image, 0, 0, delay)).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixmap(alpha: u8, noisy: bool) -> Pixmap {
        let mut p = Pixmap::new(256, 256).unwrap();
        let mut seed = 1u32;
        for px in p.data_mut().chunks_exact_mut(4) {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let v = if noisy { (seed >> 16) as u8 } else { 200 };
            px.copy_from_slice(&[v.min(alpha), v.min(alpha), v.min(alpha), alpha]);
        }
        p
    }

    #[test]
    fn translucent_pixels_survive_a_round_trip() {
        let premultiplied = [50, 25, 10, 128];
        let png = encode_bgra(1, 1, premultiplied.to_vec(), 1.);
        assert_eq!(decode_bgra(&png).unwrap().2, premultiplied);
    }

    #[test]
    fn auto_save_picks_the_smaller_format() {
        let ext = |p: &Pixmap, auto| for_save(p, 2., auto).1;
        assert_eq!(ext(&pixmap(255, true), true), "jpeg");
        assert_eq!(ext(&pixmap(255, true), false), "png");
        assert_eq!(ext(&pixmap(255, false), true), "png");
        assert_eq!(ext(&pixmap(128, true), true), "png");
    }
}
