//! Copying images. GPUI's Linux clipboard only writes text, so Kiri owns the X clipboard itself,
//! from a hidden window whose thread hands the image to apps that paste it. In a Wayland session
//! that is XWayland's clipboard, which the compositor passes on to Wayland apps.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        CLIPBOARD,
        TARGETS,
        INCR,
        PNG: b"image/png",
        UTF8_STRING,
        TEXT_UTF8: b"text/plain;charset=utf-8",
        TEXT: b"text/plain",
        URI_LIST: b"text/uri-list",
        GNOME_FILES: b"x-special/gnome-copied-files",
    }
}

/// Larger answers go in pieces, as the ICCCM's INCR asks, so no single request is too big.
const CHUNK: usize = 256 * 1024;

struct Owner {
    conn: RustConnection,
    window: u32,
    atoms: Atoms,
    offered: Mutex<Vec<(u32, Arc<Vec<u8>>)>>,
}

/// The PNG, and with `file` the file too, so pasting works in image editors and file managers alike.
pub fn copy_png(png: &[u8], file: Option<&Path>) {
    let Some(owner) = owner() else { return };
    let a = &owner.atoms;
    let mut offered = vec![(a.PNG, Arc::new(png.to_vec()))];
    if let Some(file) = file {
        let uri = file_uri(file);
        offered.push((a.URI_LIST, Arc::new(format!("{uri}\r\n").into_bytes())));
        offered.push((a.GNOME_FILES, Arc::new(format!("copy\n{uri}").into_bytes())));
    }
    owner.offer(offered);
}

/// Text copied while Kiri has no focused window, as after a capture: a Wayland client may set
/// the clipboard only while it has one, which is all GPUI's clipboard can do there.
pub fn copy_text(text: &str) {
    let Some(owner) = owner() else { return };
    let a = &owner.atoms;
    let text = Arc::new(text.as_bytes().to_vec());
    owner.offer([a.UTF8_STRING, a.TEXT_UTF8, a.TEXT].into_iter().map(|kind| (kind, text.clone())).collect());
}

fn owner() -> Option<&'static Owner> {
    static OWNER: OnceLock<Option<&'static Owner>> = OnceLock::new();
    *OWNER.get_or_init(|| {
        let owner: &'static Owner = Box::leak(Box::new(start().inspect_err(|e| eprintln!("kiri: can't copy images: {e}")).ok()?));
        std::thread::spawn(move || owner.serve());
        Some(owner)
    })
}

fn start() -> Result<Owner, Box<dyn std::error::Error>> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let window = conn.generate_id()?;
    conn.create_window(0, window, root, 0, 0, 1, 1, 0, WindowClass::INPUT_ONLY, 0, &CreateWindowAux::new())?;
    let atoms = Atoms::new(&conn)?.reply()?;
    Ok(Owner { conn, window, atoms, offered: Mutex::new(Vec::new()) })
}

impl Owner {
    fn offer(&self, offered: Vec<(u32, Arc<Vec<u8>>)>) {
        *self.offered.lock().unwrap() = offered;
        let _ = self.conn.set_selection_owner(self.window, self.atoms.CLIPBOARD, x11rb::CURRENT_TIME);
        let _ = self.conn.flush();
    }

    fn serve(&self) {
        // Answers sent in pieces, by the requestor's window and property.
        let mut pieces: HashMap<(u32, u32), (u32, Arc<Vec<u8>>, usize)> = HashMap::new();
        while let Ok(event) = self.conn.wait_for_event() {
            match event {
                Event::SelectionRequest(request) => self.answer(request, &mut pieces),
                Event::SelectionClear(_) => self.offered.lock().unwrap().clear(),
                Event::PropertyNotify(e) if e.state == Property::DELETE => {
                    let Some((kind, data, at)) = pieces.get_mut(&(e.window, e.atom)) else { continue };
                    let end = (*at + CHUNK).min(data.len());
                    let _ = self.conn.change_property8(PropMode::REPLACE, e.window, e.atom, *kind, &data[*at..end]);
                    // The empty piece after the last one ends the transfer.
                    if *at == data.len() {
                        pieces.remove(&(e.window, e.atom));
                    } else {
                        *at = end;
                    }
                }
                _ => continue,
            }
            let _ = self.conn.flush();
        }
    }

    fn answer(&self, r: SelectionRequestEvent, pieces: &mut HashMap<(u32, u32), (u32, Arc<Vec<u8>>, usize)>) {
        let a = &self.atoms;
        // Clients older than the ICCCM leave the property out and mean the target.
        let property = if r.property == x11rb::NONE { r.target } else { r.property };
        let offered = self.offered.lock().unwrap().clone();
        let found = offered.iter().find(|(kind, _)| *kind == r.target).map(|(_, data)| data.clone());
        let done = if r.selection != a.CLIPBOARD || offered.is_empty() {
            false
        } else if r.target == a.TARGETS {
            let targets: Vec<u32> = [a.TARGETS].into_iter().chain(offered.iter().map(|(kind, _)| *kind)).collect();
            self.conn.change_property32(PropMode::REPLACE, r.requestor, property, AtomEnum::ATOM, &targets).is_ok()
        } else if let Some(data) = found {
            if data.len() > CHUNK {
                let _ = self.conn.change_window_attributes(r.requestor, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE));
                pieces.insert((r.requestor, property), (r.target, data.clone(), 0));
                self.conn.change_property32(PropMode::REPLACE, r.requestor, property, a.INCR, &[data.len() as u32]).is_ok()
            } else {
                self.conn.change_property8(PropMode::REPLACE, r.requestor, property, r.target, &data).is_ok()
            }
        } else {
            false
        };
        let notify = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: r.time,
            requestor: r.requestor,
            selection: r.selection,
            target: r.target,
            property: if done { property } else { x11rb::NONE },
        };
        let _ = self.conn.send_event(false, r.requestor, EventMask::NO_EVENT, notify);
    }
}

fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => uri.push(b as char),
            _ => uri.push_str(&format!("%{b:02X}")),
        }
    }
    uri
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_uris_escape_what_a_path_may_hold() {
        assert_eq!(super::file_uri("/home/me/Pictures/Kiri 1%.png".as_ref()), "file:///home/me/Pictures/Kiri%201%25.png");
    }
}
