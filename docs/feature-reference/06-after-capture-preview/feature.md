# After-capture preview

**Settings:** General → After screenshot / After Area Crop, show / Hide preview thumbnail

## What it does

Decides what happens right after a capture. Options: open the full editor, show a small floating thumbnail with quick actions, and/or copy or save automatically.

## Shottr behavior (observed)

Settings → General:
- **After screenshot:** `Show`, `Copy`, `Save` checkboxes, combinable. Show + Copy means "open it and it's already in the clipboard".
- **After Area Crop, show:** `Thumbnail` or `Editor`.
- **Hide preview thumbnail:** `Manually (close button)` here. The dropdown is disabled while "After Area Crop, show" is `Editor`. The binary suggests the other option hides it automatically on a click outside, with no timeout (unverified).

The thumbnail is a small floating window near the capture:
- A close (×) button and a drag handle (••••) on top. Dragging the image out works like drag-and-drop to other apps.
- Actions at the bottom: **Edit** (opens the editor), **Copy**, **Save**, **Recognize text** ([a]), **Pin** to screen. The binary also has an **Upload** button and `Enter` for Edit (unverified).

Related (Advanced):
- **Action when hiding with Esc:** `Copy Image` and/or `Save Image`. Pressing `Esc` in the editor hides it and copies, which is the fastest flow.
- **Confirmation style:** `Custom Notification` (Shottr's own toast, as in the OCR screenshots), `System Notification`, or `None`. If system notifications are denied, the toast text gets "P.S. Shottr doesn't have permission to show System Notifications" (from the binary).

## Screenshots

![Thumbnail with quick actions](thumbnail.png)
