//! Paints document primitives with GPUI.

use std::sync::Arc;

use gpui_kit::*;
use lyon_tessellation::LineCap;

use crate::doc::{LINE_HEIGHT, P, Prim, R, Seg};

/// Maps image pixels to window points.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub origin: Point<Pixels>,
    /// Window points per image pixel.
    pub k: f32,
}

impl View {
    pub fn to_screen(&self, p: P) -> Point<Pixels> {
        point(
            self.origin.x + px(p.0 * self.k),
            self.origin.y + px(p.1 * self.k),
        )
    }

    pub fn to_image(&self, p: Point<Pixels>) -> P {
        (
            f32::from(p.x - self.origin.x) / self.k,
            f32::from(p.y - self.origin.y) / self.k,
        )
    }

    pub fn rect(&self, r: R) -> Bounds<Pixels> {
        Bounds::new(
            self.to_screen((r.x, r.y)),
            size(px(r.w * self.k), px(r.h * self.k)),
        )
    }
}

pub fn color(rgb_value: u32, alpha: f32) -> Hsla {
    let mut c: Hsla = rgb(rgb_value).into();
    c.a = alpha;
    c
}

pub const FONT: &str = ".SystemUIFont";

fn build(segs: &[Seg], view: View, mut pb: PathBuilder) -> Option<Path<Pixels>> {
    for s in segs {
        match *s {
            Seg::M(p) => pb.move_to(view.to_screen(p)),
            Seg::L(p) => pb.line_to(view.to_screen(p)),
            Seg::Q(c, p) => pb.curve_to(view.to_screen(p), view.to_screen(c)),
            Seg::C(a, b, p) => {
                pb.cubic_bezier_to(view.to_screen(p), view.to_screen(a), view.to_screen(b))
            }
            Seg::Z => pb.close(),
        }
    }
    pb.build().ok()
}

/// `patch` looks up a cached pixel patch by key: where it goes and its image.
pub fn prim(
    p: &Prim,
    view: View,
    window: &mut Window,
    cx: &mut App,
    patch: &dyn Fn(u64) -> Option<(R, Arc<RenderImage>)>,
) {
    match p {
        Prim::Fill { path, color: c, alpha } => {
            let pb = PathBuilder::fill().with_style(PathStyle::Fill(
                FillOptions::default().with_fill_rule(FillRule::EvenOdd),
            ));
            if let Some(path) = build(path, view, pb) {
                window.paint_path(path, color(*c, *alpha));
            }
        }
        Prim::Stroke {
            path,
            width,
            color: c,
            alpha,
            dash,
        } => {
            let w = (width * view.k).max(0.5);
            let cap = if dash.is_some() {
                LineCap::Butt
            } else {
                LineCap::Round
            };
            let mut pb = PathBuilder::stroke(px(w)).with_style(PathStyle::Stroke(
                StrokeOptions::default()
                    .with_line_width(w)
                    .with_line_cap(cap)
                    .with_line_join(lyon_tessellation::LineJoin::Round),
            ));
            if let Some(d) = dash {
                pb = pb.dash_array(&[px(d * view.k), px(d * 0.75 * view.k)]);
            }
            if let Some(path) = build(path, view, pb) {
                window.paint_path(path, color(*c, *alpha));
            }
        }
        Prim::Text {
            at,
            text,
            size,
            color: c,
            bold,
        } => {
            if text.is_empty() {
                return;
            }
            let font_size = px(size * view.k);
            let run = TextRun {
                len: text.len(),
                font: Font {
                    weight: if *bold {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    },
                    ..font(FONT)
                },
                color: color(*c, 1.),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = window
                .text_system()
                .shape_line(text.clone().into(), font_size, &[run], None);
            let _ = line.paint(
                view.to_screen(*at),
                font_size * LINE_HEIGHT,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        Prim::Pixels { key, .. } | Prim::Highlight { key, .. } => {
            if let Some((at, image)) = patch(*key) {
                let b = view.rect(at);
                let _ = window.paint_image(b, b, Corners::default(), image, 0, false);
            }
        }
        Prim::Image { key, rect, radius, .. } => {
            if let Some((_, image)) = patch(*key) {
                let b = view.rect(*rect);
                let _ = window.paint_image(b, b, Corners::all(px(radius * view.k)), image, 0, false);
            }
        }
    }
}
