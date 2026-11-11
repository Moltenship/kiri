//! Windows' own shortcuts, so Settings can say when a capture hotkey collides with one.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
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
            "super" | "cmd" | "command" | "meta" => mods.push("super"),
            other => key = other.to_string(),
        }
    }
    mods.sort_unstable();
    (mods, key)
}

/// The key that typed `typed` on the current layout, by the name a hotkey takes, and whether
/// Shift is down. GPUI names a key by the character it types and drops Shift when the character has it.
pub fn physical_key(typed: &str) -> Option<(&'static str, bool)> {
    let mut units = typed.encode_utf16();
    let (Some(c), None) = (units.next(), units.next()) else { return None };
    let scan = unsafe { VkKeyScanExW(c, GetKeyboardLayout(0)) };
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
    Some((key_name(scan)?, shift))
}

/// Names the virtual key of a `VkKeyScanExW` result. Hotkeys name keys as on the US layout, as virtual keys do.
fn key_name(scan: i16) -> Option<&'static str> {
    const DIGITS: &str = "0123456789";
    const LETTERS: &str = "abcdefghijklmnopqrstuvwxyz";
    const OEM: [(VIRTUAL_KEY, &str); 11] = [
        (VK_OEM_1, ";"), (VK_OEM_PLUS, "="), (VK_OEM_COMMA, ","), (VK_OEM_MINUS, "-"), (VK_OEM_PERIOD, "."), (VK_OEM_2, "/"),
        (VK_OEM_3, "`"), (VK_OEM_4, "["), (VK_OEM_5, "\\"), (VK_OEM_6, "]"), (VK_OEM_7, "'"),
    ];
    if scan == -1 {
        return None;
    }
    let vk = (scan & 0xFF) as u16;
    match vk {
        0x30..=0x39 => Some(&DIGITS[(vk - 0x30) as usize..][..1]),
        0x41..=0x5A => Some(&LETTERS[(vk - 0x41) as usize..][..1]),
        _ => OEM.iter().find(|(k, _)| k.0 == vk).map(|(_, name)| *name),
    }
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
    use super::{SystemShortcut, key_name, system_shortcut};

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

    /// As `VkKeyScanExW` answers: the virtual key in the low byte, Shift, Ctrl, and Alt bits in the high one.
    #[test]
    fn keys_are_named_by_their_virtual_key_whatever_the_layout() {
        // US "$" and Russian ";" are both Shift+4.
        assert_eq!(key_name(0x0134), Some("4"));
        // Russian "ф" is VK_A, "ж" VK_OEM_1.
        assert_eq!(key_name(0x0041), Some("a"));
        assert_eq!(key_name(0x00BA), Some(";"));
        // German "z" is VK_Z, "§" Shift+3, "ü" VK_OEM_1, "#" VK_OEM_2, and AltGr+Q's "@" still VK_Q.
        assert_eq!(key_name(0x005A), Some("z"));
        assert_eq!(key_name(0x0133), Some("3"));
        assert_eq!(key_name(0x00BA), Some(";"));
        assert_eq!(key_name(0x00BF), Some("/"));
        assert_eq!(key_name(0x0651), Some("q"));
        // Russian "б" is VK_OEM_COMMA, so Ctrl+б is Ctrl+,; French "&" and "é" are VK_1 and VK_2, the measuring keys.
        assert_eq!(key_name(0x00BC), Some(","));
        assert_eq!(key_name(0x0031), Some("1"));
        assert_eq!(key_name(0x0032), Some("2"));
        // Typed by no key, and German "<", VK_OEM_102, which has no US key.
        assert_eq!(key_name(-1), None);
        assert_eq!(key_name(0x00E2), None);
    }
}
