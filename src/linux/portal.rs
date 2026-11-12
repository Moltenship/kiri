//! The desktop portal, which takes screenshots and runs global shortcuts for Kiri where Wayland
//! lets no app do either itself.

use std::path::PathBuf;
use std::sync::Mutex;

use ashpd::desktop::ResponseError;
use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use ashpd::desktop::screenshot::Screenshot;
use ashpd::zbus;
use futures::StreamExt;
use futures::channel::{mpsc, oneshot};

/// Kiri's own bus connection: the portal learns an unsandboxed app's id once per connection,
/// before any other call on it, and GPUI's shared one has made calls already.
async fn connection() -> ashpd::Result<zbus::Connection> {
    static CONNECTION: Mutex<Option<zbus::Connection>> = Mutex::new(None);
    if let Some(c) = CONNECTION.lock().unwrap().clone() {
        return Ok(c);
    }
    let c = zbus::Connection::session().await?;
    // Portals older than 1.19 have no registry and go by the window's app id instead.
    if let Ok(id) = ashpd::AppID::try_from(super::APP_ID) {
        let _ = ashpd::register_host_app_with_connection(c.clone(), id).await;
    }
    *CONNECTION.lock().unwrap() = Some(c.clone());
    Ok(c)
}

/// The image file's bytes, of the whole screen or, with `interactive`, of what the user picks in
/// the desktop's own screenshot UI. `Ok(None)` when they cancel.
pub async fn screenshot(interactive: bool) -> Result<Option<Vec<u8>>, String> {
    let taken = async {
        let request = Screenshot::request().interactive(interactive).modal(true).connection(Some(connection().await?));
        request.send().await?.response()
    };
    let uri = match taken.await {
        Ok(shot) => shot.uri().as_str().to_string(),
        Err(ashpd::Error::Response(ResponseError::Cancelled)) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let path = file_path(&uri).ok_or_else(|| format!("the portal's screenshot isn't a file: {uri}"))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    // Some desktops save the file where the user keeps screenshots; only a temporary one goes.
    let temporary = [std::env::temp_dir(), super::xdg("XDG_RUNTIME_DIR", ".cache"), super::xdg("XDG_CACHE_HOME", ".cache")];
    if temporary.iter().any(|dir| path.starts_with(dir)) {
        let _ = std::fs::remove_file(&path);
    }
    Ok(Some(bytes))
}

fn file_path(uri: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let rest = uri.strip_prefix("file://")?;
    let path = &rest[rest.find('/')?..];
    let (mut bytes, mut i) = (Vec::new(), 0);
    let raw = path.as_bytes();
    while i < raw.len() {
        let hex = raw.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
        match (raw[i], hex) {
            (b'%', Some(b)) => (bytes.push(b), i += 3),
            (b, _) => (bytes.push(b), i += 1),
        };
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// Shortcuts bound through the portal until this drops.
pub struct Shortcuts(#[allow(dead_code)] oneshot::Sender<()>);

/// One shortcut: its id, what it does, and the keys asked for, in the portal's `CTRL+SHIFT+a` form.
pub struct Binding {
    pub id: String,
    pub description: String,
    pub trigger: Option<String>,
}

/// Binds `bindings` and sends the id of each one pressed to `pressed`. The desktop may ask the
/// user first and may bind other keys. `failed` hears why, when the desktop can't bind any.
pub fn bind(bindings: Vec<Binding>, pressed: mpsc::UnboundedSender<String>, failed: oneshot::Sender<String>) -> (Shortcuts, impl Future<Output = ()> + Send) {
    let (stop, stopped) = oneshot::channel::<()>();
    let run = async move {
        let bound = async {
            let portal = GlobalShortcuts::with_connection(connection().await?).await?;
            let session = portal.create_session(Default::default()).await?;
            let shortcuts: Vec<NewShortcut> = bindings
                .iter()
                .map(|b| NewShortcut::new(&b.id, &b.description).preferred_trigger(b.trigger.as_deref()))
                .collect();
            let activated = portal.receive_activated().await?;
            portal.bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default()).await?.response()?;
            ashpd::Result::Ok((session, activated))
        };
        let (session, activated) = match bound.await {
            Ok(bound) => bound,
            Err(e) => {
                let _ = failed.send(e.to_string());
                return;
            }
        };
        let mut activated = activated.fuse();
        let mut stopped = stopped;
        loop {
            futures::select! {
                a = activated.next() => match a {
                    Some(a) => { let _ = pressed.unbounded_send(a.shortcut_id().to_string()); }
                    None => break,
                },
                _ = stopped => break,
            }
        }
        let _ = session.close().await;
    };
    (Shortcuts(stop), run)
}

/// Kiri's `ctrl+shift+printscreen` as the shortcuts spec writes it, with XKB key names: `CTRL+SHIFT+Print`.
pub fn trigger(keys: &str) -> String {
    keys.split('+')
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "CTRL".to_string(),
            "shift" => "SHIFT".to_string(),
            "alt" | "option" => "ALT".to_string(),
            "cmd" | "super" | "meta" | "win" => "LOGO".to_string(),
            "printscreen" => "Print".to_string(),
            "escape" => "Escape".to_string(),
            "pageup" => "Page_Up".to_string(),
            "pagedown" => "Page_Down".to_string(),
            "backspace" => "BackSpace".to_string(),
            "enter" | "return" => "Return".to_string(),
            k @ ("tab" | "delete" | "home" | "end" | "insert" | "up" | "down" | "left" | "right") => k[..1].to_uppercase() + &k[1..],
            k if k.len() > 1 && k.starts_with('f') && k[1..].bytes().all(|b| b.is_ascii_digit()) => k.to_uppercase(),
            k => k.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    #[test]
    fn screenshot_uris_become_paths() {
        assert_eq!(super::file_path("file:///home/me/Pictures/Screenshot%20from%2012%3A00.png").unwrap().to_str(), Some("/home/me/Pictures/Screenshot from 12:00.png"));
        assert_eq!(super::file_path("file://localhost/tmp/a.png").unwrap().to_str(), Some("/tmp/a.png"));
        assert!(super::file_path("https://example.com/a.png").is_none());
    }

    #[test]
    fn triggers_use_the_spec_s_modifiers_and_xkb_key_names() {
        assert_eq!(super::trigger("ctrl+shift+2"), "CTRL+SHIFT+2");
        assert_eq!(super::trigger("alt+printscreen"), "ALT+Print");
        assert_eq!(super::trigger("super+shift+f12"), "LOGO+SHIFT+F12");
    }
}
