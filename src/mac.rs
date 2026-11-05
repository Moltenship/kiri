//! Everything that talks to AppKit, CoreGraphics or ScreenCaptureKit directly.

use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::Mutex;

use block2::RcBlock;
use futures::channel::oneshot;
use gpui_kit::Window;
use objc2::runtime::ProtocolObject;
use objc2::AnyThread;
use objc2_app_kit::{
    NSApplication, NSColor, NSCursor, NSEventModifierFlags, NSFloatingWindowLevel, NSImage, NSNormalWindowLevel, NSImageView, NSPaperOrientation, NSPasteboard, NSPasteboardItem, NSPasteboardTypeFileURL,
    NSPasteboardTypePNG, NSPrintOperation, NSPrintPanelOptions, NSPrintingPaginationMode, NSScreenSaverWindowLevel, NSStatusWindowLevel, NSView,
    NSWindowStyleMask,
};
use objc2_core_foundation::{
    CFAttributedString, CFBoolean, CFDictionary, CFRetained, CFString, CFType, CGPoint, CGRect,
    CGSize,
};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGColorSpace, CGContext, CGDataProvider, CGDirectDisplayID,
    CGDisplayBounds, CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGGetDisplaysWithPoint, CGImage, CGImageAlphaInfo, CGMainDisplayID,
    CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess, CGAssociateMouseAndMouseCursorPosition,
    CGWarpMouseCursorPosition, kCGColorSpaceSRGB,
};
use objc2_core_text::{
    CTFont, CTFontUIFontType, CTLine, kCTFontAttributeName,
    kCTForegroundColorFromContextAttributeName,
};
use objc2_foundation::{NSArray, NSData, NSError, NSString, NSURL};
use objc2_screen_capture_kit::SCScreenshotManager;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// One display frozen at capture time, as tightly packed BGRA.
pub struct Shot {
    pub display: CGDirectDisplayID,
    pub width: u32,
    pub height: u32,
    /// Pixels per point for this display.
    pub scale: f32,
    pub bgra: Vec<u8>,
}

pub fn ensure_screen_access() -> bool {
    CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess()
}

/// `captureImageInRect` (macOS 15.2+) skips the slow `SCShareableContent`
/// round trip that a filter-based screenshot needs.
pub async fn capture_display(display: CGDirectDisplayID) -> Option<Shot> {
    capture_rect(display, CGDisplayBounds(display)).await
}

/// `(x, y, w, h)` in global top-left points, on `display`.
pub async fn capture_area(display: CGDirectDisplayID, (x, y, w, h): (f32, f32, f32, f32)) -> Option<Shot> {
    capture_rect(display, CGRect::new(CGPoint::new(x as f64, y as f64), CGSize::new(w as f64, h as f64))).await
}

async fn capture_rect(display: CGDirectDisplayID, bounds: CGRect) -> Option<Shot> {
    let (tx, rx) = oneshot::channel();
    let tx = Mutex::new(Some(tx));
    let handler = RcBlock::new(move |image: *mut CGImage, _error: *mut NSError| {
        // Runs on a ScreenCaptureKit queue, so the pixel copy stays off the main thread.
        let shot = unsafe { image.as_ref() }.and_then(|image| to_bgra(display, bounds, image));
        if let Some(tx) = tx.lock().unwrap().take() {
            let _ = tx.send(shot);
        }
    });
    unsafe { SCScreenshotManager::captureImageInRect_completionHandler(bounds, Some(&handler)) };
    rx.await.ok().flatten()
}

fn to_bgra(display: CGDirectDisplayID, bounds: CGRect, image: &CGImage) -> Option<Shot> {
    let width = CGImage::width(Some(image));
    let height = CGImage::height(Some(image));
    let stride = CGImage::bytes_per_row(Some(image));
    if CGImage::bits_per_pixel(Some(image)) != 32 {
        eprintln!(
            "kiri: unexpected capture format, {} bpp",
            CGImage::bits_per_pixel(Some(image))
        );
        return None;
    }
    let data = CGDataProvider::data(CGImage::data_provider(Some(image)).as_deref())?;
    let raw = unsafe { data.as_bytes_unchecked() };
    let row = width * 4;
    let mut bgra = Vec::with_capacity(row * height);
    for y in 0..height {
        bgra.extend_from_slice(&raw[y * stride..y * stride + row]);
    }
    Some(Shot {
        display,
        width: width as u32,
        height: height as u32,
        scale: (width as f64 / bounds.size.width) as f32,
        bgra,
    })
}

/// Lifts the overlay above the menu bar and Dock and stretches it over the whole
/// display. GPUI creates even title-less windows as `Titled`, and AppKit keeps
/// titled windows below the menu bar, so the style must change first.
pub fn make_overlay(window: &Window, display: CGDirectDisplayID) {
    let Some(view) = ns_view(window) else { return };
    let Some(ns_window) = view.window() else {
        return;
    };
    let main = CGDisplayBounds(CGMainDisplayID());
    let b = CGDisplayBounds(display);
    // CoreGraphics is top-left based, AppKit is bottom-left of the main display.
    let frame = CGRect::new(
        CGPoint::new(b.origin.x, main.size.height - b.origin.y - b.size.height),
        CGSize::new(b.size.width, b.size.height),
    );
    ns_window.setStyleMask(NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel);
    ns_window.setLevel(NSScreenSaverWindowLevel);
    ns_window.setHasShadow(false);
    // The new style mask rebuilds the frame view with an opaque background; the live screen must show through.
    ns_window.setOpaque(false);
    ns_window.setBackgroundColor(Some(&NSColor::clearColor()));
    // A transparent window lets clicks through its clear pixels unless told otherwise.
    ns_window.setIgnoresMouseEvents(false);
    ns_window.setFrame_display(frame, true);
    ns_window.makeKeyAndOrderFront(None);
}

/// The system's own screenshot cursors, the ones ⇧⌘4 shows: a crosshair for an area and
/// a camera for a window. They are read from the OS at run time, never shipped.
fn screenshot_cursor(window_pick: bool) -> Option<objc2::rc::Retained<NSCursor>> {
    thread_local! {
        static CURSORS: [Option<objc2::rc::Retained<NSCursor>>; 2] = [load_cursor("screenshotselection", 15., 15.), load_cursor("screenshotwindow", 14., 11.)];
    }
    CURSORS.with(|c| c[window_pick as usize].clone())
}

pub fn has_screenshot_cursor() -> bool {
    screenshot_cursor(false).is_some()
}

/// `hot` comes from the cursor's info.plist, in points from the top-left.
fn load_cursor(name: &str, hot_x: f64, hot_y: f64) -> Option<objc2::rc::Retained<NSCursor>> {
    const DIR: &str = "/System/Library/Frameworks/ApplicationServices.framework/Versions/A/Frameworks/HIServices.framework/Versions/A/Resources/cursors";
    let path = NSString::from_str(&format!("{DIR}/{name}/cursor.pdf"));
    let Some(image) = NSImage::initWithContentsOfFile(NSImage::alloc(), &path) else {
        eprintln!("kiri: no system cursor {name}");
        return None;
    };
    Some(NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &image, CGPoint::new(hot_x, hot_y)))
}

/// Shows the system screenshot cursor over an overlay window. Kiri isn't the active app
/// while selecting, so only a cursor rect sticks, and AppKit only applies the rects a view
/// gives from `resetCursorRects`. The overlay's view gets a subclass whose rect is ours.
pub fn set_screenshot_cursor(window: &Window, window_pick: bool) -> bool {
    use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
    use std::cell::Cell;
    thread_local! {
        static PICK: Cell<bool> = const { Cell::new(false) };
        static CLASS: Cell<Option<&'static AnyClass>> = const { Cell::new(None) };
    }
    extern "C-unwind" fn reset_cursor_rects(this: &AnyObject, _: Sel) {
        // SAFETY: only overlay views, which are NSViews, get this subclass.
        let view = unsafe { &*(this as *const AnyObject as *const NSView) };
        if let Some(cursor) = screenshot_cursor(PICK.get()) {
            view.addCursorRect_cursor(view.bounds(), &cursor);
        }
    }
    let (Some(_), Some(view)) = (screenshot_cursor(window_pick), ns_view(window)) else {
        return false;
    };
    let current = view.class();
    let class = CLASS.get().filter(|c| *c == current || c.superclass() == Some(current)).or_else(|| {
        let mut builder = ClassBuilder::new(c"KiriOverlayView", current)?;
        unsafe { builder.add_method(objc2::sel!(resetCursorRects), reset_cursor_rects as extern "C-unwind" fn(_, _)) };
        let class = builder.register();
        CLASS.set(Some(class));
        Some(class)
    });
    let Some(class) = class else { return false };
    let swapped = current != class;
    if swapped {
        // SAFETY: the subclass adds no ivars, only an override, so the object layout is unchanged.
        unsafe { AnyObject::set_class(view.as_ref(), class) };
    }
    if swapped || PICK.get() != window_pick {
        PICK.set(window_pick);
        if let Some(ns_window) = view.window() {
            ns_window.invalidateCursorRectsForView(view);
        }
    }
    true
}

pub fn make_key(window: &Window) {
    if let Some(ns_window) = ns_view(window).and_then(|v| v.window()) {
        ns_window.makeKeyWindow();
    }
}

pub fn option_held() -> bool {
    CGEventSource::flags_state(CGEventSourceStateID::CombinedSessionState).contains(CGEventFlags::MaskAlternate)
}

/// Moves the pointer to global top-left points without posting an event. Re-associating
/// skips the quarter second the system would otherwise ignore the mouse after a warp.
pub fn warp_cursor(x: f32, y: f32) {
    let _ = CGWarpMouseCursorPosition(CGPoint::new(x as f64, y as f64));
    let _ = CGAssociateMouseAndMouseCursorPosition(true);
}

/// Whether a key (virtual key code) is held right now, whichever window has the keyboard.
pub fn key_pressed(code: u16) -> bool {
    CGEventSource::key_state(CGEventSourceStateID::CombinedSessionState, code)
}

/// Floating chrome-less windows: toasts, thumbnails, and pins.
pub fn make_floating(window: &Window, level: Level, shadow: bool, ignores_mouse: bool) {
    let Some(ns_window) = ns_view(window).and_then(|v| v.window()) else {
        return;
    };
    ns_window.setStyleMask(NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel);
    ns_window.setLevel(match level {
        Level::Floating => NSFloatingWindowLevel,
        Level::Status => NSStatusWindowLevel,
    });
    ns_window.setHasShadow(shadow);
    ns_window.setIgnoresMouseEvents(ignores_mouse);
    ns_window.setMovableByWindowBackground(false);
}

pub fn set_on_top(window: &Window, on: bool) {
    if let Some(ns_window) = ns_view(window).and_then(|v| v.window()) {
        ns_window.setLevel(if on { NSFloatingWindowLevel } else { NSNormalWindowLevel });
    }
}

pub fn set_opacity(window: &Window, alpha: f64) {
    if let Some(ns_window) = ns_view(window).and_then(|v| v.window()) {
        ns_window.setAlphaValue(alpha);
    }
}

pub fn set_ignores_mouse(window: &Window, ignores: bool) {
    if let Some(ns_window) = ns_view(window).and_then(|v| v.window()) {
        ns_window.setIgnoresMouseEvents(ignores);
    }
}

/// Resizes the content keeping the top-left corner put; AppKit keeps the bottom-left.
pub fn resize_from_top(window: &Window, w: f32, h: f32) {
    let Some(ns_window) = ns_view(window).and_then(|v| v.window()) else {
        return;
    };
    let frame = ns_window.frame();
    let top = frame.origin.y + frame.size.height;
    let rect = CGRect::new(
        CGPoint::new(frame.origin.x, top - h as f64),
        CGSize::new(w as f64, h as f64),
    );
    ns_window.setFrame_display(rect, true);
}

pub fn ns_view_ptr(window: &Window) -> Option<*const std::ffi::c_void> {
    ns_view(window).map(|v| v as *const NSView as *const std::ffi::c_void)
}

#[derive(Clone, Copy)]
pub enum Level {
    Floating,
    Status,
}

/// Puts the PNG and, when given, the file on the clipboard, so pasting works
/// in image editors, chat apps, and Finder alike.
pub fn copy_png(png: &[u8], file: Option<&std::path::Path>) {
    let board = NSPasteboard::generalPasteboard();
    board.clearContents();
    let item = NSPasteboardItem::new();
    let data = NSData::with_bytes(png);
    unsafe {
        item.setData_forType(&data, NSPasteboardTypePNG);
        if let Some(path) = file.and_then(|p| p.to_str()) {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path));
            if let Some(s) = url.absoluteString() {
                item.setString_forType(&s, NSPasteboardTypeFileURL);
            }
        }
    }
    let item = ProtocolObject::from_retained(item);
    board.writeObjects(&NSArray::from_retained_slice(&[item]));
}

/// Hands the menu item titled `title` to AppKit's full-screen toggle, which keeps its
/// Enter/Exit title and fn-F key in sync, as in the alternative.
pub fn adopt_full_screen_item(title: &str) {
    let Some(mtm) = objc2::MainThreadMarker::new() else { return };
    let Some(menu) = NSApplication::sharedApplication(mtm).mainMenu() else { return };
    let title = NSString::from_str(title);
    for top in menu.itemArray() {
        let Some(item) = top.submenu().and_then(|m| m.itemWithTitle(&title)) else { continue };
        unsafe {
            item.setTarget(None);
            item.setAction(Some(objc2::sel!(toggleFullScreen:)));
        }
        item.setKeyEquivalent(&NSString::from_str("f"));
        item.setKeyEquivalentModifierMask(NSEventModifierFlags::Function);
    }
}

/// Prints the PNG on one page from a sheet on the window, landscape when the image is wide.
pub fn print_png(png: &[u8], window: &Window) {
    let Some(ns_window) = ns_view(window).and_then(|v| v.window()) else { return };
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png)) else { return };
    let size = image.size();
    let view = NSImageView::imageViewWithImage(&image, objc2::MainThreadOnly::mtm(&*ns_window));
    view.setFrame(CGRect::new(CGPoint::new(0., 0.), size));
    let op = NSPrintOperation::printOperationWithView(&view);
    let info = op.printInfo();
    info.setHorizontalPagination(NSPrintingPaginationMode::Fit);
    info.setVerticalPagination(NSPrintingPaginationMode::Fit);
    info.setOrientation(if size.width > size.height { NSPaperOrientation::Landscape } else { NSPaperOrientation::Portrait });
    let panel = op.printPanel();
    panel.setOptions(panel.options() | NSPrintPanelOptions::ShowsPaperSize | NSPrintPanelOptions::ShowsOrientation | NSPrintPanelOptions::ShowsScaling);
    unsafe { op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(&ns_window, None, None, std::ptr::null_mut()) };
}

/// The clipboard's image as premultiplied BGRA plus its pixels per point, so a Retina
/// image pastes at its real size. Covers PNG, TIFF, PDF and copied image files.
pub fn clipboard_image() -> Option<(u32, u32, Vec<u8>, f32)> {
    let board = NSPasteboard::generalPasteboard();
    if !NSImage::canInitWithPasteboard(&board) {
        return None;
    }
    let image = NSImage::initWithPasteboard(NSImage::alloc(), &board)?;
    let cg = unsafe { image.CGImageForProposedRect_context_hints(std::ptr::null_mut(), None, None) }?;
    let (w, h) = (CGImage::width(Some(&cg)), CGImage::height(Some(&cg)));
    let points = image.size().width;
    if w == 0 || h == 0 || points <= 0. {
        return None;
    }
    let mut buf = vec![0u8; w * h * 4];
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))?;
    // Premultiplied BGRA: alpha first, 32-bit little-endian.
    let info = CGImageAlphaInfo::PremultipliedFirst.0 | 0x2000;
    let ctx = unsafe { CGBitmapContextCreate(buf.as_mut_ptr().cast(), w, h, 8, w * 4, Some(&space), info) }?;
    CGContext::draw_image(Some(&ctx), CGRect::new(CGPoint::new(0., 0.), CGSize::new(w as f64, h as f64)), Some(&cg));
    drop(ctx);
    Some((w as u32, h as u32, buf, (w as f64 / points) as f32))
}

fn ns_view(window: &Window) -> Option<&NSView> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::AppKit(h) => {
            let ptr: NonNull<NSView> = h.ns_view.cast();
            Some(unsafe { ptr.as_ref() })
        }
        _ => None,
    }
}

/// Top-left of the display in global points.
pub fn display_origin(display: CGDirectDisplayID) -> (f32, f32) {
    let b = CGDisplayBounds(display);
    (b.origin.x as f32, b.origin.y as f32)
}

pub fn display_points(display: CGDirectDisplayID) -> (f32, f32) {
    let b = CGDisplayBounds(display);
    (b.size.width as f32, b.size.height as f32)
}

pub fn display_under_cursor() -> CGDirectDisplayID {
    let cursor = CGEvent::location(CGEvent::new(None).as_deref());
    let (mut display, mut count) = (CGMainDisplayID(), 0u32);
    unsafe { CGGetDisplaysWithPoint(cursor, 1, &mut display, &mut count) };
    display
}

pub fn desktop_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Desktop")
}

/// Tags a saved file the way macOS tags its own screenshots, so Finder and Spotlight
/// filters for screenshots find it. The value is a binary plist holding `true`.
pub fn mark_screenshot(path: &std::path::Path) {
    const TRUE_PLIST: &[u8] = b"bplist00\x09\x08\0\0\0\0\0\0\x01\x01\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x09";
    let (Ok(file), Ok(name)) = (
        std::ffi::CString::new(path.as_os_str().as_encoded_bytes()),
        std::ffi::CString::new("com.apple.metadata:kMDItemIsScreenCapture"),
    ) else {
        return;
    };
    unsafe { libc::setxattr(file.as_ptr(), name.as_ptr(), TRUE_PLIST.as_ptr().cast(), TRUE_PLIST.len(), 0, 0) };
}

/// `Kiri 2026-11-04 at 14.05.09.png` in `dir`, the same shape macOS uses for screenshots.
/// A second capture in the same second gets ` 2`, so neither overwrites the other. Names
/// handed out are remembered, since a background save writes the file only later.
pub fn timestamp_path(dir: &std::path::Path) -> PathBuf {
    static TAKEN: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&now, &mut tm) };
    let stem = format!(
        "Kiri {:04}-{:02}-{:02} at {:02}.{:02}.{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    );
    let mut taken = TAKEN.lock().unwrap_or_else(|e| e.into_inner());
    let path = (1..)
        .map(|n| dir.join(if n == 1 { format!("{stem}.png") } else { format!("{stem} {n}.png") }))
        .find(|p| !taken.contains(p) && ["png", "jpg", "jpeg", "gif"].iter().all(|ext| !p.with_extension(ext).exists()))
        .expect("an unused name");
    taken.push(path.clone());
    path
}

fn ct_line(text: &str, size: f32, bold: bool) -> Option<CFRetained<CTLine>> {
    let ui = if bold {
        CTFontUIFontType::EmphasizedSystem
    } else {
        CTFontUIFontType::System
    };
    let font = unsafe { CTFont::new_ui_font_for_language(ui, size as f64, None) }?;
    let keys: [&CFString; 2] = unsafe { [kCTFontAttributeName, kCTForegroundColorFromContextAttributeName] };
    let values: [&CFType; 2] = [font.as_ref(), CFBoolean::new(true).as_ref()];
    let attrs = CFDictionary::from_slices(&keys, &values);
    let string = CFString::from_str(text);
    let attributed =
        unsafe { CFAttributedString::new(None, Some(&string), Some(attrs.as_ref())) }?;
    Some(unsafe { CTLine::with_attributed_string(&attributed) })
}

/// Width of one line in the system UI font, which is also what GPUI draws with.
/// Labels are measured several times a frame, so widths are cached.
pub fn text_width(text: &str, size: f32, bold: bool) -> f32 {
    thread_local! {
        static WIDTHS: std::cell::RefCell<std::collections::HashMap<(String, u32, bool), f32>> = Default::default();
    }
    WIDTHS.with_borrow_mut(|cache| {
        if let Some(&w) = cache.get(&(text.to_string(), size.to_bits(), bold)) {
            return w;
        }
        if cache.len() > 4096 {
            cache.clear();
        }
        let w = ct_line(text, size, bold).map_or(0., |line| unsafe {
            line.typographic_bounds(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
        } as f32);
        cache.insert((text.to_string(), size.to_bits(), bold), w);
        w
    })
}

/// One line of text as premultiplied RGBA, laid out in a line box of
/// `size * line_height` with the baseline where GPUI puts it.
pub fn text_image(text: &str, size: f32, line_height: f32, rgb: u32, bold: bool) -> Option<(u32, u32, Vec<u8>)> {
    let line = ct_line(text, size, bold)?;
    let (mut ascent, mut descent) = (0f64, 0f64);
    let width = unsafe { line.typographic_bounds(&mut ascent, &mut descent, std::ptr::null_mut()) };
    let (w, h) = (width.ceil() as usize + 2, (size * line_height).ceil() as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let mut buf = vec![0u8; w * h * 4];
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))?;
    let ctx = unsafe {
        CGBitmapContextCreate(
            buf.as_mut_ptr().cast(),
            w,
            h,
            8,
            w * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }?;
    let [r, g, b] = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255].map(|c| c as f64 / 255.);
    CGContext::set_rgb_fill_color(Some(&ctx), r, g, b, 1.);
    let baseline_from_top = (h as f64 - (ascent + descent)) / 2. + ascent;
    CGContext::set_text_position(Some(&ctx), 0., h as f64 - baseline_from_top);
    unsafe { line.draw(&ctx) };
    drop(ctx);
    Some((w as u32, h as u32, buf))
}
