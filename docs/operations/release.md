# Releasing Kiri

A push to `main` whose `Cargo.toml` version has no GitHub release yet releases it: [`.github/workflows/release.yml`](../../.github/workflows/release.yml) builds the macOS and Windows assets, then creates release `v<version>` at that commit with all of them, and that version's CHANGELOG section as the notes. Nothing is published unless both builds pass, so Windows Kiris never see a release without their zip. A push that keeps the version does nothing. To retry a failed release, fix it and push, or run the workflow from the Actions tab.

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

## Building locally

`scripts/release.sh` builds the same signed, notarized `dist/Kiri.dmg` on a Mac with the identity in the login keychain, the key in `ASC_KEY_ID`, `ASC_ISSUER_ID`, and `ASC_KEY_PATH`, and `python3`. The first run installs [dmgbuild](https://github.com/dmgbuild/dmgbuild), which lays out the DMG window, into `~/.cache/kiri-dmg`. `KIRI_IDENTITY` overrides the identity's name.

On Windows, `scripts\release-windows.ps1` builds `dist\Kiri-Setup.exe`, the installer from [`windows/Kiri.iss`](../../windows/Kiri.iss), which users download, and `dist\Kiri-windows-x64.zip`, with `Kiri\Kiri.exe` inside, which the updater downloads and which also serves as a portable copy. It needs the Rust toolchain and Inno Setup 6. Install that for the current user, with no admin prompt:

```powershell
winget install --id JRSoftware.InnoSetup -e --scope user --override "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /CURRENTUSER"
```

## Windows

Neither Windows asset is code-signed, so SmartScreen warns on its first run from a browser download.

Never change the setup's `AppId`: later setups find the install by it and upgrade it in place, and [`src/win/update.rs`](../../src/win/update.rs) reads the Settings → Apps entry under it.

## Things that break updates

- **The bundle identifier or the signing team.** The updater only accepts a download whose signature satisfies `REQUIREMENT` in [`src/update.rs`](../../src/update.rs), so `sh.kiri.Kiri` and team `Q9SDRFKBC6` must change there first, and the change has to ship before builds signed the new way. macOS also keys the Screen Recording and Accessibility permissions to the same signature, so users grant them again after such a change.
- **The asset name.** The updater downloads `Kiri.dmg`, so it must keep that name, with `Kiri.app` at the top level of the volume. On Windows it downloads `Kiri-windows-x64.zip` and takes `Kiri\Kiri.exe` from it. Builds up to 0.1.0-beta.5 look for `Kiri.zip` instead and don't see later releases.
- **Old betas.** Builds up to 0.1.0-beta.11 pick the newest of the first ten releases in GitHub's list, which sorts by commit day and then by tag name as text. Ten or more releases on the newest day could push the actual newest one out of their view.
