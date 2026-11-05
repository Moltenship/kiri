# Kiri feature map

Kiri is a lightweight, open-source screenshot tool for macOS and Windows. This page says what Kiri builds, in what order, and what each feature must do. How the alternative does it, with screenshots, is in [the reference](feature-reference/feature-map-reference.md). No stack is chosen yet.

## Product in one paragraph

Press a hotkey, grab part of the screen, and land in a fast editor. Annotate, redact, measure, or recognize text there, then copy, save, pin, or drag the image out. Everything runs locally: no account, no cloud, no telemetry. Most screenshots end within seconds with one keystroke, so the speed of the capture-to-clipboard loop beats feature count.

## Features

**P0** is needed for a usable first release, **P1** is expected by people coming from the alternative, **P2** is nice to have.

| # | Feature | Priority | Reference |
|---|---------|----------|-----------|
| 01 | Area capture | P0 | [01](feature-reference/01-area-capture/feature.md) |
| 02 | Fullscreen and delayed capture | P0 | [02](feature-reference/02-fullscreen-capture/feature.md) |
| 03 | Window capture | P1 | [03](feature-reference/03-window-capture/feature.md) |
| 04 | Scrolling capture | P1 | [04](feature-reference/04-scrolling-capture/feature.md) |
| 05 | Text and QR recognition | P1 | [05](feature-reference/05-text-and-qr-recognition/feature.md) |
| 06 | After-capture preview | P1 | [06](feature-reference/06-after-capture-preview/feature.md) |
| 07 | Editor | P0 | [07](feature-reference/07-editor/feature.md) |
| 08 | Crop and selection | P0 | [08](feature-reference/08-crop-and-selection/feature.md) |
| 09 | Annotations and canvas growth | P0 | [09](feature-reference/09-annotations/feature.md) |
| 10 | Blur and erase | P0 | [10](feature-reference/10-blur-and-erase/feature.md) |
| 11 | Spotlight and magnifier | P1 | [11](feature-reference/11-spotlight-and-magnifier/feature.md) |
| 12 | Ruler | P1 | [12](feature-reference/12-ruler/feature.md) |
| 13 | Color picker | P1 | [13](feature-reference/13-color-picker/feature.md) |
| 14 | Backdrop | P1 | [14](feature-reference/14-backdrop/feature.md) |
| 15 | Compose and GIF | P2 | [15](feature-reference/15-compose-and-gif/feature.md) |
| 16 | Floating pin | P1 | [16](feature-reference/16-floating-pin/feature.md) |
| 17 | Tray and hotkeys | P0 | [17](feature-reference/17-menu-bar-and-hotkeys/feature.md) |
| 18 | Settings | P0 (subset) | [18](feature-reference/18-settings/feature.md) |

## Release slices

1. **MVP:** 01, 02, 07, 08, 09, 10, 17, 18 (the subset P0 features need). Capture, annotate, redact, copy, save.
2. **Parity:** 03, 04, 05, 06, 11, 12, 13, 14, 16.
3. **Extras:** 15, the automation API, and a screenshot library.

## Out of scope

- Cloud upload and share links.
- Licensing, paid tiers, and feature gates. Everything the alternative locks behind a license (custom colors, gradient backdrops, Add Capture) is free in Kiri.
- Telemetry. Kiri collects nothing.
- A splash window.

## Requirements per feature

### 01 Area capture
- Settings → Capture → Selection picks how Capture Area selects. macOS, the default, runs the system grabber, `screencapture -i`, as the alternative does, so the crosshair, coordinates, and keys are the system's. Kiri runs Kiri's own overlay, and options that only it can offer live under that choice, such as Freeze the screen, which selects on a still of every display taken at the hotkey so menus and hover states stay put. Text, scrolling, window, and Add Capture selections always use Kiri's overlay, and freeze only when Kiri is chosen.
- Kiri's overlay copies the system grabber: the system's own crosshair, or its camera when picking a window, read from the OS, with the numbers drawn into the cursor the way the system draws them.
- Show coordinates before dragging and width × height while dragging, in the editor's unit.
- `Esc` cancels, releasing the mouse confirms. Decide what a click without a drag does (the alternative ignores it).
- Repeat area remembers the last rectangle per display layout. The system grabber doesn't report its rectangle, so only selections in Kiri's overlay count.
- On macOS Kiri's overlay behaves like the system grabber: `Space` while dragging moves it, `Shift` locks an axis, `Option` grows it from its center, and `Space` before dragging toggles window picking. Windows gets its own conventions later.
- In Kiri's overlay, arrow keys move the pointer by a point, before or during a drag, for an exact edge.
- Idea: snap to window or UI edges.

### 02 Fullscreen and delayed capture
- Fullscreen captures the display under the cursor. Settings → Capture → All displays in one image lays every display out as arranged, at the sharpest display's scale, with transparent gaps.
- Delay is configurable (3 s default), with a countdown that stays out of the shot. The delay can also start an area capture.

### 03 Window capture
- Picker overlay with hover highlight. Click captures, `Esc` cancels, `Tab` cycles through windows under the cursor.
- Capture a window even when it is partly covered.
- Background modes: wallpaper, transparent, solid color, trim shadow. Transparent output saves as PNG, never JPEG.
- Windows has DWM shadows and, on Windows 11, rounded corners. Define what transparent and trim mean there. Minimized windows can't be captured.

### 04 Scrolling capture
- Auto-scroll down or up. Stop on `Esc`, when nothing moves, or at max height. Keep what was captured so far.
- Manual mode as the fallback for apps that ignore synthetic scrolling: Settings → Scroll by hand. The user scrolls and Kiri stitches, so it needs no Accessibility. When frames stop overlapping, the HUD asks to scroll back and stitching resumes from the last good frame.
- Stitch by finding the overlap between frames. Ignore sticky headers and footers. Don't assume a fixed scroll step: smooth scrolling and inertia make it vary.
- Show progress. A tall result must not freeze the editor.
- Say why it stopped when it is not obvious: max height reached, nothing scrolled (with the Reverse scroll direction hint), or frames that stopped overlapping. Each step waits until the frame settles. A Speed slider sets how far each step scrolls, from 20% to 60% of the area, so frames always overlap.
- macOS needs Accessibility. On Windows, wheel events go to the window under the cursor, and elevated windows ignore input from non-elevated apps, so only Scroll by hand works on them.

### 05 Text and QR recognition
- On-device only. macOS: Vision. Windows: `Windows.Media.Ocr` depends on installed language packs, and QR decoding needs a separate library.
- Barcodes decode first. If one is found, copy its payload, and offer Open for a URL.
- Notification with a preview of the text and actions: remove line breaks, copy again, open.
- Language setting with an automatic option and several languages at once. On macOS the list is whatever Vision on that Mac supports, named in the user's language, so it grows with the OS.
- In-editor OCR doesn't close the editor, or that is a setting.

### 06 After-capture preview
- Show / copy / save after capture can be combined. The choice between editor and thumbnail is a setting.
- The thumbnail stays on top, drags out as a file, has quick actions, and hides after N seconds or on close. Several can stack.
- `Esc` in the editor copies and closes (on by default). `⌘C` in the editor and in the thumbnail also copies and closes; `⇧⌘C` copies and keeps the editor open.
- The editor-or-thumbnail choice applies to every capture, not only area captures.
- Copying shows nothing, as in the alternative. Saving shows "Saved to …" with a click to reveal the file; General → Confirm saving turns it off. Errors always show. No system notifications.

### 07 Editor
- Opens within ~150 ms after capture.
- Pixels stay crisp when zoomed in. The wheel, ⌘- or ⌃-scroll, and a trackpad pinch zoom around the pointer.
- Same tools and single-letter shortcuts as the alternative.
- Objects stay editable until export.
- Copy puts both PNG and a file on the clipboard, so pasting works in chat apps and file managers. The drag handle produces a real file.
- Saved PNGs record their pixels-per-point, so a Retina screenshot reopens at its real size. Saved files carry the macOS screenshot tag, as the system's own do.
- Dropping an image file on the editor adds it as an image object where it lands, like Paste Image.
- Save goes straight to the save folder, the Desktop until one is chosen in settings, where the alternative first asks for a folder. A second Save overwrites the file it wrote. Save As proposes the opened or last saved file's name and folder, and a timestamped name otherwise. General → File name is a visible strftime template, `Kiri %Y-%m-%d at %H.%M.%S` by default, where the alternative hides its `fileNameTemplate`; it names copied and dragged files too.
- Save format: Auto by default, as in the alternative, or PNG. Auto keeps a JPEG (quality 92) only when it is smaller than a compactly compressed PNG. Transparent images stay PNG. Copy always uses the fast PNG.
- Clicking the size opens the pt/px switch and Resize: 25% to 4× presets or exact pixels, with the aspect ratio locked. Resize merges annotations into the image as in the alternative, but unlike there it can be undone.
- While an editor is open, Kiri is a regular app: Dock, `⌘Tab`, and a menu bar like the alternative's (File, Edit, Draw, Zoom) that lists every command with its shortcut, including those with no toolbar button (Reset Crop, Rasterize, Print). Bare-letter shortcuts never fire from the menu while typing. With no editor open it lives only in the tray.

### 08 Crop and selection
- Non-destructive crop with reset.
- The selection resizes by its edges and corners, as in the alternative, with no handles over the content, and arrow keys nudge it. Its size shows under it in the current unit.
- `Q` and `W` center the selection's top-left or bottom-right corner at 1600%, for pixel-exact edges. Without a selection they zoom out and in.
- Autoadjust trims to the nearest uniform-background edges.
- Imprint size creates editable measurement annotations.
- Erase fills from the surrounding pixels.

### 09 Annotations and canvas growth
- All 8 tools with the same shortcuts. `Shift` constrains (45°, square, circle), `Alt` draws from the center. Arrows can curve through a midpoint handle.
- Arrows match the alternative's four styles and proportions, with the hand-drawn one as the default. `⌘` while drawing points the arrow back at the drag start.
- Counter continues from the highest number on the canvas and offers to renumber after a delete.
- Highlighter: flat ends, near-horizontal straight strokes come out level, and dragging its end handle draws on as one stroke, with no darker overlap.
- Text is editable after placing. Label background uses the selected color with automatic text contrast.
- Last color and size are remembered per tool. Kiri's own presets come first, and any custom color is one click further (palette, HSLA, hex).
- Edges snap within 5 pt to the image (or crop) edges and to objects of the same kind, while drawing, moving, and resizing, with a pink guide. `⌘` or `Shift` draws free. Kiri has only this mode, the alternative's default; its "Snap to Objects" mode is left out.
- Objects can be copied, pasted, duplicated with `Option`-drag, nudged, and deleted. ⌘C with a selected object copies the object and keeps the editor open; with nothing selected it copies the image and closes.
- Canvas = bounds(crop ∪ objects) + padding, where the crop is the whole image until cropped, recomputed after every edit and part of undo. Grow live while dragging, for every tool (the alternative clips freehand until release). The view never moves or rescales as the canvas grows, and a drag keeps tracking outside the window.
- Fill for the grown area: the crop's background color (default), transparent, or custom. A mostly transparent edge, as around a window capture, keeps the grown area transparent, as in the alternative. Cropped-away pixels never show through. Cap the canvas size.
- The editor can zoom out past fit, so there is room to draw outside the image. Backdrop wraps the grown canvas.

### 10 Blur and erase
- Blur, blur text, erase, erase text. The text-only modes reuse the OCR engine to find glyph boxes.
- Redaction is irreversible in the exported file: pixelation in cells of random size (as in the alternative, which also defeats depixelation tools that assume a regular grid) or a solid fill, and no original pixels in the file or its metadata.
- Redaction stays editable in the editor and is burned in on export.
- A solid black box as an explicit mode.

### 11 Spotlight and magnifier
- Spotlight: adjustable dim, rectangle or ellipse, optional outline, several spotlights merged into one overlay.
- Magnifier: the drag boxes the source and a 3× lens hangs off it. Source and lens move and resize separately, and the zoom is their size ratio. Round or rounded-square, drawn as clean geometric shapes. Render from the original pixels.

### 12 Ruler
- Hold `1` to measure vertically, `2` horizontally, or an arrow along its axis, as in the alternative. Holding `1` and `2` together measures both, so an element's width and height show at once, and a click pins both.
- Edge detection walks from the cursor until the color drifts slightly from the start pixel, so a faint divider or the first anti-aliased glyph pixel stops it, as in the alternative.
- Live overlay while the key is held. A click pins the measurement as an annotation.
- Units follow the px/pt toggle and the source display's scale.

### 13 Color picker
- Live color under the cursor, `Tab` copies. Format chosen in settings or by clicking.
- Pixel loupe near the cursor at high zoom.
- WCAG contrast between two colors, or the two dominant colors in a selection, with an AA/AAA badge.
- Read colors from the original image. Copy sRGB by default.

### 14 Backdrop
- Same controls as the alternative. Remember the last preset and allow named presets.
- Aspect-ratio presets (16:9, 4:3, 1:1, social formats) pad the canvas to the ratio.
- Stays editable until export. Suggest a gradient from the screenshot's colors.
- Inset adds a margin of the screenshot's own background color; Recenter crops to the content first so the margin is even.
- A gradient is a palette of four stops plus a kind (mesh, vertical, diagonal, radial) that applies to every palette, as in the alternative. Stops can be edited, reversed, or shuffled. Solid takes any custom color.
- Image backdrop: any PNG or JPEG, cropped to cover, with a blur scaled to the backdrop so the preview matches the export. No bundled wallpapers; Wallpaper takes what the desktop under the pointer shows, so dynamic and live wallpapers work too.

### 15 Compose and GIF
- Image objects on the canvas: move, scale, opacity, z-order. The canvas grows the same way it does for annotations.
- Add Capture (Draw menu, or ⋯ at the end of the tools with Paste Image, as in the alternative) takes an area capture and drops it onto the open editor at its real size instead of opening a new one.
- A pasted image's GIF button saves a two-frame GIF to the save folder: the image, then what lies under it, one second each, looping. Kiri's frames follow the Copy output (crop, backdrop, downscale to 1×).

### 16 Floating pin
- Always on top, borderless, with a shadow. Drag to move, scroll or pinch to scale.
- Opacity and click-through, so a pin can be used as an overlay. Holding ⌥ makes click-through pins take the mouse again, to move one or turn click-through off from its menu.
- Several pins at once. Pins don't survive a restart unless that becomes a setting.
- Decide whether pins float over full-screen apps on macOS and whether they show in Alt+Tab on Windows.

### 17 Tray and hotkeys
- Tray icon on both platforms with the same menu. Kiri works with no window open. Hiding the icon, as the alternative allows, waits for the app bundle: relaunching Kiri is then the only way back to Settings, and that needs a Dock or Launchpad icon.
- The first ⌘Q in a Kiri window asks whether to quit (hotkeys stop) or just close the window, and remembers the answer, as the alternative does. Settings → General → ⌘Q changes it later.
- Every capture action can have a global hotkey, with a warning when the OS or another app owns the shortcut. A hotkey that an enabled macOS shortcut also uses gets a note in its Settings row naming that shortcut and its Keyboard Shortcuts section, with a button to the Keyboard Shortcuts sheet. System Settings has no link to a section inside that sheet. Kiri reads the system defaults from Keyboard settings' own `DefaultShortcutsTable.xml` and the user's changes from `com.apple.symbolichotkeys`, since a shortcut left at its default isn't in the latter.
- A first-run window, in place of the alternative's splash: Screen Recording (macOS only; Windows asks no permission to capture), then the capture shortcuts, then appearance and launch at login. It shows while no settings file exists, so updating from an older Kiri skips it.
- Kiri defaults to ⇧⌘1/⇧⌘2, next to the system keys. Taking ⇧⌘3/⇧⌘4 instead, from the first-run window or Settings → Hotkeys (the alternative's Reassign Default), turns those two off in macOS's symbolic hotkeys and leaves ⇧⌘5 alone. On Windows the choice is Ctrl+Shift+1/Ctrl+Shift+2 next to Snipping Tool, or PrtScn/Shift+PrtScn instead of it, which turns off "Use the Print Screen key to open screen capture"; Windows reserves `Win+Shift+S`.
- One Kiri per user: launching it again opens Settings in the running one, or the file passed with `--open`, so there is never a second tray icon.
- `kiri --capture <action>` (`area`, `fullscreen`, `active-window`, …) runs a capture in the running Kiri, for scripts, Shortcuts, and launchers. `--then edit,thumbnail,copy,save` replaces the after-capture settings for that one capture, like the alternative's `then=`. Kiri holds the screen recording permission, so this is off until Settings → Advanced → Automation allows it, and a request while off shows a toast with Allow, like the alternative's deep-link prompt. A URL scheme waits for the app bundle.
- Launch at login, in Settings and as a checkmark in the tray menu, as in the alternative.
- Kiri installs from a notarized DMG: drag Kiri to Applications, with the window laid out around one arrow.
- Updates come from GitHub releases. A daily check, on by default in Settings → General, only looks: a newer release puts a red dot on the menu bar icon and an Update to Kiri item in its menu. Choosing it downloads the notarized `Kiri.dmg`, copies the app out of it, swaps it in, and relaunches. Only a build signed with Kiri's Developer ID may replace the app, and the stable signature keeps the Screen Recording permission across updates.
- Reopen Last Screenshot (tray, optional hotkey) brings back the last closed editor with its annotations still editable, crop, and backdrop, but not its undo history. Where the alternative keeps its window hidden, Kiri keeps the image as a PNG in memory, so the resident app stays small.

### 18 Settings
- One settings window: General, Capture, Editor, Hotkeys, Advanced. Short inline help for the non-obvious settings.
- Settings live in a plain config file that can be imported and exported.
- Expose the screenshot library in the UI. Cursor capture is left out until a capture path can include the cursor.
- Appearance: System, Light, or Dark, picked from small drawings of a Kiri window. It sets the app's appearance, so blurred window backgrounds follow it too.
- A choice that changes how something looks is picked from small drawings: Appearance, Show in (editor or thumbnail), and Window background. Numbers without natural steps, such as max scroll height, are typed in.
- Dropped from the alternative: uploading, license, splash, diagnostics.
- Consider WebP as a save format.

## Cross-platform notes

- **Windows today.** Tray, global hotkeys, area, fullscreen, window and active-window capture, text recognition, scrolling capture, the editor, copy, save, launch at login, and the first-run window run on Windows. Updates and printing don't yet; their actions say "Not available on Windows yet". GPUI quits with the last window off macOS, so Kiri sets an explicit quit mode.
- **Platform seam.** [`src/win.rs`](../src/win.rs) implements the functions of [`src/mac.rs`](../src/mac.rs) under the same names and is imported as `mac` on Windows, so shared code has one call per platform need and no trait. Window capture is its own module per platform; OCR keeps its copying and toasts in the shared [`src/ocr.rs`](../src/ocr.rs) and takes the engine from [`src/win/ocr.rs`](../src/win/ocr.rs) on Windows. Scrolling capture is shared in [`src/scrolling.rs`](../src/scrolling.rs), with the pointer, the wheel, and frame grabs from a `sys` module per platform: inside that file on macOS, [`src/win/scrolling.rs`](../src/win/scrolling.rs) on Windows.
- **Text recognition on Windows.** `Windows.Media.Ocr` reads in the language picked in Settings or else the user's Windows languages, and Settings lists the installed ones. It misses text that touches the image edge, so the image gets a margin first. It reads no QR codes.
- **Scrolling capture on Windows.** `SendInput` sends wheel events, and frames come from GDI like area capture. Windows has no pixel-unit wheel events, so each step's points go out as as many wheel units: a notch is 120 units and scrolls about 100 points in browsers and less in most other apps, which only narrows the step. Windows draws a shadow outside a window, so the HUD turns its compositor shadow off; it would otherwise reach into the frames above it.
- **Launch at login on Windows.** A value in the user's `Run` registry key, named like the settings folder.
- **Permissions.** macOS needs Screen Recording for any capture and Accessibility for scrolling capture. Windows has no prompts, but elevated windows don't accept input from a non-elevated app.
- **HiDPI.** Sizes show in points with a px toggle. Kiri turns on per-monitor DPI awareness (v2) before its first window, so captures come in physical pixels and Windows never rescales its windows. GPUI's logical pixels play the part of macOS points. With mixed DPI, each monitor's logical origin is its physical origin divided by its own scale, as GPUI places windows, so logical coordinates aren't one continuous space across monitors; convert per monitor.
- **Capture on Windows.** GDI copies the composed desktop for area and screen capture: no prompt and no capture border, but HDR displays come out clipped to SDR and DRM content shows black. Window capture uses Windows.Graphics.Capture, which reads a covered window as the compositor has it, without the capture border. Windows draws window shadows outside the window where capture can't reach, so Transparent and Trim shadow give the same picture, and Wallpaper and Solid color only fill the rounded corners. The wallpaper is laid out from the Windows wallpaper setting; Span is laid out per display. Windows has no system grabber, so Area always uses Kiri's overlay.
- **Shortcuts.** In-app bindings are written with `cmd`, which means Ctrl on Windows; Ctrl+Y redoes as well as Ctrl+Shift+Z, Ctrl+Shift+W closes every editor, and nothing quits on Ctrl+Q, since a Windows app quits from its tray. Global hotkeys default to Ctrl+Shift+2 for Area, Ctrl+Shift+1 for Screen, and Ctrl+Shift+3 for Text, since Win+Shift+digit belongs to the taskbar, Win+Shift+S and PrtScn to Snipping Tool, and Ctrl+Alt is AltGr on many layouts. Every key Kiri shows on Windows is written as Windows writes it ("Win+Ctrl+Shift+2", "PrtScn"): the recorder, the tray menu, tooltips, and toasts. Settings warns when a hotkey lands on a Windows shortcut: PrtScn while Snipping Tool has it, Win+Shift+S, the Win+digit taskbar keys, and the rest of Windows' Win+letter keys. Use Print Screen in Settings → Hotkeys turns off Snipping Tool on PrtScn (`PrintScreenKeyForSnippingEnabled`, the switch in Settings › Accessibility › Keyboard) and gives PrtScn to Area and Shift+PrtScn to Screen. GPUI reports no key press for PrtScn, so the recorder can't record it; only that button sets it. Text annotations follow Windows keys: Ctrl moves and deletes by word, Home and End go to the line's ends.
- **Settings on Windows.** Rows that do nothing there are hidden rather than disabled: Selection (Windows has no system grabber, so Freeze the screen is always available) and the ⌘Q choice.
- **Clipboard.** Copy puts a PNG, a bitmap (`CF_DIB`) for apps that read only that, and the saved file on the clipboard, matching macOS's PNG plus file URL.
- **Window chrome.** The editor and Settings draw Windows' minimize, maximize, and close at the right of their title area and mark them, with the drag area, as GPUI window control areas, so Windows runs the clicks, snap layouts, and double-click to maximize. The editor opens wider on Windows to fit the buttons and keep room to drag. GPUI's `start_window_move` does nothing on Windows, and a drag area under a pin would flicker its hover toolbar, since Windows reports the cursor leaving the window as it crosses between drag and client areas; so a pin starts Windows' own window move when pressed. macOS shadows and rounded corners don't exist in the same form on Windows 10/11.
