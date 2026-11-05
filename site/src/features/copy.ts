import type { DemoName } from './FeatureDemo'

export type Feature = { demo: DemoName; title: string; body: string }

export const FEATURES: Record<DemoName, Feature> = {
  scroll: { demo: 'scroll', title: 'Scrolling capture', body: 'Select the area and keep your hands off the mouse. Kiri scrolls and stitches the whole page into one image.' },
  redact: { demo: 'redact', title: 'Blur that holds', body: 'Cells of random size, burned in on export. Nothing of the original stays in the file.' },
  text: { demo: 'text', title: 'Copy text from pixels', body: 'Select an error, a slide or a QR code and the text is on your clipboard. Recognition runs on your Mac.' },
  measure: { demo: 'measure', title: 'Measure and pick', body: 'Hold 1 or 2 to measure between edges. Tab copies the color under the pointer.' },
  pin: { demo: 'pin', title: 'Pin it above everything', body: 'Keep a screenshot over every window while you work. Make it see-through, or let clicks pass.' },
  backdrop: { demo: 'backdrop', title: 'Ready for a post', body: 'A gradient backdrop at the aspect ratio you need, from square to 16:9.' },
}
