//! One Kiri per user: a second launch hands its request to the running one and exits,
//! so there is never a second tray icon or a fight over the hotkeys.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use futures::channel::mpsc::UnboundedSender;

use crate::Command;
use crate::settings::Capture;

/// Keyed by the settings file, so a Kiri started with another `HOME` runs on its own.
/// It lives in the per-user temp dir because socket paths can't exceed 104 bytes.
fn socket() -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    crate::settings::path().hash(&mut h);
    std::env::temp_dir().join(format!("kiri-{:x}.sock", h.finish()))
}

/// Sends `open` (a file to edit), a capture, or nothing (show Settings) to a running Kiri.
/// Returns false when none is running.
pub fn forward(open: Option<&Path>, capture: Option<Capture>) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket()) else {
        return false;
    };
    let line = match (open.and_then(|p| std::path::absolute(p).ok()), capture) {
        (Some(path), _) => format!("open {}\n", path.display()),
        (None, Some(capture)) => format!("capture {}\n", capture.cli_name()),
        (None, None) => "settings\n".into(),
    };
    stream.write_all(line.as_bytes()).is_ok()
}

pub fn listen(tx: UnboundedSender<Command>) {
    let path = socket();
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        return;
    };
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut line = String::new();
            if BufReader::new(stream).read_line(&mut line).is_err() {
                continue;
            }
            let command = match line.trim_end_matches('\n').split_once(' ') {
                Some(("open", path)) => Command::Open(path.into(), false),
                Some(("capture", name)) => match Capture::from_cli(name) {
                    Some(capture) => Command::Automation(capture),
                    None => continue,
                },
                _ => Command::Capture(Capture::Settings, std::time::Instant::now()),
            };
            let _ = tx.unbounded_send(command);
        }
    });
}
