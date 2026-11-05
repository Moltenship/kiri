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

Observed details:

- While dragging, the area shows only as a translucent tinted box with a thin light outline. The effect is applied on release.
- **Blur** is a mosaic of cells of random size, not a regular grid. Rows have random heights, and each row is split into cells of random widths. Each cell is the average of its pixels, so flat areas stay flat and blocks show only where colors change. Cell size follows the slider: about 4–12 pt at the minimum, 10–40 pt at the default, and 40–80 pt at the maximum.
- **Blur Text** pixelates tight boxes around words and leaves the background untouched. A box is about as tall as the line's glyphs, so lines with a normal gap never touch. At the default and higher strengths each box is a single row of cells of random widths; at the minimum the cells are small and stack two or more to a line. It missed a lone single-digit number in a table cell.
- **Erase Text** removes glyphs and fills from the neighbouring background. Faint horizontal streaks are left where the text was.
- **Erase** fills the whole rectangle from the colors around it. Card edges and fills outside the rectangle stay intact.

The selection toolbar also has **Erase (Del)**, which does the same for a crop selection ([08](../08-crop-and-selection/feature.md)).

## Screenshots

| Blur over a column of names | Erase over a text line |
|---|---|
| ![](blur.png) | ![](erase.png) |
