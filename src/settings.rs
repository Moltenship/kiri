//! Settings live in one plain JSON file that users can read, back up, or share.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

use crate::doc::{Style, Tool};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Capture {
    Area,
    Fullscreen,
    Delayed,
    DelayedArea,
    RepeatArea,
    Window,
    ActiveWindow,
    Scrolling,
    ScrollingUp,
    Text,
    Reopen,
    Settings,
}

impl Capture {
    pub const ALL: [Capture; 12] = [
        Capture::Area,
        Capture::Fullscreen,
        Capture::Delayed,
        Capture::DelayedArea,
        Capture::RepeatArea,
        Capture::Window,
        Capture::ActiveWindow,
        Capture::Scrolling,
        Capture::ScrollingUp,
        Capture::Text,
        Capture::Reopen,
        Capture::Settings,
    ];

    /// The name `kiri --capture` takes: the variant in kebab case, as `active-window`.
    pub fn cli_name(self) -> String {
        let mut name = String::new();
        for (i, c) in format!("{self:?}").chars().enumerate() {
            if c.is_uppercase() && i > 0 {
                name.push('-');
            }
            name.push(c.to_ascii_lowercase());
        }
        name
    }

    pub fn from_cli(name: &str) -> Option<Capture> {
        Capture::ALL.into_iter().find(|c| c.cli_name() == name)
    }

    pub fn label(self) -> &'static str {
        match self {
            Capture::Area => "Capture Area",
            Capture::Fullscreen => "Capture Screen",
            Capture::Delayed => "Delayed Screenshot",
            Capture::DelayedArea => "Delayed Area Capture",
            Capture::RepeatArea => "Repeat Area Capture",
            Capture::Window => "Capture Window",
            Capture::ActiveWindow => "Capture Active Window",
            Capture::Scrolling => "Scrolling Capture",
            Capture::ScrollingUp => "Scrolling Capture (Up)",
            Capture::Text => "Recognize Text or QR",
            Capture::Reopen => "Reopen Last Screenshot",
            Capture::Settings => "Settings…",
        }
    }

    fn default_hotkey(self) -> Option<&'static str> {
        match self {
            Capture::Area => Some("shift+cmd+2"),
            Capture::Fullscreen => Some("shift+cmd+1"),
            Capture::Text => Some("ctrl+alt+cmd+o"),
            _ => None,
        }
    }
}

/// What ⌘Q does in Kiri's windows; quitting also stops the hotkeys and the menu bar icon.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum CmdQ {
    #[default]
    Ask,
    Quit,
    CloseWindow,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Preview {
    Editor,
    Thumbnail,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum WindowBackground {
    Wallpaper,
    Transparent,
    Solid,
    TrimShadow,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ColorFormat {
    Hex,
    HexBare,
    CssRgb,
    Rgb01,
    Android,
    SwiftUI,
    UIColor,
    CSharp,
    Oklch,
}

impl ColorFormat {
    pub const OPTIONS: &[(ColorFormat, &str)] = &[
        (ColorFormat::Hex, "#RRGGBB"),
        (ColorFormat::HexBare, "RRGGBB"),
        (ColorFormat::CssRgb, "rgb(r, g, b)"),
        (ColorFormat::Rgb01, "(r, g, b) 0–1"),
        (ColorFormat::Android, "0xFFRRGGBB"),
        (ColorFormat::SwiftUI, "SwiftUI Color"),
        (ColorFormat::UIColor, "UIColor"),
        (ColorFormat::CSharp, "C# Color.FromArgb"),
        (ColorFormat::Oklch, "oklch()"),
    ];

    pub fn format(self, c: u32) -> String {
        let (r, g, b) = ((c >> 16) & 255, (c >> 8) & 255, c & 255);
        let f = |v: u32| v as f32 / 255.;
        match self {
            ColorFormat::Hex => format!("#{c:06X}"),
            ColorFormat::HexBare => format!("{c:06X}"),
            ColorFormat::CssRgb => format!("rgb({r}, {g}, {b})"),
            ColorFormat::Rgb01 => format!("({:.2}, {:.2}, {:.2})", f(r), f(g), f(b)),
            ColorFormat::Android => format!("0xFF{c:06X}"),
            ColorFormat::SwiftUI => {
                format!("Color(red: {:.2}, green: {:.2}, blue: {:.2})", f(r), f(g), f(b))
            }
            ColorFormat::UIColor => {
                format!("UIColor(red: {:.2}, green: {:.2}, blue: {:.2}, alpha: 1.00)", f(r), f(g), f(b))
            }
            ColorFormat::CSharp => format!("Color.FromArgb({r}, {g}, {b})"),
            ColorFormat::Oklch => {
                let (l, ch, h) = oklch(f(r), f(g), f(b));
                format!("oklch({:.1}% {:.3} {:.1})", l * 100., ch, h)
            }
        }
    }
}

fn oklch(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let lin = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(r), lin(g), lin(b));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    let ok_l = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
    let a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    let bb = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
    let h = bb.atan2(a).to_degrees().rem_euclid(360.);
    (ok_l, a.hypot(bb), h)
}

pub const FILE_NAME: &str = "Kiri %Y-%m-%d at %H.%M.%S";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub save_folder: Option<PathBuf>,
    /// strftime fields, such as `%Y` and `%H`, fill in the time of the capture.
    pub file_name: String,
    pub after_show: bool,
    pub after_copy: bool,
    pub after_save: bool,
    pub area_preview: Preview,
    /// Seconds before the thumbnail hides itself; 0 keeps it until closed.
    pub thumbnail_seconds: u32,
    pub esc_copies: bool,
    pub esc_saves: bool,
    pub downscale_retina: bool,
    /// Save writes a JPEG instead of a PNG when that is smaller.
    pub save_auto_jpeg: bool,
    pub editor_on_top: bool,
    /// "Copied" and "Saved" toasts; errors show regardless.
    pub confirmations: bool,
    pub cmd_q: CmdQ,
    pub unit_px: bool,
    pub reverse_wheel_zoom: bool,
    pub color_format: ColorFormat,
    pub ocr_languages: Vec<String>,
    pub ocr_join_lines: bool,
    pub window_background: WindowBackground,
    pub window_background_color: u32,
    pub delay_seconds: u32,
    pub scroll_max_height: u32,
    pub scroll_reverse: bool,
    /// The user scrolls; Kiri only watches and stitches.
    pub scroll_manual: bool,
    /// The overlay shows a still of the moment the hotkey was pressed, instead of the live screen.
    pub freeze_screen: bool,
    /// Capture Screen takes every display as arranged, instead of the one under the pointer.
    pub fullscreen_all_displays: bool,
    /// `kiri --capture` may run captures; off, a request asks first.
    pub automation: bool,
    pub hotkeys: BTreeMap<Capture, Option<String>>,
    pub styles: BTreeMap<Tool, Style>,
    pub backdrop: Option<crate::backdrop::Backdrop>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            save_folder: None,
            file_name: FILE_NAME.into(),
            after_show: true,
            after_copy: false,
            after_save: false,
            area_preview: Preview::Editor,
            thumbnail_seconds: 0,
            esc_copies: true,
            esc_saves: false,
            downscale_retina: false,
            save_auto_jpeg: true,
            editor_on_top: false,
            confirmations: true,
            cmd_q: CmdQ::Ask,
            unit_px: false,
            reverse_wheel_zoom: false,
            color_format: ColorFormat::Hex,
            ocr_languages: Vec::new(),
            ocr_join_lines: false,
            window_background: WindowBackground::Transparent,
            window_background_color: 0xF2F2F7,
            delay_seconds: 3,
            scroll_max_height: 20_000,
            scroll_reverse: false,
            scroll_manual: false,
            automation: false,
            freeze_screen: false,
            fullscreen_all_displays: false,
            hotkeys: BTreeMap::new(),
            styles: BTreeMap::new(),
            backdrop: None,
        }
    }
}

impl Settings {
    pub fn save_path(&self) -> PathBuf {
        crate::mac::timestamp_path(&self.save_dir(), &self.file_name)
    }

    pub fn save_dir(&self) -> PathBuf {
        self.save_folder
            .clone()
            .unwrap_or_else(crate::mac::desktop_dir)
    }

    pub fn hotkey(&self, action: Capture) -> Option<String> {
        match self.hotkeys.get(&action) {
            Some(set) => set.clone(),
            None => action.default_hotkey().map(str::to_string),
        }
    }
}

pub struct Current(Settings);

impl Global for Current {}

pub fn path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Library/Application Support/Kiri/settings.json")
}

pub fn load(cx: &mut App) {
    let settings = std::fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    cx.set_global(Current(settings));
    cx.on_app_quit(|cx| {
        save(cx);
        async {}
    })
    .detach();
}

/// Whether a write to disk is already scheduled.
#[derive(Default)]
struct Saving(bool);

impl Global for Saving {}

pub fn get(cx: &App) -> &Settings {
    &cx.global::<Current>().0
}

/// Changes settings now and writes them shortly after, so a slider drag writes the file once.
pub fn update<R>(cx: &mut App, f: impl FnOnce(&mut Settings) -> R) -> R {
    let out = f(&mut cx.global_mut::<Current>().0);
    if !std::mem::replace(&mut cx.default_global::<Saving>().0, true) {
        cx.spawn(async |cx| {
            cx.background_executor().timer(Duration::from_millis(250)).await;
            cx.update(save);
        })
        .detach();
    }
    out
}

fn save(cx: &mut App) {
    if !std::mem::take(&mut cx.default_global::<Saving>().0) {
        return;
    }
    let json = serde_json::to_string_pretty(&get(cx)).unwrap_or_default();
    let path = path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, json);
}

pub fn import(file: &std::path::Path, cx: &mut App) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let settings: Settings = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    update(cx, |s| *s = settings);
    Ok(())
}

pub fn export(file: &std::path::Path, cx: &App) -> Result<(), String> {
    let json = serde_json::to_string_pretty(get(cx)).map_err(|e| e.to_string())?;
    std::fs::write(file, json).map_err(|e| e.to_string())
}

/// A per-user LaunchAgent: unlike `SMAppService`, it works for an unbundled binary.
fn launch_agent() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Library/LaunchAgents/sh.kiri.Kiri.plist")
}

pub fn launch_at_login() -> bool {
    launch_agent().exists()
}

pub fn set_launch_at_login(on: bool) -> Result<(), String> {
    let path = launch_agent();
    if !on {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().replace('&', "&amp;").replace('<', "&lt;");
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>sh.kiri.Kiri</string>
    <key>ProgramArguments</key><array><string>{exe}</string></array>
    <key>RunAtLoad</key><true/>
    <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
"#
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, plist).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::Capture;

    #[test]
    fn file_names_fill_in_the_time_and_stay_valid() {
        let year = crate::mac::file_stem("%Y");
        assert!(year.len() == 4 && year.starts_with("20"), "{year}");
        assert_eq!(crate::mac::file_stem("a/b:c"), "a-b-c");
        assert!(crate::mac::file_stem("  ").starts_with("Kiri "));
        assert_eq!(crate::mac::file_stem(".hidden"), "hidden");
        assert_eq!(crate::mac::file_stem(&"é".repeat(200)).len(), 240);
    }

    #[test]
    fn cli_names_are_kebab_case_and_round_trip() {
        assert_eq!(Capture::ActiveWindow.cli_name(), "active-window");
        for c in Capture::ALL {
            assert_eq!(Capture::from_cli(&c.cli_name()), Some(c));
        }
    }
}
