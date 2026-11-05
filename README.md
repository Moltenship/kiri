# Kiri

A fast, open-source screenshot tool for macOS. Capture an area, a window, or a whole page that scrolls, then annotate, measure, redact, or pin it, all without leaving the keyboard.

Kiri is built in Rust and draws on the GPU. The capture-to-clipboard loop is what it optimizes for: most screenshots should be done within a couple of seconds and a single keystroke.

> Kiri is in beta. Expect rough edges, and please [open an issue](https://github.com/Moltenship/kiri/issues) when you hit one.

## Install

1. Download `Kiri.zip` from the [latest release](https://github.com/Moltenship/kiri/releases).
2. Unzip it and move `Kiri.app` to Applications.
3. Open it. Kiri lives in the menu bar.
4. Give it Screen Recording permission when macOS asks, in System Settings → Privacy & Security. Scrolling capture also asks for Accessibility.

Kiri needs macOS 15.2 or later on Apple Silicon. It updates itself from GitHub releases. Settings → General turns that off.

## What it does

- **Capture:** an area, a screen, a window, a delayed shot, or a scrolling page. The area selection can freeze the screen, and window captures can sit on your wallpaper or on transparency.
- **Edit:** arrows, text, shapes, pen, highlighter, counters, blur, spotlight, and magnifier. Crop to the pixel, trim to the content, and add a backdrop.
- **Measure:** hold `1` or `2` to measure between edges, and pick any color on screen.
- **Recognize:** copy the text or QR code from any area, on device.
- **Pin:** float a screenshot above every window, see-through and click-through if you want.
- **Automate:** run `kiri --capture area --then copy,save` from scripts, Shortcuts, or a launcher.

The [user guide](docs/user/README.md) covers every hotkey and setting.

## Privacy

Kiri has no account, no cloud, and no telemetry. The only network request it makes is the daily update check against GitHub.

## Build from source

You need a recent stable Rust and Xcode.

```sh
cargo build --release
./target/release/kiri
```

A bare binary runs fine for development. `scripts/release.sh` builds the signed and notarized app, as [docs/operations/release.md](docs/operations/release.md) describes.

## Contributing

[docs/feature-map.md](docs/feature-map.md) lists what Kiri builds and in what order.

## License

[MIT](LICENSE)
