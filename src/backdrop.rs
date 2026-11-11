//! A decorative background around the screenshot: padding, shadow, rounded
//! corners, and a solid, gradient, or mesh fill.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme, Icon, Selectable, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use tiny_skia::{FillRule, Mask, Paint, Pixmap, PixmapPaint, Transform};

use crate::doc::R;
use crate::editor::Editor;
use crate::paint::View;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Fill {
    Solid(u32),
    /// Four stops: corners (top-left, top-right, bottom-left, bottom-right) for a mesh, in order for the rest.
    Gradient([u32; 4], Kind),
    /// A picture cropped to cover the backdrop, and its blur from 0 to 1.
    Image(PathBuf, f32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    #[default]
    Mesh,
    Vertical,
    Diagonal,
    Radial,
}

pub const KINDS: [(Kind, &str); 4] =
    [(Kind::Mesh, "Mesh"), (Kind::Vertical, "Vertical"), (Kind::Diagonal, "Diagonal"), (Kind::Radial, "Radial")];

pub const SOLIDS: [u32; 10] = [
    0xF2F2F7, 0xFFFFFF, 0x1C1C1E, 0xE8EEF9, 0xFFE8D6, 0xFDE2E4, 0xD8F3DC, 0xCDE7F0, 0xE4DDF7,
    0xFFF3BF,
];

const fn two(a: u32, b: u32) -> [u32; 4] {
    let mut out = [0; 4];
    let mut i = 0;
    while i < 4 {
        let mut c = 0;
        let mut shift = 0;
        while shift < 24 {
            let (x, y) = ((a >> shift) & 255, (b >> shift) & 255);
            c |= ((x * (3 - i as u32) + y * i as u32) / 3) << shift;
            shift += 8;
        }
        out[i] = c;
        i += 1;
    }
    out
}

/// Palettes; the gradient kind is picked separately and applies to all of them.
pub const PALETTES: [[u32; 4]; 11] = [
    [0xFF9A8B, 0xFF6A88, 0xFFC796, 0xFF99AC],
    [0x8EC5FC, 0xE0C3FC, 0x7FD1FF, 0xC7B8FF],
    [0x84FAB0, 0x8FD3F4, 0xC2FFD8, 0x6FC3FF],
    [0xFDDB92, 0xD1FDFF, 0xFFB88C, 0xA1E3FF],
    two(0x4158D0, 0xC850C0),
    two(0x0093E9, 0x80D0C7),
    two(0xFA8BFF, 0x2BD2FF),
    [0x0F2027, 0x203A43, 0x2C5364, 0xF7797D],
    [0x355C7D, 0x6C5B7B, 0xC06C84, 0xF8B195],
    two(0xFFF1EB, 0xACE0F9),
    two(0x3A3F5C, 0x111320),
];

/// (label, ratio, where it fits)
pub const ASPECTS: [(&str, Option<(u32, u32)>, &str); 9] = [
    ("Auto", None, "Padding only"),
    ("1:1", Some((1, 1)), "Square posts"),
    ("4:3", Some((4, 3)), "Slides and docs"),
    ("3:2", Some((3, 2)), "Blog images"),
    ("16:9", Some((16, 9)), "YouTube, X posts, slides"),
    ("1.91:1", Some((191, 100)), "Link previews: LinkedIn, Facebook"),
    ("9:16", Some((9, 16)), "Stories and Reels"),
    ("4:5", Some((4, 5)), "Instagram portrait"),
    ("2:3", Some((2, 3)), "Pinterest"),
];

/// A soft mesh from the screenshot's own colors: each corner takes its quadrant's most vivid common color.
pub fn suggest(image: &crate::tiled::TiledImage) -> Fill {
    let (w, h) = ((image.width / 2).max(1), (image.height / 2).max(1));
    let corner = |x: u32, y: u32| {
        let mut score = std::collections::HashMap::<u32, f32>::new();
        for c in crate::pixels::sample(image, crate::tiled::PxRect { x, y, w, h }) {
            let bucket = c & 0xF0F0F0 | 0x080808;
            *score.entry(bucket).or_default() += 0.02 + saturation(c);
        }
        score.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).map_or(0xE8EEF9, |(c, _)| pastel(c))
    };
    let (x2, y2) = (image.width - w, image.height - h);
    Fill::Gradient([corner(0, 0), corner(x2, 0), corner(0, y2), corner(x2, y2)], Kind::Mesh)
}

fn saturation(c: u32) -> f32 {
    let [r, g, b] = rgb3(c);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    if max == 0. { 0. } else { (max - min) / max }
}

/// Lifts a color toward white so it sits behind a screenshot without competing with it.
fn pastel(c: u32) -> u32 {
    rgb3(c).iter().fold(0, |acc, v| acc << 8 | (v + (255. - v) * 0.45) as u32)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Backdrop {
    /// Padding, inset, inner radius, and outer radius are in points.
    pub padding: f32,
    pub shadow: f32,
    /// A margin of the screenshot's own background color inside the frame.
    pub inset: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub fill: Fill,
    pub aspect: Option<(u32, u32)>,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            padding: 48.,
            shadow: 0.5,
            inset: 0.,
            inner_radius: 10.,
            outer_radius: 0.,
            fill: Fill::Gradient(PALETTES[1], Kind::Mesh),
            aspect: None,
        }
    }
}

impl Backdrop {
    /// The full exported area around `canvas`, in image pixels.
    pub fn outer(&self, canvas: R, scale: f32) -> R {
        let mut r = canvas.inflate(self.padding * scale);
        if let Some((aw, ah)) = self.aspect {
            let target = aw as f32 / ah as f32;
            if r.w / r.h < target {
                let w = r.h * target;
                r.x -= (w - r.w) / 2.;
                r.w = w;
            } else {
                let h = r.w / target;
                r.y -= (h - r.h) / 2.;
                r.h = h;
            }
        }
        r.round_out()
    }

    fn shadow_blur(&self, scale: f32) -> f32 {
        self.shadow * 40. * scale
    }

    pub fn paint(&self, view: View, canvas: R, scale: f32, window: &mut Window) {
        let outer = view.rect(self.outer(canvas, scale));
        let o = self.outer(canvas, scale);
        let image = background_image(&self.fill, o.w / o.h.max(1.), window);
        let _ = window.paint_image(
            outer,
            outer,
            Corners::all(px(self.outer_radius * scale * view.k)),
            image,
            0,
            false,
        );
        if self.shadow > 0. {
            let blur = self.shadow_blur(scale) * view.k;
            window.paint_drop_shadows(
                view.rect(canvas),
                Corners::all(px(self.inner_radius * scale * view.k)),
                &[BoxShadow {
                    color: hsla(0., 0., 0., 0.18 + self.shadow * 0.32),
                    offset: point(px(0.), px(blur * 0.35)),
                    blur_radius: px(blur),
                    spread_radius: px(0.),
                    inset: false,
                }],
            );
        }
    }

    /// Puts the rendered canvas onto the backdrop for export.
    pub fn compose(&self, mut inner: Pixmap, canvas: R, scale: f32) -> Pixmap {
        let outer = self.outer(canvas, scale);
        let (w, h) = (outer.w as u32, outer.h as u32);
        let mut out = render_fill(&self.fill, w, h, 0);
        if self.outer_radius > 0. {
            apply_round_mask(&mut out, R::new(0., 0., w as f32, h as f32), self.outer_radius * scale);
        }
        let (ix, iy) = (canvas.x - outer.x, canvas.y - outer.y);
        if self.shadow > 0. {
            draw_shadow(
                &mut out,
                R::new(ix, iy, canvas.w, canvas.h),
                self.inner_radius * scale,
                self.shadow_blur(scale),
                0.18 + self.shadow * 0.32,
            );
        }
        if self.inner_radius > 0. {
            let all = R::new(0., 0., inner.width() as f32, inner.height() as f32);
            apply_round_mask(&mut inner, all, self.inner_radius * scale);
        }
        out.draw_pixmap(
            ix.round() as i32,
            iy.round() as i32,
            inner.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        out
    }
}

fn rounded_path(r: R, radius: f32) -> Option<tiny_skia::Path> {
    crate::export::path(&crate::doc::rect_path(r, radius))
}

fn apply_round_mask(p: &mut Pixmap, r: R, radius: f32) {
    let Some(mut mask) = Mask::new(p.width(), p.height()) else {
        return;
    };
    if let Some(path) = rounded_path(r, radius) {
        mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
        p.apply_mask(&mask);
    }
}

/// A blurred rounded rectangle, rendered at a quarter of the size and scaled
/// up: the blur hides the resampling and it keeps big exports fast.
fn draw_shadow(out: &mut Pixmap, r: R, radius: f32, blur: f32, alpha: f32) {
    const DOWN: f32 = 4.;
    let pad = blur * 1.5;
    let (sw, sh) = (
        ((r.w + 2. * pad) / DOWN).ceil() as u32 + 1,
        ((r.h + 2. * pad) / DOWN).ceil() as u32 + 1,
    );
    let Some(mut small) = Pixmap::new(sw, sh) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, (alpha * 255.) as u8);
    paint.anti_alias = true;
    if let Some(path) = rounded_path(
        R::new(pad / DOWN, pad / DOWN, r.w / DOWN, r.h / DOWN),
        radius / DOWN,
    ) {
        small.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
    let sigma = blur / 2. / DOWN;
    let box_r = ((sigma * sigma * 3.).sqrt()).round().max(1.) as usize;
    for _ in 0..3 {
        box_blur_alpha(&mut small, box_r);
    }
    out.draw_pixmap(
        0,
        0,
        small.as_ref(),
        &PixmapPaint {
            quality: tiny_skia::FilterQuality::Bilinear,
            ..PixmapPaint::default()
        },
        Transform::from_scale(DOWN, DOWN).post_translate(r.x - pad, r.y - pad + blur * 0.35),
        None,
    );
}

/// Blurs a black pixmap, which only carries alpha.
fn box_blur_alpha(p: &mut Pixmap, r: usize) {
    let (w, h) = (p.width() as usize, p.height() as usize);
    let mut a: Vec<f32> = p.data().chunks_exact(4).map(|px| px[3] as f32).collect();
    let mut tmp = vec![0f32; a.len()];
    let pass = |src: &[f32], dst: &mut [f32], len: usize, lines: usize, idx: &dyn Fn(usize, usize) -> usize| {
        let n = (2 * r + 1) as f32;
        for l in 0..lines {
            let mut sum = 0.;
            for i in 0..=r.min(len - 1) {
                sum += src[idx(l, i)];
            }
            for i in 0..len {
                dst[idx(l, i)] = sum / n;
                if i + r + 1 < len {
                    sum += src[idx(l, i + r + 1)];
                }
                if i >= r {
                    sum -= src[idx(l, i - r)];
                }
            }
        }
    };
    pass(&a, &mut tmp, w, h, &|l, i| l * w + i);
    pass(&tmp, &mut a, h, w, &|l, i| i * w + l);
    for (px, v) in p.data_mut().chunks_exact_mut(4).zip(a) {
        let v = v.round().clamp(0., 255.) as u8;
        px.copy_from_slice(&[0, 0, 0, v]);
    }
}

fn to_u32(c: Hsla) -> u32 {
    let c = c.to_rgb();
    let ch = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
    ch(c.r) << 16 | ch(c.g) << 8 | ch(c.b)
}

fn rgb3(c: u32) -> [f32; 3] {
    [(c >> 16) & 255, (c >> 8) & 255, c & 255].map(|v| v as f32)
}

/// Renders a fill as an opaque RGBA pixmap; a picture is read at most `max_side` large (see `source`).
fn render_fill(fill: &Fill, w: u32, h: u32, max_side: u32) -> Pixmap {
    if let Fill::Image(path, blur) = fill {
        return match source(path, max_side) {
            Some(src) => cover(&src, w, h, *blur),
            None => render_fill(&Fill::Gradient(PALETTES[1], Kind::Mesh), w, h, 0),
        };
    }
    let mut p = Pixmap::new(w.max(1), h.max(1)).expect("backdrop size");
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let sample: Box<dyn Fn(f32, f32) -> [f32; 3]> = match *fill {
        Fill::Image(..) => unreachable!(),
        Fill::Solid(c) => {
            let c = rgb3(c);
            Box::new(move |_, _| c)
        }
        Fill::Gradient(c, Kind::Mesh) => {
            let c = c.map(rgb3);
            Box::new(move |u, v| {
                let (u, v) = (smooth(u), smooth(v));
                [0, 1, 2].map(|i| {
                    let top = c[0][i] + (c[1][i] - c[0][i]) * u;
                    let bottom = c[2][i] + (c[3][i] - c[2][i]) * u;
                    top + (bottom - top) * v
                })
            })
        }
        Fill::Gradient(c, kind) => {
            let c = c.map(rgb3);
            Box::new(move |u, v| {
                let t = match kind {
                    Kind::Vertical => v,
                    Kind::Diagonal => (u + v) / 2.,
                    _ => ((u - 0.5).hypot(v - 0.5) / 0.7071).min(1.),
                } * 3.;
                let k = (t as usize).min(2);
                let f = t - k as f32;
                [0, 1, 2].map(|i| c[k][i] + (c[k + 1][i] - c[k][i]) * f)
            })
        }
    };
    let data = p.data_mut();
    for y in 0..h.max(1) {
        for x in 0..w.max(1) {
            let c = sample((x as f32 + 0.5) / fw, (y as f32 + 0.5) / fh);
            let i = ((y * w.max(1) + x) * 4) as usize;
            data[i] = c[0] as u8;
            data[i + 1] = c[1] as u8;
            data[i + 2] = c[2] as u8;
            data[i + 3] = 255;
        }
    }
    p
}

/// Scales `src` to cover `w`×`h`, centered, then blurs by a share of the size so previews match exports.
fn cover(src: &image::RgbaImage, w: u32, h: u32, blur: f32) -> Pixmap {
    use image::imageops::{self, FilterType};
    let (w, h) = (w.max(1), h.max(1));
    let (sw, sh) = src.dimensions();
    let k = (w as f32 / sw as f32).max(h as f32 / sh as f32);
    let (cw, ch) = (((w as f32 / k) as u32).clamp(1, sw), ((h as f32 / k) as u32).clamp(1, sh));
    let crop = imageops::crop_imm(src, (sw - cw) / 2, (sh - ch) / 2, cw, ch).to_image();
    let mut img = imageops::resize(&crop, w, h, FilterType::Triangle);
    if blur > 0. {
        img = imageops::fast_blur(&img, blur * w.max(h) as f32 / 40.);
    }
    let mut rgba = img.into_raw();
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 255;
    }
    Pixmap::from_vec(rgba, tiny_skia::IntSize::from_wh(w, h).expect("backdrop size")).expect("backdrop size")
}

thread_local! {
    static SOURCES: RefCell<Vec<(PathBuf, u32, Arc<image::RgbaImage>)>> = const { RefCell::new(Vec::new()) };
}

/// The decoded picture, at most `max_side` pixels on its long side (0 keeps it whole).
/// The screen and export each keep their own copy, so slider drags don't rescale a photo.
fn source(path: &Path, max_side: u32) -> Option<Arc<image::RgbaImage>> {
    SOURCES.with(|sources| {
        let mut sources = sources.borrow_mut();
        if let Some((_, _, img)) = sources.iter().find(|(p, m, _)| p == path && *m == max_side) {
            return Some(img.clone());
        }
        let (w, h, rgba, _) = std::fs::read(path).ok().and_then(|b| crate::export::decode_rgba(&b))?;
        let mut img = image::RgbaImage::from_raw(w, h, rgba)?;
        if max_side > 0 && w.max(h) > max_side {
            let k = max_side as f32 / w.max(h) as f32;
            let (nw, nh) = (((w as f32 * k) as u32).max(1), ((h as f32 * k) as u32).max(1));
            img = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
        }
        let img = Arc::new(img);
        sources.retain(|(_, m, _)| *m != max_side);
        sources.push((path.to_path_buf(), max_side, img.clone()));
        Some(img)
    })
}

/// A different order of the same stops each time.
fn shuffle(c: [u32; 4]) -> [u32; 4] {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let mut code = 1 + nanos as usize % 23;
    let (mut pool, mut out) = (c.to_vec(), [0; 4]);
    for (i, slot) in out.iter_mut().enumerate() {
        let radix = [6, 2, 1, 1][i];
        *slot = pool.remove(code / radix);
        code %= radix;
    }
    out
}

fn smooth(t: f32) -> f32 {
    t * t * (3. - 2. * t)
}

thread_local! {
    static CACHE: RefCell<Option<(Fill, (u32, u32), Arc<RenderImage>)>> = const { RefCell::new(None) };
}

fn wallpaper_path() -> PathBuf {
    crate::settings::path().with_file_name("Wallpaper.png")
}

/// Drops the decoded copies of a file that was rewritten in place.
fn forget(path: &Path, window: &mut Window) {
    SOURCES.with_borrow_mut(|sources| sources.retain(|(p, ..)| p != path));
    let stale = CACHE.with_borrow_mut(|cache| cache.take_if(|(fill, ..)| matches!(fill, Fill::Image(p, _) if p == path)));
    if let Some((_, _, old)) = stale {
        let _ = window.drop_image(old);
    }
}

/// The fill at low resolution for the screen; gradients scale up smoothly, pictures keep their proportions.
fn background_image(fill: &Fill, aspect: f32, window: &mut Window) -> Arc<RenderImage> {
    let size = match fill {
        Fill::Image(..) if aspect >= 1. => (768, ((768. / aspect) as u32).max(1)),
        Fill::Image(..) => (((768. * aspect) as u32).max(1), 768),
        _ => (256, 256),
    };
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((f, s, img)) = cache.as_ref()
            && f == fill
            && *s == size
        {
            return img.clone();
        }
        let mut bgra = render_fill(fill, size.0, size.1, 1024).take();
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        let img = Arc::new(crate::tiled::frame_image(size.0, size.1, bgra));
        if let Some((_, _, old)) = cache.replace((fill.clone(), size, img.clone())) {
            let _ = window.drop_image(old);
        }
        img
    })
}

/// The side panel, rebuilt from the editor's backdrop each render.
pub struct Panel {
    editor: WeakEntity<Editor>,
    sliders: [Entity<SliderState>; 5],
    solid: Entity<ColorPickerState>,
    stops: [Entity<ColorPickerState>; 4],
    blur: Entity<SliderState>,
    suggested: Fill,
    _subs: Vec<Subscription>,
}

const SLIDERS: [(&str, f32); 5] =
    [("Padding", 160.), ("Shadow", 1.), ("Inset", 120.), ("Inner radius", 32.), ("Outer radius", 48.)];

impl Panel {
    pub fn new(editor: WeakEntity<Editor>, b: &Backdrop, suggested: Fill, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let values = [b.padding, b.shadow, b.inset, b.inner_radius, b.outer_radius];
        let sliders = [0, 1, 2, 3, 4].map(|i| {
            let max = SLIDERS[i].1;
            cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(max)
                    .step(max / 100.)
                    .default_value(values[i])
            })
        });
        let solid = cx.new(|cx| ColorPickerState::new(window, cx));
        let stops = [0, 1, 2, 3].map(|_| cx.new(|cx| ColorPickerState::new(window, cx)));
        let blur = cx.new(|_| SliderState::new().min(0.).max(1.).step(0.01));
        let mut subs: Vec<Subscription> = sliders
            .iter()
            .enumerate()
            .map(|(i, s)| {
                cx.subscribe_in(s, window, move |this: &mut Self, _, e: &SliderEvent, window, cx| {
                    let (SliderEvent::Change(v) | SliderEvent::Release(v)) = e;
                    let v = v.start();
                    this.edit(window, cx, |b| match i {
                        0 => b.padding = v,
                        1 => b.shadow = v,
                        2 => b.inset = v,
                        3 => b.inner_radius = v,
                        _ => b.outer_radius = v,
                    });
                })
            })
            .collect();
        subs.push(cx.subscribe_in(&solid, window, |this: &mut Self, _, e: &ColorPickerEvent, window, cx| {
            if let ColorPickerEvent::Change(Some(c)) = e {
                let c = to_u32(*c);
                this.edit(window, cx, |b| b.fill = Fill::Solid(c));
            }
        }));
        for (i, stop) in stops.iter().enumerate() {
            subs.push(cx.subscribe_in(stop, window, move |this: &mut Self, _, e: &ColorPickerEvent, window, cx| {
                if let ColorPickerEvent::Change(Some(c)) = e {
                    let c = to_u32(*c);
                    this.edit(window, cx, |b| {
                        if let Fill::Gradient(stops, _) = &mut b.fill {
                            stops[i] = c;
                        }
                    });
                }
            }));
        }
        subs.push(cx.subscribe_in(&blur, window, |this: &mut Self, _, e: &SliderEvent, window, cx| {
            let (SliderEvent::Change(v) | SliderEvent::Release(v)) = e;
            let v = v.start();
            this.edit(window, cx, |b| {
                if let Fill::Image(_, blur) = &mut b.fill {
                    *blur = v;
                }
            });
        }));
        let mut panel = Self {
            editor,
            sliders,
            suggested,
            solid,
            stops,
            blur,
            _subs: subs,
        };
        panel.sync(&b.fill, window, cx);
        panel
    }

    fn sync(&mut self, fill: &Fill, window: &mut Window, cx: &mut Context<Self>) {
        match fill {
            Fill::Solid(c) => self.solid.update(cx, |s, cx| s.set_value(rgb(*c), window, cx)),
            Fill::Gradient(colors, _) => {
                for (stop, c) in self.stops.iter().zip(colors) {
                    stop.update(cx, |s, cx| s.set_value(rgb(*c), window, cx));
                }
            }
            Fill::Image(_, blur) => self.blur.update(cx, |s, cx| s.set_value(*blur, window, cx)),
        }
    }

    fn choose_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let _ = this.update_in(cx, |this, window, cx| this.set_image(path, window, cx));
        })
        .detach();
    }

    /// Saves what the desktop shows now, so dynamic and live wallpapers work too.
    fn use_wallpaper(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let path = wallpaper_path();
            let saved = match crate::window_capture::desktop_picture().await {
                Some(shot) => {
                    let path = path.clone();
                    cx.background_spawn(async move {
                        let png = crate::export::encode_bgra(shot.width, shot.height, shot.bgra, 1.);
                        !png.is_empty() && std::fs::write(&path, png).is_ok()
                    })
                    .await
                }
                None => false,
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if !saved {
                    crate::toast::show("Couldn't capture the wallpaper", cx);
                    return;
                }
                forget(&path, window);
                this.set_image(path, window, cx);
            });
        })
        .detach();
    }

    fn set_image(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if source(&path, 1024).is_none() {
            crate::toast::show("Couldn't open that image", cx);
            return;
        }
        let blur = match self.editor.upgrade().and_then(|e| e.read(cx).backdrop.clone()).map(|b| b.fill) {
            Some(Fill::Image(_, blur)) => blur,
            _ => 0.,
        };
        self.edit(window, cx, |b| b.fill = Fill::Image(path, blur));
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Backdrop)) {
        let fill = self.editor.update(cx, |editor, cx| {
            let mut b = editor.backdrop.clone()?;
            f(&mut b);
            crate::settings::update(cx, |s| s.backdrop = Some(b.clone()));
            let fill = b.fill.clone();
            editor.set_backdrop(Some(b), cx);
            Some(fill)
        });
        if let Ok(Some(fill)) = fill {
            self.sync(&fill, window, cx);
        }
    }
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(b) = self.editor.upgrade().and_then(|e| e.read(cx).backdrop.clone()) else {
            return div().id("backdrop-panel");
        };
        let (border, background, muted) = (cx.theme().border, cx.theme().background, cx.theme().muted_foreground);
        let section = |title: &'static str| div().text_xs().font_semibold().text_color(muted).child(title);
        let mut sliders = SLIDERS.iter().zip(&self.sliders).map(|((name, _), s)| {
            let label = h_flex().justify_between().child(div().text_xs().child(*name));
            let label = if *name == "Inset" {
                label.child(
                    Button::new("recenter")
                        .label("Recenter")
                        .xsmall()
                        .ghost()
                        .tooltip("Crop to the content so the inset is even on every side")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let _ = this.editor.update(cx, |e, cx| e.recenter(cx));
                        })),
                )
            } else {
                label
            };
            Some(v_flex().flex_1().min_w_0().gap_1().child(label).child(Slider::new(s)))
        })
        .collect::<Vec<_>>();
        let row = |a: Option<Div>, b: Option<Div>| h_flex().gap_4().children(a).children(b.or(Some(div().flex_1())));
        let rows = [
            row(sliders[0].take(), sliders[1].take()),
            row(sliders[2].take(), None),
            row(sliders[4].take(), sliders[3].take()),
        ];
        let kind = match b.fill {
            Fill::Gradient(_, k) => Some(k),
            _ => None,
        };
        let swatch = |id: usize, fill: Fill, cx: &mut Context<Self>| {
            let (selected, from, to, angle) = match (&fill, &b.fill) {
                (Fill::Image(..), _) => (false, 0, 0, 0.),
                (Fill::Solid(c), _) => (b.fill == fill, *c, *c, 0.),
                (&Fill::Gradient(c, k), current) => {
                    let sorted = |mut c: [u32; 4]| {
                        c.sort();
                        c
                    };
                    let selected = matches!(current, Fill::Gradient(cur, _) if sorted(*cur) == sorted(c));
                    (selected, c[0], c[3], if k == Kind::Vertical || k == Kind::Radial { 180. } else { 135. })
                }
            };
            div()
                .id(("fill", id))
                .size(px(30.))
                .rounded(px(7.))
                .border_2()
                .border_color(if selected { cx.theme().foreground } else { cx.theme().border })
                .bg(linear_gradient(angle, linear_color_stop(rgb(from), 0.), linear_color_stop(rgb(to), 1.)))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| {
                    let fill = fill.clone();
                    this.edit(window, cx, |b| b.fill = fill)
                }))
        };
        let mut solids: Vec<AnyElement> =
            SOLIDS.iter().enumerate().map(|(i, c)| swatch(i, Fill::Solid(*c), cx).into_any_element()).collect();
        solids.push(ColorPicker::new(&self.solid).anchor(Anchor::TopRight).into_any_element());
        let suggested = match self.suggested {
            Fill::Gradient(c, _) => c,
            _ => PALETTES[1],
        };
        let gradients: Vec<_> = std::iter::once(suggested)
            .chain(PALETTES)
            .enumerate()
            .map(|(i, c)| swatch(100 + i, Fill::Gradient(c, kind.unwrap_or_default()), cx))
            .collect();
        let reorder = |id: &'static str, icon: IconName, tip: &'static str, f: fn([u32; 4]) -> [u32; 4], cx: &mut Context<Self>| {
            Button::new(id).icon(Icon::new(icon)).xsmall().ghost().tooltip(tip).on_click(cx.listener(
                move |this, _, window, cx| {
                    this.edit(window, cx, |b| {
                        if let Fill::Gradient(c, _) = &mut b.fill {
                            *c = f(*c);
                        }
                    })
                },
            ))
        };
        let gradient_header = h_flex().justify_between().child(section("Gradient")).when(kind.is_some(), |row| {
            row.child(
                h_flex()
                    .gap_1()
                    .children(self.stops.iter().map(|s| ColorPicker::new(s).xsmall().anchor(Anchor::TopRight)))
                    .child(reorder("swap", IconName::ArrowLeftRight, "Reverse direction", |mut c| {
                        c.reverse();
                        c
                    }, cx))
                    .child(reorder("shuffle", IconName::Shuffle, "Shuffle colors", shuffle, cx)),
            )
        });
        let kinds = kind.map(|current| {
            h_flex().gap_1().children(KINDS.iter().enumerate().map(|(i, (k, name))| {
                let k = *k;
                Button::new(("kind", i))
                    .label(*name)
                    .xsmall()
                    .ghost()
                    .selected(current == k)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |b| {
                            if let Fill::Gradient(_, kind) = &mut b.fill {
                                *kind = k;
                            }
                        })
                    }))
            }))
        });
        let picture = match &b.fill {
            Fill::Image(path, _) => Some(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            _ => None,
        };
        let wallpaper = matches!(&b.fill, Fill::Image(path, _) if *path == wallpaper_path());
        let name = picture.clone().filter(|_| !wallpaper);
        let image_header = h_flex()
            .gap_2()
            .child(section("Image"))
            .children(name.clone().map(|name| div().flex_1().min_w_0().truncate().text_xs().text_color(muted).child(name)))
            .when(name.is_none(), |row| row.child(div().flex_1()))
            .child(
                Button::new("wallpaper")
                    .label("Wallpaper")
                    .xsmall()
                    .ghost()
                    .selected(wallpaper)
                    .on_click(cx.listener(|this, _, window, cx| this.use_wallpaper(window, cx))),
            )
            .child(
                Button::new("choose-image")
                    .label("Choose…")
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(|this, _, window, cx| this.choose_image(window, cx))),
            );
        let image_blur = picture.map(|_| {
            v_flex().gap_1().child(div().text_xs().child("Blur")).child(Slider::new(&self.blur))
        });
        let aspects = ASPECTS.iter().enumerate().map(|(i, (name, aspect, fits))| {
            let aspect = *aspect;
            Button::new(("aspect", i))
                .label(*name)
                .tooltip(*fits)
                .xsmall()
                .ghost()
                .selected(b.aspect == aspect)
                .on_click(cx.listener(move |this, _, window, cx| this.edit(window, cx, |b| b.aspect = aspect)))
        });
        div().id("backdrop-panel").h_full().overflow_y_scroll().child(
            v_flex()
                .w(px(260.))
                .min_h_full()
                .p_4()
                .gap_4()
                .border_l_1()
                .border_color(border)
                .bg(background)
                .child(
                    h_flex().justify_between().child(div().text_sm().font_semibold().child("Backdrop")).child(
                        Button::new("reset").label("Reset").xsmall().ghost().on_click(cx.listener(|this, _, window, cx| {
                            // Closing first makes the editor build a fresh panel, so the sliders reset too.
                            crate::settings::update(cx, |s| s.backdrop = None);
                            let _ = this.editor.update(cx, |e, cx| {
                                e.set_backdrop(None, cx);
                                e.toggle_backdrop_panel(window, cx);
                            });
                        })),
                    ),
                )
                .children(rows)
                .child(section("Aspect ratio"))
                .child(h_flex().flex_wrap().gap_1().children(aspects))
                .child(section("Solid"))
                .child(h_flex().flex_wrap().gap(px(6.)).children(solids))
                .child(gradient_header)
                .child(h_flex().flex_wrap().gap(px(6.)).children(gradients))
                .children(kinds)
                .child(image_header)
                .children(image_blur)
                .child(div().flex_1())
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("remove")
                                .label("Remove")
                                .small()
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let _ = this.editor.update(cx, |e, cx| e.set_backdrop(None, cx));
                                })),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("done")
                                .label("Done")
                                .small()
                                .primary()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let _ = this.editor.update(cx, |e, cx| e.toggle_backdrop_panel(window, cx));
                                })),
                        ),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{cover, shuffle};

    #[test]
    fn shuffle_reorders_the_same_stops() {
        let c = [1, 2, 3, 4];
        for _ in 0..50 {
            let mut s = shuffle(c);
            assert_ne!(s, c);
            s.sort();
            assert_eq!(s, c);
        }
    }

    #[test]
    fn cover_fills_any_aspect() {
        let src = image::RgbaImage::from_pixel(40, 10, image::Rgba([10, 20, 30, 0]));
        for (w, h) in [(100, 100), (300, 20), (7, 90)] {
            let p = cover(&src, w, h, 0.5);
            assert_eq!((p.width(), p.height()), (w, h));
            assert!(p.pixels().iter().all(|px| px.alpha() == 255));
        }
    }
}
