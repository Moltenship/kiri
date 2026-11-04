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
- On macOS the selection behaves like the system grabber the alternative uses: `Space` while dragging moves it, `Shift` locks an axis, `Option` grows it from its center, and `Space` before dragging toggles window picking. Windows gets its own conventions later.
- Ideas: arrow keys nudge the selection, snap to window or UI edges.

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
- Manual mode as the fallback for apps that ignore synthetic scrolling: Settings → Scroll by hand. The user scrolls and Kiri stitches, so it needs no Accessibility. When frames stop overlapping, the HUD asks to scroll back and stitching resumes from the last good frame.
- Stitch by finding the overlap between frames. Ignore sticky headers and footers. Don't assume a fixed scroll step: smooth scrolling and inertia make it vary.
- Show progress. A tall result must not freeze the editor.
- Say why it stopped when it is not obvious: max height reached, nothing scrolled (with the Reverse scroll direction hint), or frames that stopped overlapping. No speed setting: each step waits until the frame settles, so the pace adapts to the app.
- macOS needs Accessibility. On Windows, wheel events go to the window under the cursor, and elevated windows ignore input from non-elevated apps.

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
- Pixels stay crisp when zoomed in.
- Same tools and single-letter shortcuts as the alternative.
- Objects stay editable until export.
- Copy puts both PNG and a file on the clipboard, so pasting works in chat apps and file managers. The drag handle produces a real file.
- Saved PNGs record their pixels-per-point, so a Retina screenshot reopens at its real size. Saved files carry the macOS screenshot tag, as the system's own do.
- Dropping an image file on the editor adds it as an image object where it lands, like Paste Image.
- Save goes straight to the save folder, the Desktop until one is chosen in settings, where the alternative first asks for a folder. A second Save overwrites the file it wrote. Save As proposes the opened or last saved file's name and folder, and a timestamped name otherwise.
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
- Fill for the grown area: the crop's background color (default), transparent, or custom. Cropped-away pixels never show through. Cap the canvas size.
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
- Hold `1` to measure vertically, `2` horizontally, or an arrow along its axis, as in the alternative.
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
- Image backdrop: any PNG or JPEG, cropped to cover, with a blur scaled to the backdrop so the preview matches the export. No bundled wallpapers.

### 15 Compose and GIF
- Image objects on the canvas: move, scale, opacity, z-order. The canvas grows the same way it does for annotations.
- Add Capture (Draw menu, or ⋯ at the end of the tools with Paste Image, as in the alternative) takes an area capture and drops it onto the open editor at its real size instead of opening a new one.
- A pasted image's GIF button saves a two-frame GIF to the save folder: the image, then what lies under it, one second each, looping. Kiri's frames follow the Copy output (crop, backdrop, downscale to 1×).

### 16 Floating pin
- Always on top, borderless, with a shadow. Drag to move, scroll or pinch to scale.
- Opacity and click-through, so a pin can be used as an overlay.
- Several pins at once. Pins don't survive a restart unless that becomes a setting.
- Decide whether pins float over full-screen apps on macOS and whether they show in Alt+Tab on Windows.

### 17 Tray and hotkeys
- Tray icon on both platforms with the same menu. Kiri works with no window open. Hiding the icon, as the alternative allows, waits for the app bundle: relaunching Kiri is then the only way back to Settings, and that needs a Dock or Launchpad icon.
- The first ⌘Q in a Kiri window asks whether to quit (hotkeys stop) or just close the window, and remembers the answer, as the alternative does. Settings → General → ⌘Q changes it later.
- Every capture action can have a global hotkey, with a warning when the OS or another app owns the shortcut.
- First-run guidance for freeing the system screenshot keys: ⇧⌘3/⇧⌘4 on macOS, `PrintScreen` on Windows ("Use Print Screen to open screen capture" must be off). Windows reserves `Win+Shift+S`.
- Kiri defaults to ⇧⌘1/⇧⌘2 so it works before the system keys are freed. Settings → Hotkeys starts with how to free ⇧⌘3/⇧⌘4 and a button that moves Capture Screen and Capture Area there, like the alternative's Reassign Default.
- One Kiri per user: launching it again opens Settings in the running one, or the file passed with `--open`, so there is never a second tray icon.
- CLI and/or URL scheme for automation, off by default, with a confirmation prompt.
- Launch at login, in Settings and as a checkmark in the tray menu, as in the alternative.
- Reopen Last Screenshot (tray, optional hotkey) brings back the last closed editor with its annotations still editable, crop, and backdrop, but not its undo history. Where the alternative keeps its window hidden, Kiri keeps the image as a PNG in memory, so the resident app stays small.

### 18 Settings
- One settings window: General, Capture, Editor, Hotkeys, Advanced. Short inline help for the non-obvious settings.
- Settings live in a plain config file that can be imported and exported.
- Expose the screenshot library in the UI. Cursor capture is left out until a capture path can include the cursor.
- Dropped from the alternative: uploading, license, splash, diagnostics.
- Consider WebP as a save format.

## Cross-platform notes

- **Permissions.** macOS needs Screen Recording for any capture and Accessibility for scrolling capture. Windows has no prompts, but elevated windows don't accept input from a non-elevated app.
- **HiDPI.** Sizes show in points with a px toggle. Windows display scaling needs the same px/DIP split, including mixed-DPI setups.
- **Window chrome.** macOS shadows and rounded corners don't exist in the same form on Windows 10/11.
