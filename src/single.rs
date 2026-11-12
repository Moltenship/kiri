//! One Kiri per user: a second launch hands its request to the running one and exits,
//! so there is never a second tray icon or a fight over the hotkeys.

use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use futures::channel::mpsc::UnboundedSender;

use crate::Command;
use crate::settings::Capture;

/// Keyed by the settings file, so a Kiri started with another `HOME` runs on its own.
/// It lives in the per-user temp dir because socket paths can't exceed 104 bytes.
/// On Windows it is a named pipe instead.
fn socket() -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    crate::settings::path().hash(&mut h);
    if cfg!(windows) {
        return PathBuf::from(format!(r"\\.\pipe\kiri-{:x}", h.finish()));
    }
    // Linux has a runtime dir only its user can enter.
    #[cfg(target_os = "linux")]
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|d| d.is_absolute()).unwrap_or_else(std::env::temp_dir);
    #[cfg(not(target_os = "linux"))]
    let dir = std::env::temp_dir();
    dir.join(format!("kiri-{:x}.sock", h.finish()))
}

/// What `--then` asks for after one automated capture, in place of the after-capture settings.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Then {
    pub edit: bool,
    pub thumbnail: bool,
    pub copy: bool,
    pub save: bool,
}

impl Then {
    pub const WORDS: [&str; 4] = ["edit", "thumbnail", "copy", "save"];

    /// A comma-separated list of `WORDS`, as in `copy,save`.
    pub fn parse(list: &str) -> Result<Then, String> {
        let mut then = Then::default();
        for word in list.split(',').map(str::trim).filter(|w| !w.is_empty()) {
            match word {
                "edit" => then.edit = true,
                "thumbnail" => then.thumbnail = true,
                "copy" => then.copy = true,
                "save" => then.save = true,
                _ => return Err(word.into()),
            }
        }
        Ok(then)
    }

    fn arg(self) -> String {
        let on = [self.edit, self.thumbnail, self.copy, self.save];
        Then::WORDS.iter().zip(on).filter(|(_, on)| *on).map(|(w, _)| *w).collect::<Vec<_>>().join(",")
    }
}

/// Sends `open` (a file to edit), a capture, or nothing (show Settings) to a running Kiri.
/// Returns false when none is running.
pub fn forward(open: Option<&Path>, capture: Option<(Capture, Option<Then>)>) -> bool {
    let line = match (open.and_then(|p| std::path::absolute(p).ok()), capture) {
        (Some(path), _) => format!("open {}\n", path.display()),
        (None, Some((capture, None))) => format!("capture {}\n", capture.cli_name()),
        (None, Some((capture, Some(then)))) => format!("capture {} then={}\n", capture.cli_name(), then.arg()),
        (None, None) => "settings\n".into(),
    };
    send(&line)
}

/// Quits a running Kiri as its tray menu's Quit does; the Windows installer and uninstaller use it.
pub fn quit() {
    send("quit\n");
}

fn send(line: &str) -> bool {
    #[cfg(unix)]
    let stream = UnixStream::connect(socket());
    #[cfg(windows)]
    let stream = std::fs::OpenOptions::new().write(true).open(socket());
    stream.is_ok_and(|mut stream| stream.write_all(line.as_bytes()).is_ok())
}

pub fn listen(tx: UnboundedSender<Command>) {
    let path = socket();
    #[cfg(unix)]
    let _ = std::fs::remove_file(&path);
    #[cfg(unix)]
    let Ok(listener) = UnixListener::bind(&path) else {
        return;
    };
    #[cfg(unix)]
    let incoming = std::iter::repeat_with(move || listener.accept()).flatten().map(|(stream, _)| stream);
    // Each client gets a fresh pipe instance; the loop ends when one can't be made.
    #[cfg(windows)]
    let incoming = std::iter::from_fn(move || crate::mac::accept_pipe(&path));
    std::thread::spawn(move || {
        for stream in incoming {
            let mut line = String::new();
            if BufReader::new(stream).read_line(&mut line).is_err() {
                continue;
            }
            let line = line.trim_end_matches('\n');
            let command = match line.split_once(' ') {
                _ if line == "quit" => Command::Quit,
                Some(("open", path)) => Command::Open(path.into()),
                Some(("capture", rest)) => {
                    let (name, then) = match rest.split_once(" then=") {
                        Some((name, list)) => (name, Then::parse(list).ok()),
                        None => (rest, None),
                    };
                    match Capture::from_cli(name) {
                        Some(capture) => Command::Automation(capture, then),
                        None => continue,
                    }
                }
                _ => Command::Capture(Capture::Settings, std::time::Instant::now()),
            };
            let _ = tx.unbounded_send(command);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::Then;

    #[test]
    fn then_lists_round_trip_and_reject_unknown_words() {
        let then = Then::parse("copy, save").unwrap();
        assert_eq!(then, Then { copy: true, save: true, ..Then::default() });
        assert_eq!(Then::parse(&then.arg()), Ok(then));
        assert_eq!(Then::parse("copy,upload"), Err("upload".into()));
    }
}
