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
- A built-in updater. Kiri uses each platform's normal update channel.
- A splash window.

## Requirements per feature

### 01 Area capture
- Freeze all displays at the moment of capture. The selection can span monitors.
- Show coordinates before dragging and width × height while dragging, in the editor's unit.
- `Esc` cancels, releasing the mouse confirms. Decide what a click without a drag does (the alternative ignores it).
- Repeat area remembers the last rectangle per display layout.
- Ideas: arrow keys nudge the selection by 1 px, `Shift`+arrows by 10 px, hold `Space` to move it. Snap to window or UI edges.

### 02 Fullscreen and delayed capture
- Fullscreen captures the display under the cursor. "All displays stitched" is an option.
- Delay is configurable (3 s default), with a countdown that stays out of the shot. The delay can also start an area capture.

### 03 Window capture
- Picker overlay with hover highlight. Click captures, `Esc` cancels, `Tab` cycles through windows under the cursor.
- Capture a window even when it is partly covered.
- Background modes: wallpaper, transparent, solid color, trim shadow. Transparent output saves as PNG, never JPEG.
- Windows has DWM shadows and, on Windows 11, rounded corners. Define what transparent and trim mean there. Minimized windows can't be captured.

### 04 Scrolling capture
- Auto-scroll down or up. Stop on `Esc`, when nothing moves, or at max height. Keep what was captured so far.
- Manual mode as the fallback for apps that ignore synthetic scrolling.
- Stitch by finding the overlap between frames. Ignore sticky headers and footers. Don't assume a fixed scroll step: smooth scrolling and inertia make it vary.
- Show progress. A tall result must not freeze the editor.
- macOS needs Accessibility. On Windows, wheel events go to the window under the cursor, and elevated windows ignore input from non-elevated apps.

### 05 Text and QR recognition
- On-device only. macOS: Vision. Windows: `Windows.Media.Ocr` depends on installed language packs, and QR decoding needs a separate library.
- Barcodes decode first. If one is found, copy its payload, and offer Open for a URL.
- Notification with a preview of the text and actions: remove line breaks, copy again, open.
- Language setting with an automatic option and several languages at once.
- In-editor OCR doesn't close the editor, or that is a setting.

### 06 After-capture preview
- Show / copy / save after capture can be combined. The choice between editor and thumbnail is a setting.
- The thumbnail stays on top, drags out as a file, has quick actions, and hides after N seconds or on close. Several can stack.
- `Esc` in the editor copies and closes (on by default).
- Own toasts: "Copied", and "Saved to …" with a click to reveal the file.

### 07 Editor
- Opens within ~150 ms after capture.
- Same tools and single-letter shortcuts as the alternative.
- Objects stay editable until export.
- Copy puts both PNG and a file on the clipboard, so pasting works in chat apps and file managers. The drag handle produces a real file.

### 08 Crop and selection
- Non-destructive crop with reset.
- Handles and arrow-key nudging on the selection. Size shown in the current unit.
- Autoadjust trims to the nearest uniform-background edges.
- Imprint size creates editable measurement annotations.
- Erase fills from the surrounding pixels.

### 09 Annotations and canvas growth
- All 8 tools with the same shortcuts. `Shift` constrains (45°, square, circle), `Alt` draws from the center. Arrows can curve through a midpoint handle.
- Counter continues from the highest number on the canvas and offers to renumber after a delete.
- Text is editable after placing. Label background uses the selected color with automatic text contrast.
- Last color and size are remembered per tool. Objects can be copied, pasted, and deleted.
- Canvas = bounds(image ∪ objects) + padding, recomputed after every edit and part of undo. Grow live while dragging: the alternative clips the stroke until release.
- Fill for the grown area: auto background color (default), transparent, or custom. Cap the canvas size.
- The editor can zoom out past fit, so there is room to draw outside the image. Backdrop wraps the grown canvas.

### 10 Blur and erase
- Blur, blur text, erase, erase text. The text-only modes reuse the OCR engine to find glyph boxes.
- Redaction is irreversible in the exported file: strong pixelation with noise or a solid fill, and no original pixels in the file or its metadata.
- Redaction stays editable in the editor and is burned in on export.
- A solid black box as an explicit mode.

### 11 Spotlight and magnifier
- Spotlight: adjustable dim, rectangle or ellipse, optional outline, several spotlights merged into one overlay.
- Magnifier: source and lens move separately, adjustable zoom factor, round or rounded-square. Render from the original pixels.

### 12 Ruler
- Edge detection walks from the cursor until the color changes past a threshold, with a tolerance for anti-aliasing, gradients, and noise.
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

### 15 Compose and GIF
- Image objects on the canvas: move, scale, opacity, z-order. The canvas grows the same way it does for annotations.
- Before/after GIF export with a frame delay. Consider animated WebP or APNG.

### 16 Floating pin
- Always on top, borderless, with a shadow. Drag to move, scroll or pinch to scale.
- Opacity and click-through, so a pin can be used as an overlay.
- Several pins at once. Pins don't survive a restart unless that becomes a setting.
- Decide whether pins float over full-screen apps on macOS and whether they show in Alt+Tab on Windows.

### 17 Tray and hotkeys
- Tray icon on both platforms with the same menu. Kiri works with no window open.
- Every capture action can have a global hotkey, with a warning when the OS or another app owns the shortcut.
- First-run guidance for freeing the system screenshot keys: ⇧⌘3/⇧⌘4 on macOS, `PrintScreen` on Windows ("Use Print Screen to open screen capture" must be off). Windows reserves `Win+Shift+S`.
- CLI and/or URL scheme for automation, off by default, with a confirmation prompt.
- Launch at login.

### 18 Settings
- One settings window: General, Capture, Editor, Hotkeys, Advanced. Short inline help for the non-obvious settings.
- Settings live in a plain config file that can be imported and exported.
- Expose cursor capture (auto / include / exclude) and the screenshot library in the UI.
- Dropped from the alternative: uploading, license, splash, diagnostics.
- Consider WebP as a save format.

## Cross-platform notes

- **Permissions.** macOS needs Screen Recording for any capture and Accessibility for scrolling capture. Windows has no prompts, but elevated windows don't accept input from a non-elevated app.
- **HiDPI.** Sizes show in points with a px toggle. Windows display scaling needs the same px/DIP split, including mixed-DPI setups.
- **Window chrome.** macOS shadows and rounded corners don't exist in the same form on Windows 10/11.
