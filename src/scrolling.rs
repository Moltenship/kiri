//! Scrolling capture: scrolls the area under the selection and stitches frames.

use std::time::Duration;

use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme, Sizable, h_flex};
use gpui_kit::*;

use crate::mac;
#[cfg(windows)]
use crate::mac::scrolling as sys;
use crate::stitch::{Step, Stitcher};
use crate::tiled::TiledImage;

const HUD_W: f32 = 300.;
const HUD_H: f32 = 44.;
const GAP: f32 = 8.;

#[derive(Default)]
struct Session {
    running: bool,
    stop: bool,
}

impl Global for Session {}

/// `rect` is in global points with a top-left origin. Ends in `crate::deliver`.
pub fn start(rect: (f32, f32, f32, f32), display: u32, up: bool, cx: &mut App) {
    if cx.default_global::<Session>().running || rect.2 < 1. || rect.3 < 1. {
        return;
    }
    let s = crate::settings::get(cx);
    let (max_height, reverse, manual) = (s.scroll_max_height, s.scroll_reverse, s.scroll_manual);
    // Each wheel step moves 20% to 60% of the area, so frames always overlap.
    let stride = 0.1 + 0.1 * s.scroll_speed.clamp(1, 5) as f32;
    if !manual && !sys::accessibility_trusted() {
        crate::toast::show("Scrolling capture needs Accessibility permission, or turn on Scroll by hand in Settings", cx);
        return;
    }
    cx.set_global(Session { running: true, stop: false });
    crate::grab_escape(true, cx);
    let hud = open_hud(rect, display, manual, cx);
    cx.spawn(async move |cx| {
        let result = run(rect, display, up, reverse, manual, max_height, stride, hud.as_ref().map(|h| &h.1), cx).await;
        let (result, note) = result.unzip();
        let image = match result {
            Some((w, h, bgra, scale)) => {
                let image = cx
                    .background_executor()
                    .spawn(async move { TiledImage::from_bgra(w, h, &bgra, (w * 4) as usize) })
                    .await;
                Some((image, scale))
            }
            None => None,
        };
        cx.update(|cx| {
            if let Some((handle, _)) = hud {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
            crate::grab_escape(false, cx);
            cx.set_global(Session::default());
            if let Some((image, scale)) = image {
                crate::deliver(image, scale, Some(display), cx);
            }
            if let Some(note) = note.flatten() {
                crate::toast::show(note, cx);
            }
        });
    })
    .detach();
}

/// Stops a running capture and keeps what was stitched so far.
pub fn stop(cx: &mut App) {
    let session = cx.default_global::<Session>();
    if session.running {
        session.stop = true;
    }
}

/// Returns the stitched frames as (width, height, BGRA, scale), and why it stopped if the user should know.
async fn run(
    rect: (f32, f32, f32, f32),
    display: u32,
    up: bool,
    reverse: bool,
    manual: bool,
    max_height: u32,
    stride: f32,
    hud: Option<&Entity<Hud>>,
    cx: &mut AsyncApp,
) -> Option<((u32, u32, Vec<u8>, f32), Option<&'static str>)> {
    let executor = cx.background_executor().clone();
    let wait = |ms| executor.timer(Duration::from_millis(ms));
    if !manual {
        sys::warp(display, (rect.0 + rect.2 / 2., rect.1 + rect.3 / 2.));
    }
    // Lets the overlay leave the screen before the first frame.
    wait(200).await;
    let first = sys::grab(display, rect).await?;
    let (w, h, scale) = (first.width, first.height, first.scale);
    let mut stitcher = Stitcher::new(&orient(first.bgra, w, up), w, h, max_height);
    let set_hud = |height: u32, lost: bool, cx: &mut AsyncApp| {
        if let Some(hud) = hud {
            hud.update(cx, |hud, cx| {
                hud.height = height;
                hud.lost = lost;
                cx.notify();
            });
        }
    };
    set_hud(stitcher.height(), false, cx);

    let step = (rect.3 * stride).round().max(1.) as i32;
    let delta = if up != reverse { step } else { -step };
    let mut still = 0;
    let mut note = None;
    while !cx.update(|cx| cx.global::<Session>().stop) {
        let frame = if manual {
            wait(100).await;
            sys::grab(display, rect).await
        } else {
            // The wheel goes to whatever is under the cursor, so scroll only while the user leaves it in place.
            let cursor = sys::cursor(display);
            if !contains(rect, cursor) {
                wait(100).await;
                continue;
            }
            sys::scroll(cursor, delta);
            settled(rect, display, &executor).await
        };
        let Some(frame) = frame else { break };
        if (frame.width, frame.height) != (w, h) {
            break;
        }
        let frame = orient(frame.bgra, w, up);
        let (back, result) = executor
            .spawn(async move {
                let result = stitcher.push(&frame);
                (stitcher, result)
            })
            .await;
        stitcher = back;
        match result {
            Step::Appended { .. } => {
                still = 0;
                set_hud(stitcher.height(), false, cx);
                if stitcher.height() >= max_height {
                    note = Some("Max height reached");
                    break;
                }
            }
            Step::NoMovement if manual => {}
            Step::NoMovement => {
                still += 1;
                if still == 2 {
                    if stitcher.height() == h {
                        note = Some("Nothing scrolled. Try Reverse scroll direction or Scroll by hand in Settings.");
                    }
                    break;
                }
            }
            // The last good frame stays the reference, so scrolling back to it resumes.
            Step::Failed if manual => set_hud(stitcher.height(), true, cx),
            Step::Failed => {
                note = Some("Stopped where the next frame didn't overlap the last one");
                break;
            }
        }
    }
    let (w, h, bgra) = executor
        .spawn(async move {
            let (w, h, bgra) = stitcher.finish();
            (w, h, orient(bgra, w, up))
        })
        .await;
    Some(((w, h, bgra, scale), note))
}

/// Grabs until two frames in a row match, so smooth scrolling and inertia have finished.
async fn settled(rect: (f32, f32, f32, f32), display: u32, executor: &BackgroundExecutor) -> Option<Frame> {
    let wait = |ms| executor.timer(Duration::from_millis(ms));
    wait(60).await;
    let mut last = sys::grab(display, rect).await?;
    for _ in 0..12 {
        wait(40).await;
        let next = sys::grab(display, rect).await?;
        if next.bgra == last.bgra {
            return Some(next);
        }
        last = next;
    }
    Some(last)
}

fn contains(rect: (f32, f32, f32, f32), (x, y): (f32, f32)) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

/// The stitcher only grows downward; scrolling up is the same problem upside down.
fn orient(bgra: Vec<u8>, width: u32, up: bool) -> Vec<u8> {
    if !up {
        return bgra;
    }
    bgra.chunks_exact(width as usize * 4).rev().flatten().copied().collect()
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub bgra: Vec<u8>,
}

struct Hud {
    height: u32,
    manual: bool,
    lost: bool,
}

impl Render for Hud {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().items_center().justify_center().child(
            h_flex()
                .gap_3()
                .pl_4()
                .pr_1p5()
                .py_1p5()
                .rounded_full()
                .bg(cx.theme().popover)
                .border_1()
                .border_color(cx.theme().border)
                .text_sm()
                .text_color(cx.theme().popover_foreground)
                .child(match (self.manual, self.lost) {
                    (true, true) => "Lost track. Scroll back a little".to_string(),
                    (true, false) => format!("Scroll slowly… {} px", self.height),
                    _ => format!("Scrolling… {} px", self.height),
                })
                .child(
                    Button::new("stop")
                        .label(if self.manual { "Done" } else { "Stop" })
                        .small()
                        .rounded_full()
                        .on_click(|_, _, cx| stop(cx)),
                ),
        )
    }
}

/// Sits just below the selection, or above it, so it never shows up in a frame.
fn open_hud(
    rect: (f32, f32, f32, f32),
    display: u32,
    manual: bool,
    cx: &mut App,
) -> Option<(AnyWindowHandle, Entity<Hud>)> {
    let target = cx.displays().into_iter().find(|d| u64::from(d.id()) as u32 == display)?;
    let (ox, oy) = mac::display_origin(display);
    let (dw, dh) = mac::display_points(display);
    let (x, y, w, h) = (rect.0 - ox, rect.1 - oy, rect.2, rect.3);
    let top = if y + h + GAP + HUD_H <= dh {
        y + h + GAP
    } else if y - GAP - HUD_H >= 0. {
        y - GAP - HUD_H
    } else {
        return None;
    };
    let left = (x + w / 2. - HUD_W / 2.).min(dw - HUD_W - GAP).max(GAP);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(left), px(top)),
            size(px(HUD_W), px(HUD_H)),
        ))),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: false,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(target.id()),
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let (handle, hud) = crate::open_clear_window(options, cx, |_, cx| cx.new(|_| Hud { height: 0, manual, lost: false })).ok()?;
    let _ = handle.update(cx, |_, window, _| {
        mac::make_floating(window, mac::Level::Status, false, false)
    });
    Some((handle, hud))
}

#[cfg(target_os = "macos")]
mod sys {
    use std::sync::Mutex;

    use block2::RcBlock;
    use futures::channel::oneshot;
    use objc2_core_foundation::{CFBoolean, CFDictionary, CFString, CFType, CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{
        CGDataProvider, CGEvent, CGEventTapLocation, CGImage, CGScrollEventUnit,
        CGWarpMouseCursorPosition,
    };
    use objc2_foundation::NSError;
    use objc2_screen_capture_kit::SCScreenshotManager;

    use super::Frame;

    pub fn warp(_display: u32, (x, y): (f32, f32)) {
        let _ = CGWarpMouseCursorPosition(CGPoint::new(x as f64, y as f64));
    }

    pub fn cursor(_display: u32) -> (f32, f32) {
        let at = CGEvent::location(CGEvent::new(None).as_deref());
        (at.x as f32, at.y as f32)
    }

    pub fn scroll((x, y): (f32, f32), delta: i32) {
        let Some(event) = CGEvent::new_scroll_wheel_event2(None, CGScrollEventUnit::Pixel, 1, delta, 0, 0)
        else {
            return;
        };
        CGEvent::set_location(Some(&event), CGPoint::new(x as f64, y as f64));
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
    }

    pub fn accessibility_trusted() -> bool {
        #[link(name = "ApplicationServices", kind = "framework")]
        unsafe extern "C" {
            static kAXTrustedCheckOptionPrompt: &'static CFString;
            fn AXIsProcessTrustedWithOptions(options: &CFDictionary) -> u8;
        }
        let keys: [&CFString; 1] = [unsafe { kAXTrustedCheckOptionPrompt }];
        let values: [&CFType; 1] = [CFBoolean::new(true).as_ref()];
        let options = CFDictionary::from_slices(&keys, &values);
        unsafe { AXIsProcessTrustedWithOptions(options.as_ref()) != 0 }
    }

    pub async fn grab(_display: u32, rect: (f32, f32, f32, f32)) -> Option<Frame> {
        let bounds = CGRect::new(
            CGPoint::new(rect.0 as f64, rect.1 as f64),
            CGSize::new(rect.2 as f64, rect.3 as f64),
        );
        let (tx, rx) = oneshot::channel();
        let tx = Mutex::new(Some(tx));
        let handler = RcBlock::new(move |image: *mut CGImage, _error: *mut NSError| {
            let frame = unsafe { image.as_ref() }.and_then(|image| to_frame(bounds, image));
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(frame);
            }
        });
        unsafe { SCScreenshotManager::captureImageInRect_completionHandler(bounds, Some(&handler)) };
        rx.await.ok().flatten()
    }

    fn to_frame(bounds: CGRect, image: &CGImage) -> Option<Frame> {
        let width = CGImage::width(Some(image));
        let height = CGImage::height(Some(image));
        let stride = CGImage::bytes_per_row(Some(image));
        if CGImage::bits_per_pixel(Some(image)) != 32 || width == 0 || height == 0 {
            return None;
        }
        let data = CGDataProvider::data(CGImage::data_provider(Some(image)).as_deref())?;
        let raw = unsafe { data.as_bytes_unchecked() };
        let row = width * 4;
        let mut bgra = Vec::with_capacity(row * height);
        for y in 0..height {
            bgra.extend_from_slice(&raw[y * stride..y * stride + row]);
        }
        Some(Frame {
            width: width as u32,
            height: height as u32,
            scale: (width as f64 / bounds.size.width) as f32,
            bgra,
        })
    }
}
