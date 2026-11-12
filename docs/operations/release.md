# Releasing Kiri

A push to `main` whose `Cargo.toml` version has no GitHub release yet releases it: [`.github/workflows/release.yml`](../../.github/workflows/release.yml) builds the macOS, Windows, and Linux assets, then creates release `v<version>` at that commit with all of them, and that version's CHANGELOG section as the notes. Nothing is published unless every build passes, so Windows and Linux Kiris never see a release without their archive. A push that keeps the version does nothing. To retry a failed release, fix it and push, or run the workflow from the Actions tab.

Versions are stable, with no `-beta` suffix, and each release is published as Latest: the updater and the site's Download button both read GitHub's latest release, which skips pre-releases.

## Each release

1. Set `version` in `Cargo.toml`, for example `0.1.1`.
2. Add a `## [0.1.1] - YYYY-MM-DD` section at the top of `CHANGELOG.md`. Without it the run fails before building.
3. Commit and push to `main`.

Running Kiris see the release within a day, or right away from Check for Updates… in the tray menu.

## Repository secrets

The macOS build signs with the `Developer ID Application: Timofei Maksimov (Q9SDRFKBC6)` identity and notarizes with an App Store Connect API key of the same team. Only the Account Holder can create the identity, at developer.apple.com → Certificates.

- `MACOS_CERTIFICATE_P12`: the identity with its private key, exported as a `.p12` from Keychain Access → My Certificates, base64-encoded: `base64 -i Kiri.p12 | gh secret set MACOS_CERTIFICATE_P12`.
- `MACOS_CERTIFICATE_PASSWORD`: the password set on that export.
- `ASC_KEY_ID`, `ASC_ISSUER_ID`, and `ASC_KEY_P8`, the contents of the `.p8` file: `gh secret set ASC_KEY_P8 < AuthKey_<id>.p8`.

## Releasing by hand

For a quick release from your own machines, skip the workflow and publish as before:

1. Bump the version and the CHANGELOG as above, and put `[skip ci]` in the commit message, so the push doesn't start the workflow too. Without it both race to create the release, and the loser fails.
2. Push to `main`: GitHub only tags commits it has.
3. On a Mac, run `scripts/release.sh --publish`. It builds, signs, and notarizes `dist/Kiri.dmg`, then creates release `v<version>` at HEAD with it.
4. Right after, on Windows at the same commit, run `scripts\release-windows.ps1 -Upload`. It builds `dist\Kiri-Setup.exe` and `dist\Kiri-windows-x64.zip` and adds both to the release. Until the zip is there, Windows Kiris that check report that the release has none, and the site's Download for Windows links the latest release's setup.
5. On the Mac, run `scripts/release-linux.sh --upload`. It builds `dist/Kiri-linux-x64.tar.gz` and adds it to the release. Until it is there, Linux Kiris that check report that the release has none.

Without `--publish`, `-Upload`, or `--upload` the scripts only build.

`release.sh` needs the identity in the login keychain, the key in `ASC_KEY_ID`, `ASC_ISSUER_ID`, and `ASC_KEY_PATH`, `gh` signed in with push access, and `python3`. Its first run installs [dmgbuild](https://github.com/dmgbuild/dmgbuild), which lays out the DMG window, into `~/.cache/kiri-dmg`. `KIRI_IDENTITY` overrides the identity's name.

`release-windows.ps1` needs the Rust toolchain, `gh` signed in, and Inno Setup 6, which builds the setup from [`windows/Kiri.iss`](../../windows/Kiri.iss). Install it for the current user, with no admin prompt:

```powershell
winget install --id JRSoftware.InnoSetup -e --scope user --override "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /CURRENTUSER"
```

`release-linux.sh` needs Docker on a Mac (OrbStack or Docker Desktop) and `gh` signed in. Off Linux it runs itself in an x86_64 `ubuntu:22.04` container, the image the `linux` job uses too, and installs the build's packages and Rust there; `gh` uploads from the Mac afterwards, so no token enters the container. The toolchain, the crates, and the build stay in the `kiri-linux-x64` Docker volume, not in `target/`. On Apple Silicon the container is emulated, which makes the build slower than a native one: on an M-series Mac with OrbStack a clean release build took about three minutes once the volume held Rust and the crates, and the first run adds a few more minutes to install them. On an x86_64 Ubuntu machine the script builds natively: pass `--setup` once to install the packages and Rust (as root), or install them yourself from `PACKAGES` in the script.

## Linux

The asset must be built on Ubuntu 22.04, as the script and the job do. A binary needs the glibc it was built against or older, so one built on a newer distro, a 24.04 runner included, won't start on 22.04, and one built on 26.04 not on 24.04 either. Its shared libraries are libc, libm, libgcc_s, libxcb, libxkbcommon, and libxkbcommon-x11; Wayland and the GPU drivers load at run time. [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) builds, tests, and packages Linux on every pull request, since no one builds it by hand otherwise.

To check a change on Linux from a Mac, build and test it in the same container, which builds the release first: `docker run --rm --platform linux/amd64 -v "$PWD:/src" -w /src -v kiri-linux-x64:/cache -e CARGO_HOME=/cache/cargo -e RUSTUP_HOME=/cache/rustup -e CARGO_TARGET_DIR=/cache/target ubuntu:22.04 sh -c 'scripts/release-linux.sh --setup && /cache/cargo/bin/cargo test'`.

## Windows

Neither Windows asset is code-signed, so SmartScreen warns on its first run from a browser download.

Never change the setup's `AppId`: later setups find the install by it and upgrade it in place, and [`src/win/update.rs`](../../src/win/update.rs) reads the Settings → Apps entry under it.

## Things that break updates

- **The bundle identifier or the signing team.** The updater only accepts a download whose signature satisfies `REQUIREMENT` in [`src/update.rs`](../../src/update.rs), so `sh.kiri.Kiri` and team `Q9SDRFKBC6` must change there first, and the change has to ship before builds signed the new way. macOS also keys the Screen Recording and Accessibility permissions to the same signature, so users grant them again after such a change.
- **The asset name.** The updater downloads `Kiri.dmg`, so it must keep that name, with `Kiri.app` at the top level of the volume. On Windows it downloads `Kiri-windows-x64.zip` and takes `Kiri\Kiri.exe` from it. Builds up to 0.1.0-beta.5 look for `Kiri.zip` instead and don't see later releases. On Linux it downloads `Kiri-linux-x64.tar.gz` and takes `Kiri/kiri` from it.
- **The Linux desktop entry id.** Kiri writes `sh.kiri.Kiri.desktop` and names its windows and portal requests by `sh.kiri.Kiri`. Renaming it leaves the old entry in users' app menus, and desktops may ask again for screenshot and shortcut permissions.
- **Old betas.** Builds up to 0.1.0-beta.11 pick the newest of the first ten releases in GitHub's list, which sorts by commit day and then by tag name as text. Ten or more releases on the newest day could push the actual newest one out of their view.
