//! The editable document: objects in image pixel space, and the primitives that
//! both the screen painter and the exporter draw, so the two can't drift apart.

pub type P = (f32, f32);

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct R {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl R {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn from_points(a: P, b: P) -> Self {
        Self::new(
            a.0.min(b.0),
            a.1.min(b.1),
            (a.0 - b.0).abs(),
            (a.1 - b.1).abs(),
        )
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> P {
        (self.x + self.w / 2., self.y + self.h / 2.)
    }

    pub fn union(&self, o: &R) -> R {
        let (x, y) = (self.x.min(o.x), self.y.min(o.y));
        R::new(x, y, self.right().max(o.right()) - x, self.bottom().max(o.bottom()) - y)
    }

    pub fn intersect(&self, o: &R) -> Option<R> {
        let (x, y) = (self.x.max(o.x), self.y.max(o.y));
        let (r, b) = (self.right().min(o.right()), self.bottom().min(o.bottom()));
        (r > x && b > y).then(|| R::new(x, y, r - x, b - y))
    }

    pub fn inflate(&self, d: f32) -> R {
        R::new(self.x - d, self.y - d, self.w + 2. * d, self.h + 2. * d)
    }

    pub fn contains(&self, p: P) -> bool {
        p.0 >= self.x && p.0 <= self.right() && p.1 >= self.y && p.1 <= self.bottom()
    }

    pub fn contains_rect(&self, o: &R) -> bool {
        o.x >= self.x - 0.01
            && o.y >= self.y - 0.01
            && o.right() <= self.right() + 0.01
            && o.bottom() <= self.bottom() + 0.01
    }

    pub fn round_out(&self) -> R {
        let (x, y) = (self.x.floor(), self.y.floor());
        R::new(x, y, self.right().ceil() - x, self.bottom().ceil() - y)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Tool {
    Select,
    Arrow,
    Text,
    Rect,
    Oval,
    Line,
    Pen,
    Highlighter,
    Counter,
    Redact,
    Spotlight,
    Magnifier,
    /// Pasted images; not in the toolbar.
    Image,
}

pub const RED: u32 = 0xFF3B30;
pub const MEASURE: u32 = 0xFF2D78;

/// Kiri's own presets: four warm, three cool, and black and white for labels.
pub const PALETTE: [u32; 10] = [
    0xFF3B30, 0xFF9500, 0xFFD60A, 0x34C759, 0x00B5C8, 0x0A84FF, 0x7D5BFF, 0xFF4FA3, 0x1C1C1E,
    0xFFFFFF,
];

/// Pastel, because highlighters multiply.
pub const HIGHLIGHTS: [u32; 6] = [0xFFED9E, 0xD4F79C, 0xB2F1FC, 0xFFCBE4, 0xDFD3FF, 0xFFD4A6];

impl Tool {
    pub fn key(self) -> &'static str {
        match self {
            Tool::Select => "v",
            Tool::Arrow => "a",
            Tool::Text => "t",
            Tool::Rect => "r",
            Tool::Oval => "o",
            Tool::Line => "l",
            Tool::Pen => "d",
            Tool::Highlighter => "h",
            Tool::Counter => "c",
            Tool::Redact => "b",
            Tool::Spotlight => "s",
            Tool::Magnifier => "m",
            Tool::Image => "",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select and crop",
            Tool::Arrow => "Arrow",
            Tool::Text => "Text",
            Tool::Rect => "Rectangle",
            Tool::Oval => "Oval",
            Tool::Line => "Line",
            Tool::Pen => "Pen",
            Tool::Highlighter => "Highlighter",
            Tool::Counter => "Counter",
            Tool::Redact => "Blur and erase",
            Tool::Spotlight => "Spotlight",
            Tool::Magnifier => "Magnifier",
            Tool::Image => "Image",
        }
    }

    pub fn variants(self) -> &'static [&'static str] {
        match self {
            Tool::Arrow => &["Block", "Bold", "Thin", "Double"],
            Tool::Text => &["Label", "Plain"],
            Tool::Rect => &["Sketch", "Outline", "Tinted", "Solid"],
            Tool::Oval => &["Sketch", "Outline", "Tinted", "Solid"],
            Tool::Line => &["Solid", "Dashed"],
            Tool::Pen => &["Free", "Rectangle", "Oval"],
            Tool::Redact => &["Pixelate", "Pixelate text", "Erase", "Erase text", "Black box"],
            Tool::Spotlight => &["Rectangle", "Ellipse"],
            Tool::Magnifier => &["Round", "Square"],
            Tool::Image => &["Opaque", "See-through"],
            Tool::Counter => &["Bubble", "Circle"],
            Tool::Select | Tool::Highlighter => &[],
        }
    }

    /// Slider range and default, in points.
    pub fn size_range(self) -> Option<(f32, f32, f32)> {
        match self {
            Tool::Select => None,
            Tool::Arrow => Some((1., 23., 3.5)),
            Tool::Text => Some((10., 72., 18.)),
            Tool::Rect | Tool::Oval | Tool::Line | Tool::Pen => Some((1., 16., 3.)),
            Tool::Highlighter => Some((6., 48., 28.)),
            Tool::Counter => Some((8., 48., 18.)),
            Tool::Redact => Some((8., 40., 20.)),
            Tool::Spotlight => Some((0., 8., 2.5)),
            Tool::Magnifier => Some((1., 10., 2.5)),
            // Border width; none at zero.
            Tool::Image => Some((0., 10., 2.)),
        }
    }

    pub fn uses_color(self) -> bool {
        !matches!(self, Tool::Select | Tool::Redact)
    }

    pub fn default_style(self) -> Style {
        Style {
            color: match self {
                Tool::Highlighter => HIGHLIGHTS[0],
                _ => RED,
            },
            size: self.size_range().map_or(1., |r| r.2),
            variant: if self == Tool::Arrow { 1 } else { 0 },
        }
    }
}

/// What the property bar edits. `size` is in points.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Style {
    pub color: u32,
    pub size: f32,
    pub variant: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Arrow { from: P, to: P, bend: Option<P> },
    /// `caps` per end: 0 none, 1 arrowhead, 2 dot.
    Line { from: P, to: P, caps: [u8; 2] },
    Rect { a: P, b: P },
    Oval { a: P, b: P },
    Pen { points: Vec<P> },
    Highlighter { points: Vec<P> },
    Text { at: P, text: String },
    /// `angle` points the bubble's tail, in radians.
    Counter { at: P, n: u32, angle: f32 },
    Redact { a: P, b: P },
    Spotlight { a: P, b: P },
    /// The lens shows exactly the source circle, so the zoom is `lens_radius / radius`.
    Magnifier { src: P, radius: f32, lens: P, lens_radius: f32 },
    Measure { from: P, to: P },
    Image { a: P, b: P, src: Bitmap },
}

/// Pasted pixels, shared between undo snapshots: premultiplied BGRA.
#[derive(Clone)]
pub struct Bitmap(pub std::sync::Arc<BitmapData>);

pub struct BitmapData {
    pub w: u32,
    pub h: u32,
    pub bgra: Vec<u8>,
}

impl PartialEq for Bitmap {
    fn eq(&self, o: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &o.0)
    }
}

impl std::fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Bitmap({}x{})", self.0.w, self.0.h)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    pub id: u64,
    pub shape: Shape,
    pub color: u32,
    /// Stroke width, font size, or cell size in image pixels.
    pub size: f32,
    pub variant: u8,
}

impl Shape {
    pub fn tool(&self) -> Option<Tool> {
        Some(match self {
            Shape::Arrow { .. } => Tool::Arrow,
            Shape::Line { .. } => Tool::Line,
            Shape::Rect { .. } => Tool::Rect,
            Shape::Oval { .. } => Tool::Oval,
            Shape::Pen { .. } => Tool::Pen,
            Shape::Highlighter { .. } => Tool::Highlighter,
            Shape::Text { .. } => Tool::Text,
            Shape::Counter { .. } => Tool::Counter,
            Shape::Redact { .. } => Tool::Redact,
            Shape::Spotlight { .. } => Tool::Spotlight,
            Shape::Magnifier { .. } => Tool::Magnifier,
            Shape::Image { .. } => Tool::Image,
            Shape::Measure { .. } => return None,
        })
    }

    pub fn translate(&mut self, d: P, part: Part) {
        let mv = |p: &mut P| {
            p.0 += d.0;
            p.1 += d.1;
        };
        match self {
            Shape::Arrow { from, to, bend } => {
                mv(from);
                mv(to);
                if let Some(b) = bend {
                    mv(b);
                }
            }
            Shape::Line { from, to, .. } | Shape::Measure { from, to } => {
                mv(from);
                mv(to);
            }
            Shape::Rect { a, b }
            | Shape::Oval { a, b }
            | Shape::Redact { a, b }
            | Shape::Spotlight { a, b }
            | Shape::Image { a, b, .. } => {
                mv(a);
                mv(b);
            }
            Shape::Pen { points } | Shape::Highlighter { points } => points.iter_mut().for_each(mv),
            Shape::Text { at, .. } | Shape::Counter { at, .. } => mv(at),
            Shape::Magnifier { src, lens, .. } => match part {
                Part::Lens => mv(lens),
                _ => {
                    mv(src);
                    mv(lens);
                }
            },
        }
    }
}

/// Which piece of an object a click landed on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Body,
    Lens,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Handle {
    Start,
    End,
    Bend,
    /// Box handles clockwise from the top-left corner: 0 TL, 1 T, 2 TR, 3 R, 4 BR, 5 B, 6 BL, 7 L.
    Box(u8),
    Lens,
    /// Where a counter's tail points.
    Tail,
}

/// Measures one line of text: (text, font size in image px, bold) → width in image px.
pub type Measure<'a> = &'a dyn Fn(&str, f32, bool) -> f32;

pub struct Ctx<'a> {
    pub measure: Measure<'a>,
    /// Image pixels per point.
    pub scale: f32,
    pub unit_px: bool,
    /// Text line boxes in the capture, once OCR has found them.
    pub text: Option<std::rc::Rc<[R]>>,
}

pub const LINE_HEIGHT: f32 = 1.25;

#[derive(Clone, Debug)]
pub enum Seg {
    M(P),
    L(P),
    Q(P, P),
    C(P, P, P),
    Z,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PixelOp {
    Pixelate { cell: u32, seed: u64 },
    Erase,
    /// Magnify `src` into the prim's rect.
    Lens { src: R, round: bool },
}

#[derive(Clone, Debug)]
pub enum Prim {
    Fill {
        path: Vec<Seg>,
        color: u32,
        alpha: f32,
    },
    Stroke {
        path: Vec<Seg>,
        width: f32,
        color: u32,
        dash: Option<f32>,
    },
    /// One line of text; `at` is the top-left of its line box.
    Text {
        at: P,
        text: String,
        size: f32,
        color: u32,
        bold: bool,
    },
    /// Pixels computed from the original image; `key` names the cached patch.
    Pixels { key: u64, op: PixelOp, rect: R },
    /// A marker stroke multiplied into the pixels under it, so dark text stays dark.
    Highlight {
        key: u64,
        path: Vec<Seg>,
        width: f32,
        color: u32,
        rect: R,
    },
    /// A bitmap scaled into `rect`; `key` names its cached texture.
    Image {
        key: u64,
        src: Bitmap,
        rect: R,
        radius: f32,
        alpha: f32,
    },
}

fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}

fn add(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}

fn mul(a: P, k: f32) -> P {
    (a.0 * k, a.1 * k)
}

fn len(a: P) -> f32 {
    a.0.hypot(a.1)
}

fn norm(a: P) -> P {
    let l = len(a).max(1e-4);
    (a.0 / l, a.1 / l)
}

fn perp(a: P) -> P {
    (-a.1, a.0)
}

fn mid(a: P, b: P) -> P {
    ((a.0 + b.0) / 2., (a.1 + b.1) / 2.)
}

pub fn dist_to_segment(p: P, a: P, b: P) -> f32 {
    let ab = sub(b, a);
    let l2 = ab.0 * ab.0 + ab.1 * ab.1;
    let t = if l2 < 1e-6 {
        0.
    } else {
        (((p.0 - a.0) * ab.0 + (p.1 - a.1) * ab.1) / l2).clamp(0., 1.)
    };
    len(sub(p, add(a, mul(ab, t))))
}

fn dist_to_polyline(p: P, pts: &[P]) -> f32 {
    match pts {
        [] => f32::MAX,
        [a] => len(sub(p, *a)),
        _ => pts
            .windows(2)
            .map(|w| dist_to_segment(p, w[0], w[1]))
            .fold(f32::MAX, f32::min),
    }
}

fn bbox(pts: &[P]) -> R {
    let mut r = R::new(pts[0].0, pts[0].1, 0., 0.);
    for p in pts {
        r = r.union(&R::new(p.0, p.1, 0., 0.));
    }
    r
}

/// Snaps `p` around `origin` to the nearest 45°.
pub fn snap45(origin: P, p: P) -> P {
    let d = sub(p, origin);
    let a = (d.1.atan2(d.0) / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
    let l = len(d);
    (origin.0 + l * a.cos(), origin.1 + l * a.sin())
}

/// Box handle positions clockwise from the top-left corner, matching `Handle::Box`.
pub fn box_points(r: R) -> [P; 8] {
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    let (mx, my) = r.center();
    [(x0, y0), (mx, y0), (x1, y0), (x1, my), (x1, y1), (mx, y1), (x0, y1), (x0, my)]
}

/// The corners of `r` once box handle `i` is dragged to `p`; they cross when the drag passes the far edge.
pub fn resize_box(r: R, i: u8, p: P) -> (P, P) {
    let (mut x0, mut y0, mut x1, mut y1) = (r.x, r.y, r.right(), r.bottom());
    match i {
        0 => (x0, y0) = p,
        1 => y0 = p.1,
        2 => (x1, y0) = p,
        3 => x1 = p.0,
        4 => (x1, y1) = p,
        5 => y1 = p.1,
        6 => (x0, y1) = p,
        _ => x0 = p.0,
    }
    ((x0, y0), (x1, y1))
}

fn box_handles(r: R) -> impl Iterator<Item = (Handle, P)> {
    box_points(r).into_iter().enumerate().map(|(i, p)| (Handle::Box(i as u8), p))
}

/// Constrains a drag from `a` to `b` to a square.
pub fn square(a: P, b: P) -> P {
    let d = sub(b, a);
    let s = d.0.abs().max(d.1.abs());
    (a.0 + s * d.0.signum(), a.1 + s * d.1.signum())
}

/// The quadratic control point that makes the curve pass through `bend` at t = 0.5.
fn bend_ctrl(from: P, to: P, bend: P) -> P {
    sub(mul(bend, 2.), mid(from, to))
}

fn arrow_spine(from: P, to: P, bend: Option<P>) -> Vec<P> {
    match bend {
        None => vec![from, to],
        Some(b) => {
            let c = bend_ctrl(from, to, b);
            (0..=32)
                .map(|i| {
                    let t = i as f32 / 32.;
                    let u = 1. - t;
                    (
                        u * u * from.0 + 2. * u * t * c.0 + t * t * to.0,
                        u * u * from.1 + 2. * u * t * c.1 + t * t * to.1,
                    )
                })
                .collect()
        }
    }
}

/// Cuts the spine `head` px before its end. Returns the shortened spine and the
/// direction at the tip.
fn cut_spine(spine: &[P], head: f32) -> (Vec<P>, P) {
    let tip = *spine.last().unwrap();
    let mut out = spine.to_vec();
    let mut left = head;
    while out.len() >= 2 {
        let n = out.len();
        let seg = len(sub(out[n - 1], out[n - 2]));
        if seg >= left {
            let dir = norm(sub(out[n - 1], out[n - 2]));
            out[n - 1] = sub(out[n - 1], mul(dir, left));
            break;
        }
        left -= seg;
        out.pop();
    }
    if out.len() < 2 {
        out = vec![spine[0], spine[0]];
    }
    let base = *out.last().unwrap();
    let dir = if len(sub(tip, base)) > 1e-3 {
        norm(sub(tip, base))
    } else {
        norm(sub(tip, spine[0]))
    };
    (out, dir)
}

fn polygon(pts: &[P]) -> Vec<Seg> {
    let mut path = polyline(pts);
    path.push(Seg::Z);
    path
}

/// `to` moved `d` further along the ray from `from`, stopping short of `from`.
fn along(from: P, to: P, d: f32) -> P {
    let v = sub(to, from);
    let l = len(v);
    if l < 1e-3 {
        return to;
    }
    add(from, mul(v, (l + d).max(0.5) / l))
}

fn polyline(pts: &[P]) -> Vec<Seg> {
    let mut path = vec![Seg::M(pts[0])];
    path.extend(pts[1..].iter().map(|p| Seg::L(*p)));
    path
}

fn smooth(pts: &[P]) -> Vec<Seg> {
    if pts.len() < 3 {
        return polyline(pts);
    }
    let mut path = vec![Seg::M(pts[0])];
    for w in pts[1..].windows(2) {
        path.push(Seg::Q(w[0], mid(w[0], w[1])));
    }
    path.push(Seg::L(*pts.last().unwrap()));
    path
}

/// A rounded rectangle drawn as by hand, as the alternative's default: corners and edges
/// drift by about a point, and the stroke runs on past its start.
fn sketch_rect(r: R, seed: u64, scale: f32) -> Vec<Seg> {
    let mut j = Jitter(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let drift = (1.1 * scale).min(r.w.min(r.h) * 0.05);
    let corners: Vec<P> = [(r.x, r.y), (r.right(), r.y), (r.right(), r.bottom()), (r.x, r.bottom())]
        .iter()
        .map(|c| (c.0 + j.next(-drift, drift), c.1 + j.next(-drift, drift)))
        .collect();
    let base = (11. * scale).min(r.w / 3.).min(r.h / 3.);
    let radii: Vec<f32> = (0..4).map(|_| base * j.next(0.8, 1.2)).collect();
    let toward = |a: P, b: P, d: f32| add(a, mul(norm(sub(b, a)), d));
    let start = toward(corners[0], corners[1], radii[0]);
    let mut out = vec![Seg::M(start)];
    for i in 0..4 {
        let (a, b, c) = (corners[i], corners[(i + 1) % 4], corners[(i + 2) % 4]);
        let (from, to) = (toward(a, b, radii[i]), toward(b, a, radii[(i + 1) % 4]));
        let bow = mul(perp(norm(sub(to, from))), j.next(-1.5, 1.5) * drift);
        out.push(Seg::Q(add(mid(from, to), bow), to));
        out.push(Seg::Q(b, toward(b, c, radii[(i + 1) % 4])));
    }
    let run_on = toward(start, corners[1], 3. * scale + radii[0]);
    out.push(Seg::L((run_on.0, run_on.1 + j.next(-0.5, 0.5) * drift)));
    out
}

pub fn rect_path(r: R, radius: f32) -> Vec<Seg> {
    let k = radius.min(r.w / 2.).min(r.h / 2.).max(0.);
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    if k < 0.01 {
        return polygon(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]);
    }
    // 0.5523 places cubic controls so each corner is a quarter circle.
    let c = k * (1. - 0.5523);
    vec![
        Seg::M((x0 + k, y0)),
        Seg::L((x1 - k, y0)),
        Seg::C((x1 - c, y0), (x1, y0 + c), (x1, y0 + k)),
        Seg::L((x1, y1 - k)),
        Seg::C((x1, y1 - c), (x1 - c, y1), (x1 - k, y1)),
        Seg::L((x0 + k, y1)),
        Seg::C((x0 + c, y1), (x0, y1 - c), (x0, y1 - k)),
        Seg::L((x0, y0 + k)),
        Seg::C((x0, y0 + c), (x0 + c, y0), (x0 + k, y0)),
        Seg::Z,
    ]
}

/// An ellipse drawn as by hand in one loop, as the alternative's default: slightly
/// tilted, and the end runs past the start without meeting it.
fn sketch_oval(r: R, seed: u64) -> Vec<Seg> {
    let mut j = Jitter(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let (c, rx, ry) = (r.center(), r.w / 2., r.h / 2.);
    let start = j.next(1.3, 2.2);
    let sweep = std::f32::consts::TAU + j.next(0.25, 0.45);
    let (k0, k1) = (j.next(-0.07, -0.03), j.next(0.02, 0.07));
    let tilt = j.next(-0.06, 0.06);
    let n = 48;
    let pts: Vec<P> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let (a, k) = (start + sweep * t, 1. + k0 + (k1 - k0) * t);
            add(c, rotate((rx * k * a.cos(), ry * k * a.sin()), tilt))
        })
        .collect();
    smooth(&pts)
}

/// Reshapes a freehand stroke onto the superellipse of exponent `n` that fills its bounds
/// (2 an ellipse, 4 a rounded rectangle), keeping where it starts and ends, gap included.
fn tidy_stroke(points: &[P], n: f32) -> Vec<Seg> {
    use std::f32::consts::TAU;
    let r = bbox(points);
    let (c, rx, ry) = (r.center(), (r.w / 2.).max(0.5), (r.h / 2.).max(0.5));
    let mut angles: Vec<f32> = Vec::with_capacity(points.len());
    for p in points {
        let mut a = ((p.1 - c.1) / ry).atan2((p.0 - c.0) / rx);
        if let Some(&last) = angles.last() {
            a += ((last - a) / TAU).round() * TAU;
        }
        angles.push(a);
    }
    let (a0, a1) = (angles[0], *angles.last().unwrap());
    let e = 2. / n;
    let steps = 64;
    let pts: Vec<P> = (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            let (co, si) = (a.cos(), a.sin());
            (c.0 + rx * co.signum() * co.abs().powf(e), c.1 + ry * si.signum() * si.abs().powf(e))
        })
        .collect();
    smooth(&pts)
}

/// The alternative draws its ovals as superellipses, flatter at the sides than an ellipse.
fn squircle(r: R) -> Vec<Seg> {
    let (c, rx, ry) = (r.center(), r.w / 2., r.h / 2.);
    let e = 2. / 3.;
    let n = 96;
    let pts: Vec<P> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let (co, si) = (a.cos(), a.sin());
            (c.0 + rx * co.signum() * co.abs().powf(e), c.1 + ry * si.signum() * si.abs().powf(e))
        })
        .collect();
    polygon(&pts)
}

/// How far a counter's tail reaches, in radii.
const TAIL: f32 = 1.47;

/// A disc of radius `r` with a pointed tail toward `angle`, as the alternative's counters.
fn bubble(c: P, r: f32, angle: f32) -> Vec<Seg> {
    // Asymmetric like a speech-bubble tail: one side drops almost straight along the circle's tangent,
    // the other sags slightly into the tip.
    let (sa, sb) = (std::f32::consts::FRAC_PI_4, 0.45);
    let dir = |a: f32| (a.cos(), a.sin());
    let tangent = |a: f32| (-a.sin(), a.cos());
    let tip = add(c, mul(dir(angle), r * TAIL));
    let (a0, a1) = (angle + sa, angle - sb + std::f32::consts::TAU);
    let steps = 40;
    let mut out = vec![Seg::M(add(c, mul(dir(a0), r)))];
    for i in 1..=steps {
        out.push(Seg::L(add(c, mul(dir(a0 + (a1 - a0) * i as f32 / steps as f32), r))));
    }
    let pb = add(c, mul(dir(a1), r));
    let pa = add(c, mul(dir(a0), r));
    out.push(Seg::C(add(pb, mul(tangent(a1), r * 0.25)), add(tip, mul(sub(pb, tip), 0.3)), tip));
    out.push(Seg::Q(sub(pa, mul(tangent(a0), r * 0.85)), pa));
    out.push(Seg::Z);
    out
}

fn ellipse_path(r: R) -> Vec<Seg> {
    let (cx, cy) = r.center();
    let (rx, ry) = (r.w / 2., r.h / 2.);
    let (kx, ky) = (rx * 0.5523, ry * 0.5523);
    vec![
        Seg::M((cx + rx, cy)),
        Seg::C((cx + rx, cy + ky), (cx + kx, cy + ry), (cx, cy + ry)),
        Seg::C((cx - kx, cy + ry), (cx - rx, cy + ky), (cx - rx, cy)),
        Seg::C((cx - rx, cy - ky), (cx - kx, cy - ry), (cx, cy - ry)),
        Seg::C((cx + kx, cy - ry), (cx + rx, cy - ky), (cx + rx, cy)),
        Seg::Z,
    ]
}

/// Black or white, whichever reads better on `bg`.
pub fn contrast(bg: u32) -> u32 {
    let [r, g, b] = [(bg >> 16) & 255, (bg >> 8) & 255, bg & 255].map(|c| c as f32 / 255.);
    if 0.299 * r + 0.587 * g + 0.114 * b > 0.62 {
        0x111111
    } else {
        0xFFFFFF
    }
}

impl Object {
    /// A spotlight's outline, which is also the hole it cuts in the dimming.
    fn spotlight_path(&self, a: P, b: P) -> Vec<Seg> {
        let r = R::from_points(a, b);
        if self.variant == 1 { ellipse_path(r) } else { rect_path(r, self.size * 0.6) }
    }

    fn spine(&self) -> Vec<P> {
        match &self.shape {
            Shape::Arrow { from, to, bend } => arrow_spine(*from, *to, *bend),
            _ => Vec::new(),
        }
    }

    /// How far the arrow's ink reaches past its spine.
    fn arrow_reach(&self, scale: f32) -> f32 {
        let s = self.size / scale;
        scale
            * match self.variant {
                0 => (15.4 + 4.72 * s) / 2.,
                1 => 23. + 2.2 * s,
                _ => (6.8 + 2.47 * s) * 0.8 + 1.7 * s,
            }
    }

    /// Proportions follow the alternative's styles, measured at its default and largest sizes:
    /// each dimension is `a + b·size` points.
    fn arrow_prims(&self, scale: f32) -> Vec<Prim> {
        let spine = self.spine();
        let s = self.size / scale;
        let pt = |a: f32, b: f32| (a + b * s) * scale;
        let length = polyline_len(&spine);
        let tip = *spine.last().unwrap();
        let fill = |pts: Vec<P>| Prim::Fill {
            path: polygon(&pts),
            color: self.color,
            alpha: 1.,
        };
        match self.variant {
            0 => {
                let k = (length * 0.6 / pt(17.6, 4.97)).min(1.);
                let head = pt(17.6, 4.97) * k;
                let half = |a: f32, b: f32| pt(a, b) * k.max(0.5) / 2.;
                vec![fill(block_arrow(&spine, half(5.3, 1.42), half(7.4, 1.85), half(15.4, 4.72), head))]
            }
            1 => {
                let mut j = Jitter(self.id.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
                let barb = pt(22.6, 1.62).min(length * 0.5);
                let barb_w = pt(0.5, 0.76);
                let (tail_w, end_w) = (pt(1., 0.76), pt(0.83, 0.51));
                let gap = barb * j.next(0., 0.5);
                let (shaft, dir) = cut_spine(&spine, gap);
                let shaft_len = polyline_len(&shaft);
                let wobble = (length * 0.025).min(barb * 0.3);
                let (a1, a2) = (wobble * j.next(-1., 1.), wobble * j.next(-0.5, 0.5));
                let steps = 24;
                let (mut pts, mut halves) = (Vec::with_capacity(steps + 1), Vec::with_capacity(steps + 1));
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    let (p, d) = point_along(&shaft, shaft_len * t);
                    let pi = std::f32::consts::PI;
                    let off = a1 * (pi * t).sin() + a2 * (2. * pi * t).sin();
                    pts.push(add(p, mul(perp(d), off)));
                    halves.push((tail_w + (end_w - tail_w) * t) / 2.);
                }
                let mut out = vec![
                    fill(ribbon(&pts, &halves, dir)),
                    fill(disc(pts[0], tail_w / 2.)),
                    fill(disc(pts[steps], end_w / 2.)),
                ];
                for side in [1., -1.] {
                    let angle = (40. + j.next(-5., 5.)).to_radians() * side;
                    let l = barb * j.next(0.9, 1.1);
                    let end = add(tip, mul(rotate(mul(dir, -1.), angle), l));
                    let ctrl = add(mid(tip, end), mul(dir, l * j.next(0.1, 0.2)));
                    out.push(Prim::Stroke {
                        path: vec![Seg::M(tip), Seg::Q(ctrl, end)],
                        width: barb_w,
                        color: self.color,
                        dash: None,
                    });
                }
                out
            }
            v => {
                let angle = 50f32.to_radians();
                let half = self.size.max(scale * 0.5) / 2.;
                let arm = pt(6.8, 2.47).min(length * 0.45);
                // The end point sits on the chevron's centerline corner, so the point overshoots it.
                let apex = half / angle.sin();
                let n = spine.len();
                let dir = norm(sub(spine[n - 1], spine[n - 2]));
                let mut out = vec![fill(chevron(add(tip, mul(dir, apex)), dir, arm, half, angle))];
                if v == 3 {
                    let back = norm(sub(spine[0], spine[1]));
                    out.push(fill(chevron(add(spine[0], mul(back, apex)), back, arm, half, angle)));
                }
                out.push(fill(ribbon(&spine, &vec![half; n], dir)));
                out
            }
        }
    }

    pub fn text_box(&self, cx: &Ctx) -> Option<R> {
        let Shape::Text { at, text } = &self.shape else {
            return None;
        };
        let lines: Vec<&str> = text.split('\n').collect();
        let w = lines
            .iter()
            .map(|l| (cx.measure)(l, self.size, self.variant == 0))
            .fold(0f32, f32::max)
            .max(self.size * 0.6);
        let h = lines.len() as f32 * self.size * LINE_HEIGHT;
        let (px, py) = self.label_padding();
        Some(R::new(at.0, at.1, w + 2. * px, h + 2. * py))
    }

    pub fn label_padding(&self) -> (f32, f32) {
        match self.variant {
            0 => (self.size * 0.4, self.size * 0.18),
            _ => (0., 0.),
        }
    }

    pub fn measure_label(&self, cx: &Ctx) -> String {
        let Shape::Measure { from, to } = &self.shape else {
            return String::new();
        };
        let d = len(sub(*to, *from));
        if cx.unit_px {
            format!("{} px", d.round())
        } else {
            format!("{} pt", (d / cx.scale).round())
        }
    }

    /// Visual bounds, used for canvas growth and selection outlines.
    pub fn bounds(&self, cx: &Ctx) -> R {
        let pad = self.size / 2.;
        match &self.shape {
            // Tight to the drawn head, so the canvas grows to the tip as in the alternative.
            Shape::Arrow { .. } => prims_bounds(&self.arrow_prims(cx.scale)),
            Shape::Line { from, to, caps } => bbox(&[*from, *to]).inflate(pad + if *caps == [0, 0] { 0. } else { 4. * self.size }),
            Shape::Measure { from, to } => bbox(&[*from, *to]).inflate(6. * cx.scale),
            Shape::Oval { .. } if self.variant == 0 => prims_bounds(&self.prims(cx)),
            Shape::Rect { a, b } | Shape::Oval { a, b } => R::from_points(*a, *b).inflate(pad),
            Shape::Redact { a, b } | Shape::Image { a, b, .. } => R::from_points(*a, *b),
            Shape::Spotlight { a, b } => R::from_points(*a, *b).inflate(pad),
            Shape::Pen { points } | Shape::Highlighter { points } => bbox(points).inflate(pad),
            Shape::Text { .. } => self.text_box(cx).unwrap_or_default(),
            Shape::Counter { at, angle, .. } => {
                let disc = R::new(at.0, at.1, 0., 0.).inflate(self.size);
                let tip = add(*at, mul((angle.cos(), angle.sin()), self.size * TAIL));
                if self.variant == 0 { disc.union(&R::new(tip.0, tip.1, 0., 0.)) } else { disc }
            }
            Shape::Magnifier { src, radius, lens, lens_radius } => R::new(src.0, src.1, 0., 0.)
                .inflate(*radius)
                .union(&R::new(lens.0, lens.1, 0., 0.).inflate(*lens_radius))
                .inflate(pad),
        }
    }

    /// The box that snaps and is snapped to. Freehand strokes, text, and counters don't snap.
    pub fn snap_box(&self) -> Option<R> {
        match &self.shape {
            Shape::Rect { a, b }
            | Shape::Oval { a, b }
            | Shape::Redact { a, b }
            | Shape::Spotlight { a, b }
            | Shape::Image { a, b, .. } => Some(R::from_points(*a, *b)),
            Shape::Arrow { from, to, .. } | Shape::Line { from, to, .. } | Shape::Measure { from, to } => {
                Some(R::from_points(*from, *to))
            }
            Shape::Magnifier { src, radius, .. } => Some(R::new(src.0, src.1, 0., 0.).inflate(*radius)),
            _ => None,
        }
    }

    pub fn hit(&self, p: P, tol: f32, cx: &Ctx) -> Option<Part> {
        let near = |d: f32| d <= tol + self.size / 2.;
        let hit = match &self.shape {
            Shape::Arrow { .. } => dist_to_polyline(p, &self.spine()) <= tol + self.arrow_reach(cx.scale) * 0.5,
            Shape::Line { from, to, .. } => near(dist_to_segment(p, *from, *to)),
            Shape::Measure { from, to } => dist_to_segment(p, *from, *to) <= tol + 4. * cx.scale,
            Shape::Rect { a, b } => {
                let r = R::from_points(*a, *b);
                if self.variant >= 2 {
                    r.inflate(tol).contains(p)
                } else {
                    near(rect_edge_dist(r, p))
                }
            }
            Shape::Oval { a, b } => {
                let r = R::from_points(*a, *b);
                if self.variant >= 2 {
                    ellipse_inside(r.inflate(tol), p)
                } else {
                    near(ellipse_edge_dist(r, p))
                }
            }
            Shape::Pen { points } => match self.variant {
                0 => near(dist_to_polyline(p, points)),
                1 => near(rect_edge_dist(bbox(points), p)),
                _ => near(ellipse_edge_dist(bbox(points), p)),
            },
            Shape::Highlighter { points } => near(dist_to_polyline(p, points)),
            Shape::Text { .. } => self.text_box(cx).is_some_and(|r| r.inflate(tol).contains(p)),
            Shape::Counter { at, .. } => len(sub(p, *at)) <= self.size + tol,
            Shape::Redact { a, b } | Shape::Image { a, b, .. } => R::from_points(*a, *b).inflate(tol).contains(p),
            Shape::Spotlight { a, b } => {
                let r = R::from_points(*a, *b);
                near(if self.variant == 1 {
                    ellipse_edge_dist(r, p)
                } else {
                    rect_edge_dist(r, p)
                }) || tol * 2. >= rect_edge_dist(r, p)
            }
            Shape::Magnifier { src, radius, lens, lens_radius } => {
                if len(sub(p, *lens)) <= lens_radius + tol {
                    return Some(Part::Lens);
                }
                len(sub(p, *src)) <= radius + tol
            }
        };
        hit.then_some(Part::Body)
    }

    pub fn handles(&self) -> Vec<(Handle, P)> {
        match &self.shape {
            Shape::Arrow { from, to, bend } => vec![
                (Handle::Start, *from),
                (Handle::End, *to),
                (Handle::Bend, bend.unwrap_or(mid(*from, *to))),
            ],
            // Line handles sit past the ends, as in the alternative, so they don't hide the caps.
            Shape::Line { from, to, .. } => {
                let gap = self.size * 2.4;
                vec![(Handle::Start, along(*to, *from, gap)), (Handle::End, along(*from, *to, gap))]
            }
            Shape::Measure { from, to } => vec![(Handle::Start, *from), (Handle::End, *to)],
            Shape::Rect { a, b }
            | Shape::Oval { a, b }
            | Shape::Redact { a, b }
            | Shape::Spotlight { a, b }
            | Shape::Image { a, b, .. } => box_handles(R::from_points(*a, *b)).collect(),
            // Box handles stand off the source circle so they don't break up its outline.
            Shape::Magnifier { src, radius, lens, lens_radius } => box_handles(R::new(src.0, src.1, 0., 0.).inflate(radius + self.size * 3.))
                .chain([(Handle::Lens, (lens.0 + lens_radius, lens.1))])
                .collect(),
            Shape::Counter { at, angle, .. } if self.variant == 0 => {
                vec![(Handle::Tail, add(*at, mul((angle.cos(), angle.sin()), self.size * 1.95)))]
            }
            // Dragging it draws on from the end, as in the alternative.
            Shape::Highlighter { points } => vec![(Handle::End, points[points.len() - 1])],
            _ => Vec::new(),
        }
    }

    pub fn drag_handle(&mut self, handle: Handle, p: P, shift: bool) {
        match (&mut self.shape, handle) {
            (Shape::Arrow { from, to, .. } | Shape::Measure { from, to }, Handle::Start) => {
                *from = if shift { snap45(*to, p) } else { p }
            }
            (Shape::Arrow { from, to, .. } | Shape::Measure { from, to }, Handle::End) => {
                *to = if shift { snap45(*from, p) } else { p }
            }
            (Shape::Arrow { from, to, bend }, Handle::Bend) => {
                // Close to the straight line, the arrow snaps back to straight.
                *bend = (dist_to_segment(p, *from, *to) > 4.).then_some(p);
            }
            (Shape::Line { from, to, .. }, Handle::Start) => {
                let p = if shift { snap45(*to, p) } else { p };
                *from = along(*to, p, -self.size * 2.4);
            }
            (Shape::Line { from, to, .. }, Handle::End) => {
                let p = if shift { snap45(*from, p) } else { p };
                *to = along(*from, p, -self.size * 2.4);
            }
            (Shape::Image { a, b, .. }, Handle::Box(i)) if i % 2 == 0 => {
                let r = R::from_points(*a, *b);
                let anchor = box_points(r)[(i as usize + 4) % 8];
                let (dx, dy) = sub(p, anchor);
                let k = (dx.abs() / r.w.max(1.)).max(dy.abs() / r.h.max(1.)).max(1. / r.w.min(r.h).max(1.));
                *a = anchor;
                *b = (anchor.0 + r.w * k * dx.signum(), anchor.1 + r.h * k * dy.signum());
            }
            (
                Shape::Rect { a, b }
                | Shape::Oval { a, b }
                | Shape::Redact { a, b }
                | Shape::Spotlight { a, b }
                | Shape::Image { a, b, .. },
                Handle::Box(i),
            ) => {
                let r = R::from_points(*a, *b);
                (*a, *b) = if shift && i % 2 == 0 {
                    let anchor = box_points(r)[(i as usize + 4) % 8];
                    (anchor, square(anchor, p))
                } else {
                    resize_box(r, i, p)
                };
            }
            (Shape::Counter { at, angle, .. }, Handle::Tail) => {
                let d = sub(p, *at);
                *angle = d.1.atan2(d.0);
            }
            // The opposite handle stays put and the circle takes the larger side of the dragged box.
            (Shape::Magnifier { src, radius, .. }, Handle::Box(i)) => {
                let m = self.size * 3.;
                let anchor = box_points(R::new(src.0, src.1, 0., 0.).inflate(*radius + m))[(i as usize + 4) % 8];
                let d = sub(p, anchor);
                let (mx, my) = (i % 4 != 1, i % 4 != 3);
                let side = (if mx { d.0.abs() } else { 0. }).max(if my { d.1.abs() } else { 0. }).max(2. * m + 4.);
                let dir = |on: bool, v: f32| if on { v.signum() * side / 2. } else { 0. };
                *src = add(anchor, (dir(mx, d.0), dir(my, d.1)));
                *radius = side / 2. - m;
            }
            (Shape::Highlighter { points }, Handle::End) => {
                if len(sub(p, points[points.len() - 1])) > 0.5 {
                    points.push(p);
                }
            }
            (Shape::Magnifier { lens, lens_radius, .. }, Handle::Lens) => {
                *lens_radius = len(sub(p, *lens)).max(4.);
            }
            _ => {}
        }
    }

    pub fn prims(&self, cx: &Ctx) -> Vec<Prim> {
        let (color, w) = (self.color, self.size);
        let stroke = |path: Vec<Seg>, width: f32| Prim::Stroke {
            path,
            width,
            color,
            dash: None,
        };
        let fill = |path: Vec<Seg>, alpha: f32| Prim::Fill { path, color, alpha };
        match &self.shape {
            Shape::Arrow { .. } => self.arrow_prims(cx.scale),
            Shape::Line { from, to, caps } => {
                let mut out = vec![Prim::Stroke {
                    path: polyline(&[*from, *to]),
                    width: w,
                    color,
                    dash: (self.variant == 1).then_some(w * 2.6),
                }];
                for (end, other, cap) in [(*from, *to, caps[0]), (*to, *from, caps[1])] {
                    let back = sub(other, end);
                    let d = len(back);
                    if d < 1e-3 {
                        continue;
                    }
                    let u = mul(back, 1. / d);
                    let arm = |a: f32| add(end, mul((u.0 * a.cos() - u.1 * a.sin(), u.0 * a.sin() + u.1 * a.cos()), w * 3.8));
                    match cap {
                        1 => out.push(stroke(polyline(&[arm(0.8), end, arm(-0.8)]), w)),
                        2 => out.push(fill(ellipse_path(R::new(end.0, end.1, 0., 0.).inflate(w * 1.2)), 1.)),
                        _ => {}
                    }
                }
                out
            }
            Shape::Rect { a, b } => {
                let r = R::from_points(*a, *b);
                let radius = w * 0.5;
                match self.variant {
                    0 => vec![stroke(sketch_rect(r, self.id, cx.scale), w)],
                    2 => vec![fill(rect_path(r, radius), 0.4), stroke(rect_path(r, radius), w)],
                    3 => vec![fill(rect_path(r.inflate(w / 2.), radius + w / 2.), 1.)],
                    _ => vec![stroke(rect_path(r, radius), w)],
                }
            }
            Shape::Oval { a, b } => {
                let r = R::from_points(*a, *b);
                match self.variant {
                    0 => vec![stroke(sketch_oval(r, self.id), w)],
                    2 => vec![fill(squircle(r), 0.4), stroke(squircle(r), w)],
                    3 => vec![fill(squircle(r.inflate(w / 2.)), 1.)],
                    _ => vec![stroke(squircle(r), w)],
                }
            }
            Shape::Pen { points } => match self.variant {
                1 => vec![stroke(tidy_stroke(points, 4.), w)],
                2 => vec![stroke(tidy_stroke(points, 2.), w)],
                _ if points.len() == 1 => vec![fill(ellipse_path(R::new(points[0].0, points[0].1, 0., 0.).inflate(w / 2.)), 1.)],
                _ => vec![stroke(smooth(&steady(points, cx.scale)), w)],
            },
            Shape::Highlighter { points } => vec![Prim::Highlight {
                key: self.id,
                path: smooth(&marker_ends(&level(&steady(points, cx.scale), w), w * 0.3)),
                width: w,
                color,
                rect: bbox(points).inflate(w / 2. + 1.),
            }],
            Shape::Text { text, .. } => {
                let Some(r) = self.text_box(cx) else {
                    return Vec::new();
                };
                let (px, py) = self.label_padding();
                let mut out = Vec::new();
                let text_color = if self.variant == 0 {
                    out.push(fill(rect_path(r, w * 0.28), 1.));
                    contrast(color)
                } else {
                    color
                };
                for (i, line) in text.split('\n').enumerate() {
                    out.push(Prim::Text {
                        at: (r.x + px, r.y + py + i as f32 * w * LINE_HEIGHT),
                        text: line.to_string(),
                        size: w,
                        color: text_color,
                        bold: self.variant == 0,
                    });
                }
                out
            }
            Shape::Counter { at, n, angle } => {
                let label = n.to_string();
                let size = w;
                let tw = (cx.measure)(&label, size, false);
                let badge = match self.variant {
                    0 => bubble(*at, w, *angle),
                    _ => ellipse_path(R::new(at.0, at.1, 0., 0.).inflate(w)),
                };
                vec![
                    fill(badge, 1.),
                    Prim::Text {
                        at: (at.0 - tw / 2., at.1 - size * LINE_HEIGHT / 2.),
                        text: label,
                        size,
                        color: contrast(color),
                        bold: false,
                    },
                ]
            }
            Shape::Redact { a, b } => {
                let r = R::from_points(*a, *b);
                let op = match self.variant {
                    4 => {
                        return vec![Prim::Fill {
                            path: rect_path(r, 0.),
                            color: 0x000000,
                            alpha: 1.,
                        }];
                    }
                    2 | 3 => PixelOp::Erase,
                    _ => PixelOp::Pixelate {
                        cell: w.round().max(2.) as u32,
                        seed: self.id,
                    },
                };
                // Until OCR answers, a text mode covers the whole area rather than nothing.
                let text = (self.variant % 2 == 1).then(|| cx.text.clone()).flatten();
                let Some(lines) = text else {
                    return vec![Prim::Pixels { key: self.id, op, rect: r }];
                };
                // OCR boxes clip ascenders and descenders. Erase needs a wider margin to sample
                // clean background, and merges overlapping boxes so it never samples its edge
                // colours from a neighbouring line's glyphs. Pixelate stays tight, as in the
                // alternative, and merges only boxes on the same line.
                let erase = op == PixelOp::Erase;
                let joins = |a: &R, b: &R| a.intersect(b).is_some_and(|i| erase || i.h > a.h.min(b.h) / 2.);
                let mut areas: Vec<R> = Vec::new();
                for line in lines.iter() {
                    let margin = if erase { (line.h * 0.25).max(2. * cx.scale) } else { line.h * 0.15 };
                    let Some(mut a) = line.inflate(margin).intersect(&r) else {
                        continue;
                    };
                    while let Some(i) = areas.iter().position(|b| joins(&a, b)) {
                        a = a.union(&areas.swap_remove(i));
                    }
                    areas.push(a);
                }
                areas
                    .into_iter()
                    .enumerate()
                    .map(|(i, rect)| Prim::Pixels {
                        key: self.id | (i as u64 + 1) << 48,
                        op,
                        rect,
                    })
                    .collect()
            }
            // The dimming is shared by all spotlights; see `spotlight_dim`.
            Shape::Spotlight { a, b } if w > 0. => vec![stroke(self.spotlight_path(*a, *b), w)],
            Shape::Spotlight { .. } => Vec::new(),
            Shape::Magnifier { src, radius, lens, lens_radius } => {
                let round = self.variant == 0;
                let src_r = R::new(src.0, src.1, 0., 0.).inflate(*radius);
                let lens_r = R::new(lens.0, lens.1, 0., 0.).inflate(*lens_radius);
                let shape = |r: R| if round { ellipse_path(r) } else { rect_path(r, r.w * 0.12) };
                let mut out = Vec::new();
                if len(sub(*lens, *src)) > radius + lens_radius {
                    let d = norm(sub(*lens, *src));
                    out.push(stroke(polyline(&[add(*src, mul(d, *radius)), sub(*lens, mul(d, *lens_radius))]), w));
                }
                out.push(stroke(shape(src_r), w));
                out.push(Prim::Pixels { key: self.id, op: PixelOp::Lens { src: src_r, round }, rect: lens_r });
                out.push(stroke(shape(lens_r), w));
                out
            }
            Shape::Image { a, b, src } => {
                let r = R::from_points(*a, *b);
                let alpha = if self.variant == 1 { 0.5 } else { 1. };
                let w = w.min(r.w / 2.).min(r.h / 2.);
                if w <= 0. {
                    return vec![Prim::Image { key: self.id, src: src.clone(), rect: r, radius: 0., alpha }];
                }
                // The border frames the image instead of covering it, so see-through
                // mode fades the whole object evenly.
                let (outer, inner) = (w * 1.5, w * 0.5);
                let mut ring = rect_path(r, outer);
                ring.extend(rect_path(r.inflate(-w), inner));
                vec![
                    Prim::Image { key: self.id, src: src.clone(), rect: r.inflate(-w), radius: inner, alpha },
                    Prim::Fill { path: ring, color, alpha },
                ]
            }
            Shape::Measure { from, to } => {
                let d = norm(sub(*to, *from));
                let n = mul(perp(d), 5. * cx.scale);
                let lw = 1.5 * cx.scale;
                let label = self.measure_label(cx);
                let size = 11. * cx.scale;
                let tw = (cx.measure)(&label, size, true);
                let (pad_x, pad_y) = (size * 0.45, size * 0.2);
                let m = mid(*from, *to);
                let lh = size * LINE_HEIGHT;
                let pill = R::new(m.0 - tw / 2. - pad_x, m.1 - lh / 2. - pad_y, tw + 2. * pad_x, lh + 2. * pad_y);
                vec![
                    stroke(polyline(&[*from, *to]), lw),
                    stroke(polyline(&[add(*from, n), sub(*from, n)]), lw),
                    stroke(polyline(&[add(*to, n), sub(*to, n)]), lw),
                    fill(rect_path(pill, pill.h / 2.), 1.),
                    Prim::Text {
                        at: (pill.x + pad_x, pill.y + pad_y),
                        text: label,
                        size,
                        color: contrast(color),
                        bold: true,
                    },
                ]
            }
        }
    }
}

/// A filled block arrow along `spine`; the widths are half-widths.
fn block_arrow(spine: &[P], tail: f32, neck: f32, wing: f32, head: f32) -> Vec<P> {
    let tip = *spine.last().unwrap();
    let (shaft, dir) = cut_spine(spine, head);
    let total = polyline_len(&shaft).max(1e-3);
    let mut run = 0.;
    let halves: Vec<f32> = (0..shaft.len())
        .map(|i| {
            if i > 0 {
                run += len(sub(shaft[i], shaft[i - 1]));
            }
            tail + (neck - tail) * (run / total)
        })
        .collect();
    let (left, right) = ribbon_sides(&shaft, &halves, dir);
    let base = *shaft.last().unwrap();
    let n = mul(perp(dir), wing);
    let mut pts = left;
    pts.push(add(base, n));
    pts.push(tip);
    pts.push(sub(base, n));
    pts.extend(right.into_iter().rev());
    pts
}

/// Open chevron head with a mitered point at `tip`; arms run back at ±`angle` from `dir`.
fn chevron(tip: P, dir: P, arm: f32, half: f32, angle: f32) -> Vec<P> {
    let back = mul(dir, -1.);
    let (u1, u2) = (rotate(back, angle), rotate(back, -angle));
    let (n1, n2) = (mul(perp(u1), -1.), perp(u2));
    let inner = add(tip, mul(back, 2. * half / angle.sin()));
    vec![
        tip,
        add(tip, mul(u1, arm)),
        add(add(tip, mul(u1, arm)), mul(n1, 2. * half)),
        inner,
        add(add(tip, mul(u2, arm)), mul(n2, 2. * half)),
        add(tip, mul(u2, arm)),
    ]
}

/// Left and right edges of a variable-width band along `pts`.
fn ribbon_sides(pts: &[P], halves: &[f32], fallback: P) -> (Vec<P>, Vec<P>) {
    let mut left = Vec::with_capacity(pts.len());
    let mut right = Vec::with_capacity(pts.len());
    for i in 0..pts.len() {
        let d = if i + 1 < pts.len() {
            sub(pts[i + 1], pts[i])
        } else if i > 0 {
            sub(pts[i], pts[i - 1])
        } else {
            fallback
        };
        let n = mul(perp(norm(d)), halves[i]);
        left.push(add(pts[i], n));
        right.push(sub(pts[i], n));
    }
    (left, right)
}

fn ribbon(pts: &[P], halves: &[f32], fallback: P) -> Vec<P> {
    let (mut left, right) = ribbon_sides(pts, halves, fallback);
    left.extend(right.into_iter().rev());
    left
}

fn disc(c: P, r: f32) -> Vec<P> {
    (0..24)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 24.;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

fn rotate(v: P, a: f32) -> P {
    let (s, c) = a.sin_cos();
    (v.0 * c - v.1 * s, v.0 * s + v.1 * c)
}

/// Evens out a hand-drawn stroke as the alternative does: resampled every 2 pt, then
/// averaged over about 32 pt, with the ends pinned.
fn steady(points: &[P], scale: f32) -> Vec<P> {
    let total = polyline_len(points);
    let step = 2. * scale;
    if total < step * 3. {
        return points.to_vec();
    }
    let n = (total / step) as usize;
    let step = total / n as f32;
    let mut even = vec![points[0]];
    let (mut seg, mut walked) = (0, 0.);
    for i in 1..n {
        let at = step * i as f32;
        while seg + 2 < points.len() && walked + len(sub(points[seg + 1], points[seg])) < at {
            walked += len(sub(points[seg + 1], points[seg]));
            seg += 1;
        }
        let (a, b) = (points[seg], points[seg + 1]);
        let l = len(sub(b, a));
        even.push(if l > 1e-6 { add(a, mul(sub(b, a), ((at - walked) / l).min(1.))) } else { a });
    }
    even.push(*points.last().unwrap());
    (0..=n)
        .map(|i| {
            let k = 8.min(i).min(n - i);
            let window = &even[i - k..=i + k];
            let sum = window.iter().fold((0., 0.), |a, p| add(a, *p));
            mul(sum, 1. / window.len() as f32)
        })
        .collect()
}

fn polyline_len(pts: &[P]) -> f32 {
    pts.windows(2).map(|w| len(sub(w[1], w[0]))).sum()
}

/// Point at arc length `at` along `pts`, with the direction there.
fn point_along(pts: &[P], at: f32) -> (P, P) {
    let mut left = at;
    for w in pts.windows(2) {
        let seg = len(sub(w[1], w[0]));
        if left <= seg && seg > 1e-6 {
            let d = norm(sub(w[1], w[0]));
            return (add(w[0], mul(d, left)), d);
        }
        left -= seg;
    }
    let n = pts.len();
    (pts[n - 1], norm(sub(pts[n - 1], pts[n.saturating_sub(2)])))
}

/// Deterministic jitter, so a hand-drawn arrow keeps its shape across redraws and export.
pub struct Jitter(pub u64);

impl Jitter {
    pub fn next(&mut self, lo: f32, hi: f32) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        lo + (hi - lo) * ((self.0 >> 40) as f32 / (1u64 << 24) as f32)
    }
}

fn prims_bounds(prims: &[Prim]) -> R {
    let mut points = Vec::new();
    let mut pad = 0f32;
    for prim in prims {
        let (path, width) = match prim {
            Prim::Fill { path, .. } => (path, 0.),
            Prim::Stroke { path, width, .. } => (path, *width),
            _ => continue,
        };
        pad = pad.max(width / 2.);
        for seg in path {
            match *seg {
                Seg::M(p) | Seg::L(p) => points.push(p),
                Seg::Q(a, b) => points.extend([a, b]),
                Seg::C(a, b, c) => points.extend([a, b, c]),
                Seg::Z => {}
            }
        }
    }
    bbox(&points).inflate(pad)
}

fn rect_edge_dist(r: R, p: P) -> f32 {
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    [
        dist_to_segment(p, (x0, y0), (x1, y0)),
        dist_to_segment(p, (x1, y0), (x1, y1)),
        dist_to_segment(p, (x1, y1), (x0, y1)),
        dist_to_segment(p, (x0, y1), (x0, y0)),
    ]
    .into_iter()
    .fold(f32::MAX, f32::min)
}

fn ellipse_inside(r: R, p: P) -> bool {
    let (cx, cy) = r.center();
    let (rx, ry) = ((r.w / 2.).max(1e-3), (r.h / 2.).max(1e-3));
    ((p.0 - cx) / rx).powi(2) + ((p.1 - cy) / ry).powi(2) <= 1.
}

/// Approximate distance to the ellipse outline, exact for circles.
fn ellipse_edge_dist(r: R, p: P) -> f32 {
    let (cx, cy) = r.center();
    let (rx, ry) = ((r.w / 2.).max(1e-3), (r.h / 2.).max(1e-3));
    let (dx, dy) = ((p.0 - cx) / rx, (p.1 - cy) / ry);
    let k = dx.hypot(dy);
    if k < 1e-4 {
        return rx.min(ry);
    }
    let on = (cx + dx / k * rx, cy + dy / k * ry);
    len(sub(p, on))
}

/// Everything to paint, in order. Highlighters go under every other object: they multiply the image
/// itself, and redaction must cover them. The dim over `canvas` goes under the first spotlight.
pub fn scene(objects: &[Object], cx: &Ctx, canvas: R) -> Vec<Prim> {
    let marker = |o: &&Object| matches!(o.shape, Shape::Highlighter { .. });
    let mut dim = spotlight_dim(objects, canvas);
    let ordered = objects.iter().filter(marker).chain(objects.iter().filter(|o| !marker(o)));
    ordered
        .flat_map(|o| {
            let dim = dim.take_if(|_| matches!(o.shape, Shape::Spotlight { .. }));
            dim.into_iter().chain(o.prims(cx))
        })
        .collect()
}

/// One dim layer for all spotlights, with their shapes cut out of it.
fn spotlight_dim(objects: &[Object], canvas: R) -> Option<Prim> {
    let mut path = rect_path(canvas, 0.);
    let frame = path.len();
    for o in objects {
        if let Shape::Spotlight { a, b } = &o.shape {
            path.extend(o.spotlight_path(*a, *b));
        }
    }
    (path.len() > frame).then_some(Prim::Fill { path, color: 0x000000, alpha: 0.21 })
}

/// The canvas is the crop until an object crosses its edge. Then it becomes
/// the bounds of everything plus `pad` on every side, capped at `limit`.
pub fn canvas(crop: R, objects: &[Object], cx: &Ctx, pad: f32, limit: f32) -> R {
    let mut all = crop;
    for o in objects {
        if matches!(o.shape, Shape::Spotlight { .. }) {
            continue;
        }
        all = all.union(&o.bounds(cx));
    }
    if crop.contains_rect(&all) {
        return crop;
    }
    let grown = all.inflate(pad).round_out();
    grown
        .intersect(&crop.inflate(limit))
        .unwrap_or(crop)
}

/// Lines a drag snaps to, in image pixels.
#[derive(Default)]
pub struct SnapLines {
    pub xs: Vec<f32>,
    pub ys: Vec<f32>,
}

/// The edges of `base` and of every other object of the same kind as `moving`, as in the
/// alternative's default "Snap to Similar Objects".
pub fn snap_lines(base: R, objects: &[Object], moving: &Object) -> SnapLines {
    let mut lines = SnapLines { xs: vec![base.x, base.right()], ys: vec![base.y, base.bottom()] };
    let kind = moving.shape.tool();
    for r in objects.iter().filter(|o| o.id != moving.id && o.shape.tool() == kind).filter_map(Object::snap_box) {
        lines.xs.extend([r.x, r.right()]);
        lines.ys.extend([r.y, r.bottom()]);
    }
    lines
}

/// The shift that puts the closest of `edges` on one of `lines`, if any is within `tol`, and that line.
pub fn snap_axis(edges: &[f32], lines: &[f32], tol: f32) -> Option<(f32, f32)> {
    edges
        .iter()
        .flat_map(|e| lines.iter().map(move |l| (l - e, *l)))
        .filter(|(d, _)| d.abs() <= tol)
        .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
}

/// As in the alternative, a straight stroke within 1° of horizontal comes out level and one
/// up to 6° is tilted less, easing back to its own slope: markers mostly go over lines of text.
fn level(points: &[P], w: f32) -> Vec<P> {
    let (a, b) = (points[0], points[points.len() - 1]);
    let d = sub(b, a);
    let straight = points.iter().all(|p| dist_to_polyline(*p, &[a, b]) <= w / 4.);
    if d.0.abs() < w || !straight {
        return points.to_vec();
    }
    let slope = d.1 / d.0;
    let angle = slope.atan().to_degrees();
    if angle.abs() > 6. {
        return points.to_vec();
    }
    let y = points.iter().map(|p| p.1).sum::<f32>() / points.len() as f32;
    if angle.abs() <= 1. {
        return points.iter().map(|p| (p.0, y)).collect();
    }
    // 1° maps to level and 6° to itself, so the damping has no jump at either end.
    let damped = (angle.signum() * (angle.abs() - 1.) * 1.2).to_radians().tan();
    let x = (a.0 + b.0) / 2.;
    points.iter().map(|p| (p.0, p.1 - (p.0 - x) * (slope - damped))).collect()
}

/// As in the alternative, marker ends are cut flat and run a little past the cursor.
fn marker_ends(points: &[P], ext: f32) -> Vec<P> {
    let (first, last) = (points[0], points[points.len() - 1]);
    let away = |end: P, inner: Option<&P>, fallback: P| inner.map_or(fallback, |p| norm(sub(end, *p)));
    let da = away(first, points.iter().find(|p| len(sub(**p, first)) > ext), (-1., 0.));
    let db = away(last, points.iter().rev().find(|p| len(sub(**p, last)) > ext), (1., 0.));
    let mut out = Vec::with_capacity(points.len() + 2);
    out.push(add(first, mul(da, ext)));
    out.extend_from_slice(points);
    out.push(add(last, mul(db, ext)));
    out
}

fn counter_numbers(objects: &[Object]) -> Vec<u32> {
    let mut ns: Vec<u32> = objects
        .iter()
        .filter_map(|o| match o.shape {
            Shape::Counter { n, .. } => Some(n),
            _ => None,
        })
        .collect();
    ns.sort_unstable();
    ns.dedup();
    ns
}

pub fn counters_have_gap(objects: &[Object]) -> bool {
    let ns = counter_numbers(objects);
    ns.last().is_some_and(|&n| n as usize != ns.len())
}

/// Numbers the counters 1, 2, 3… in their current order; equal numbers stay equal.
pub fn renumber_counters(objects: &mut [Object]) {
    let ns = counter_numbers(objects);
    for o in objects {
        if let Shape::Counter { n, .. } = &mut o.shape {
            *n = ns.binary_search(n).map_or(*n, |i| i as u32 + 1);
        }
    }
}

pub fn next_counter(objects: &[Object]) -> u32 {
    counter_numbers(objects).last().map_or(1, |n| n + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renumber_closes_gaps_and_keeps_order() {
        let counter = |id, n| Object { id, shape: Shape::Counter { at: (0., 0.), n, angle: 0. }, color: 0, size: 1., variant: 0 };
        let mut objects = vec![counter(1, 5), counter(2, 2), counter(3, 5), counter(4, 9)];
        assert!(counters_have_gap(&objects));
        renumber_counters(&mut objects);
        let ns: Vec<u32> = objects.iter().map(|o| match o.shape { Shape::Counter { n, .. } => n, _ => 0 }).collect();
        assert_eq!(ns, [2, 1, 2, 3]);
        assert!(!counters_have_gap(&objects));
    }

    #[test]
    fn marker_levels_small_slopes_and_damps_larger_ones() {
        let stroke = |deg: f32| -> Vec<P> { (0..=10).map(|i| (i as f32 * 40., i as f32 * 40. * deg.to_radians().tan())).collect() };
        let slope = |p: &[P]| ((p[10].1 - p[0].1) / (p[10].0 - p[0].0)).atan().to_degrees();
        assert_eq!(slope(&level(&stroke(0.8), 28.)), 0.);
        assert!((slope(&level(&stroke(3.), 28.)) - 2.4).abs() < 0.05);
        assert!((slope(&level(&stroke(-3.), 28.)) + 2.4).abs() < 0.05);
        assert!((slope(&level(&stroke(10.), 28.)) - 10.).abs() < 0.01);
    }
}
