//! Updates on Windows: the release's zip holds `Kiri\Kiri.exe`, which takes the running exe's place.
//! Windows' own `curl` and `tar` download and unzip it.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::update::run;

pub const ASSET: &str = "Kiri-windows-x64.zip";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The folder Kiri runs from; `None` for a build in Cargo's `target`, which can't update itself.
pub fn bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let built = dir.ancestors().take(3).any(|d| d.file_name().is_some_and(|n| n.eq_ignore_ascii_case("target")));
    (!built).then(|| dir.to_path_buf())
}

/// Explorer runs an exe inside a zip from a copy in the temp folder.
pub fn misplaced(app: &Path) -> Option<&'static str> {
    let temp = std::env::temp_dir().canonicalize().ok()?;
    app.canonicalize().ok()?.starts_with(temp).then_some("Extract Kiri from the zip to get updates")
}

fn system(tool: &str) -> Command {
    let root = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let mut command = Command::new(root.join("System32").join(tool));
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

pub fn curl() -> Command {
    system("curl.exe")
}

pub fn cache() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default().join(r"Kiri\Update")
}

/// No signature to check: Kiri for Windows isn't code-signed yet.
pub fn unpack(dir: &Path, zip: &Path) -> Result<PathBuf, String> {
    let out = dir.join("unzipped");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    run(system("tar.exe").arg("-xf").arg(zip).arg("-C").arg(&out))?;
    let exe = out.join(r"Kiri\Kiri.exe");
    exe.is_file().then_some(exe).ok_or_else(|| "the download has no Kiri.exe".to_string())
}

/// Windows lets a running exe be renamed but not replaced, so it moves aside for `staged`, and
/// goes back if that fails. The relaunch deletes it.
pub fn swap(_app: &Path, staged: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let old = old(&exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(|e| e.to_string())?;
    // A rename can't cross drives; the cache is on the system drive.
    if let Err(e) = std::fs::rename(staged, &exe).or_else(|_| std::fs::copy(staged, &exe).map(drop)) {
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::rename(&old, &exe);
        return Err(e.to_string());
    }
    Ok(())
}

/// Settings → Apps shows the version Kiri-Setup.exe installed, so an update in place corrects it.
pub fn note_installed_version() {
    use windows::Win32::System::Registry::*;
    use windows::core::w;
    // The AppId in windows/Kiri.iss.
    let key = w!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{6D9B7D82-FF2E-4481-BBD7-6A4A225A5970}_is1");
    let read = |name| {
        let mut buf = [0u16; 512];
        let mut size = (buf.len() * 2) as u32;
        unsafe { RegGetValueW(HKEY_CURRENT_USER, key, name, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size)) }
            .ok()
            .ok()?;
        Some(String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0)?]))
    };
    let (Some(dir), Some(shown)) = (read(w!("InstallLocation")), read(w!("DisplayVersion"))) else { return };
    let here = bundle().is_some_and(|b| b.to_string_lossy().eq_ignore_ascii_case(dir.trim_end_matches('\\')));
    let version = env!("CARGO_PKG_VERSION");
    if here && shown != version {
        let value: Vec<u16> = version.encode_utf16().chain([0]).collect();
        let _ = unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key, w!("DisplayVersion"), REG_SZ.0, Some(value.as_ptr().cast()), (value.len() * 2) as u32) };
    }
}

fn old(exe: &Path) -> PathBuf {
    let mut name = exe.as_os_str().to_owned();
    name.push(".old");
    PathBuf::from(name)
}

/// Starts the new exe once this process is gone, or it would hand its launch to this one.
pub fn relaunch(_app: &Path) {
    let Ok(exe) = std::env::current_exe() else { return };
    let script = "Wait-Process -Id $env:KIRI_PID -ErrorAction SilentlyContinue; \
                  Remove-Item -LiteralPath $env:KIRI_OLD -ErrorAction SilentlyContinue; \
                  Start-Process -FilePath $env:KIRI_EXE";
    let _ = system(r"WindowsPowerShell\v1.0\powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("KIRI_PID", std::process::id().to_string())
        .env("KIRI_OLD", old(&exe))
        .env("KIRI_EXE", &exe)
        .spawn();
}
