//! The Windows counterpart of `mac.rs`, under the same names. Screens come from GDI, which
//! reads the composed desktop in physical pixels; everything else in Kiri works in the
//! logical units GPUI uses, physical pixels over the monitor's scale.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

use gpui_kit::{Window, WindowAppearance};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{CF_DIB, CF_HDROP};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_MENU};
use windows::Win32::UI::Shell::{DROPFILES, FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

/// One display frozen at capture time, as tightly packed BGRA.
pub struct Shot {
    pub display: u32,
    pub width: u32,
    pub height: u32,
    /// Pixels per logical unit for this display.
    pub scale: f32,
    pub bgra: Vec<u8>,
}

/// Must run before the first window exists; without it Windows scales Kiri's windows
/// and screen grabs come out at the scaled size.
pub fn dpi_aware() {
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

/// GPUI's display id is the `HMONITOR`; monitor handles fit in 32 bits.
fn monitor(display: u32) -> HMONITOR {
    HMONITOR(display as usize as *mut c_void)
}

/// The monitor's rectangle in physical pixels and its scale.
fn monitor_rect(display: u32) -> Option<(RECT, f32)> {
    let handle = monitor(display);
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe { GetMonitorInfoW(handle, &mut info) }.ok().ok()?;
    let (mut dpi, mut dpi_y) = (0, 0);
    unsafe { GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y) }.ok()?;
    Some((info.rcMonitor, dpi as f32 / 96.))
}

pub fn has_screen_access() -> bool {
    true
}

pub fn ensure_screen_access() -> bool {
    true
}

static APPEARANCE: AtomicU8 = AtomicU8::new(0);

/// `None` follows the system. GPUI only reads the system's, so Kiri's colors take this through
/// `appearance`, and the blur behind a window through `theme_window`.
pub fn set_appearance(dark: Option<bool>) {
    APPEARANCE.store(dark.map_or(0, |dark| 1 + dark as u8), Ordering::Relaxed);
}

pub fn appearance(system: WindowAppearance) -> WindowAppearance {
    match APPEARANCE.load(Ordering::Relaxed) {
        1 => WindowAppearance::Light,
        2 => WindowAppearance::Dark,
        _ => system,
    }
}

/// GPUI sets a window's light or dark compositor blur from the system's appearance.
pub fn theme_window(window: &Window) {
    use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
    let Some(hwnd) = hwnd(window) else { return };
    let dark = windows::core::BOOL::from(matches!(appearance(window.appearance()), WindowAppearance::Dark | WindowAppearance::VibrantDark));
    let _ = unsafe { DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const _ as *const c_void, size_of_val(&dark) as u32) };
}

mod keys;
pub use keys::{SystemKeysOff, SystemShortcut, physical_key, release_screenshot_keys, system_shortcut, system_shortcuts};
mod shortcuts;
pub use shortcuts::bind_keys;

pub async fn capture_display(display: u32) -> Option<Shot> {
    let (rect, scale) = monitor_rect(display)?;
    grab(display, rect, scale).await
}

/// `(x, y, w, h)` in global logical units, on `display`.
pub async fn capture_area(display: u32, (x, y, w, h): (f32, f32, f32, f32)) -> Option<Shot> {
    let (m, scale) = monitor_rect(display)?;
    let (ox, oy) = (m.left as f32 / scale, m.top as f32 / scale);
    let at = |v: f32| (v * scale).round() as i32;
    let (left, top) = (m.left + at(x - ox), m.top + at(y - oy));
    grab(display, RECT { left, top, right: left + at(w), bottom: top + at(h) }, scale).await
}

/// Copies the pixels on a thread of its own, so the main thread keeps painting.
async fn grab(display: u32, rect: RECT, scale: f32) -> Option<Shot> {
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(unsafe { blit(rect) }.map(|(width, height, bgra)| Shot { display, width, height, scale, bgra }));
    });
    rx.await.ok().flatten()
}

unsafe fn blit(r: RECT) -> Option<(u32, u32, Vec<u8>)> {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    if w <= 0 || h <= 0 {
        return None;
    }
    unsafe {
        let screen = GetDC(None);
        let (bgra, ok) = dib(Some(screen), w, h, |mem| {
            BitBlt(mem, 0, 0, w, h, Some(screen), r.left, r.top, SRCCOPY | CAPTUREBLT).is_ok()
        });
        ReleaseDC(None, screen);
        let mut bgra = bgra.filter(|_| ok)?;
        // GDI leaves the fourth byte undefined.
        for px in bgra.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Some((w as u32, h as u32, bgra))
    }
}

/// Runs `draw` on a top-down 32-bit DIB of `w`×`h` and returns its pixels as BGRA.
unsafe fn dib<T>(like: Option<HDC>, w: i32, h: i32, draw: impl FnOnce(HDC) -> T) -> (Option<Vec<u8>>, T) {
    unsafe {
        let mem = CreateCompatibleDC(like);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let out = draw(mem);
            let _ = DeleteDC(mem);
            return (None, out);
        };
        let old = SelectObject(mem, bitmap.into());
        let out = draw(mem);
        let _ = GdiFlush();
        let pixels = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize).to_vec();
        SelectObject(mem, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem);
        (Some(pixels), out)
    }
}

fn hwnd(window: &Window) -> Option<HWND> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut c_void)),
        _ => None,
    }
}

/// Stretches the overlay over the whole monitor, taskbar included, above everything.
pub fn make_overlay(window: &Window, display: u32) {
    let (Some(hwnd), Some((r, _))) = (hwnd(window), monitor_rect(display)) else { return };
    unsafe {
        // GPUI's pop-ups keep a border at the sides and bottom, which would inset the client
        // area from the monitor's edges.
        let style = WINDOW_STYLE(GetWindowLongPtrW(hwnd, GWL_STYLE) as u32);
        SetWindowLongPtrW(hwnd, GWL_STYLE, ((style & !(WS_CAPTION | WS_THICKFRAME)) | WS_POPUP).0 as isize);
        let flags = SWP_SHOWWINDOW | SWP_FRAMECHANGED;
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), r.left, r.top, r.right - r.left, r.bottom - r.top, flags);
        let _ = SetForegroundWindow(hwnd);
    }
}

pub fn has_screenshot_cursor() -> bool {
    false
}

pub fn set_screenshot_cursor(_window: &Window, _window_pick: bool, _numbers: Option<[i32; 2]>) -> bool {
    false
}

pub fn make_key(window: &Window) {
    if let Some(hwnd) = hwnd(window) {
        let _ = unsafe { SetForegroundWindow(hwnd) };
    }
}

pub fn option_held() -> bool {
    unsafe { GetAsyncKeyState(VK_MENU.0 as i32) < 0 }
}

/// Moves the pointer to global logical units on the monitor under it.
pub fn warp_cursor(x: f32, y: f32) {
    let Some((m, scale)) = monitor_rect(display_under_cursor()) else { return };
    let at = |v: f32, origin: i32| origin + ((v - origin as f32 / scale) * scale).round() as i32;
    let _ = unsafe { SetCursorPos(at(x, m.left), at(y, m.top)) };
}

/// Whether a key is held right now. `code` is the macOS key code the shared code uses.
pub fn key_pressed(code: u16) -> bool {
    let vk = match code {
        18 => b'1',
        19 => b'2',
        48 => 0x09,
        49 => 0x20,
        123 => 0x25,
        124 => 0x27,
        125 => 0x28,
        126 => 0x26,
        _ => return false,
    };
    unsafe { GetAsyncKeyState(vk as i32) < 0 }
}

#[derive(Clone, Copy)]
pub enum Level {
    Floating,
    Status,
}

fn ex_style(hwnd: HWND, on: WINDOW_EX_STYLE, off: WINDOW_EX_STYLE) {
    unsafe {
        let old = WINDOW_EX_STYLE(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32);
        let new = (old | on) & !off;
        if new == old {
            return;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new.0 as isize);
        // A window that just became layered draws nothing until it has an alpha.
        if new.contains(WS_EX_LAYERED) && !old.contains(WS_EX_LAYERED) {
            let _ = SetLayeredWindowAttributes(hwnd, Default::default(), 255, LWA_ALPHA);
        }
    }
}

/// Floating chrome-less windows: toasts, thumbnails, and pins.
pub fn make_floating(window: &Window, _level: Level, shadow: bool, ignores_mouse: bool) {
    use windows::Win32::Graphics::Dwm::{DWMNCRP_DISABLED, DWMWA_NCRENDERING_POLICY, DwmSetWindowAttribute};
    let Some(hwnd) = hwnd(window) else { return };
    ex_style(hwnd, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, WS_EX_APPWINDOW);
    // The compositor's shadow reaches past the window, into a scrolling capture's frames under its HUD.
    if !shadow {
        let off = DWMNCRP_DISABLED;
        let _ = unsafe { DwmSetWindowAttribute(hwnd, DWMWA_NCRENDERING_POLICY, &off as *const _ as *const c_void, size_of_val(&off) as u32) };
    }
    set_ignores_mouse(window, ignores_mouse);
    set_on_top(window, true);
}

pub fn set_on_top(window: &Window, on: bool) {
    let Some(hwnd) = hwnd(window) else { return };
    let after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
    let _ = unsafe { SetWindowPos(hwnd, Some(after), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
}

pub fn set_opacity(window: &Window, alpha: f64) {
    let Some(hwnd) = hwnd(window) else { return };
    ex_style(hwnd, WS_EX_LAYERED, WINDOW_EX_STYLE(0));
    let _ = unsafe { SetLayeredWindowAttributes(hwnd, Default::default(), (alpha.clamp(0., 1.) * 255.) as u8, LWA_ALPHA) };
}

/// Clicks reach a window through it only when it is both layered and transparent.
pub fn set_ignores_mouse(window: &Window, ignores: bool) {
    let Some(hwnd) = hwnd(window) else { return };
    match ignores {
        true => ex_style(hwnd, WS_EX_LAYERED | WS_EX_TRANSPARENT, WINDOW_EX_STYLE(0)),
        false => ex_style(hwnd, WINDOW_EX_STYLE(0), WS_EX_TRANSPARENT),
    }
}

/// Windows already keeps the top-left corner when a window resizes.
pub fn resize_from_top(window: &Window, w: f32, h: f32) {
    let Some(hwnd) = hwnd(window) else { return };
    let scale = unsafe { GetDpiForWindow(hwnd) } as f32 / 96.;
    let (w, h) = ((w * scale).round() as i32, (h * scale).round() as i32);
    let _ = unsafe { SetWindowPos(hwnd, None, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) };
}

/// Moves the window with the mouse as its title bar would, once gpui is done with the press.
pub fn start_window_move(window: &Window) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            let _ = ReleaseCapture();
            let _ = PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM((SC_MOVE | HTCAPTION) as usize), LPARAM(0));
        }
    }
}

/// Minimize, maximize, and close for a window that draws its own title bar. Windows runs
/// their clicks and snap layouts from the areas they mark, as it does for its own.
pub fn caption_buttons(window: &Window, cx: &gpui_kit::App) -> impl gpui_kit::IntoElement {
    use gpui_kit::component::ActiveTheme;
    use gpui_kit::prelude::FluentBuilder;
    use gpui_kit::*;
    let (fg, hover) = (cx.theme().foreground, cx.theme().secondary_hover);
    let button = |area: WindowControlArea, glyph: &'static str| {
        let close = matches!(area, WindowControlArea::Close);
        div()
            .id(glyph)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .font_family("Segoe MDL2 Assets")
            .text_size(px(10.))
            .text_color(fg)
            .hover(move |s| if close { s.bg(rgb(0xC42B1C)).text_color(white()) } else { s.bg(hover) })
            .window_control_area(area)
            .child(glyph)
    };
    let max = if window.is_maximized() { "\u{E923}" } else { "\u{E922}" };
    let style = hwnd(window).map_or(0, |h| unsafe { GetWindowLongW(h, GWL_STYLE) } as u32);
    let has = |flag: WINDOW_STYLE| style & flag.0 != 0;
    // Occluding keeps a drag area underneath from claiming the buttons' clicks.
    div()
        .id("caption-buttons")
        .occlude()
        .flex()
        .flex_none()
        .h_full()
        .when(has(WS_MINIMIZEBOX), |d| d.child(button(WindowControlArea::Min, "\u{E921}")))
        .when(has(WS_MAXIMIZEBOX), |d| d.child(button(WindowControlArea::Max, max)))
        .child(button(WindowControlArea::Close, "\u{E8BB}"))
}

/// The `HWND`, which is what a context menu needs here.
pub fn ns_view_ptr(window: &Window) -> Option<*const c_void> {
    hwnd(window).map(|h| h.0 as *const c_void)
}

fn png_format() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("PNG")) }
}

/// Puts the PNG, a bitmap for apps that only read those, and the file when given on the
/// clipboard, so pasting works in image editors, chat apps, and Explorer alike.
pub fn copy_png(png: &[u8], file: Option<&Path>) {
    let bitmap = crate::export::decode_rgba(png).map(|(w, h, rgba, _)| dib_bytes(w, h, &rgba));
    unsafe {
        if OpenClipboard(None).is_err() {
            eprintln!("kiri: the clipboard is busy");
            return;
        }
        let _ = EmptyClipboard();
        put(png_format(), png);
        if let Some(bitmap) = bitmap {
            put(CF_DIB.0 as u32, &bitmap);
        }
        if let Some(file) = file {
            put(CF_HDROP.0 as u32, &drop_files(file));
        }
        let _ = CloseClipboard();
    }
}

unsafe fn put(format: u32, bytes: &[u8]) {
    unsafe {
        let Ok(mem) = GlobalAlloc(GMEM_MOVEABLE, bytes.len()) else { return };
        let ptr = GlobalLock(mem);
        if ptr.is_null() {
            let _ = GlobalFree(Some(mem));
            return;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast(), bytes.len());
        let _ = GlobalUnlock(mem);
        // The clipboard owns the memory once this succeeds.
        if SetClipboardData(format, Some(HANDLE(mem.0))).is_err() {
            let _ = GlobalFree(Some(mem));
        }
    }
}

/// A bottom-up 32-bit `CF_DIB`: the header, then the rows last to first.
fn dib_bytes(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let header = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w as i32,
        biHeight: h as i32,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: w * h * 4,
        ..Default::default()
    };
    let mut out = Vec::with_capacity(size_of::<BITMAPINFOHEADER>() + rgba.len());
    out.extend_from_slice(unsafe {
        std::slice::from_raw_parts((&header as *const BITMAPINFOHEADER).cast::<u8>(), size_of::<BITMAPINFOHEADER>())
    });
    for row in rgba.chunks_exact(w as usize * 4).rev() {
        out.extend(row.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]));
    }
    out
}

fn drop_files(file: &Path) -> Vec<u8> {
    let header = DROPFILES { pFiles: size_of::<DROPFILES>() as u32, fWide: true.into(), ..Default::default() };
    let mut out = unsafe {
        std::slice::from_raw_parts((&header as *const DROPFILES).cast::<u8>(), size_of::<DROPFILES>())
    }
    .to_vec();
    use std::os::windows::ffi::OsStrExt;
    // The list ends with an empty name.
    for unit in file.as_os_str().encode_wide().chain([0, 0]) {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

pub fn adopt_full_screen_item(_title: &str) {}

mod print;

pub fn print_png(png: &[u8], window: &Window) {
    let (png, over) = (png.to_vec(), hwnd(window).map(|h| h.0 as usize));
    std::thread::spawn(move || {
        if let Err(err) = print::print(&png, over) {
            eprintln!("kiri: print: {err}");
        }
    });
}

/// The clipboard's PNG as premultiplied BGRA plus its pixels per logical unit.
pub fn clipboard_image() -> Option<(u32, u32, Vec<u8>, f32)> {
    let bytes = unsafe {
        OpenClipboard(None).ok()?;
        let bytes = GetClipboardData(png_format()).ok().and_then(|data| {
            let mem = HGLOBAL(data.0);
            let ptr = GlobalLock(mem);
            (!ptr.is_null()).then(|| {
                let bytes = std::slice::from_raw_parts(ptr as *const u8, GlobalSize(mem)).to_vec();
                let _ = GlobalUnlock(mem);
                bytes
            })
        });
        let _ = CloseClipboard();
        bytes?
    };
    let (w, h, mut px, scale) = crate::export::decode_rgba(&bytes)?;
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        let [r, g, b] = [p[0], p[1], p[2]].map(|c| (c as u32 * a / 255) as u8);
        p.copy_from_slice(&[b, g, r, a as u8]);
    }
    Some((w, h, px, scale))
}

/// Top-left of the display in global logical units, as GPUI places windows.
pub fn display_origin(display: u32) -> (f32, f32) {
    monitor_rect(display).map_or((0., 0.), |(r, s)| (r.left as f32 / s, r.top as f32 / s))
}

pub fn display_points(display: u32) -> (f32, f32) {
    monitor_rect(display).map_or((0., 0.), |(r, s)| ((r.right - r.left) as f32 / s, (r.bottom - r.top) as f32 / s))
}

pub fn display_under_cursor() -> u32 {
    let mut at = POINT::default();
    let _ = unsafe { GetCursorPos(&mut at) };
    unsafe { MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST) }.0 as usize as u32
}

pub fn desktop_dir() -> PathBuf {
    unsafe { SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None) }
        .ok()
        .and_then(|p| {
            let path = unsafe { p.to_string() }.ok();
            unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const c_void)) };
            path
        })
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default().join("Desktop"))
}

pub fn mark_screenshot(_path: &Path) {}

/// Drags `path` into other apps as Explorer drags a file, until it's dropped.
pub fn drag_file_out(path: PathBuf, cx: &mut gpui_kit::App) {
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{DROPEFFECT_COPY, IDropSource};
    use windows::Win32::UI::Shell::{BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop};
    let wide = windows::core::HSTRING::from(path.as_os_str());
    // The shell's drag runs a modal loop, which must not run Kiri's window messages mid-update.
    cx.spawn(async move |_| unsafe {
        let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&wide, None) else { return };
        let Ok(data) = item.BindToHandler::<_, IDataObject>(None, &BHID_DataObject) else { return };
        let _ = SHDoDragDrop(None, &data, None::<&IDropSource>, DROPEFFECT_COPY);
    })
    .detach();
}

/// `template` with strftime fields filled in, without the characters Windows file names
/// can't hold. A field chrono doesn't know leaves the template as typed.
pub fn strftime(template: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    if write!(out, "{}", chrono::Local::now().format(template)).is_err() {
        out = template.to_string();
    }
    out.replace(['\\', '*', '?', '"', '<', '>', '|'], "-")
}

/// Whether the taskbar is light, so the tray glyph can take the opposite ink.
pub fn light_taskbar() -> bool {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    let read = unsafe {
        windows::Win32::System::Registry::RegGetValueW(
            windows::Win32::System::Registry::HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            windows::Win32::System::Registry::RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    read.is_ok() && value != 0
}

fn font(size: f32, bold: bool) -> HFONT {
    let weight = if bold { FW_SEMIBOLD } else { FW_NORMAL };
    unsafe {
        CreateFontW(
            -(size.round() as i32).max(1),
            0,
            0,
            0,
            weight.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_TT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        )
    }
}

/// Runs `f` on a memory DC with the UI font selected.
fn with_font<T>(size: f32, bold: bool, f: impl FnOnce(HDC) -> T) -> T {
    unsafe {
        let dc = CreateCompatibleDC(None);
        let font = font(size, bold);
        let old = SelectObject(dc, font.into());
        let out = f(dc);
        SelectObject(dc, old);
        let _ = DeleteObject(font.into());
        let _ = DeleteDC(dc);
        out
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// Width of one line in the UI font. Labels are measured several times a frame, so widths are cached.
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
        let w = with_font(size, bold, |dc| {
            let mut extent = SIZE::default();
            let _ = unsafe { GetTextExtentPoint32W(dc, &wide(text), &mut extent) };
            extent.cx as f32
        });
        cache.insert((text.to_string(), size.to_bits(), bold), w);
        w
    })
}

/// One line of text as premultiplied RGBA, laid out in a line box of `size * line_height`
/// with the line centered in it. GDI draws white on black and the gray becomes the coverage.
pub fn text_image(text: &str, size: f32, line_height: f32, rgb: u32, bold: bool) -> Option<(u32, u32, Vec<u8>)> {
    let w = text_width(text, size, bold).ceil() as i32 + 2;
    let text = wide(text);
    let h = (size * line_height).ceil() as i32;
    if w <= 2 || h <= 0 {
        return None;
    }
    let (pixels, ()) = unsafe {
        dib(None, w, h, |dc| {
            let font = font(size, bold);
            let old = SelectObject(dc, font.into());
            let mut metrics = TEXTMETRICW::default();
            let _ = GetTextMetricsW(dc, &mut metrics);
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, windows::Win32::Foundation::COLORREF(0xFFFFFF));
            let top = (h - metrics.tmAscent - metrics.tmDescent) / 2;
            let _ = TextOutW(dc, 0, top, &text);
            SelectObject(dc, old);
            let _ = DeleteObject(font.into());
        })
    };
    let [r, g, b] = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255];
    let rgba = pixels?
        .chunks_exact(4)
        .flat_map(|p| {
            let a = p[0].max(p[1]).max(p[2]) as u32;
            [r * a / 255, g * a / 255, b * a / 255, a].map(|c| c as u8)
        })
        .collect();
    Some((w as u32, h as u32, rgba))
}

/// A server end of the named pipe `name` with a client connected, as a file to read.
pub fn accept_pipe(name: &Path) -> Option<std::fs::File> {
    use std::os::windows::io::FromRawHandle;
    use windows::Win32::Storage::FileSystem::PIPE_ACCESS_INBOUND;
    use windows::Win32::System::Pipes::*;
    let name: Vec<u16> = {
        use std::os::windows::ffi::OsStrExt;
        name.as_os_str().encode_wide().chain([0]).collect()
    };
    unsafe {
        let pipe = CreateNamedPipeW(
            windows::core::PCWSTR(name.as_ptr()),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            0,
            4096,
            0,
            None,
        );
        if pipe.is_invalid() {
            return None;
        }
        // A client that connected between create and connect reports an error, but is connected.
        let _ = ConnectNamedPipe(pipe, None);
        Some(std::fs::File::from_raw_handle(pipe.0))
    }
}

const RUN_KEY: windows::core::PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");

fn run_name() -> windows::core::PCWSTR {
    if crate::settings::dev() { w!("Kiri Dev") } else { w!("Kiri") }
}

pub fn launch_at_login() -> bool {
    use windows::Win32::System::Registry::*;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, run_name(), RRF_RT_REG_SZ, None, None, None) }.is_ok()
}

pub fn set_launch_at_login(on: bool) -> Result<(), String> {
    use windows::Win32::System::Registry::*;
    let done = if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let value: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().chain([0]).collect();
        unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, RUN_KEY, run_name(), REG_SZ.0, Some(value.as_ptr().cast()), (value.len() * 2) as u32) }
    } else {
        match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, run_name()) } {
            windows::Win32::Foundation::ERROR_FILE_NOT_FOUND => Default::default(),
            other => other,
        }
    };
    done.ok().map_err(|e| e.message())
}

pub mod window_capture;

pub mod scrolling;

pub mod ocr;

pub mod update;

