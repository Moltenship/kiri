//! Kiri's own connection to the X server, which reads the root window and places Kiri's windows
//! by their X ids. An X screen spans every monitor and GPUI reports it as one display, so
//! display ids here are always 0. On Wayland there is no connection and every call does nothing.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use gpui_kit::{App, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::connection::Connection;
use x11rb::protocol::shape::{self, SK, SO};
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::{Level, Shot};

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        _NET_ACTIVE_WINDOW,
        _NET_CLIENT_LIST_STACKING,
        _NET_CURRENT_DESKTOP,
        _NET_FRAME_EXTENTS,
        _NET_WM_DESKTOP,
        _NET_WM_NAME,
        _NET_WM_PID,
        _NET_WM_STATE,
        _NET_WM_STATE_ABOVE,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_WINDOW_OPACITY,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_DOCK,
        _GTK_FRAME_EXTENTS,
        _XROOTPMAP_ID,
        UTF8_STRING,
    }
}

pub struct X {
    pub conn: RustConnection,
    pub root: u32,
    pub size: (u16, u16),
    pub atoms: Atoms,
}

static X: OnceLock<Option<X>> = OnceLock::new();
static SCALE: AtomicU32 = AtomicU32::new(0x3F80_0000);

pub fn x() -> Option<&'static X> {
    if !super::direct() {
        return None;
    }
    X.get_or_init(|| connect().inspect_err(|e| eprintln!("kiri: X11: {e}")).ok()).as_ref()
}

fn connect() -> Result<X, Box<dyn std::error::Error>> {
    let (conn, screen) = x11rb::connect(None)?;
    let s = &conn.setup().roots[screen];
    let (root, size) = (s.root, (s.width_in_pixels, s.height_in_pixels));
    let atoms = Atoms::new(&conn)?.reply()?;
    Ok(X { conn, root, size, atoms })
}

/// GPUI's scale on X11 is one for the whole client, so it is the root's pixels over its display's points.
pub fn init(cx: &App) {
    let (Some(x), Some(display)) = (x(), cx.primary_display()) else { return };
    let points = f32::from(display.bounds().size.width);
    if points > 0. {
        SCALE.store((x.size.0 as f32 / points).to_bits(), Ordering::Relaxed);
    }
}

pub fn scale() -> f32 {
    f32::from_bits(SCALE.load(Ordering::Relaxed))
}

fn xid(window: &Window) -> Option<u32> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        RawWindowHandle::Xlib(h) => Some(h.window as u32),
        _ => None,
    }
}

type Done = Result<(), Box<dyn std::error::Error>>;

/// Runs `f` on the window's X id and sends it to the server. Failures only mean the window is gone.
fn with(window: &Window, f: impl FnOnce(&X, u32) -> Done) {
    let (Some(x), Some(id)) = (x(), xid(window)) else { return };
    let _ = f(x, id);
    let _ = x.conn.flush();
}

fn px(v: f32) -> i32 {
    (v * scale()).round() as i32
}

pub async fn capture_display(display: u32) -> Option<Shot> {
    let x = x()?;
    grab(display, x.root, (0, 0, x.size.0, x.size.1)).await
}

/// `(x, y, w, h)` in global logical units.
pub async fn capture_area(display: u32, (ax, ay, aw, ah): (f32, f32, f32, f32)) -> Option<Shot> {
    let x = x()?;
    let (left, top) = (px(ax).clamp(0, x.size.0 as i32), px(ay).clamp(0, x.size.1 as i32));
    let (right, bottom) = ((px(ax) + px(aw)).min(x.size.0 as i32), (px(ay) + px(ah)).min(x.size.1 as i32));
    if right <= left || bottom <= top {
        return None;
    }
    grab(display, x.root, (left as i16, top as i16, (right - left) as u16, (bottom - top) as u16)).await
}

/// The RandR monitor under the pointer, in `capture_area`'s units. `None` before RandR 1.5.
pub fn monitor_under_cursor() -> Option<(f32, f32, f32, f32)> {
    use x11rb::protocol::randr::ConnectionExt as _;
    let x = x()?;
    let p = x.conn.query_pointer(x.root).ok()?.reply().ok()?;
    let (px, py) = (p.root_x as i32, p.root_y as i32);
    let monitors = x.conn.randr_get_monitors(x.root, true).ok()?.reply().ok()?.monitors;
    let m = monitors.iter().find(|m| {
        let (mx, my) = (m.x as i32, m.y as i32);
        (mx..mx + m.width as i32).contains(&px) && (my..my + m.height as i32).contains(&py)
    })?;
    let s = scale();
    Some((m.x as f32 / s, m.y as f32 / s, m.width as f32 / s, m.height as f32 / s))
}

/// Reads on a thread of its own, so the main thread keeps painting.
async fn grab(display: u32, drawable: u32, rect: (i16, i16, u16, u16)) -> Option<Shot> {
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let shot = read(drawable, rect).map(|(width, height, bgra)| Shot { display, width, height, scale: scale(), bgra });
        let _ = tx.send(shot);
    });
    rx.await.ok().flatten()
}

/// Pixels of `drawable` as BGRA. Only a 32-bit drawable has alpha; the rest come out opaque.
pub fn read(drawable: u32, (x0, y0, w, h): (i16, i16, u16, u16)) -> Option<(u32, u32, Vec<u8>)> {
    let x = x()?;
    if w == 0 || h == 0 {
        return None;
    }
    let image = x.conn.get_image(ImageFormat::Z_PIXMAP, drawable, x0, y0, w, h, !0).ok()?.reply();
    let image = image.inspect_err(|e| eprintln!("kiri: X11 GetImage: {e}")).ok()?;
    let setup = x.conn.setup();
    let bpp = setup.pixmap_formats.iter().find(|f| f.depth == image.depth).map_or(0, |f| f.bits_per_pixel);
    let (w, h) = (w as u32, h as u32);
    // Every desktop X server in use stores 24- and 32-bit pixels as little-endian 32-bit words.
    if bpp != 32 || setup.image_byte_order != ImageOrder::LSB_FIRST || image.data.len() < (w * h * 4) as usize {
        eprintln!("kiri: X11 pixels of depth {} at {bpp} bits aren't supported", image.depth);
        return None;
    }
    let mut bgra = image.data;
    bgra.truncate((w * h * 4) as usize);
    if image.depth != 32 {
        for p in bgra.chunks_exact_mut(4) {
            p[3] = 255;
        }
    }
    Some((w, h, bgra))
}

/// Covers the whole X screen, above everything, with the keyboard.
pub fn make_overlay(window: &Window, _display: u32) {
    with(window, |x, id| {
        let at = ConfigureWindowAux::new().x(0).y(0).width(x.size.0 as u32).height(x.size.1 as u32).stack_mode(StackMode::ABOVE);
        x.conn.configure_window(id, &at)?;
        x.conn.set_input_focus(InputFocus::PARENT, id, x11rb::CURRENT_TIME)?;
        Ok(())
    });
}

/// Pop-ups skip the window manager, so they take the keyboard themselves.
pub fn make_key(window: &Window) {
    with(window, |x, id| {
        x.conn.set_input_focus(InputFocus::PARENT, id, x11rb::CURRENT_TIME)?;
        Ok(())
    });
}

/// Toasts, thumbnails, pins, and the scrolling HUD are GPUI pop-ups, which X11 places 2 px right
/// of where they were asked for; this puts them back.
pub fn make_floating(window: &Window, _level: Level, _shadow: bool, ignores_mouse: bool) {
    let origin = window.bounds().origin;
    with(window, |x, id| {
        let at = ConfigureWindowAux::new().x(px(origin.x.as_f32())).y(px(origin.y.as_f32())).stack_mode(StackMode::ABOVE);
        x.conn.configure_window(id, &at)?;
        Ok(())
    });
    set_ignores_mouse(window, ignores_mouse);
}

/// A managed window asks the window manager; a pop-up has no manager and only rises.
pub fn set_on_top(window: &Window, on: bool) {
    with(window, |x, id| {
        let state = ClientMessageEvent::new(32, id, x.atoms._NET_WM_STATE, [on as u32, x.atoms._NET_WM_STATE_ABOVE, 0, 1, 0]);
        let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        x.conn.send_event(false, x.root, mask, state)?;
        if on {
            x.conn.configure_window(id, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE))?;
        }
        Ok(())
    });
}

/// The compositor reads the window's opacity from a property; without one nothing changes.
pub fn set_opacity(window: &Window, alpha: f64) {
    with(window, |x, id| {
        let opacity = (alpha.clamp(0., 1.) * u32::MAX as f64) as u32;
        match opacity {
            u32::MAX => x.conn.delete_property(id, x.atoms._NET_WM_WINDOW_OPACITY)?,
            _ => x.conn.change_property32(PropMode::REPLACE, id, x.atoms._NET_WM_WINDOW_OPACITY, AtomEnum::CARDINAL, &[opacity])?,
        };
        Ok(())
    });
}

/// An empty input shape lets clicks through to whatever is below.
pub fn set_ignores_mouse(window: &Window, ignores: bool) {
    with(window, |x, id| {
        match ignores {
            true => shape::rectangles(&x.conn, SO::SET, SK::INPUT, ClipOrdering::UNSORTED, id, 0, 0, &[])?,
            false => shape::mask(&x.conn, SO::SET, SK::INPUT, id, 0, 0, x11rb::NONE)?,
        };
        Ok(())
    });
}

/// X keeps the top-left corner when a window resizes.
pub fn resize_from_top(window: &Window, w: f32, h: f32) {
    with(window, |x, id| {
        x.conn.configure_window(id, &ConfigureWindowAux::new().width(px(w).max(1) as u32).height(px(h).max(1) as u32))?;
        Ok(())
    });
}

/// A pop-up has no window manager to move it, so it follows the pointer until the button is up.
pub fn start_window_move(window: &Window) {
    let (Some(x), Some(id)) = (x(), xid(window)) else { return };
    let popup = x.conn.get_window_attributes(id).ok().and_then(|c| c.reply().ok()).is_some_and(|a| a.override_redirect);
    if !popup {
        return window.start_window_move();
    }
    std::thread::spawn(move || {
        let pointer = || x.conn.query_pointer(x.root).ok()?.reply().ok();
        let (Some(start), Some(from)) = (pointer(), x.conn.get_geometry(id).ok().and_then(|c| c.reply().ok())) else { return };
        let mut last = (start.root_x, start.root_y);
        while let Some(p) = pointer().filter(|p| p.mask.contains(KeyButMask::BUTTON1)) {
            if (p.root_x, p.root_y) != last {
                last = (p.root_x, p.root_y);
                let (dx, dy) = ((p.root_x - start.root_x) as i32, (p.root_y - start.root_y) as i32);
                let at = ConfigureWindowAux::new().x(from.x as i32 + dx).y(from.y as i32 + dy);
                if x.conn.configure_window(id, &at).is_err() || x.conn.flush().is_err() {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(8));
        }
    });
}

/// Moves the pointer to global logical units.
pub fn warp_cursor(_display: u32, at_x: f32, at_y: f32) {
    let Some(x) = x() else { return };
    let _ = x.conn.warp_pointer(x11rb::NONE, x.root, 0, 0, 0, 0, px(at_x) as i16, px(at_y) as i16);
    let _ = x.conn.flush();
}

pub fn option_held() -> bool {
    pointer_mask().is_some_and(|m| m.contains(KeyButMask::MOD1))
}

fn pointer_mask() -> Option<KeyButMask> {
    let x = x()?;
    Some(x.conn.query_pointer(x.root).ok()?.reply().ok()?.mask)
}

/// Whether a key is held right now. `code` is the macOS key code the shared code uses.
pub fn key_pressed(code: u16) -> bool {
    let keysym = match code {
        18 => 0x31,
        19 => 0x32,
        48 => 0xFF09,
        49 => 0x20,
        123 => 0xFF51,
        124 => 0xFF53,
        125 => 0xFF54,
        126 => 0xFF52,
        _ => return false,
    };
    let Some(x) = x() else { return false };
    let Some(&keycode) = keycodes().get(&keysym) else { return false };
    let Some(keys) = x.conn.query_keymap().ok().and_then(|c| c.reply().ok()) else { return false };
    keys.keys[keycode as usize / 8] & (1 << (keycode % 8)) != 0
}

/// The first key code for each keysym in the keyboard's current layout.
fn keycodes() -> &'static HashMap<u32, u8> {
    static CODES: OnceLock<HashMap<u32, u8>> = OnceLock::new();
    CODES.get_or_init(|| {
        let Some(x) = x() else { return HashMap::new() };
        let (min, max) = (x.conn.setup().min_keycode, x.conn.setup().max_keycode);
        let Some(map) = x.conn.get_keyboard_mapping(min, max - min + 1).ok().and_then(|c| c.reply().ok()) else {
            return HashMap::new();
        };
        let per = map.keysyms_per_keycode.max(1) as usize;
        let mut codes = HashMap::new();
        for (i, syms) in map.keysyms.chunks(per).enumerate() {
            for &sym in syms {
                codes.entry(sym).or_insert(min + i as u8);
            }
        }
        codes
    })
}

pub fn display_origin(_display: u32) -> (f32, f32) {
    (0., 0.)
}

pub fn display_points(_display: u32) -> (f32, f32) {
    x().map_or((0., 0.), |x| (x.size.0 as f32 / scale(), x.size.1 as f32 / scale()))
}

/// A window property as 32-bit values; empty when it isn't set.
pub fn property32(window: u32, name: u32) -> Vec<u32> {
    let Some(x) = x() else { return Vec::new() };
    let reply = x.conn.get_property(false, window, name, AtomEnum::ANY, 0, 1024).ok().and_then(|c| c.reply().ok());
    reply.and_then(|r| r.value32().map(|v| v.collect())).unwrap_or_default()
}

/// A window property as text.
pub fn property_text(window: u32, name: u32) -> Option<String> {
    let x = x()?;
    let reply = x.conn.get_property(false, window, name, AtomEnum::ANY, 0, 4096).ok()?.reply().ok()?;
    (reply.format == 8 && !reply.value.is_empty()).then(|| String::from_utf8_lossy(&reply.value).into_owned())
}
