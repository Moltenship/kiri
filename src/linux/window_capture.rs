//! Window capture on X11. The window manager lists its windows in stacking order, and a
//! compositing one keeps each window's pixels, covered or not, in a pixmap Composite can name.
//! Without a compositor the window is cropped from the screen. Wayland lists no windows.

use x11rb::connection::Connection;
use x11rb::protocol::composite::ConnectionExt as _;
use x11rb::protocol::xproto::*;

use super::Shot;
use super::x11::{property32, property_text, read, scale, x};
use crate::settings::WindowBackground;

/// An on-screen app window. `frame` is in global logical units, around what the window shows:
/// the window manager's title bar included, a client-drawn shadow left out.
#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub id: u32,
    pub app: String,
    pub title: String,
    pub frame: (f32, f32, f32, f32),
}

/// Titled, visible app windows, front to back, without Kiri's own.
pub fn list_windows() -> Vec<WindowInfo> {
    let Some(x) = x() else { return Vec::new() };
    let mut stack = property32(x.root, x.atoms._NET_CLIENT_LIST_STACKING);
    stack.reverse();
    let desktop = property32(x.root, x.atoms._NET_CURRENT_DESKTOP).first().copied();
    stack.into_iter().filter_map(|id| info(id, desktop)).collect()
}

/// The active window, or the frontmost one when that isn't capturable.
pub fn frontmost_window() -> Option<WindowInfo> {
    let x = x()?;
    let active = property32(x.root, x.atoms._NET_ACTIVE_WINDOW).first().copied();
    let mut windows = list_windows();
    let i = windows.iter().position(|w| Some(w.id) == active).unwrap_or(0);
    (i < windows.len()).then(|| windows.swap_remove(i))
}

fn info(id: u32, desktop: Option<u32>) -> Option<WindowInfo> {
    let x = x()?;
    let a = &x.atoms;
    let attributes = x.conn.get_window_attributes(id).ok()?.reply().ok()?;
    if attributes.map_state != MapState::VIEWABLE || property32(id, a._NET_WM_STATE).contains(&a._NET_WM_STATE_HIDDEN) {
        return None;
    }
    let kind = property32(id, a._NET_WM_WINDOW_TYPE);
    if kind.contains(&a._NET_WM_WINDOW_TYPE_DOCK) || kind.contains(&a._NET_WM_WINDOW_TYPE_DESKTOP) {
        return None;
    }
    // u32::MAX is on every desktop.
    let on = property32(id, a._NET_WM_DESKTOP).first().copied();
    if on.zip(desktop).is_some_and(|(on, now)| on != now && on != u32::MAX) {
        return None;
    }
    if property32(id, a._NET_WM_PID).first() == Some(&std::process::id()) {
        return None;
    }
    let title = property_text(id, a._NET_WM_NAME).or_else(|| property_text(id, AtomEnum::WM_NAME.into()))?;
    let app = property_text(id, AtomEnum::WM_CLASS.into()).and_then(|c| c.split('\0').nth(1).map(str::to_string));
    let (l, t, w, h) = frame(id)?;
    let s = scale();
    let (w, h) = (w as f32 / s, h as f32 / s);
    // Smaller than this is an invisible helper or a sliver, not something to capture.
    if w < 24. || h < 24. {
        return None;
    }
    Some(WindowInfo { id, app: app.unwrap_or_default(), title, frame: (l as f32 / s, t as f32 / s, w, h) })
}

/// Visible bounds on the root in pixels: the client grown by the manager's frame and shrunk by
/// the shadow a client-decorated window draws around itself.
fn frame(id: u32) -> Option<(i32, i32, i32, i32)> {
    let x = x()?;
    let geometry = x.conn.get_geometry(id).ok()?.reply().ok()?;
    let at = x.conn.translate_coordinates(id, x.root, 0, 0).ok()?.reply().ok()?;
    let side = |v: &[u32], i: usize| v.get(i).copied().unwrap_or(0) as i32;
    let (wm, gtk) = (property32(id, x.atoms._NET_FRAME_EXTENTS), property32(id, x.atoms._GTK_FRAME_EXTENTS));
    let [l, r, t, b] = [0, 1, 2, 3].map(|i| side(&wm, i) - side(&gtk, i));
    let (w, h) = (geometry.width as i32 + l + r, geometry.height as i32 + t + b);
    (w > 0 && h > 0).then_some((at.dst_x as i32 - l, at.dst_y as i32 - t, w, h))
}

/// Captures one window. Its rounded corners or other clear pixels show the background.
pub async fn capture(id: u32, background: WindowBackground, color: u32) -> Option<Shot> {
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(shot(id, background, color));
    });
    rx.await.ok().flatten()
}

fn shot(id: u32, background: WindowBackground, color: u32) -> Option<Shot> {
    let x = x()?;
    let (left, top, w, h) = frame(id)?;
    let (left, top) = (left.max(0), top.max(0));
    let (w, h) = (w.min(x.size.0 as i32 - left), h.min(x.size.1 as i32 - top));
    if w <= 0 || h <= 0 {
        return None;
    }
    let (width, height, mut bgra) = composited(id, (left, top, w, h)).or_else(|| read(x.root, (left as i16, top as i16, w as u16, h as u16)))?;
    let under = match background {
        WindowBackground::Solid => Some([color, color >> 8, color >> 16, 255].map(|c| (c & 255) as u8).repeat((width * height) as usize)),
        WindowBackground::Wallpaper => root_picture().and_then(|p| read(p, (left as i16, top as i16, width as u16, height as u16))).map(|(_, _, b)| b),
        _ => None,
    };
    if let Some(under) = under.filter(|u| u.len() == bgra.len()) {
        for (p, u) in bgra.chunks_exact_mut(4).zip(under.chunks_exact(4)) {
            let clear = 255 - p[3] as u32;
            for (c, u) in p.iter_mut().zip(u) {
                *c = (*c as u32 + *u as u32 * clear / 255).min(255) as u8;
            }
        }
    }
    Some(Shot { display: 0, width, height, scale: scale(), bgra })
}

/// The part of the top-level window holding `id` that `rect` covers, from the pixmap the
/// compositor keeps for it. `None` when nothing composites it.
fn composited(id: u32, (left, top, w, h): (i32, i32, i32, i32)) -> Option<(u32, u32, Vec<u8>)> {
    let x = x()?;
    let mut top_level = id;
    loop {
        let tree = x.conn.query_tree(top_level).ok()?.reply().ok()?;
        if tree.parent == x.root || tree.parent == x11rb::NONE {
            break;
        }
        top_level = tree.parent;
    }
    let geometry = x.conn.get_geometry(top_level).ok()?.reply().ok()?;
    let border = geometry.border_width as i32;
    let (fx, fy) = (geometry.x as i32 + border, geometry.y as i32 + border);
    let pixmap = x.conn.generate_id().ok()?;
    x.conn.composite_name_window_pixmap(top_level, pixmap).ok()?.check().ok()?;
    let pixels = read(pixmap, ((left - fx) as i16, (top - fy) as i16, w as u16, h as u16));
    let _ = x.conn.free_pixmap(pixmap);
    let _ = x.conn.flush();
    pixels
}

/// The desktop picture as some desktops leave it on the root window.
fn root_picture() -> Option<u32> {
    let x = x()?;
    property32(x.root, x.atoms._XROOTPMAP_ID).first().copied().filter(|&p| p != 0)
}

/// The desktop picture, whole and at the screen's pixel size, where the desktop publishes it.
pub async fn desktop_picture() -> Option<Shot> {
    let x = x()?;
    let picture = root_picture()?;
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let shot = read(picture, (0, 0, x.size.0, x.size.1)).map(|(width, height, bgra)| Shot { display: 0, width, height, scale: scale(), bgra });
        let _ = tx.send(shot);
    });
    rx.await.ok().flatten()
}
