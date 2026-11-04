# Color picker

**Shortcut:** `Tab` copies the color under the cursor

## What it does

In the editor, the toolbar always shows the color of the pixel under the cursor. Pressing `Tab` copies it.

## Shottr behavior (observed)

- A swatch plus hex value in the toolbar info area (`#FF8D28` over the orange card), with the hint "Tab to copy".
- Formats in the binary (cycled by clicking the value, presumably): `HEX FFC200`, `RGB (1.00, 0.76, 0.00)`, `CSS rgba(255, 194, 0, 1)`, `Android 0xFFFFC200`.
- **Contrast ratio:** with a selection, Shottr can show "WCAG 2.0 Contrast Ratio" for the colors inside it: "W3C accessibility guidelines recommend WCAG contrast ratio of at least 4.5 for regular text and 3.0 for large text and UI elements."
- A "Recalculate Global Top Colors" string suggests a dominant-colors summary.

## Screenshots

![Color under cursor in the toolbar](color-under-cursor.png)
