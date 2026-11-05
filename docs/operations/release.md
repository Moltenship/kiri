# Releasing Kiri for macOS

`scripts/release.sh` builds `dist/Kiri.app`, signs it with the Developer ID, notarizes and staples it, and packs it in `dist/Kiri.dmg`, which is notarized too. With `--publish` it also creates the GitHub release `v<version>` at HEAD with the DMG, and that version's CHANGELOG section as the notes. Every release, a beta included, is a full release marked Latest: the site's Download button links to `releases/latest/download/Kiri.dmg`, which skips pre-releases.

## Once per machine

- The `Developer ID Application: Timofei Maksimov (Q9SDRFKBC6)` identity, with its private key, in the login keychain. Only the Account Holder can create one, at developer.apple.com → Certificates. Export it as a `.p12` from Keychain Access to move it to another Mac. `KIRI_IDENTITY` overrides the name.
- An App Store Connect API key of the same team, for `notarytool`, in `ASC_KEY_ID`, `ASC_ISSUER_ID`, and `ASC_KEY_PATH`.
- `gh` signed in with push access to the repository.
- `python3`. The first run installs [dmgbuild](https://github.com/dmgbuild/dmgbuild), which lays out the DMG window, into `~/.cache/kiri-dmg`.

## Each release

1. Set `version` in `Cargo.toml`, for example `0.1.0-beta.2`.
2. Add a `## [0.1.0-beta.2] - YYYY-MM-DD` section at the top of `CHANGELOG.md`.
3. Commit and push.
4. Run `scripts/release.sh --publish`.

Running Kiris see the release within a day, or right away from Check for Updates… in the tray menu.

## Things that break updates

- **The bundle identifier or the signing team.** The updater only accepts a download whose signature satisfies `REQUIREMENT` in [`src/update.rs`](../../src/update.rs), so `sh.kiri.Kiri` and team `Q9SDRFKBC6` must change there first, and the change has to ship before builds signed the new way. macOS also keys the Screen Recording and Accessibility permissions to the same signature, so users grant them again after such a change.
- **The asset name.** The updater downloads `Kiri.dmg`, so it must keep that name, with `Kiri.app` at the top level of the volume. Builds up to 0.1.0-beta.5 look for `Kiri.zip` instead and don't see later releases.
