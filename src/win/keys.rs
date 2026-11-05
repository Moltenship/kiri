//! Windows' own shortcuts, so Settings can say when a capture hotkey collides with one.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Registry::*;
use windows::Win32::UI::WindowsAndMessaging::{HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE};
use windows::core::{PCWSTR, w};

#[derive(Clone, Debug, PartialEq)]
pub struct SystemShortcut {
    pub name: String,
    /// The page of Windows Settings that turns it off, as "Accessibility › Keyboard"; empty when none does.
    pub section: String,
    /// As Kiri stores hotkeys; `*` stands for any letter or digit, `#` for any digit.
    keys: &'static str,
}

/// The ones a capture hotkey could plausibly land on. Windows reserves the rest of the Windows key's.
const RESERVED: &[(&str, &str)] = &[
    ("super+shift+s", "Snipping Tool"),
    ("super+shift+r", "Snipping Tool recording"),
    ("super+printscreen", "Save a screenshot to Pictures"),
    ("alt+printscreen", "Copy the active window"),
    ("alt+super+printscreen", "Game Bar screenshot"),
    ("alt+super+r", "Game Bar recording"),
    ("ctrl+shift+escape", "Task Manager"),
    ("super+v", "Clipboard history"),
    ("super+space", "Switch the input language"),
    ("super+tab", "Task View"),
    ("super+#", "Taskbar apps"),
    ("shift+super+#", "Taskbar apps"),
    ("ctrl+super+#", "Taskbar apps"),
    ("alt+super+#", "Taskbar apps"),
    ("super+*", "Windows key shortcuts"),
];

const KEYBOARD: &str = "Accessibility › Keyboard";

pub fn system_shortcuts() -> Vec<SystemShortcut> {
    let snipping = snipping_on_print_screen().then(|| SystemShortcut {
        name: "Snipping Tool".into(),
        section: KEYBOARD.into(),
        keys: "printscreen",
    });
    let reserved = RESERVED.iter().map(|&(keys, name)| SystemShortcut { name: name.into(), section: String::new(), keys });
    snipping.into_iter().chain(reserved).collect()
}

/// The Windows shortcut on `keys`, written as Kiri stores hotkeys ("ctrl+shift+2").
pub fn system_shortcut<'a>(all: &'a [SystemShortcut], keys: &str) -> Option<&'a SystemShortcut> {
    let (mods, key) = split(keys);
    all.iter().find(|s| {
        let (want_mods, want_key) = split(s.keys);
        let single = key.chars().count() == 1;
        mods == want_mods
            && match want_key.as_str() {
                "*" => single && key.chars().all(|c| c.is_ascii_alphanumeric()),
                "#" => single && key.chars().all(|c| c.is_ascii_digit()),
                want => want == key,
            }
    })
}

/// Modifiers as a sorted list, so their order doesn't matter, and the key in lower case.
fn split(keys: &str) -> (Vec<&'static str>, String) {
    let mut mods = Vec::new();
    let mut key = String::new();
    for part in keys.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods.push("ctrl"),
            "alt" | "option" => mods.push("alt"),
            "shift" => mods.push("shift"),
            "super" | "cmd" | "command" | "meta" | "win" => mods.push("super"),
            other => key = other.to_string(),
        }
    }
    mods.sort_unstable();
    (mods, key)
}

/// Windows 11 opens Snipping Tool on Print Screen unless the user turned that off.
fn snipping_on_print_screen() -> bool {
    match read_dword(w!(r"Control Panel\Keyboard"), w!("PrintScreenKeyForSnippingEnabled")) {
        Some(on) => on != 0,
        None => windows_11(),
    }
}

fn windows_11() -> bool {
    let mut build = [0u16; 16];
    let mut size = (build.len() * 2) as u32;
    let read = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
            w!("CurrentBuildNumber"),
            RRF_RT_REG_SZ,
            None,
            Some(build.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    let text = String::from_utf16_lossy(&build[..build.iter().position(|&c| c == 0).unwrap_or(0)]);
    read.is_ok() && text.parse::<u32>().is_ok_and(|b| b >= 22000)
}

fn read_dword(key: PCWSTR, name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut size = 4u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, key, name, RRF_RT_REG_DWORD, None, Some((&mut value as *mut u32).cast()), Some(&mut size)) }
        .is_ok()
        .then_some(value)
}

/// Turns off Snipping Tool on Print Screen, as the switch in Settings › Accessibility › Keyboard does.
pub fn release_screenshot_keys() -> std::io::Result<()> {
    let off = 0u32.to_le_bytes();
    unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, w!(r"Control Panel\Keyboard"), w!("PrintScreenKeyForSnippingEnabled"), REG_DWORD.0, Some(off.as_ptr().cast()), 4)
            .ok()
            .map_err(std::io::Error::other)?;
    }
    // Waits on every top-level window in turn, so not on the UI thread.
    std::thread::spawn(|| unsafe { SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(0), SMTO_ABORTIFHUNG, 500, None) });
    Ok(())
}

pub struct SystemKeysOff;

impl SystemKeysOff {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::{SystemShortcut, system_shortcut};

    #[test]
    fn hotkeys_match_windows_shortcuts_whatever_the_modifier_order() {
        let all: Vec<SystemShortcut> = super::RESERVED
            .iter()
            .map(|&(keys, name)| SystemShortcut { name: name.into(), section: String::new(), keys })
            .collect();
        let name = |keys| system_shortcut(&all, keys).map(|s| s.name.as_str());
        assert_eq!(name("shift+super+s"), Some("Snipping Tool"));
        assert_eq!(name("super+shift+3"), Some("Taskbar apps"));
        assert_eq!(name("cmd+e"), Some("Windows key shortcuts"));
        assert_eq!(name("super+f5"), None);
        assert_eq!(name("ctrl+shift+2"), None);
        assert_eq!(name("ctrl+shift+s"), None);
    }
}
