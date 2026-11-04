# Backdrop

**Tool:** Backdrop (K)

## What it does

Puts the screenshot on a decorative background with padding, shadow, and rounded corners, ready to share on social media or in docs.

## Shottr behavior (observed)

Backdrop opens a side panel (the editor window widens to fit it) with live preview:

- **Backdrop** slider: padding around the image. Its `…` menu has **Reset** and aspect-ratio presets: 1:1, 4:3, 3:2, 16:9, Twitter, Facebook, YouTube, LinkedIn, Pinterest. A preset pads the canvas to the ratio (16:9 turned 800×500 into 1131×636pt) and shows the ratio as a label.
- **Shadow** slider: drop-shadow strength under the image.
- **Inset** slider: inner padding, with a **Recenter** link.
- **Outer Radius** / **Inner Radius**: corner rounding of the canvas and of the screenshot.
- Background type tabs:
  - **Solid:** 13 preset colors plus a custom `…`.
  - **Colormix:** 17 gradient presets plus custom, a gradient type dropdown (**Mesh**, **Vertical**, **Diagonal**, **Radial**), a swap-direction button, and a randomize button.
  - **Image:** a built-in wallpaper or a custom file (`…`), plus an **Image Blur** slider.
- **Remove** and **Ok** buttons.
- The image size grows to include the backdrop (`800×500` → `936×636pt`).
- Images over 15 MP are refused: "Crop or resize to 15 megapixel first". A second backdrop on a window capture that already has one gives "Window already has backdrop" (both from the binary).
- Custom backdrop images can be loaded from a folder ("Scan + Load custom backdrops", from the binary).
- Gradient and custom backgrounds are behind a license in Shottr ("Gradient Backdrop is only available after the license activation").

## Screenshots

| Colormix (mesh gradient) | Solid | Image + blur |
|---|---|---|
| ![](colormix.png) | ![](solid.png) | ![](image.png) |

Gradient types:

![](gradient-types.png)

Aspect-ratio presets in the `…` menu:

![](aspect-presets.png)
