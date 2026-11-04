# Feature map: reference

How the alternative (Shottr 1.9.3, macOS) behaves, as observed. Each feature has a folder with a `feature.md` and screenshots. Screenshots were taken on synthetic demo images. What Kiri builds from this lives in [the Kiri feature map](../feature-map.md).

## Core flow

```
hotkey / menu bar
      │
      ▼
capture overlay ──► (area | fullscreen | window | scrolling | OCR-only)
      │
      ▼
after-capture action:  show editor  |  show thumbnail  |  copy  |  save
      │
      ▼
editor: crop · annotate · redact · measure · pick color · backdrop
      │
      ▼
output: copy · save · save as · pin to screen · drag-and-drop · print · upload
```

Most sessions end with one keystroke: `Esc` copies the image and closes the editor.

## Features

| # | Feature | Summary |
|---|---------|---------|
| 01 | [Area capture](01-area-capture/feature.md) | Frozen-screen crosshair with live coordinates and size; repeat the last area |
| 02 | [Fullscreen and delayed capture](02-fullscreen-capture/feature.md) | Whole screen instantly or after 3 s |
| 03 | [Window capture](03-window-capture/feature.md) | Pick any window or grab the active one, with a background and shadow setting |
| 04 | [Scrolling capture](04-scrolling-capture/feature.md) | Auto-scroll a region and stitch it into one tall image |
| 05 | [Text and QR recognition](05-text-and-qr-recognition/feature.md) | OCR or QR decode from the screen or the editor, straight to the clipboard |
| 06 | [After-capture preview](06-after-capture-preview/feature.md) | Floating thumbnail with quick actions, or auto copy/save |
| 07 | [Editor](07-editor/feature.md) | Canvas, toolbar, menus, zoom, copy/save/print, open file or clipboard |
| 08 | [Crop and selection](08-crop-and-selection/feature.md) | Select, crop, autoadjust to content, imprint size, erase region |
| 09 | [Annotations](09-annotations/feature.md) | Arrow, text, rectangle, oval, line, freehand, highlighter, counter; canvas grows to fit them |
| 10 | [Blur and erase](10-blur-and-erase/feature.md) | Blur, blur text only, erase, erase text only |
| 11 | [Spotlight and magnifier](11-spotlight-and-magnifier/feature.md) | Dim everything except an area; zoom-in callout |
| 12 | [Ruler](12-ruler/feature.md) | Hold a key to measure between edges in px/pt |
| 13 | [Color picker](13-color-picker/feature.md) | Color under cursor, several copy formats, WCAG contrast |
| 14 | [Backdrop](14-backdrop/feature.md) | Solid, gradient, or image background with padding, shadow, and radius |
| 15 | [Compose and GIF](15-compose-and-gif/feature.md) | Add another capture or clipboard image to the canvas; two-frame GIF |
| 16 | [Floating pin](16-floating-pin/feature.md) | Keep a screenshot on top of all windows |
| 17 | [Menu bar and hotkeys](17-menu-bar-and-hotkeys/feature.md) | Menu bar menu, global shortcuts, URL-scheme automation |
| 18 | [Settings](18-settings/feature.md) | Every preference with its default |

## Also in Shottr, not covered by a folder

- **Cloud upload and share links.** Shottr Cloud plus S3-compatible storage (Amazon, Cloudflare, Backblaze). File → Upload Image (⌘E).
- **Licensing.** Paid tiers gate some features: custom colors, gradient backdrops, Add Capture. There is also a "Friends Club" gate for experimental features and a joke "Show Annoying Fellow" menu item.
- **Telemetry.** Anonymous usage metrics, on by default, opt-out in Advanced.
- **Updater.** It downloads and reinstalls the app itself.

## Seen in the binary, not explored

These strings show up in the Shottr binary, but the UI wasn't reached:

- **Screenshot library.** It indexes the screenshots folder and lists saved shots ("Every time you save a screenshot with Shottr, it will appear here"). It can be turned on and off and has a size limit (`settings/library/enable|disable|limit`).
- **Cursor in screenshots.** auto / include / exclude (`settings/capturecursor/*`).
- **Custom area grabber** (`settings/areacustomgrabber/`) and a manual scrolling-capture mode (`settings/scrolling/manual/*`).
