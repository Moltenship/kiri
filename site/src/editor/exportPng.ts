import { SHOT } from './shapes'

const inlined = new Map<string, Promise<string>>()
const inline = (src: string) => {
  if (!inlined.has(src))
    inlined.set(src, fetch(src)
      .then((r) => r.blob())
      .then((b) => new Promise<string>((ok) => {
        const fr = new FileReader()
        fr.onload = () => ok(fr.result as string)
        fr.readAsDataURL(b)
      })))
  return inlined.get(src)!
}

/** Renders the canvas SVG at 2x to a PNG. An SVG drawn as an image can't fetch, so the screenshot goes in inline. */
export async function renderPng(svg: SVGSVGElement): Promise<Blob> {
  const clone = svg.cloneNode(true) as SVGSVGElement
  const href = await inline(svg.querySelector('image')?.getAttribute('href') ?? SHOT.src)
  clone.querySelectorAll('image').forEach((i) => i.setAttribute('href', href))
  clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg')
  clone.setAttribute('width', String(SHOT.w))
  clone.setAttribute('height', String(SHOT.h))
  clone.setAttribute('font-family', '-apple-system, BlinkMacSystemFont, system-ui, sans-serif')
  const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(clone)], { type: 'image/svg+xml' }))
  try {
    const img = new Image()
    img.src = url
    await img.decode()
    const c = document.createElement('canvas')
    c.width = SHOT.w * 2
    c.height = SHOT.h * 2
    c.getContext('2d')!.drawImage(img, 0, 0, c.width, c.height)
    return await new Promise<Blob>((ok, err) => c.toBlob((b) => (b ? ok(b) : err(new Error('toBlob'))), 'image/png'))
  } finally {
    URL.revokeObjectURL(url)
  }
}

/** Safari wants the ClipboardItem created inside the gesture, with the blob still pending. */
export async function copyPng(svg: SVGSVGElement) {
  await navigator.clipboard.write([new ClipboardItem({ 'image/png': renderPng(svg) })])
}
