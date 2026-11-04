# Blur and erase (redaction)

**Tool:** Blur/Erase (B)

## What it does

Hides sensitive content (names, emails, tokens) before the screenshot is shared.

## Shottr behavior (observed)

Drag a rectangle with the Blur/Erase tool. The property bar has a strength slider and four modes:

| Mode | Effect |
|---|---|
| **Blur** | Pixelated or blurred block over the whole area. Internally it's `pixelate`. |
| **Blur Text** | Pixelates only the text inside the area and leaves the background, so the UI still looks intact. |
| **Erase** | Fills the area from the surrounding background, as if the content was never there. |
| **Erase Text** | Removes only the text glyphs and keeps the background. |

The selection toolbar also has **Erase (Del)**, which does the same for a crop selection ([08](../08-crop-and-selection/feature.md)).

## Screenshots

| Blur over a column of names | Erase over a text line |
|---|---|
| ![](blur.png) | ![](erase.png) |
