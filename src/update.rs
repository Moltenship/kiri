//! Updates from GitHub releases: a daily check marks the menu bar icon when a newer release is
//! out, and Update downloads it, verifies its signature, swaps the bundle, and relaunches.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use gpui_kit::{App, Global};

const RELEASES: &str = "https://api.github.com/repos/Moltenship/kiri/releases?per_page=10";
const ASSET: &str = "Kiri.zip";
/// Only a build signed by Kiri's own Developer ID may replace the running app.
const REQUIREMENT: &str =
    r#"=anchor apple generic and identifier "sh.kiri.Kiri" and certificate leaf[subject.OU] = "Q9SDRFKBC6""#;

#[derive(Default)]
pub struct Update {
    busy: bool,
    /// The newest release above this build: its version and the download URL.
    pub available: Option<(String, String)>,
}

impl Global for Update {}

/// The `.app` Kiri runs from; `None` for a bare binary, which can't update itself.
fn bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let app = exe.parent()?.parent()?.parent()?;
    (app.extension()? == "app").then(|| app.to_path_buf())
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
    let Some(app) = bundle() else {
        if manual {
            crate::toast::show("Updates need Kiri.app; this build runs on its own", cx);
        }
        return;
    };
    if app.to_string_lossy().contains("/AppTranslocation/") {
        if manual {
            crate::toast::show("Move Kiri to Applications to get updates", cx);
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
                    let version = release.0.clone();
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
    let (Some(app), Some((version, url))) = (bundle(), cx.default_global::<Update>().available.clone()) else {
        return;
    };
    if std::mem::replace(&mut cx.global_mut::<Update>().busy, true) {
        return;
    }
    crate::toast::show(format!("Downloading Kiri {version}…"), cx);
    cx.spawn(async move |cx| {
        let done = cx.background_executor().spawn(async move { download(&url).and_then(|staged| swap(&staged)) }).await;
        cx.update(|cx| {
            cx.global_mut::<Update>().busy = false;
            if let Err(err) = done {
                crate::toast::show(format!("Couldn't update: {err}"), cx);
                return;
            }
            // `open` waits for this process to exit, or it would just activate the running copy.
            let script = format!("while kill -0 {} 2>/dev/null; do sleep 0.1; done; open \"$0\"", std::process::id());
            let _ = Command::new("/bin/sh").args(["-c", &script]).arg(&app).spawn();
            cx.quit();
        });
    })
    .detach();
}

/// The newest release above this build, as its version and download URL; `None` when up to date.
fn latest() -> Result<Option<(String, String)>, String> {
    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
        draft: bool,
        assets: Vec<Asset>,
    }
    #[derive(serde::Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
    }
    let body = run(Command::new("/usr/bin/curl").args(["-fsSL", "--max-time", "30", "-H", "Accept: application/vnd.github+json", RELEASES]))?;
    let releases: Vec<Release> = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    Ok(releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let url = r.assets.into_iter().find(|a| a.name == ASSET)?.browser_download_url;
            Some((r.tag_name.trim_start_matches('v').to_string(), url))
        })
        .filter(|(v, _)| newer(v, env!("CARGO_PKG_VERSION")))
        .max_by(|a, b| if newer(&a.0, &b.0) { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less }))
}

/// Downloads and unpacks `url`, and checks that Kiri's Developer ID signed it.
fn download(url: &str) -> Result<PathBuf, String> {
    let dir = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join("Library/Caches/Kiri/Update");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let zip = dir.join(ASSET);
    run(Command::new("/usr/bin/curl").args(["-fsSL", "--max-time", "600", "-o"]).arg(&zip).arg(url))?;
    run(Command::new("/usr/bin/ditto").args(["-x", "-k"]).arg(&zip).arg(&dir))?;
    let staged = dir.join("Kiri.app");
    run(Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict", "-R", REQUIREMENT]).arg(&staged))
        .map_err(|_| "the download isn't signed by Kiri".to_string())?;
    Ok(staged)
}

/// Swaps the running bundle for `staged`. The old one goes back in place if the move fails.
fn swap(staged: &Path) -> Result<(), String> {
    let app = bundle().ok_or("not running from Kiri.app")?;
    let old = app.with_file_name(".Kiri.app.old");
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(&app, &old).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(staged, &app)
        .or_else(|_| run(Command::new("/usr/bin/ditto").arg(staged).arg(&app)).map(drop).map_err(std::io::Error::other))
    {
        let _ = std::fs::remove_dir_all(&app);
        let _ = std::fs::rename(&old, &app);
        return Err(e.to_string());
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
}

fn run(command: &mut Command) -> Result<Vec<u8>, String> {
    let out = command.output().map_err(|e| e.to_string())?;
    match out.status.success() {
        true => Ok(out.stdout),
        false => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
    }
}

/// Semver precedence, enough for `1.2.3` and `1.2.3-beta.4`: a release beats its pre-releases.
fn newer(a: &str, b: &str) -> bool {
    fn key(v: &str) -> (Vec<u64>, Option<Vec<(u64, String)>>) {
        let (core, pre) = v.split_once('-').map_or((v, None), |(c, p)| (c, Some(p)));
        let core = core.split('.').map(|n| n.parse().unwrap_or(0)).collect();
        // Numeric identifiers sort before alphanumeric ones, as in semver.
        let pre = pre.map(|p| p.split('.').map(|s| s.parse().map_or((u64::MAX, s.to_string()), |n| (n, String::new()))).collect());
        (core, pre)
    }
    let ((ca, pa), (cb, pb)) = (key(a), key(b));
    match ca.cmp(&cb) {
        std::cmp::Ordering::Equal => match (pa, pb) {
            (None, Some(_)) => true,
            (Some(pa), Some(pb)) => pa > pb,
            _ => false,
        },
        order => order.is_gt(),
    }
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn versions_order_as_semver() {
        assert!(newer("0.1.0", "0.1.0-beta.2"));
        assert!(newer("0.1.0-beta.10", "0.1.0-beta.2"));
        assert!(newer("0.2.0-beta.1", "0.1.9"));
        assert!(newer("0.1.0-rc.1", "0.1.0-beta.3"));
        assert!(!newer("0.1.0-beta.1", "0.1.0-beta.1"));
        assert!(!newer("0.1.0-beta.1", "0.1.0"));
    }
}
