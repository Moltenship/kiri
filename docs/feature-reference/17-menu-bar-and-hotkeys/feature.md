# Menu bar / tray and hotkeys

## Menu bar (tray) menu

Shottr is a menu bar app. Its icon menu (shown when "Hide menubar icon" is off):

- Reopen Shottr (shows the last screenshot)
- **Capture Screen** ⇧⌘3
- **Capture Area** ⇧⌘4
- **Scrolling Capture**
- **Recognize Text/QR** ⌃⌥⌘O
- **more ›** Repeat Area Capture · Capture Active Window ⇧⌘2 · Capture Any Window · Delayed Screenshot (3s) · Scrolling (Up) · Open File · Load From Clipboard
- Shottr on Twitter
- Launch at Startup ✓
- Settings ⌘,
- Quit

Closing the last window keeps the app running in the menu bar. On first close, Shottr explains: "If you quit the app, shortcuts will stop working. If you close the window, dock icon will go away, but the app will stay in the menu bar."

## Global hotkeys

Settings → Hotkeys has one recorder per action, each clearable:

| Action | Default |
|---|---|
| Fullscreen screenshot | ⇧⌘3 |
| Area screenshot | ⇧⌘4 |
| Repeat area screenshot | none |
| Any window screenshot | none |
| Active window screenshot | ⇧⌘2 (probably user-set, see below) |
| Scrolling screenshot | none |
| Show Shottr | none |
| Instant Text/QR Recognition | ⌃⌥⌘O |

The tab starts with "How to reassign default shortcuts": 1) open System Settings → Keyboard → Keyboard Shortcuts (with a button), 2) turn off the system Screenshots shortcuts, 3) put ⇧⌘3 and ⇧⌘4 into the Fullscreen and Area fields.

## URL scheme (automation)

`shottr://` deep links are off by default (Advanced → **URL Scheme API: Enable deep links**). When an app calls one while it's off, Shottr asks: "Some app, perhaps on your behalf, tries to execute deep link shottr://… Do you want to enable deep links in Shottr?"

Checked with deep links on:
- `grab/area`, `grab/repeat`, `grab/window`, `grab/scrolling/reverse` work. The first three run the system `screencapture` (`-i`, `-i`, `-w`), scrolling runs `screencapture -i -s`.
- An unknown command shows a toast "Shottr: command is not recognized. Unknown URL Scheme command: <path>". `grab/scrollingup` is one of them.

From the binary, unverified:
- Capture: `grab/fullscreen`, `grab/area`, `grab/repeat`, `grab/window`, `grab/window/active`, `grab/scrolling`, `grab/scrolling/reverse`, `grab/delayed`, `grab/append`, and `ocr`.
- Load and show: `load/clipboard`, `load/file`, `show`, `lib`, `lib/search`, `uploads`.
- Settings: `settings`, `settings/token`, `settings/license`, `settings/scrolling/manual/enable|disable`, `settings/capturecursor/auto|include|exclude`, `settings/library/enable|disable|limit`, `settings/areacustomgrabber/`.
- A `then=` parameter chains actions after a capture: `copy`, `save`, `edit`, `pin`, and a hidden `thumbnail`.
- Some commands need a token ("Shottr: no valid token provided"). Failures show "URL Scheme command failed:".

## Other app behavior

- The menu bar menu adds items only when they apply: Update the app, Manual Scrolling Capture, Manage Uploads, Purchase Shottr, Report a bug (from the binary).
- The first ⌘Q asks "Red pill or blue pill?": quit for real, or just close the window and stay in the menu bar. The answer is remembered (from the binary, unverified).
- The welcome splash has **Run at Startup** and **Setup Hotkeys** buttons.
- The Hotkeys tab has a **Reassign Default** button next to the steps above.
- Only Fullscreen, Area, and Instant Text/QR Recognition have defaults in the binary. ⇧⌘2 for the active window in this install is probably user-set.
- Launch at login uses the system login item. Updates come from Shottr's own updater, not Sparkle.

## Screenshots

| Menu bar menu | Hotkeys settings |
|---|---|
| ![](menu-bar-menu.png) | ![](hotkeys-settings.png) |
