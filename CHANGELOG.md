# Changelog

All notable changes to Kiri. Versions follow [Semantic Versioning](https://semver.org).

## [Unreleased]

### Added
- Windows: capture a window or the active window, even when it's covered.
- Windows: Capture Text, Recognize Text, and Blur Text read text with Windows' own text recognition, in the languages installed in Windows.
- Windows: Capture Text and Recognize Text read QR codes.
- Windows: Launch at Login.
- Windows: Print, on one page, landscape when the image is wide.
- Windows: Kiri.exe has Kiri's icon, in Explorer, the taskbar, and Alt+Tab.
- Windows: `Kiri-Setup.exe` installs Kiri for the current user with no admin prompt, with a Start menu shortcut and an entry in Settings → Apps to uninstall it. A newer setup upgrades Kiri in place, closing it first.
- Windows: updates from GitHub releases, which also carry Kiri for Windows as a portable `Kiri-windows-x64.zip`; a red dot on the tray icon tells of one.
- `kiri --quit` quits the running Kiri.
- Windows: Scrolling Capture, down or up, by itself or by hand.
- Windows: area capture shows the pointer's position, and the selection's size while dragging, beside the pointer.
- Windows: drag the image out of the editor, a pin, or the thumbnail into another app.
- Windows: the editor's Menu button lists every command with its shortcut, as the macOS menu bar does, and F11 toggles full screen.
- Windows: the first-run window, with Ctrl+Shift+1 and Ctrl+Shift+2 next to Snipping Tool or Print Screen in its place.
- Windows: Settings warns when a hotkey is one of Windows' own, and Use Print Screen gives PrtScn to Capture Area in place of Snipping Tool.

### Changed
- Windows: the editor and Settings have minimize, maximize, and close buttons, and drag by their title area. Pins drag too.
- Windows: in text annotations, Ctrl moves and deletes by word, and Home and End go to the start and end of the line.
- Windows: keys read as Windows writes them (Ctrl+Shift+2, Win, PrtScn) everywhere, Capture Text defaults to Ctrl+Shift+3, Ctrl+Y redoes, and Ctrl+Shift+W closes all editors.
- Windows: Settings hides the macOS-only Selection and ⌘Q rows, and Freeze the screen is always available.

### Fixed
- Windows: Alt, not the Win key, points a new arrow back at where you started, and Ctrl draws an arrow from its center. Ctrl takes the Win key's place in other editor drags too.
- Windows: a hotkey records the key you press whatever the keyboard layout, so Alt+Shift+4 on a Russian layout is Alt+Shift+4, not Alt+;.

## [0.1.0] - 2026-11-05

The first stable release.

### Fixed
- Updates find the newest release, however many come out in one day.

## [0.1.0-beta.11] - 2026-11-05

### Added
- A Speed slider for scrolling capture, in Settings → Capture. Faster scrolls further between frames.

### Changed
- Window background and Show in are picked from small pictures, as Appearance is.
- Max height for scrolling capture is a number you type.

## [0.1.0-beta.10] - 2026-11-05

### Changed
- Style variants in the editor are icons, as in Shottr. Hover one to see its name. Blur and erase modes keep their names.

## [0.1.0-beta.9] - 2026-11-05

### Changed
- Updating shows the download percentage, then checking and installing, until Kiri restarts. The menu bar item reads Updating to Kiri while it runs. Updates from this version on show it.

### Fixed
- Pixelate text covers each line of text and leaves the gaps between lines alone, as in Shottr.

## [0.1.0-beta.8] - 2026-11-05

### Added
- Backdrop → Image → Wallpaper puts the screenshot on your current desktop picture.

### Changed
- Capture Area selects the same way as the macOS ⇧⌘4. Settings → Capture → Selection → Kiri brings back Kiri's own selection, with arrow keys, Repeat Area Capture, and Freeze the screen. If Freeze the screen was on, Kiri's selection stays.

## [0.1.0-beta.7] - 2026-11-05

### Fixed
- Capture Area coordinates move with the pointer instead of trailing it, and look like the ones macOS shows for ⇧⌘4.

## [0.1.0-beta.6] - 2026-11-05

### Changed
- Kiri comes as a DMG: open it and drag Kiri to Applications.
- Updates download the DMG. Kiri 0.1.0-beta.5 and earlier don't see this release, so install it from the DMG once.

## [0.1.0-beta.5] - 2026-11-05

### Fixed
- Capture Area no longer flashes a gray screen when it opens.

## [0.1.0-beta.4] - 2026-11-05

### Fixed
- Capture Area no longer makes the screen blink: the app in front stays active while you select.
- Capture Area always shows the system crosshair, not the arrow.
- Recording a shortcut in Settings → Hotkeys no longer starts a macOS or Kiri capture, so ⇧⌘4 or ⇧⌘5 can be recorded.

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
