# Annotations

## What it does

Vector objects drawn over the screenshot. Each one stays editable (move, resize, restyle, delete) until export or rasterize.

## Shared behavior (observed)

- Pick a tool from the toolbar, the Draw menu, or its single-letter shortcut, then drag on the canvas.
- A new object stays selected with handles, and a **property bar** appears at the top right of the canvas:
  - `<` collapses the bar.
  - The **color swatch** opens a palette of 8 presets (red, purple, yellow, pink, teal, blue, navy, cream) plus **Custom**. Custom colors need a license in Shottr.
  - The **size slider** sets stroke width or font size. The tapered track shows thin → thick.
  - **Style variants** depend on the tool (below).
- **Snapping.** The Draw menu has **Snap to Objects** and **Snap to Similar Objects**, and the second is checked by default. Observed with rectangles:
  - An edge within about 5 pt of an image edge, or of the same edge line of another rectangle, jumps onto it, from either side. At 8 pt it stays free.
  - While drawing, only the dragged corner snaps. The start point stays where it was pressed.
  - A snapped edge shows a thin pink dashed guide (`#F71563`, about 2.5 pt dashes with 1.5 pt gaps) across the whole canvas. It disappears on release.
- `Option`-drag duplicates an object. Arrow keys nudge the selected object. `Backspace` deletes it and switches to the select tool (observed with a counter).
- ⌘C with an object selected keeps the editor open and puts the text "Shottr Rectangle" on the clipboard as a marker; the object itself stays inside Shottr. ⌘V pastes that object about 10 pt down and right while the marker is still on the clipboard, and pastes the clipboard image otherwise.
- `Shift` while drawing a rectangle makes a square on the longer side. From the binary, unverified: holding ⌘ or `Shift` skips snapping, and `Shift` constrains angles too.

## Tools

| Tool | Key | Variants and notes |
|---|---|---|
| **Arrow** | A | 4 styles, in toolbar order (observed frame by frame): **block** (the hollow ⇨ icon, but it draws a solid block arrow with a wide triangular head and no outline, even on dark backgrounds); **bold**, the default: a hand-drawn arrow with a slightly wavy, tapering shaft, round ends, and a head of two bowed strokes at about ±40°, whose shape differs per arrow; **thin**: an even shaft with flat ends and an open chevron with a mitered point, edges about 50° off the shaft; **double**: thin with a chevron at both ends. For thin and double the end point is the chevron's centerline corner, so the point overshoots it by half a stroke. "Thin" is only the shape: at the largest size its shaft is about 23 pt. The size slider is non-linear (about 1–23 pt, default about 3.3 pt), and each style scales its head on its own curve. Pressing ⌘ while drawing flips the arrow at once: the head lands on the drag's start point. Releasing a modifier mid-draw (⌘, or `Option`, which draws from the center) undoes it at once, without moving the mouse (seen with a real keyboard; synthetic key events showed a delay). No handles show while drawing. A middle handle bends the arrow into a curve. |
| **Text** | T | Two styles: text on a solid color label (default) or plain text. Click to place and type in place. The binary also has a "goofy" handwritten font (Noteworthy Bold) and a callout pointer (unverified). |
| **Rectangle** | R | 4 styles, in order: **hand-drawn** outline with rounded, slightly uneven corners whose stroke runs a little past its start (default); crisp **outline** with slightly rounded corners; **tinted**, a ~40% fill plus the outline; **solid** with ~4 pt corners. Rectangle, oval, spotlight, and magnifier share one shape model in the binary, but nothing in the UI converts one into another: pressing O with a rectangle selected only switches the tool and deselects. |
| **Oval** | O | Same 4 styles: a **hand-drawn** loop whose end overshoots its start (default), a squircle-like **outline**, **tinted**, and **solid**. |
| **Line** | L | Plain line, with a style menu (…). Solid or dashed. ⌘-clicking an end cycles its cap: none → open arrowhead → dot → none, each end on its own. The end handles sit about 6 pt past the line's ends, so they never cover a cap. |
| **Freehand Drawing** | D | Pen. Three variants: **free stroke**, **rectangle**, **oval**. The free stroke is heavily smoothed. Rectangle and oval reshape the drawn stroke into a rounded rectangle or an ellipse around it, keeping the gap between its ends open. The variant can be switched after drawing, while the stroke is selected. |
| **Highlighter** | H | A freehand marker stroke, pale yellow by default and about 28 pt thick, with a slightly ragged start. Its ends are cut flat and run a few points past the cursor. A straight stroke within about 1° of horizontal comes out level; at 2–6° the slope is damped. A selected highlighter has a pill handle at its end: dragging it draws on from there, and the overlap with the old stroke multiplies twice, so it shows darker. Freehand strokes have no such handle. It multiplies with the pixels under it, so black text stays pure black. Its own pastel palette, from the binary: `#ADF6FF #C2EAFE #E6E6FF #FFDAEF #FFF68D #F2FE01 #D8FF55 #DDF5C6`. |
| **Counter** | C | Click to drop numbered badges 1, 2, 3… The next number is one more than the highest on the canvas. A click always adds a badge, even on top of an existing one. The default badge is a speech bubble about 18 pt in radius with a regular-weight number and a tail pointing down-left; a handle past the tail rotates it, and new badges keep the last tail direction. The property bar's ⌃⌄ button switches between bubble and plain circle. Arrow keys nudge a badge like any object. |

Magnifier and Spotlight are in [11](../11-spotlight-and-magnifier/feature.md), and Blur/Erase in [10](../10-blur-and-erase/feature.md).

## Canvas grows to fit annotations (observed)

Annotations can go past the edge of the screenshot. The canvas grows to fit them, so an arrow can point at something from outside the image.

- **Size rule:** once any object crosses the image edge, the canvas becomes the bounding box of the image and all objects, **plus ~20pt padding on all four sides**. That includes sides that no object touches. Measured three times:
  - arrow 80pt past the right edge and 40pt above the top: 800×500 → 920×580
  - rectangle 70pt past the left edge and 45pt below the bottom: 911×585
  - arrow and freehand together: 1020×624
- **When it grows depends on the tool:** an arrow, a rectangle, and a magnifier lens grow the canvas live, and the "Image size" label updates on every mouse move. A freehand stroke is clipped at the current canvas edge and grows it only on release (see GIF). Other tools are unverified.
- **The view never moves:** zoom and the image position stay put while the canvas grows, including growth to the left or up. Whatever grows past the window edge is simply off screen. This holds even when the drag leaves the window.
- **Edge guide:** the pink dashed line along the image edge is the snapping guide (see above): edges snap to the image edge on their way out.
- **Shrinks back:** moving the object back inside the image, or undoing it, returns the canvas to the original size. The size is recomputed from object bounds, not accumulated. The "Image size" label updates on the next mouse move.
- **Fill:** the new area is one flat color picked from the image's dominant background: `#F3F5F8` in the demo, even next to the dark `#464E5F` sidebar. It is not an edge-pixel stretch, and on an opaque image it is not transparent. On a window capture with a transparent background the grown area stays transparent (checkerboard), as seen by a maintainer. Internal names hint at this: `expandableCanvas`, `bestCanvasColor`, `canvasLimitH/V`. Those last two suggest the growth has a cap.
- **Export:** Copy/Save output the expanded canvas with the fill (911×585pt → 1821×1170 px @2x).
- **After a crop:** the canvas grows around the crop the same way (a 250×200 crop plus a rectangle 39 pt past its right edge: 329×240). The new area gets the fill, here white, the cropped region's background rather than the whole image's. Pixels cropped away stay hidden under it.
- The color picker reads only the original pixels. Over the added area it shows `#000000`.
- **Zoom:** the view can't zoom out below fit. On a window that fits the image exactly, the margin around the image is tiny. The window has to be larger than the image before there is room to draw outside it.
- A crop selection is clamped to the current canvas bounds.

## Screenshots

| Arrow (curvable, 4 styles) | Text on label | Color palette |
|---|---|---|
| ![](arrow.png) | ![](text.png) | ![](color-palette.png) |

| Rectangle | Oval | Line |
|---|---|---|
| ![](rectangle.png) | ![](oval.png) | ![](line.png) |

| Freehand | Highlighter | Counter |
|---|---|---|
| ![](freehand.png) | ![](highlighter.png) | ![](counter.png) |

Freehand variants on the same stroke (free · rectangle · oval):

![](freehand-variants.png)

Canvas growth:

| Arrow past the edge: 800×500 → 920×580 | Arrow moved back inside: 800×500 again |
|---|---|
| ![](canvas-expanded-by-arrow.png) | ![](canvas-shrinks-back.png) |

| Fill is one background color, even next to the dark sidebar | Copied image includes the grown canvas |
|---|---|
| ![](canvas-fill-color.png) | ![](canvas-exported.png) |

A freehand stroke is clipped while dragging, and the canvas grows on release:

![](canvas-grows-on-release.gif)
