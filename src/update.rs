//! Updates from GitHub releases: a daily check marks the menu bar icon when a newer release is
//! out, and Update downloads it, swaps it in, and relaunches. On macOS only a build signed with
//! Kiri's Developer ID may replace the app.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc::UnboundedSender;
use gpui_kit::{App, Global};

/// Every release is published as Latest; the release list isn't sorted by version.
const LATEST: &str = "https://api.github.com/repos/Moltenship/kiri/releases/latest";

#[cfg(windows)]
use crate::mac::update as sys;
#[cfg(target_os = "linux")]
use crate::linux::update as sys;
pub use sys::bundle;

#[derive(Default)]
pub struct Update {
    busy: bool,
    /// Between Update and the relaunch.
    pub installing: bool,
    /// The newest release above this build.
    pub available: Option<Release>,
}

impl Global for Update {}

#[derive(Clone)]
pub struct Release {
    pub version: String,
    url: String,
    /// Of the download, in bytes, for its percentage.
    size: u64,
}

enum Step {
    Download(Option<f32>),
    Check,
    Install,
}

fn say(version: &str, step: Step, cx: &mut App) {
    let (text, fraction) = match step {
        Step::Download(Some(f)) => (format!("Downloading Kiri {version}… {}%", (f * 100.) as u32), Some(f)),
        Step::Download(None) => (format!("Downloading Kiri {version}…"), None),
        Step::Check => ("Checking the download…".to_string(), None),
        Step::Install => (format!("Installing Kiri {version}…"), None),
    };
    crate::toast::progress(text, fraction, cx);
}

pub fn start(cx: &mut App) {
    if bundle().is_none() {
        return;
    }
    cx.spawn(async |cx| {
        cx.background_executor().timer(Duration::from_secs(10)).await;
        loop {
            if cx.update(|cx| crate::settings::get(cx).auto_update) {
                cx.update(|cx| check(false, cx));
            }
            cx.background_executor().timer(Duration::from_secs(24 * 60 * 60)).await;
        }
    })
    .detach();
}

/// Looks for a newer release. Only a check from the menu reports what it found in a toast.
pub fn check(manual: bool, cx: &mut App) {
    let Some(app) = bundle() else { return };
    if let Some(why) = sys::misplaced(&app) {
        if manual {
            crate::toast::show(why, cx);
        }
        return;
    }
    if std::mem::replace(&mut cx.default_global::<Update>().busy, true) {
        return;
    }
    cx.spawn(async move |cx| {
        let found = cx.background_executor().spawn(async move { latest() }).await;
        cx.update(|cx| {
            cx.global_mut::<Update>().busy = false;
            match found {
                Ok(Some(release)) => {
                    let version = release.version.clone();
                    cx.global_mut::<Update>().available = Some(release);
                    crate::refresh_tray(cx);
                    if manual {
                        crate::toast::with_action(format!("Kiri {version} is available"), "Update", install, cx);
                    }
                }
                Ok(None) if manual => crate::toast::show(format!("Kiri {} is the latest", env!("CARGO_PKG_VERSION")), cx),
                Err(err) if manual => crate::toast::show(format!("Couldn't check for updates: {err}"), cx),
                Err(err) => eprintln!("kiri: update check failed: {err}"),
                Ok(None) => {}
            }
        });
    })
    .detach();
}

/// Downloads the available release, swaps it in, and relaunches Kiri.
pub fn install(cx: &mut App) {
    let (Some(app), Some(release)) = (bundle(), cx.default_global::<Update>().available.clone()) else {
        return;
    };
    if std::mem::replace(&mut cx.global_mut::<Update>().busy, true) {
        return;
    }
    cx.global_mut::<Update>().installing = true;
    crate::refresh_tray(cx);
    let version = release.version.clone();
    say(&version, Step::Download(None), cx);
    let target = app.clone();
    cx.spawn(async move |cx| {
        let (tx, mut steps) = futures::channel::mpsc::unbounded();
        let work = cx.background_executor().spawn(async move {
            let staged = download(&release, &tx)?;
            let _ = tx.unbounded_send(Step::Install);
            sys::swap(&target, &staged)
        });
        // Ends when `work` finishes and drops `tx`.
        while let Some(step) = steps.next().await {
            cx.update(|cx| say(&version, step, cx));
        }
        let done = work.await;
        cx.update(|cx| {
            let update = cx.global_mut::<Update>();
            update.busy = false;
            update.installing = false;
            if let Err(err) = done {
                crate::refresh_tray(cx);
                crate::toast::show(format!("Couldn't update: {err}"), cx);
                return;
            }
            sys::relaunch(&app);
            cx.quit();
        });
    })
    .detach();
}

/// The newest release above this build; `None` when up to date.
fn latest() -> Result<Option<Release>, String> {
    #[derive(serde::Deserialize)]
    struct Published {
        tag_name: String,
        assets: Vec<Asset>,
    }
    #[derive(serde::Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
        size: u64,
    }
    let body = run(&mut fetch(LATEST, None, 30, Some("application/vnd.github+json")))?;
    let release: Published = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let version = release.tag_name.trim_start_matches('v').to_string();
    if !newer(&version, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let asset = release.assets.into_iter().find(|a| a.name == sys::ASSET).ok_or(format!("the release has no {}", sys::ASSET))?;
    Ok(Some(Release { version, url: asset.browser_download_url, size: asset.size }))
}

/// Downloads the release's asset and unpacks Kiri from it.
fn download(release: &Release, steps: &UnboundedSender<Step>) -> Result<PathBuf, String> {
    let dir = sys::cache();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let archive = dir.join(sys::ASSET);
    let mut curl = fetch(&release.url, Some(&archive), 600, None)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut shown = None;
    while curl.try_wait().map_err(|e| e.to_string())?.is_none() {
        let got = std::fs::metadata(&archive).map_or(0, |m| m.len());
        let percent = (release.size > 0).then(|| (got * 100 / release.size).min(100));
        if percent != shown {
            shown = percent;
            let _ = steps.unbounded_send(Step::Download(percent.map(|p| p as f32 / 100.)));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let out = curl.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let _ = steps.unbounded_send(Step::Check);
    sys::unpack(&dir, &archive)
}

/// `url` into `out`, or to stdout without one, giving up after `seconds`.
fn fetch(url: &str, out: Option<&Path>, seconds: u32, accept: Option<&str>) -> Command {
    #[cfg(target_os = "linux")]
    if let Some(wget) = sys::wget(url, out, seconds, accept) {
        return wget;
    }
    let mut curl = sys::curl();
    curl.args(["-fsSL", "--max-time"]).arg(seconds.to_string());
    if let Some(accept) = accept {
        curl.arg("-H").arg(format!("Accept: {accept}"));
    }
    if let Some(out) = out {
        curl.arg("-o").arg(out);
    }
    curl.arg(url);
    curl
}

pub(crate) fn run(command: &mut Command) -> Result<Vec<u8>, String> {
    let out = command.output().map_err(|e| e.to_string())?;
    match out.status.success() {
        true => Ok(out.stdout),
        false => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
    }
}

/// Releases are plain `x.y.z` (docs/operations/release.md), so a `-suffix` is ignored.
fn newer(a: &str, b: &str) -> bool {
    let key = |v: &str| v.split('-').next().unwrap_or(v).split('.').map(|n| n.parse().unwrap_or(0)).collect::<Vec<u64>>();
    key(a) > key(b)
}

#[cfg(target_os = "macos")]
mod sys {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::run;

    pub const ASSET: &str = "Kiri.dmg";
    /// Only a build signed by Kiri's own Developer ID may replace the running app.
    const REQUIREMENT: &str =
        r#"=anchor apple generic and identifier "sh.kiri.Kiri" and certificate leaf[subject.OU] = "Q9SDRFKBC6""#;

    /// The `.app` Kiri runs from; `None` for a bare binary, which can't update itself.
    pub fn bundle() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let app = exe.parent()?.parent()?.parent()?;
        (app.extension()? == "app").then(|| app.to_path_buf())
    }

    pub fn misplaced(app: &Path) -> Option<&'static str> {
        app.to_string_lossy().contains("/AppTranslocation/").then_some("Move Kiri to Applications to get updates")
    }

    pub fn curl() -> Command {
        Command::new("/usr/bin/curl")
    }

    pub fn cache() -> PathBuf {
        std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join("Library/Caches/Kiri/Update")
    }

    /// Copies Kiri out of the DMG and checks that Kiri's Developer ID signed it.
    pub fn unpack(dir: &Path, dmg: &Path) -> Result<PathBuf, String> {
        let volume = dir.join("volume");
        run(Command::new("/usr/bin/hdiutil").args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"]).arg(&volume).arg(dmg))?;
        let staged = dir.join("Kiri.app");
        let copied = run(Command::new("/usr/bin/ditto").arg(volume.join("Kiri.app")).arg(&staged));
        let _ = run(Command::new("/usr/bin/hdiutil").args(["detach", "-force"]).arg(&volume));
        copied?;
        run(Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict", "-R", REQUIREMENT]).arg(&staged))
            .map_err(|_| "the download isn't signed by Kiri".to_string())?;
        Ok(staged)
    }

    /// Swaps the running bundle `app` for `staged`. The old one goes back in place if the move fails.
    pub fn swap(app: &Path, staged: &Path) -> Result<(), String> {
        let old = app.with_file_name(".Kiri.app.old");
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(app, &old).map_err(|e| e.to_string())?;
        if let Err(e) = std::fs::rename(staged, app)
            .or_else(|_| run(Command::new("/usr/bin/ditto").arg(staged).arg(app)).map(drop).map_err(std::io::Error::other))
        {
            let _ = std::fs::remove_dir_all(app);
            let _ = std::fs::rename(&old, app);
            return Err(e.to_string());
        }
        let _ = std::fs::remove_dir_all(&old);
        Ok(())
    }

    /// `open` waits for this process to exit, or it would just activate the running copy.
    pub fn relaunch(app: &Path) {
        let script = format!("while kill -0 {} 2>/dev/null; do sleep 0.1; done; open \"$0\"", std::process::id());
        let _ = Command::new("/bin/sh").args(["-c", &script]).arg(app).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("0.2.10", "0.2.9"));
        assert!(newer("1.0.0", "0.9.9"));
        assert!(!newer("0.2.3", "0.2.3"));
        assert!(!newer("0.2.2", "0.2.3"));
    }
}
