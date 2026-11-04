# Editor

## What it does

The window every capture lands in. A canvas with the image, a single-row toolbar, and an info area. All tools from features 08–15 live here.

## Layout (observed)

Toolbar, left to right:

| Group | Items |
|---|---|
| Output | **Copy** (`Esc` or ⇧⌘C), **Save** (⌘S), **Floating Pin** (⌘P), **Drag'n'Drop Image** (drag this handle into another app) |
| Tools | **Crop/Select** (V), **Arrow** (A), **Text** (T), **Ruler** (1 or 2), **Rectangle** (R), **Backdrop** (K), **Freehand Drawing** (D), **Magnifier** (M) |
| Overflow | **More tools (…)**: Add Capture (⌘A), Blur/Erase (B), Highlighter (H), Spotlight (S), Counter (C), Oval (O), Line (L), Paste Image (⌘V) |
| Info | Color under cursor (`#F1F3F7`, "Tab to copy"), Image size (`800×500pt`; with a selection it shows "Selection size"), Zoom (`100%`) |

When the window is wide enough, the overflow tools move onto the toolbar (see `editor-wide-toolbar.png`). Single-letter shortcuts switch tools while the canvas has focus.

Every drawn object is selected after creation and shows a **contextual property bar** at the top right: color, size slider, and style variants for that tool (see [annotations](../09-annotations/feature.md)).

## Menus (macOS menu bar)

- **File:** Copy Image ⇧⌘C, Save Image ⌘S, Save Image As ⇧⌘S, Pin to Screen ⌘P, Print ⇧⌘P, Open File ⇧⌘O, Open From Clipboard ⇧⌘V, Close ⌘W, Close All ⌥⌘W. Upload Image ⌘E is out of scope.
- **Edit:** Undo ⌘Z, Redo ⇧⌘Z, **Reset Crop**, **Rasterize (Merge) Image** (flattens objects into pixels).
- **Draw:** every tool, plus **Snap to Objects** and **Snap to Similar Objects**.
- **Zoom:** Zoom In ⌘+, Zoom Out ⌘−, Zoom To Fit ⌘1, Zoom To 100% ⌘0, Zoom To Selection ⌘2, Selection Top Left `Q` / Selection Bottom Right `W` (zoom to 1600% with that corner of the selection in the middle of the view, for placing it on an exact pixel), Enter Full Screen.

## Behavior

- Annotations stay editable objects until saved or rasterized. Undo and redo cover both objects and pixel operations.
- Drawing past the image edge grows the canvas, and the export includes it. See [canvas growth](../09-annotations/feature.md#canvas-grows-to-fit-annotations-observed).
- Zoom goes up to at least 400% with crisp pixels at high zoom. The mouse wheel zooms, and Advanced has a reverse-direction option. The menu lists `Q` and `W` for the selection corners, but synthetic `Q`/`W` presses did nothing in a test, with or without a selection, at 100% and 400%; the menu items worked.
- Image size is shown in points by default. Clicking the size opens a popover:
  - **Show Size in Logical Points** toggle, also on `P`.
  - **Resize Image (can't be undone!)**: presets 25%, 33%, 50%, 1×, 2×, 4×, or a width × height in px, then **Resize**.
  - A preset fills the px fields from the current pixel size (1× is selected on open; 50% of 1400×1000 gives 700×500). Typing one field updates the other at the same aspect ratio.
  - Resize merges annotations into the pixels (they can no longer be selected), keeps the pixels-per-point (700×500 pt at 50% shows as 350×250 pt) and the zoom, and closes the popover.
- The app opens PNG, JPEG, and GIF files only (document types), and images from the clipboard. Dropping an image file on the editor and `Ctrl`-dragging the image out to another app are in the binary (unverified).
- Advanced: **Main window: Always on top**.
- ⌘-clicking the logo starts a Snake easter egg (from the binary, unverified).

## Saving

- Save format: **PNG** or **Auto (PNG/JPEG)**. **Downscale to 1× when saving** for Retina shots. Shottr retries with fallbacks if a save fails.
- The default folder is chosen in settings. Saving goes there without a dialog. If no folder is set, Save Image first opens a "Choose default folder" panel. Save As always asks.
- File name: `SCR-20261104-mnyv`, i.e. `SCR-` + date + a short time code (Save As proposed this for a fresh capture; an opened file keeps its own name). In the binary it is a hidden `fileNameTemplate` setting with strftime-like macros, default `SCR-%Y%m%d-%t`.
- From the binary, unverified:
  - Auto encodes JPEG (quality 0.92, `.jpeg`) and keeps it only if it is smaller than the PNG. Images taller than about 65 534 px are always PNG.
  - Saved files get the `kMDItemIsScreenCapture` attribute, like macOS screenshots.
  - A second ⌘S on the same image overwrites the file it saved before.
  - Copy puts PNG data on the clipboard, not a file.

## Screenshots

| Editor (default width) | Wide window: all tools in the toolbar |
|---|---|
| ![](editor.png) | ![](editor-wide-toolbar.png) |

| More tools menu | Zoom 400% |
|---|---|
| ![](more-tools-menu.png) | ![](zoom-400.png) |

Size popover (click the image size or press `P`):

![](size-popover.png)
