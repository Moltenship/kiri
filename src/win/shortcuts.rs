//! Kiri's shortcuts follow the key, not the character the layout types on it, as on macOS.
//!
//! GPUI names a key by what the current layout types, so on Russian Ctrl+, arrives as "ctrl-б".
//! Kiri's bindings are written for the US layout and are mapped onto the current one by virtual
//! key, as Windows matches accelerators. They are mapped again when the layout changes.

use std::cell::RefCell;

use gpui_kit::*;

/// Marks Kiri's bindings in the keymap: this plus the binding's place in `SHORTCUTS`.
const KIRI: u32 = 0x4B00_0000;

thread_local! {
    /// The bindings as written, for the US layout.
    static SHORTCUTS: RefCell<Vec<KeyBinding>> = const { RefCell::new(Vec::new()) };
    static LAYOUT_CHANGE: RefCell<Option<Subscription>> = const { RefCell::new(None) };
}

pub fn bind_keys(cx: &mut App, bindings: impl IntoIterator<Item = KeyBinding>) {
    let mapped: Vec<KeyBinding> = SHORTCUTS.with_borrow_mut(|all| {
        bindings
            .into_iter()
            .map(|b| {
                all.push(b);
                map(all.len() - 1, all, cx)
            })
            .collect()
    });
    cx.bind_keys(mapped);
    if LAYOUT_CHANGE.with_borrow(Option::is_none) {
        let subscription = cx.on_keyboard_layout_change(remap);
        LAYOUT_CHANGE.set(Some(subscription));
    }
}

/// Swaps Kiri's bindings for ones mapped onto the new layout, in place, so precedence stays.
fn remap(cx: &mut App) {
    let keymap = cx.key_bindings();
    let bindings: Vec<KeyBinding> = SHORTCUTS.with_borrow(|all| {
        keymap
            .borrow()
            .bindings()
            .map(|b| match b.meta() {
                Some(meta) if meta.0 >= KIRI => map((meta.0 - KIRI) as usize, all, cx),
                _ => b.clone(),
            })
            .collect()
    });
    let mut keymap = keymap.borrow_mut();
    keymap.clear();
    keymap.add_bindings(bindings);
    drop(keymap);
    cx.refresh_windows();
}

fn map(index: usize, all: &[KeyBinding], cx: &App) -> KeyBinding {
    let b = &all[index];
    let os = cx.platform_info().os;
    let keys = b.keystrokes().iter().map(|k| k.inner().unparse(os)).collect::<Vec<_>>().join(" ");
    KeyBinding::load(&keys, b.action().boxed_clone(), b.predicate(), true, b.action_input(), cx)
        .unwrap_or_else(|_| b.clone())
        .with_meta(KeyBindingMetaIndex(KIRI + index as u32))
}
