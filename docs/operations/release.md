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

## Releasing by hand

For a quick release from your own machines, skip the workflow and publish as before:

1. Bump the version and the CHANGELOG as above, and put `[skip ci]` in the commit message, so the push doesn't start the workflow too. Without it both race to create the release, and the loser fails.
2. Push to `main`: GitHub only tags commits it has.
3. On a Mac, run `scripts/release.sh --publish`. It builds, signs, and notarizes `dist/Kiri.dmg`, then creates release `v<version>` at HEAD with it.
4. Right after, on Windows at the same commit, run `scripts\release-windows.ps1 -Upload`. It builds `dist\Kiri-Setup.exe` and `dist\Kiri-windows-x64.zip` and adds both to the release. Until the zip is there, Windows Kiris that check report that the release has none, and the site's Download for Windows links the latest release's setup.

Without `--publish` or `-Upload` the scripts only build.

`release.sh` needs the identity in the login keychain, the key in `ASC_KEY_ID`, `ASC_ISSUER_ID`, and `ASC_KEY_PATH`, `gh` signed in with push access, and `python3`. Its first run installs [dmgbuild](https://github.com/dmgbuild/dmgbuild), which lays out the DMG window, into `~/.cache/kiri-dmg`. `KIRI_IDENTITY` overrides the identity's name.

`release-windows.ps1` needs the Rust toolchain, `gh` signed in, and Inno Setup 6, which builds the setup from [`windows/Kiri.iss`](../../windows/Kiri.iss). Install it for the current user, with no admin prompt:

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
