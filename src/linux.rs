//! The Linux counterpart of `mac.rs`, under the same names. In an X11 session Kiri reads the
//! screen and places its windows itself (`x11`). A Wayland session allows neither, so there
//! `crate::direct_screen()` is false, captures go through the desktop's portal, and the
//! functions that move or read windows do nothing.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use gpui_kit::{App, Window, WindowAppearance};

mod clipboard;
pub mod portal;
mod text;
mod x11;

pub use text::{text_image, text_width};
pub use x11::{
    capture_area, capture_display, display_origin, display_points, key_pressed, make_floating, make_key, make_overlay, monitor_under_cursor, option_held,
    resize_from_top, set_ignores_mouse, set_on_top, set_opacity, start_window_move, warp_cursor,
};

/// One display frozen at capture time, as tightly packed BGRA.
pub struct Shot {
    pub display: u32,
    pub width: u32,
    pub height: u32,
    /// Pixels per logical unit for this display.
    pub scale: f32,
    pub bgra: Vec<u8>,
}

static WAYLAND: OnceLock<bool> = OnceLock::new();

/// GPUI picks the backend once, from `WAYLAND_DISPLAY`; Kiri follows its choice.
pub fn init(cx: &App) {
    let _ = WAYLAND.set(cx.compositor_name() == "Wayland");
    x11::init(cx);
}

pub fn direct() -> bool {
    !WAYLAND.get().copied().unwrap_or(false)
}

pub fn has_screen_access() -> bool {
    true
}

pub fn ensure_screen_access() -> bool {
    true
}

static APPEARANCE: AtomicU8 = AtomicU8::new(0);

/// `None` follows the system. GPUI only reads the system's, so Kiri's colors take this through `appearance`.
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

/// Linux has no screenshot shortcuts for Kiri to take over, so none conflict.
#[derive(Clone)]
pub struct SystemShortcut {
    pub name: String,
}

pub fn system_shortcuts() -> Vec<SystemShortcut> {
    Vec::new()
}

pub fn system_shortcut<'a>(_all: &'a [SystemShortcut], _keys: &str) -> Option<&'a SystemShortcut> {
    None
}

pub fn release_screenshot_keys() -> std::io::Result<()> {
    Ok(())
}

pub struct SystemKeysOff;

impl SystemKeysOff {
    pub fn new() -> Self {
        Self
    }
}

pub fn has_screenshot_cursor() -> bool {
    false
}

pub fn set_screenshot_cursor(_window: &Window, _window_pick: bool, _numbers: Option<[i32; 2]>) {}

#[derive(Clone, Copy)]
pub enum Level {
    Floating,
    Status,
}

/// Minimize, maximize, and close for a window that draws its own title bar, once the
/// desktop has agreed to let it.
pub fn caption_buttons(window: &Window, cx: &App) -> impl gpui_kit::IntoElement {
    use gpui_kit::assets::IconName;
    use gpui_kit::component::{ActiveTheme, Icon, Sizable};
    use gpui_kit::prelude::FluentBuilder;
    use gpui_kit::*;
    let (fg, hover) = (cx.theme().foreground, cx.theme().secondary_hover);
    let button = |id: &'static str, icon: IconName, run: fn(&mut Window)| {
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(fg)
            .hover(move |s| s.bg(hover))
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(move |_, window, _| run(window))
            .child(Icon::new(icon).small())
    };
    let client = matches!(window.window_decorations(), Decorations::Client { .. });
    let controls = window.window_controls();
    let max = if window.is_maximized() { IconName::WindowRestore } else { IconName::WindowMaximize };
    div()
        .id("caption-buttons")
        .occlude()
        .flex()
        .flex_none()
        .h_full()
        .when(client, |d| {
            d.when(controls.minimize, |d| d.child(button("minimize", IconName::WindowMinimize, |w| w.minimize_window())))
                .when(controls.maximize, |d| d.child(button("maximize", max, |w| w.zoom_window())))
                .child(button("close", IconName::WindowClose, |w| w.remove_window()))
        })
}

pub fn copy_png(png: &[u8], file: Option<&Path>) {
    clipboard::copy_png(png, file);
}

pub fn copy_text(text: &str) {
    clipboard::copy_text(text);
}

pub fn adopt_full_screen_item(_title: &str) {}

/// The clipboard's image as premultiplied BGRA plus its pixels per logical unit.
pub fn clipboard_image(cx: &App) -> Option<(u32, u32, Vec<u8>, f32)> {
    use gpui_kit::ClipboardEntry;
    cx.read_from_clipboard()?.entries().iter().find_map(|e| match e {
        ClipboardEntry::Image(image) => crate::export::decode_bgra(image.bytes()),
        _ => None,
    })
}

/// X11 has one display, the whole desktop.
pub fn display_under_cursor() -> u32 {
    0
}

/// `xdg-user-dir`'s answer, which follows the user's language.
pub fn desktop_dir() -> PathBuf {
    let asked = std::process::Command::new("xdg-user-dir").arg("DESKTOP").output().ok();
    asked
        .filter(|out| out.status.success())
        .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .unwrap_or_else(|| home().join("Desktop"))
}

pub fn mark_screenshot(_path: &Path) {}

/// `template` with strftime fields filled in. A field chrono doesn't know leaves the template as typed.
pub fn strftime(template: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    if write!(out, "{}", chrono::Local::now().format(template)).is_err() {
        out = template.to_string();
    }
    out
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// An XDG base directory: `var` when it is set to an absolute path, else `fallback` under home.
pub fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

pub const APP_ID: &str = "sh.kiri.Kiri";

fn entry_name() -> String {
    if crate::settings::dev() { format!("{APP_ID}.dev.desktop") } else { format!("{APP_ID}.desktop") }
}

fn autostart() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("autostart").join(entry_name())
}

pub fn launch_at_login() -> bool {
    autostart().exists()
}

pub fn set_launch_at_login(on: bool) -> Result<(), String> {
    let path = autostart();
    if !on {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    write_file(&path, desktop_entry(&exe, true).as_bytes())
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// Puts Kiri in the desktop's app list, under the name and icon portals and notifications show,
/// and points it at this binary again after a move. An installed Kiri does this at every start.
pub fn install_menu_entry() {
    let (Some(_), Ok(exe)) = (crate::update::bundle(), std::env::current_exe()) else { return };
    let data = xdg("XDG_DATA_HOME", ".local/share");
    let entry = data.join("applications").join(entry_name());
    let text = desktop_entry(&exe, false);
    if std::fs::read_to_string(&entry).ok().as_deref() != Some(text.as_str())
        && let Err(err) = write_file(&entry, text.as_bytes())
    {
        eprintln!("kiri: couldn't write {}: {err}", entry.display());
    }
    let icon = data.join("icons/hicolor/512x512/apps").join(format!("{APP_ID}.png"));
    let png = include_bytes!("icon.png");
    if std::fs::metadata(&icon).map_or(true, |m| m.len() != png.len() as u64) {
        let _ = write_file(&icon, png);
    }
    if launch_at_login() {
        let _ = set_launch_at_login(true);
    }
}

/// The app menu's entry, or with `login` the autostart one, which starts Kiri without its
/// actions and keeps it out of the app list.
fn desktop_entry(exe: &Path, login: bool) -> String {
    let exec = quote_exec(&exe.to_string_lossy());
    let name = if crate::settings::dev() { "Kiri Dev" } else { "Kiri" };
    let mut out = format!(
        "[Desktop Entry]\nType=Application\nName={name}\nComment=Screenshots and annotation\nExec={exec}\nIcon={APP_ID}\nTerminal=false\nCategories=Graphics;Utility;\nStartupNotify=false\n"
    );
    if login {
        out.push_str("NoDisplay=true\nX-GNOME-Autostart-enabled=true\n");
        return out;
    }
    out.push_str("Actions=area;screen;settings;\n");
    for (id, label, args) in
        [("area", "Capture Area", " --capture area"), ("screen", "Capture Screen", " --capture fullscreen"), ("settings", "Settings", "")]
    {
        out.push_str(&format!("\n[Desktop Action {id}]\nName={label}\nExec={exec}{args}\n"));
    }
    out
}

/// One argument of `Exec`, quoted as the Desktop Entry spec asks when it has to be, then with
/// backslashes doubled again for the string escaping the key's value goes through.
fn quote_exec(arg: &str) -> String {
    let plain = !arg.is_empty() && arg.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+,:@".contains(c));
    if plain {
        return arg.to_string();
    }
    let mut quoted = String::from("\"");
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted.replace('\\', "\\\\").replace('%', "%%")
}

pub mod ocr;
pub mod scrolling;
pub mod update;
pub mod window_capture;

#[cfg(test)]
mod tests {
    use super::{desktop_entry, quote_exec};

    #[test]
    fn exec_paths_are_quoted_only_when_they_must_be() {
        assert_eq!(quote_exec("/home/me/Kiri/kiri"), "/home/me/Kiri/kiri");
        assert_eq!(quote_exec("/home/me/My Apps/kiri"), r#""/home/me/My Apps/kiri""#);
        assert_eq!(quote_exec(r#"/opt/a"b$c/kiri"#), r#""/opt/a\\"b\\$c/kiri""#);
        assert_eq!(quote_exec("/opt/100%/kiri"), r#""/opt/100%%/kiri""#);
    }

    #[test]
    fn menu_entry_has_the_capture_actions_and_login_entry_has_none() {
        let menu = desktop_entry(std::path::Path::new("/home/me/My Apps/kiri"), false);
        assert!(menu.contains("Exec=\"/home/me/My Apps/kiri\"\n"));
        assert!(menu.contains("Exec=\"/home/me/My Apps/kiri\" --capture area\n"));
        assert!(menu.contains("Actions=area;screen;settings;\n"));
        let login = desktop_entry(std::path::Path::new("/home/me/kiri"), true);
        assert!(login.contains("NoDisplay=true") && !login.contains("Actions="));
    }
}
