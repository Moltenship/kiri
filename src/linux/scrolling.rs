//! Scrolling capture's input on X11: wheel clicks through XTest.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, ConnectionExt as _};
use x11rb::protocol::xtest::ConnectionExt as _;

use super::x11::{scale, x};

pub fn cursor(_display: u32) -> (f32, f32) {
    let Some(x) = x() else { return (0., 0.) };
    let at = x.conn.query_pointer(x.root).ok().and_then(|c| c.reply().ok());
    at.map_or((0., 0.), |p| (p.root_x as f32 / scale(), p.root_y as f32 / scale()))
}

/// X has no pixel wheel events, so `delta` points go out as one click per 100, at least one:
/// a click scrolls about that far in browsers and less in most other apps, which only narrows
/// each step and keeps frames overlapping. Buttons 4 and 5 scroll up and down.
pub fn scroll(_at: (f32, f32), delta: i32) {
    let Some(x) = x() else { return };
    let button = if delta > 0 { 4 } else { 5 };
    for _ in 0..(delta.unsigned_abs() / 100).max(1) {
        let _ = x.conn.xtest_fake_input(BUTTON_PRESS_EVENT, button, x11rb::CURRENT_TIME, x.root, 0, 0, 0);
        let _ = x.conn.xtest_fake_input(BUTTON_RELEASE_EVENT, button, x11rb::CURRENT_TIME, x.root, 0, 0, 0);
    }
    let _ = x.conn.flush();
}

/// X asks no permission to send input.
pub fn accessibility_trusted() -> bool {
    true
}
