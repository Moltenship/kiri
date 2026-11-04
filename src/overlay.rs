//! The capture overlay: one frozen, borderless window per display. It selects an
//! area for a plain capture, OCR, or scrolling capture, or picks a window.
//!
//! On macOS it behaves like the system grabber (⇧⌘4), which the alternative uses
//! as is: Space while dragging moves the selection, Shift locks the axis the cursor
//! moves least along, Option grows it from its center, and Space before dragging
//! switches between area and window picking. Arrow keys move the pointer by a point.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

use crate::mac::{self, Shot};
use crate::tiled::{PxRect, TiledImage};
use crate::window_capture::WindowInfo;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Area,
    Text,
    Scroll { up: bool },
    Window,
}

const SPACE: u16 = 49;
const TAB: u16 = 48;
const LEFT: u16 = 123;
const RIGHT: u16 = 124;
const DOWN: u16 = 125;
const UP: u16 = 126;

#[derive(Default)]
struct Session {
    windows: Vec<AnyWindowHandle>,
    last_area: Option<(u32, PxRect)>,
    /// The editor window that asked for this capture with Add Capture.
    add_to: Option<AnyWindowHandle>,
}

impl Global for Session {}

pub fn last_area(cx: &App) -> Option<(u32, PxRect)> {
    cx.try_global::<Session>().and_then(|s| s.last_area)
}

pub fn start(shots: Vec<Shot>, mode: Mode, targets: Vec<WindowInfo>, hotkey_at: Instant, add_to: Option<AnyWindowHandle>, cx: &mut App) {
    close_all(cx);
    if mode == Mode::Window && targets.is_empty() {
        crate::toast::show("No windows to capture. Try Capture Screen instead", cx);
        return;
    }
    let first_paint = Rc::new(Cell::new(Some(hotkey_at)));
    let targets = Rc::new(targets);
    let mut windows = Vec::new();
    for shot in shots {
        let display = shot.display;
        let (w, h) = mac::display_points(display);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(point(px(0.), px(0.)), size(px(w), px(h))))),
            titlebar: None,
            kind: WindowKind::PopUp,
            display_id: Some(DisplayId::new(display as u64)),
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            focus: true,
            show: true,
            ..Default::default()
        };
        let first_paint = first_paint.clone();
        let targets = targets.clone();
        let opened = gpui_kit::open_window(options, cx, |window, cx| {
            let overlay = cx.new(|cx| Overlay::new(shot, mode, targets, first_paint, cx.focus_handle()));
            window.focus(&overlay.read(cx).focus.clone(), cx);
            overlay
        });
        match opened {
            Ok((handle, _)) => {
                // After open, so GPUI's own show does not reorder the window again.
                let _ = handle.update(cx, |_, window, _| mac::make_overlay(window, display));
                windows.push(handle);
            }
            Err(err) => eprintln!("kiri: overlay window failed: {err:#}"),
        }
    }
    cx.activate(true);
    if !windows.is_empty() {
        crate::grab_escape(true, cx);
    }
    let last_area = last_area(cx);
    cx.set_global(Session { windows, last_area, add_to });
}

pub fn close_all(cx: &mut App) {
    let windows = cx.try_global::<Session>().map(|s| s.windows.clone()).unwrap_or_default();
    for handle in &windows {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
    if !windows.is_empty() {
        crate::grab_escape(false, cx);
    }
    let last_area = last_area(cx);
    cx.set_global(Session { windows: Vec::new(), last_area, add_to: None });
}

pub struct Overlay {
    display: u32,
    origin: (f32, f32),
    scale: f32,
    mode: Mode,
    image: TiledImage,
    targets: Rc<Vec<WindowInfo>>,
    picking: bool,
    /// How many windows under the cursor `Tab` has skipped.
    skip: usize,
    drag: Option<Drag>,
    cursor: Option<Point<Pixels>>,
    /// Key codes that are down; see `press`.
    held: Vec<u16>,
    modifiers: Modifiers,
    first_paint: Rc<Cell<Option<Instant>>>,
    focus: FocusHandle,
}

impl Overlay {
    fn new(
        shot: Shot,
        mode: Mode,
        targets: Rc<Vec<WindowInfo>>,
        first_paint: Rc<Cell<Option<Instant>>>,
        focus: FocusHandle,
    ) -> Self {
        let image = TiledImage::from_bgra(shot.width, shot.height, &shot.bgra, (shot.width * 4) as usize);
        Self {
            display: shot.display,
            origin: mac::display_origin(shot.display),
            scale: shot.scale,
            mode,
            image,
            targets,
            picking: mode == Mode::Window,
            skip: 0,
            drag: None,
            cursor: None,
            held: Vec::new(),
            modifiers: Modifiers::default(),
            first_paint,
            focus,
        }
    }

    fn selection(&self) -> Option<Bounds<Pixels>> {
        let (a, b) = (self.drag?.anchor, self.drag?.corner);
        let (w, h) = (self.image.width as f32 / self.scale, self.image.height as f32 / self.scale);
        let clamp = |v: Pixels, max: f32| px(v.as_f32().clamp(0., max));
        Some(Bounds::from_corners(
            point(clamp(a.x.min(b.x), w), clamp(a.y.min(b.y), h)),
            point(clamp(a.x.max(b.x), w), clamp(a.y.max(b.y), h)),
        ))
    }

    fn track(&mut self, cursor: Point<Pixels>, cx: &mut Context<Self>) {
        if self.picking && self.cursor.is_some_and(|c| c != cursor) {
            self.skip = 0;
        }
        self.cursor = Some(cursor);
        if let Some(drag) = &mut self.drag {
            drag.update(cursor, self.held.contains(&SPACE), self.modifiers.shift, self.modifiers.alt);
        }
        cx.notify();
    }

    /// Returns false for a key that is already down. The panel gets each key-down
    /// twice and never a key-up, so the key itself is watched until it is released.
    fn press(&mut self, code: u16, cx: &mut Context<Self>) -> bool {
        if self.held.contains(&code) {
            return false;
        }
        self.held.push(code);
        cx.spawn(async move |this, cx| {
            while mac::key_pressed(code) {
                cx.background_executor().timer(Duration::from_millis(20)).await;
            }
            let _ = this.update(cx, |this, cx| {
                this.held.retain(|&c| c != code);
                this.refresh(cx);
            });
        })
        .detach();
        true
    }

    /// Arrow keys move the pointer a point at a time, before or during a drag, and repeat
    /// while held. Repeat is timed here because the panel's own key repeat arrives doubled.
    fn nudge(&mut self, code: u16, dx: f32, dy: f32, cx: &mut Context<Self>) {
        if !self.press(code, cx) {
            return;
        }
        self.step(dx, dy, cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(350)).await;
            while mac::key_pressed(code) {
                if this.update(cx, |this, cx| this.step(dx, dy, cx)).is_err() {
                    return;
                }
                cx.background_executor().timer(Duration::from_millis(30)).await;
            }
        })
        .detach();
    }

    fn step(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        let Some(c) = self.cursor else { return };
        let (w, h) = (self.image.width as f32 / self.scale, self.image.height as f32 / self.scale);
        let to = point(px((c.x.as_f32() + dx).clamp(0., w - 1.)), px((c.y.as_f32() + dy).clamp(0., h - 1.)));
        mac::warp_cursor(self.origin.0 + to.x.as_f32(), self.origin.1 + to.y.as_f32());
        self.track(to, cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        match self.cursor {
            Some(c) => self.track(c, cx),
            None => cx.notify(),
        }
    }

    /// The window under the cursor, in this display's local points.
    fn target(&self) -> Option<(&WindowInfo, Bounds<Pixels>)> {
        let c = self.cursor?;
        let (gx, gy) = (self.origin.0 + c.x.as_f32(), self.origin.1 + c.y.as_f32());
        let under: Vec<_> = self
            .targets
            .iter()
            .filter(|w| {
                let (x, y, ww, hh) = w.frame;
                gx >= x && gx < x + ww && gy >= y && gy < y + hh
            })
            .collect();
        let w = *under.get(self.skip % under.len().max(1))?;
        let (x, y, ww, hh) = w.frame;
        Some((w, Bounds::new(point(px(x - self.origin.0), px(y - self.origin.1)), size(px(ww), px(hh)))))
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        if self.picking {
            return self.finish_window(cx);
        }
        let Some(sel) = self.selection() else { return };
        let s = self.scale;
        let x = (sel.origin.x.as_f32() * s).round().max(0.) as u32;
        let y = (sel.origin.y.as_f32() * s).round().max(0.) as u32;
        let w = ((sel.size.width.as_f32() * s).round() as u32).min(self.image.width - x);
        let h = ((sel.size.height.as_f32() * s).round() as u32).min(self.image.height - y);
        if w < 4 || h < 4 {
            // A click without a drag does nothing, like the system grabber.
            self.drag = None;
            cx.notify();
            return;
        }
        let rect = PxRect { x, y, w, h };
        let display = self.display;
        let mode = self.mode;
        let global = (
            self.origin.0 + sel.origin.x.as_f32(),
            self.origin.1 + sel.origin.y.as_f32(),
            sel.size.width.as_f32(),
            sel.size.height.as_f32(),
        );
        let bgra = self.image.read(rect);
        // App-level defer: a window can't be updated from inside its own `window.defer`.
        cx.defer(move |cx| {
            let add_to = cx.global_mut::<Session>().add_to.take();
            close_all(cx);
            match mode {
                Mode::Area | Mode::Window => {
                    cx.global_mut::<Session>().last_area = Some((display, rect));
                    let image = TiledImage::from_bgra(w, h, &bgra, (w * 4) as usize);
                    drop(bgra);
                    match add_to {
                        Some(editor) => crate::editor::add_capture(editor, image, s, cx),
                        None => crate::deliver(image, s, Some(display), cx),
                    }
                }
                Mode::Text => crate::ocr::recognize_and_copy(bgra, w, h, cx),
                Mode::Scroll { up } => crate::scrolling::start(global, display, up, cx),
            }
        });
    }

    fn finish_window(&mut self, cx: &mut Context<Self>) {
        let Some((target, _)) = self.target() else { return };
        let id = target.id;
        let (background, color) = crate::window_style(cx);
        cx.defer(move |cx| {
            let add_to = cx.global_mut::<Session>().add_to.take();
            close_all(cx);
            cx.spawn(async move |cx| {
                let shot = crate::window_capture::capture(id, background, color).await;
                cx.update(|cx| match shot {
                    Some(shot) => match add_to {
                        Some(editor) => {
                            let image = TiledImage::from_bgra(shot.width, shot.height, &shot.bgra, (shot.width * 4) as usize);
                            crate::editor::add_capture(editor, image, shot.scale, cx)
                        }
                        None => crate::deliver_shot(shot, cx),
                    },
                    None => crate::toast::show("Couldn't capture that window", cx),
                });
            })
            .detach();
        });
    }
}

impl Render for Overlay {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tiles: Vec<(PxRect, Arc<RenderImage>)> = self.image.tiles().map(|(r, t)| (r, t.clone())).collect();
        let scale = self.scale;
        let picking = self.picking;
        let selection = if picking { self.target().map(|(_, b)| b) } else { self.selection() };
        let cursor = self.cursor;
        let first_paint = self.first_paint.clone();

        let label = match (picking, selection, cursor) {
            (true, _, Some(c)) => self.target().map(|(w, _)| {
                let name = if w.title.is_empty() { w.app.clone() } else { format!("{} — {}", w.app, w.title) };
                (c, name)
            }),
            (false, Some(sel), Some(c)) => Some((
                c,
                format!("{}\n{}", sel.size.width.as_f32().round(), sel.size.height.as_f32().round()),
            )),
            (false, None, Some(c)) => Some((c, format!("{}\n{}", c.x.as_f32().round(), c.y.as_f32().round()))),
            _ => None,
        };
        let hint = match self.mode {
            Mode::Text => Some("Select text or a QR code"),
            Mode::Scroll { .. } if crate::settings::get(cx).scroll_manual => Some("Select the area, then scroll it slowly · Esc stops"),
            Mode::Scroll { .. } => Some("Select the area to scroll · Hands off the mouse until it stops · Esc stops"),
            Mode::Window => Some("Click a window · Tab for the one behind · Space for an area"),
            Mode::Area => None,
        };

        div()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .cursor(if picking { CursorStyle::PointingHand } else { CursorStyle::Crosshair })
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "tab" if this.picking && this.press(TAB, cx) => {
                    this.skip += 1;
                    cx.notify();
                }
                "left" => this.nudge(LEFT, -1., 0., cx),
                "right" => this.nudge(RIGHT, 1., 0., cx),
                "up" => this.nudge(UP, 0., -1., cx),
                "down" => this.nudge(DOWN, 0., 1., cx),
                "space" if this.press(SPACE, cx) => {
                    if this.drag.is_none() && matches!(this.mode, Mode::Area | Mode::Window) {
                        this.picking = !this.picking;
                        this.skip = 0;
                    }
                    this.refresh(cx);
                }
                _ => {}
            }))
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.defer(close_all))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                    this.modifiers = e.modifiers;
                    if !this.picking {
                        this.drag = Some(Drag::new(e.position));
                    }
                    this.track(e.position, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                this.modifiers = e.modifiers;
                this.track(e.position, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e: &MouseUpEvent, _, cx| {
                    this.track(e.position, cx);
                    this.finish(cx);
                }),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        for (r, tile) in tiles {
                            let b = Bounds::new(
                                point(px(r.x as f32 / scale), px(r.y as f32 / scale)),
                                size(px(r.w as f32 / scale), px(r.h as f32 / scale)),
                            );
                            let _ = window.paint_image(b, b, Corners::default(), tile, 0, false);
                        }
                        if let Some(sel) = selection {
                            let tint = if picking { hsla(0.58, 0.9, 0.6, 0.3) } else { hsla(0., 0., 0.5, 0.2) };
                            window.paint_quad(fill(sel, tint));
                        }
                        if let Some(at) = first_paint.take() {
                            eprintln!("kiri: hotkey → overlay painted in {} ms", at.elapsed().as_millis());
                        }
                    },
                )
                .size_full(),
            )
            .children(hint.map(|hint| {
                div().absolute().top(px(48.)).w_full().flex().justify_center().child(
                    div()
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .bg(hsla(0., 0., 0.08, 0.82))
                        .text_color(hsla(0., 0., 1., 0.95))
                        .text_sm()
                        .child(hint),
                )
            }))
            .children(label.map(|(c, text)| {
                div()
                    .absolute()
                    .left(c.x + px(14.))
                    .top(c.y + px(14.))
                    .px_1p5()
                    .py(px(2.))
                    .rounded(cx.theme().radius)
                    .bg(hsla(0., 0., 0.08, 0.72))
                    .text_color(hsla(0., 0., 1., 0.95))
                    .text_xs()
                    .line_height(px(14.))
                    .font_family(cx.theme().mono_font_family.clone())
                    .children(text.lines().map(|l| div().child(l.to_string())).collect::<Vec<_>>())
            }))
    }
}

/// A selection drag: `anchor` stays put and `corner` follows the cursor.
#[derive(Clone, Copy, Debug)]
struct Drag {
    anchor: Point<Pixels>,
    corner: Point<Pixels>,
    cursor: Point<Pixels>,
    lock: Lock,
    /// Set while Option is held: the selection grows around it.
    center: Option<Point<Pixels>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Lock {
    Off,
    /// Shift is down but the cursor hasn't shown which way it is going yet.
    Pending(Point<Pixels>),
    /// Only this axis follows the cursor.
    X,
    Y,
}

impl Drag {
    fn new(at: Point<Pixels>) -> Self {
        Self { anchor: at, corner: at, cursor: at, lock: Lock::Off, center: None }
    }

    fn update(&mut self, cursor: Point<Pixels>, space: bool, shift: bool, alt: bool) {
        let d = cursor - self.cursor;
        self.cursor = cursor;
        if space {
            self.anchor += d;
            self.corner += d;
            self.center = self.center.map(|c| c + d);
            if let Lock::Pending(p) = &mut self.lock {
                *p += d;
            }
            return;
        }
        self.lock = match (shift, self.lock) {
            (false, _) => Lock::Off,
            (true, Lock::Off) => Lock::Pending(cursor),
            (true, Lock::Pending(p)) => {
                let (dx, dy) = ((cursor.x - p.x).abs(), (cursor.y - p.y).abs());
                match dx.max(dy) < px(3.) {
                    true => Lock::Pending(p),
                    false if dx >= dy => Lock::X,
                    false => Lock::Y,
                }
            }
            (true, lock) => lock,
        };
        self.corner = match self.lock {
            Lock::Off => cursor,
            Lock::Pending(_) => self.corner,
            Lock::X => point(cursor.x, self.corner.y),
            Lock::Y => point(self.corner.x, cursor.y),
        };
        self.center = match (alt, self.center) {
            (false, _) => None,
            (true, None) => Some(point((self.anchor.x + self.corner.x) / 2., (self.anchor.y + self.corner.y) / 2.)),
            (true, c) => c,
        };
        if let Some(c) = self.center {
            self.anchor = point(c.x * 2. - self.corner.x, c.y * 2. - self.corner.y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Drag;
    use gpui_kit::{Pixels, Point, point, px};

    fn p(x: f32, y: f32) -> Point<Pixels> {
        point(px(x), px(y))
    }

    #[test]
    fn space_moves_the_whole_selection() {
        let mut d = Drag::new(p(10., 10.));
        d.update(p(50., 30.), false, false, false);
        d.update(p(70., 40.), true, false, false);
        assert_eq!((d.anchor, d.corner), (p(30., 20.), p(70., 40.)));
        d.update(p(80., 40.), false, false, false);
        assert_eq!((d.anchor, d.corner), (p(30., 20.), p(80., 40.)));
    }

    #[test]
    fn shift_locks_the_axis_moved_least() {
        let mut d = Drag::new(p(0., 0.));
        d.update(p(50., 50.), false, false, false);
        d.update(p(51., 50.), false, true, false);
        d.update(p(70., 52.), false, true, false);
        assert_eq!(d.corner, p(70., 50.));
        d.update(p(90., 80.), false, true, false);
        assert_eq!(d.corner, p(90., 50.));
        d.update(p(90., 80.), false, false, false);
        assert_eq!(d.corner, p(90., 80.));
    }

    #[test]
    fn option_grows_from_the_center() {
        let mut d = Drag::new(p(0., 0.));
        d.update(p(40., 20.), false, false, false);
        d.update(p(40., 20.), false, false, true);
        assert_eq!((d.anchor, d.corner), (p(0., 0.), p(40., 20.)));
        d.update(p(50., 30.), false, false, true);
        assert_eq!((d.anchor, d.corner), (p(-10., -10.), p(50., 30.)));
        d.update(p(60., 30.), false, false, false);
        assert_eq!((d.anchor, d.corner), (p(-10., -10.), p(60., 30.)));
    }
}
