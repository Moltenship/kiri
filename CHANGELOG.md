# Changelog

All notable changes to Kiri. Versions follow [Semantic Versioning](https://semver.org).

## [0.1.0-beta.3] - 2026-11-05

### Changed
- Kiri no longer downloads updates on its own. A red dot on the menu bar icon says a new version is out, and the Update to Kiri item in its menu installs it.

### Added
- Settings → Hotkeys marks a shortcut that macOS also uses, names the macOS shortcut and its Keyboard Shortcuts section, and opens Keyboard Shortcuts.

## [0.1.0-beta.2] - 2026-11-05

### Added
- A welcome window on first launch: Screen Recording, the capture shortcuts, and the look.
- Kiri can take ⇧⌘3 and ⇧⌘4 from macOS in one click, from the welcome window or Settings → Hotkeys.
- Light and Dark appearances, besides following the system, in Settings → General.

### Changed
- A new app icon and menu bar icon.

### Fixed
- Toasts, thumbnails, pins, and the scrolling HUD no longer sit on a square background.

## [0.1.0-beta.1] - 2026-11-05

The first public beta, for macOS 15.2 or later on Apple Silicon.

### Capture
- Area, screen, window, and delayed captures, with the system's own crosshair and window cursors. Arrow keys nudge the pointer one point at a time.
- An optional freeze of the screen while selecting, so menus and hover states stay put.
- Capture Screen can take every display in one image.
- Window captures with wallpaper, transparency, a solid color, or nothing behind the window.
- Scrolling capture, automatic or by hand.
- Text and QR recognition, on device.
- Repeat Area Capture and Reopen Last Screenshot.

### Editor
- Arrow, text, rectangle, oval, line, pen, highlighter, counter, blur and erase, spotlight, and magnifier.
- Annotations past the edge grow the canvas.
- Crop, trim to content, and zoom to pixel-exact edges.
- Backdrops.
- The ruler measures between edges, and holding `1` and `2` together measures width and height at once.
- A color picker for colors and text colors.
- Copy, save, and Esc to close return at once, and encoding runs in the background.

### Pins and more
- Pins float above every window, with opacity and click-through. Hold ⌥ to reach a click-through pin again.
- `kiri --capture <action> --then <list>` runs captures from scripts, once allowed.
- Hotkeys for every action, launch at login, and a file name template.
- Settings can be imported and exported.
- Kiri updates itself from GitHub releases.
