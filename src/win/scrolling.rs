//! Scrolling capture's input: the wheel through `SendInput`.
//! Points are global logical units, which on a monitor are its physical pixels over its scale.

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub fn cursor(display: u32) -> (f32, f32) {
    let mut at = POINT::default();
    let _ = unsafe { GetCursorPos(&mut at) };
    let scale = super::monitor_rect(display).map_or(1., |(_, scale)| scale);
    (at.x as f32 / scale, at.y as f32 / scale)
}

/// Windows has no pixel wheel events, so `delta` points go out as as many wheel units: a notch
/// is 120 and scrolls about 100 points in browsers and less in most other apps, which only
/// narrows each step and keeps frames overlapping.
pub fn scroll(_at: (f32, f32), delta: i32) {
    let wheel = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { mouseData: delta as u32, dwFlags: MOUSEEVENTF_WHEEL, ..Default::default() } },
    };
    unsafe { SendInput(&[wheel], size_of::<INPUT>() as i32) };
}

/// Windows asks no permission to send input.
pub fn accessibility_trusted() -> bool {
    true
}
