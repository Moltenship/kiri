//! Updates on Linux: the release's tarball holds `Kiri/kiri`, which takes the running binary's
//! place. Nothing signs it; HTTPS from GitHub is what vouches for the download.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::update::run;

pub const ASSET: &str = "Kiri-linux-x64.tar.gz";

/// The folder Kiri runs from; `None` for a build in Cargo's `target`, which can't update itself.
pub fn bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let built = dir.ancestors().take(3).any(|d| d.file_name().is_some_and(|n| n == "target"));
    (!built).then(|| dir.to_path_buf())
}

/// File managers open a binary inside an archive from a copy in the temp folder, and an
/// install the user doesn't own can't be replaced.
pub fn misplaced(app: &Path) -> Option<&'static str> {
    use std::os::unix::ffi::OsStrExt;
    let temp = std::env::temp_dir().canonicalize().ok()?;
    if app.canonicalize().ok()?.starts_with(temp) {
        return Some("Extract Kiri from the archive to get updates");
    }
    let dir = std::ffi::CString::new(app.as_os_str().as_bytes()).ok()?;
    (unsafe { libc::access(dir.as_ptr(), libc::W_OK) } != 0).then_some("Move Kiri to a folder you own to get updates")
}

pub fn curl() -> Command {
    Command::new("curl")
}

/// Ubuntu's desktop comes with wget and not curl, so wget fetches when curl isn't installed.
pub fn wget(url: &str, out: Option<&Path>, seconds: u32, accept: Option<&str>) -> Option<Command> {
    let has_curl = Command::new("curl").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
    if has_curl {
        return None;
    }
    let mut wget = Command::new("wget");
    wget.args(["-nv", "-T"]).arg(seconds.to_string());
    if let Some(accept) = accept {
        wget.arg(format!("--header=Accept: {accept}"));
    }
    wget.arg("-O").arg(out.map_or(std::ffi::OsStr::new("-"), Path::as_os_str)).arg(url);
    Some(wget)
}

pub fn cache() -> PathBuf {
    super::xdg("XDG_CACHE_HOME", ".cache").join("kiri/update")
}

pub fn unpack(dir: &Path, archive: &Path) -> Result<PathBuf, String> {
    let out = dir.join("unpacked");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    run(Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(&out))?;
    let exe = out.join("Kiri/kiri");
    exe.is_file().then_some(exe).ok_or_else(|| "the download has no Kiri/kiri".to_string())
}

/// Copies `staged` next to the running binary, then renames it over that one, so the swap is
/// whole or not at all. The running process keeps its old file until it exits.
pub fn swap(_app: &Path, staged: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let next = exe.with_file_name(".kiri.new");
    let placed = std::fs::copy(staged, &next)
        .and_then(|_| std::fs::set_permissions(&next, std::fs::Permissions::from_mode(0o755)))
        .and_then(|_| std::fs::rename(&next, &exe));
    if let Err(e) = placed {
        let _ = std::fs::remove_file(&next);
        return Err(e.to_string());
    }
    Ok(())
}

/// Starts the new binary once this process is gone, or it would hand its launch to this one.
pub fn relaunch(_app: &Path) {
    let Ok(exe) = std::env::current_exe() else { return };
    // Linux names the replaced file this way once a new one sits at its path.
    let exe = exe.to_string_lossy().trim_end_matches(" (deleted)").to_string();
    let script = format!("while kill -0 {} 2>/dev/null; do sleep 0.1; done; exec \"$0\"", std::process::id());
    let _ = Command::new("/bin/sh").args(["-c", &script]).arg(&exe).stdin(Stdio::null()).spawn();
}
