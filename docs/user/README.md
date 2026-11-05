# Using Kiri

Kiri lives in the menu bar. Every capture is in its menu, and Settings → Hotkeys gives any of them a shortcut.

The first launch walks through Screen Recording access, the capture shortcuts, and the look. Settings → General changes the appearance later: System, Light, or Dark.

## Capturing

- **Capture Area** (⇧⌘2): drag a rectangle. Hold `Space` while dragging to move it, `Shift` to lock an axis, or `Option` to grow it from the center. Arrow keys move the pointer one point at a time, before or during a drag, so an edge lands exactly where you want it. `Space` before dragging switches to picking a window. `Esc` cancels. To select on a still of the moment you pressed the hotkey, so menus and hover states stay put, turn on Settings → Capture → Freeze the screen while selecting.
- **Capture Screen** (⇧⌘1) takes the display under the pointer, or every display in one image with Settings → Capture → All displays in one image. **Delayed Screenshot** waits a few seconds first (3 by default, set in Settings → Capture).
- **Capture Window**: click a window. `Tab` picks the one behind it. Settings → Capture chooses what goes behind the window: wallpaper, transparency, a color, or nothing but the window.
- **Scrolling Capture**: select the area, then keep your hands off the mouse while Kiri scrolls and stitches. `Esc` stops and keeps what was captured. It needs Accessibility access. If an app ignores it, turn on Settings → Capture → Scroll by hand and scroll yourself.
- **Recognize Text or QR** (⌃⌥⌘O): select an area and its text or QR code is copied.
- **Repeat Area Capture** takes the last area again, and **Reopen Last Screenshot** brings back the last editor you closed, with its annotations still editable.

To capture with ⇧⌘3 and ⇧⌘4 instead, pick Instead of macOS on first launch, or Use ⇧⌘3 and ⇧⌘4 in Settings → Hotkeys. Kiri turns those two shortcuts off in macOS; ⇧⌘5 still opens the macOS screenshot toolbar. To get them back, turn them on in System Settings → Keyboard → Keyboard Shortcuts → Screenshots.

When a shortcut you give Kiri is also a macOS shortcut, macOS gets it first. Settings → Hotkeys marks that shortcut in red, names the macOS shortcut and where it is, and Open Settings takes you to Keyboard Shortcuts to turn it off.

## After a capture

A capture opens the editor or a small thumbnail (Settings → General). It can also be copied or saved at once. Drag the thumbnail into another app to drop the file there.

In the editor, `Esc` copies and closes, `⌘C` does the same, and `⇧⌘C` copies and keeps the editor open. `⌘S` saves to the save folder, the Desktop until you choose one in Settings → General. File name there sets how files are named: `%Y`, `%m`, `%d`, `%H`, `%M`, and `%S` fill in the date and time.

## Editing

Each tool has a letter: `V` select and crop, `A` arrow, `T` text, `R` rectangle, `O` oval, `L` line, `D` pen, `H` highlighter, `C` counter, `B` blur and erase, `S` spotlight, `M` magnifier.

- `Shift` straightens lines and makes squares and circles. `Option` draws from the center. Holding `⌘` while drawing an arrow points it back at where you started.
- Annotations past the image edge grow the canvas. The new area takes the image's background color, or stays transparent around a window capture.
- Zoom with the scroll wheel, a pinch, `⌘+` and `⌘-`. `⌘1` fits the image, `⌘0` shows it at 100%.
- With the select tool, drag to select an area: `Enter` crops to it, `A` trims it to its content, and `Q` or `W` zooms in on its corners for a pixel-exact edge.
- `K` adds a backdrop. Under Image, Wallpaper puts the screenshot on your current desktop picture. `P` switches the units between points and pixels.

## Measuring and colors

- Hold `1` to measure vertically, or `2` horizontally, between the nearest edges around the pointer. Hold both to see an element's width and height at once. Click to keep a measurement on the image.
- `Tab` copies the color under the pointer, and `⇧Tab` the color of the text under it. With an area selected, `C` copies its average color. Settings → Editor picks the color format.

## Pins

`⌘P` pins the screenshot above all windows. Scroll or pinch to resize a pin, and right-click it for opacity and click-through. A click-through pin lets clicks pass to whatever is under it. Hold `Option` to grab it again or turn click-through off. Close All Pins in the menu bar removes every pin.

## Automation

`kiri --capture <action>` runs a capture from a script, Shortcuts, or a launcher. The actions are `area`, `fullscreen`, `delayed`, `delayed-area`, `repeat-area`, `window`, `active-window`, `scrolling`, `scrolling-up`, `text`, `reopen`, and `settings`.

Add `--then` to choose what happens after that one capture, instead of the after-capture settings: any of `edit`, `thumbnail`, `copy`, and `save`, separated by commas, as in `kiri --capture area --then copy,save`.

Kiri can record your screen, so it won't take a capture for another app until you allow it. The first request shows an Allow button. Settings → Advanced → Automation turns it on or off.

## Updates

Kiri checks GitHub for a new version once a day. When one is out, a red dot appears on the menu bar icon. Choose Update to Kiri in its menu to download it and restart into it. Check for Updates… checks right away, and Settings → General turns the daily check off.
