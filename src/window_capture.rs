//! Window capture through ScreenCaptureKit; the window list comes from CoreGraphics,
//! which is the only source that guarantees front-to-back order.

use std::sync::Mutex;

use block2::RcBlock;
use futures::channel::oneshot;
use objc2::AllocAnyThread;
use objc2::rc::Retained;
use objc2_app_kit::NSWorkspace;
use objc2_core_foundation::{
    CFArray, CFData, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGPoint, CGRect,
    CGSize,
};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGColor, CGColorSpace, CGContext, CGDataProvider, CGDirectDisplayID,
    CGDisplayBounds, CGGetDisplaysWithPoint, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo,
    CGRectMakeWithDictionaryRepresentation, CGWindowLevelForKey, CGWindowLevelKey,
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGColorSpaceSRGB, kCGWindowAlpha,
    kCGWindowBounds, kCGWindowLayer, kCGWindowName, kCGWindowNumber, kCGWindowOwnerName,
    kCGWindowOwnerPID,
};
use objc2_foundation::{NSArray, NSError};
use objc2_screen_capture_kit::{
    SCContentFilter, SCScreenshotManager, SCShareableContent, SCStreamConfiguration, SCWindow,
};

use crate::mac::Shot;
use crate::settings::WindowBackground;

/// Room around the window for its shadow, in points. Generous: an undersized canvas
/// makes ScreenCaptureKit shrink the window instead of clipping the shadow.
const SHADOW_ROOM: f64 = 100.;

/// An on-screen app window. `frame` is in global points with a top-left origin,
/// the same space as `CGDisplayBounds`.
#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub id: u32,
    pub app: String,
    pub title: String,
    pub frame: (f32, f32, f32, f32),
}

/// Normal-layer windows on screen, front to back, without Kiri's own.
pub async fn list_windows() -> Vec<WindowInfo> {
    windows().into_iter().map(|(_, w)| w).collect()
}

/// The focused window of the frontmost app other than Kiri.
pub async fn frontmost_window() -> Option<WindowInfo> {
    let front = NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|app| app.processIdentifier());
    let mut windows = windows();
    let i = windows.iter().position(|(pid, _)| Some(*pid) == front).unwrap_or(0);
    (i < windows.len()).then(|| windows.swap_remove(i).1)
}

/// Captures one window, even if covered, and fills around it per `background`.
pub async fn capture(id: u32, background: WindowBackground, color: u32) -> Option<Shot> {
    let content = shareable_content().await?;
    let window = unsafe { content.windows() }
        .iter()
        .find(|w| unsafe { w.windowID() } == id)?;
    let frame = unsafe { window.frame() };
    let filter = unsafe {
        SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &window)
    };
    let scale = unsafe { SCShareableContent::infoForFilter(&filter).pointPixelScale() } as f64;
    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        config.setWidth(((frame.size.width + 2. * SHADOW_ROOM) * scale) as usize);
        config.setHeight(((frame.size.height + 2. * SHADOW_ROOM) * scale) as usize);
        // Unscaled, the window and its shadow land in the top-left corner.
        config.setScalesToFit(false);
        config.setShowsCursor(false);
        config.setIgnoreShadowsSingleWindow(background == WindowBackground::TrimShadow);
    }
    let image = capture_image(&filter, &config).await?;
    let alpha = Alpha::of(&image)?;
    let bounds = alpha.bounds()?;
    let center = CGPoint::new(
        frame.origin.x + frame.size.width / 2.,
        frame.origin.y + frame.size.height / 2.,
    );
    let display = display_at(center);
    let (crop, backdrop) = match background {
        WindowBackground::Wallpaper => {
            let window_px = (frame.size.width * scale, frame.size.height * scale);
            let origin = alpha.window_origin(bounds, window_px);
            // Image pixels of the global point `p`.
            let px = |p: CGPoint| {
                (
                    (p.x - frame.origin.x) * scale + origin.0,
                    (p.y - frame.origin.y) * scale + origin.1,
                )
            };
            // There's no wallpaper past the display edge, so the picture stops there.
            let b = CGDisplayBounds(display);
            let (dx0, dy0) = px(b.origin);
            let (dx1, dy1) =
                px(CGPoint::new(b.origin.x + b.size.width, b.origin.y + b.size.height));
            let crop = intersect(bounds, (dx0, dy0, dx1, dy1))?;
            let source = CGRect::new(
                CGPoint::new(
                    (crop.0 as f64 - origin.0) / scale + frame.origin.x - b.origin.x,
                    (crop.1 as f64 - origin.1) / scale + frame.origin.y - b.origin.y,
                ),
                CGSize::new(crop.2 as f64 / scale, crop.3 as f64 / scale),
            );
            let wallpaper = wallpaper(&content, display, source, (crop.2, crop.3)).await;
            (crop, wallpaper.map(Backdrop::Image))
        }
        WindowBackground::Solid => (bounds, Some(Backdrop::Color(color))),
        _ => (bounds, None),
    };
    compose(&image, crop, backdrop, display, scale as f32)
}

fn windows() -> Vec<(i32, WindowInfo)> {
    let options =
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
    let Some(list) = CGWindowListCopyWindowInfo(options, 0) else {
        return Vec::new();
    };
    let list: &CFArray<CFDictionary<CFString, CFType>> = unsafe { list.cast_unchecked() };
    let me = std::process::id() as i32;
    list.iter()
        .filter_map(|d| {
            let number = |key: &CFString| d.get(key)?.downcast::<CFNumber>().ok();
            let text = |key: &CFString| {
                d.get(key)
                    .and_then(|v| v.downcast::<CFString>().ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            };
            let pid = number(unsafe { kCGWindowOwnerPID })?.as_i32()?;
            let layer = number(unsafe { kCGWindowLayer })?.as_i32()?;
            let alpha = number(unsafe { kCGWindowAlpha }).and_then(|a| a.as_f64()).unwrap_or(1.);
            if layer != 0 || pid == me || alpha <= 0. {
                return None;
            }
            let bounds = d.get(unsafe { kCGWindowBounds })?.downcast::<CFDictionary>().ok()?;
            let mut r = CGRect::default();
            if !unsafe { CGRectMakeWithDictionaryRepresentation(Some(&bounds), &mut r) } {
                return None;
            }
            // Smaller than this is an invisible helper or a sliver, not something to capture.
            if r.size.width < 24. || r.size.height < 24. {
                return None;
            }
            let info = WindowInfo {
                id: number(unsafe { kCGWindowNumber })?.as_i64()? as u32,
                app: text(unsafe { kCGWindowOwnerName }),
                title: text(unsafe { kCGWindowName }),
                frame: (
                    r.origin.x as f32,
                    r.origin.y as f32,
                    r.size.width as f32,
                    r.size.height as f32,
                ),
            };
            Some((pid, info))
        })
        .collect()
}

fn display_at(point: CGPoint) -> CGDirectDisplayID {
    let (mut display, mut count) = (0, 0u32);
    unsafe { CGGetDisplaysWithPoint(point, 1, &mut display, &mut count) };
    if count == 0 { 0 } else { display }
}

/// Desktop windows are included so Wallpaper mode can find the picture.
async fn shareable_content() -> Option<Retained<SCShareableContent>> {
    let (tx, rx) = oneshot::channel();
    let tx = Mutex::new(Some(tx));
    let handler = RcBlock::new(move |content: *mut SCShareableContent, _error: *mut NSError| {
        let content = unsafe { Retained::retain(content) };
        if let Some(tx) = tx.lock().unwrap().take() {
            let _ = tx.send(content);
        }
    });
    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
            false, true, &handler,
        )
    };
    rx.await.ok().flatten()
}

async fn capture_image(
    filter: &SCContentFilter,
    config: &SCStreamConfiguration,
) -> Option<CFRetained<CGImage>> {
    let (tx, rx) = oneshot::channel();
    let tx = Mutex::new(Some(tx));
    let handler = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
        if let Some(error) = unsafe { error.as_ref() } {
            eprintln!("kiri: window capture failed: {}", error.localizedDescription());
        }
        let image = std::ptr::NonNull::new(image).map(|i| unsafe { CFRetained::retain(i) });
        if let Some(tx) = tx.lock().unwrap().take() {
            let _ = tx.send(image);
        }
    });
    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            filter,
            config,
            Some(&handler),
        )
    };
    rx.await.ok().flatten()
}

/// The desktop picture alone (no windows, no icons) under `source`, in display-local points.
async fn wallpaper(
    content: &SCShareableContent,
    display: CGDirectDisplayID,
    source: CGRect,
    (width, height): (usize, usize),
) -> Option<CFRetained<CGImage>> {
    let screen = unsafe { content.displays() }
        .iter()
        .find(|d| unsafe { d.displayID() } == display)?;
    let icons = CGWindowLevelForKey(CGWindowLevelKey::DesktopIconWindowLevelKey) as isize;
    let below: Vec<Retained<SCWindow>> = unsafe { content.windows() }
        .iter()
        .filter(|w| unsafe { w.windowLayer() } < icons)
        .collect();
    let filter = unsafe {
        SCContentFilter::initWithDisplay_includingWindows(
            SCContentFilter::alloc(),
            &screen,
            &NSArray::from_retained_slice(&below),
        )
    };
    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        config.setSourceRect(source);
        config.setWidth(width);
        config.setHeight(height);
        config.setShowsCursor(false);
    }
    capture_image(&filter, &config).await
}

/// Pixel rect as (x, y, width, height) from the top-left.
type PxRect = (usize, usize, usize, usize);

fn intersect(r: PxRect, (x0, y0, x1, y1): (f64, f64, f64, f64)) -> Option<PxRect> {
    let px = |v: f64| v.max(0.).round() as usize;
    let (left, top) = (r.0.max(px(x0)), r.1.max(px(y0)));
    let (right, bottom) = ((r.0 + r.2).min(px(x1)), (r.1 + r.3).min(px(y1)));
    (right > left && bottom > top).then(|| (left, top, right - left, bottom - top))
}

/// The alpha channel of a ScreenCaptureKit image, which is always BGRA.
struct Alpha {
    data: CFRetained<CFData>,
    width: usize,
    height: usize,
    stride: usize,
}

impl Alpha {
    fn of(image: &CGImage) -> Option<Self> {
        if CGImage::bits_per_pixel(Some(image)) != 32 {
            eprintln!("kiri: unexpected window capture format");
            return None;
        }
        Some(Self {
            data: CGDataProvider::data(CGImage::data_provider(Some(image)).as_deref())?,
            width: CGImage::width(Some(image)),
            height: CGImage::height(Some(image)),
            stride: CGImage::bytes_per_row(Some(image)),
        })
    }

    fn bytes(&self) -> &[u8] {
        unsafe { self.data.as_bytes_unchecked() }
    }

    fn at(&self, x: usize, y: usize) -> u8 {
        self.bytes()[y * self.stride + x * 4 + 3]
    }

    fn bounds(&self) -> Option<PxRect> {
        let raw = self.bytes();
        let (mut x0, mut y0, mut x1, mut y1) = (self.width, self.height, 0, 0);
        for y in 0..self.height {
            let row = &raw[y * self.stride..y * self.stride + self.width * 4];
            let Some(first) = row.chunks_exact(4).position(|p| p[3] != 0) else { continue };
            let last = row.chunks_exact(4).rposition(|p| p[3] != 0).unwrap_or(first);
            (x0, x1) = (x0.min(first), x1.max(last + 1));
            (y0, y1) = (y0.min(y), y + 1);
        }
        (x1 > x0).then(|| (x0, y0, x1 - x0, y1 - y0))
    }

    /// Where the window sits inside its shadow. The shadow is symmetric sideways but
    /// pushed down by an amount nothing reports, so the top edge is the sharpest alpha
    /// step down the middle column: shadows fade in smoothly, window edges don't.
    fn window_origin(&self, (x, y, w, h): PxRect, (ww, wh): (f64, f64)) -> (f64, f64) {
        let left = x as f64 + (w as f64 - ww) / 2.;
        let column = ((left + ww / 2.) as usize).min(self.width - 1);
        let last = (y + (h as f64 - wh).max(0.) as usize).min(self.height - 1);
        let top = (y..=last)
            .max_by_key(|&row| {
                let above = if row > 0 { self.at(column, row - 1) } else { 0 };
                self.at(column, row) as i32 - above as i32
            })
            .unwrap_or(y);
        (left, top as f64)
    }
}

enum Backdrop {
    Color(u32),
    Image(CFRetained<CGImage>),
}

/// Cuts `crop` out of `window` and flattens it over the backdrop into premultiplied BGRA.
/// A backdrop image covers exactly the crop.
fn compose(
    window: &CGImage,
    (x, y, w, h): PxRect,
    backdrop: Option<Backdrop>,
    display: CGDirectDisplayID,
    scale: f32,
) -> Option<Shot> {
    let space = CGImage::color_space(Some(window))
        .or_else(|| CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })))?;
    let mut bgra = vec![0u8; w * h * 4];
    let ctx = unsafe {
        CGBitmapContextCreate(
            bgra.as_mut_ptr().cast(),
            w,
            h,
            8,
            w * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0,
        )
    }?;
    let all = CGRect::new(CGPoint::ZERO, CGSize::new(w as f64, h as f64));
    match backdrop {
        Some(Backdrop::Color(rgb)) => {
            let [r, g, b] = [rgb >> 16, rgb >> 8, rgb].map(|c| (c & 255) as f64 / 255.);
            let color = CGColor::new_srgb(r, g, b, 1.);
            CGContext::set_fill_color_with_color(Some(&ctx), Some(&color));
            CGContext::fill_rect(Some(&ctx), all);
        }
        Some(Backdrop::Image(image)) => CGContext::draw_image(Some(&ctx), all, Some(&image)),
        None => {}
    }
    let (iw, ih) = (CGImage::width(Some(window)), CGImage::height(Some(window)));
    // CoreGraphics is bottom-left based.
    let at = CGRect::new(
        CGPoint::new(-(x as f64), (y + h) as f64 - ih as f64),
        CGSize::new(iw as f64, ih as f64),
    );
    CGContext::draw_image(Some(&ctx), at, Some(window));
    drop(ctx);
    Some(Shot { display, width: w as u32, height: h as u32, scale, bgra })
}
