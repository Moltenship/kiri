import { useEffect, useState } from 'react'
import { useDark } from '../theme'

export const SHOT = { src: '/shots/github-kiri.svg', dark: '/shots/github-kiri-dark.svg', w: 1200, h: 760 }
/** The screenshot matches the page: a dark page shows GitHub in dark mode. */
export const useShotSrc = () => (useDark() ? SHOT.dark : SHOT.src)

export type P = { x: number; y: number }

export type Shape =
  | { id: string; kind: 'arrow' | 'rect' | 'oval' | 'line' | 'highlight' | 'redact' | 'spotlight' | 'magnifier'; a: P; b: P; color: string }
  | { id: string; kind: 'pen'; pts: P[]; color: string }
  | { id: string; kind: 'counter'; at: P; n: number; color: string }
  | { id: string; kind: 'text'; at: P; text: string; color: string }

export type ToolId = 'select' | 'arrow' | 'text' | 'rect' | 'oval' | 'line' | 'pen' | 'highlight' | 'counter' | 'redact' | 'spotlight' | 'magnifier'

/** Kiri's presets (src/doc.rs PALETTE), first seven. */
export const PALETTE = ['#FF3B30', '#FF9500', '#FFD60A', '#34C759', '#00B5C8', '#0A84FF', '#7D5BFF']
export const HIGHLIGHT = '#FFED9E'
export const RED = PALETTE[0]

const STROKE = 6

export const norm = (a: P, b: P) => ({ x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), w: Math.abs(b.x - a.x), h: Math.abs(b.y - a.y) })

export function arrowPath(a: P, b: P) {
  const dx = b.x - a.x
  const dy = b.y - a.y
  const len = Math.hypot(dx, dy) || 1
  // The shaft bows a little, as Kiri's hand-drawn default does.
  const bow = Math.min(40, len * 0.08)
  const c = { x: (a.x + b.x) / 2 - (dy / len) * bow, y: (a.y + b.y) / 2 + (dx / len) * bow }
  const tx = b.x - c.x
  const ty = b.y - c.y
  const head = Math.min(34, len * 0.4)
  const ang = Math.atan2(ty, tx)
  const arm = (s: number) => ({ x: b.x - head * Math.cos(ang + s * 0.5), y: b.y - head * Math.sin(ang + s * 0.5) })
  const l = arm(1)
  const r = arm(-1)
  return { shaft: `M${a.x},${a.y} Q${c.x},${c.y} ${b.x},${b.y}`, head: `M${l.x},${l.y} L${b.x},${b.y} L${r.x},${r.y}` }
}

/** A seeded random, so a redaction keeps its cells while it is redrawn. */
export function rng(seed: number) {
  let t = seed >>> 0
  return () => {
    t += 0x6d2b79f5
    let r = Math.imul(t ^ (t >>> 15), 1 | t)
    r ^= r + Math.imul(r ^ (r >>> 7), 61 | r)
    return ((r ^ (r >>> 14)) >>> 0) / 4294967296
  }
}

export const splits = (from: number, to: number, rand: () => number) => {
  const out = [from]
  let at = from
  while (at < to) {
    at = Math.min(to, at + 9 + rand() * 13)
    out.push(at)
  }
  return out
}

function average(px: ImageData | null, x: number, y: number, w: number, h: number) {
  if (!px) return '#d0d7de'
  let r = 0, g = 0, b = 0, n = 0
  for (let i = 0; i < 4; i++)
    for (let j = 0; j < 4; j++) {
      const sx = Math.min(px.width - 1, Math.max(0, Math.round(x + ((i + 0.5) * w) / 4)))
      const sy = Math.min(px.height - 1, Math.max(0, Math.round(y + ((j + 0.5) * h) / 4)))
      const k = (sy * px.width + sx) * 4
      r += px.data[k]; g += px.data[k + 1]; b += px.data[k + 2]; n++
    }
  return `rgb(${Math.round(r / n)},${Math.round(g / n)},${Math.round(b / n)})`
}

export function pixelAt(px: ImageData | null, p: P) {
  if (!px || p.x < 0 || p.y < 0 || p.x >= px.width || p.y >= px.height) return null
  const k = (Math.floor(p.y) * px.width + Math.floor(p.x)) * 4
  return '#' + [px.data[k], px.data[k + 1], px.data[k + 2]].map((v) => v.toString(16).padStart(2, '0')).join('').toUpperCase()
}

/** The screenshot's pixels, for redaction cells and the color readout. */
export function useShotPixels() {
  const src = useShotSrc()
  const [px, setPx] = useState<ImageData | null>(null)
  useEffect(() => {
    const img = new Image()
    img.onload = () => {
      const c = document.createElement('canvas')
      c.width = SHOT.w
      c.height = SHOT.h
      const ctx = c.getContext('2d')!
      ctx.drawImage(img, 0, 0, SHOT.w, SHOT.h)
      setPx(ctx.getImageData(0, 0, SHOT.w, SHOT.h))
    }
    img.src = src
  }, [src])
  return px
}

function Redact({ s, px }: { s: Extract<Shape, { a: P }>; px: ImageData | null }) {
  const r = norm(s.a, s.b)
  if (r.w < 2 || r.h < 2) return null
  const rand = rng(Number.parseInt(s.id.replace(/\D/g, '').slice(-8) || '7', 10))
  const xs = splits(r.x, r.x + r.w, rand)
  const ys = splits(r.y, r.y + r.h, rand)
  const cells = []
  for (let i = 0; i < xs.length - 1; i++)
    for (let j = 0; j < ys.length - 1; j++) {
      const x = xs[i], y = ys[j], w = xs[i + 1] - x, h = ys[j + 1] - y
      cells.push(<rect key={`${i}-${j}`} x={x} y={y} width={w + 0.5} height={h + 0.5} fill={average(px, x, y, w, h)} />)
    }
  return <g className="k-redact">{cells}</g>
}

function Counter({ at, n, color }: { at: P; n: number; color: string }) {
  // src/variants/counter-bubble.svg, scaled around its circle.
  const k = 2.6
  return (
    <g transform={`translate(${at.x - 12 * k},${at.y - 12 * k}) scale(${k})`}>
      <g className="k-pop">
        <path d="M12 3.5a8.5 8.5 0 1 1-4.4 15.8L3.5 20.5l1.2-4.1A8.5 8.5 0 0 1 12 3.5z" fill={color} />
        <text x="12" y="12" dy="0.36em" textAnchor="middle" fontSize="9.6" fontWeight="600" fill="#fff">{n}</text>
      </g>
    </g>
  )
}

let ctx: CanvasRenderingContext2D | null = null
/** The label's text width at 24px semibold; demos render only in the browser. */
export const textWidth = (text: string) => {
  ctx ??= document.createElement('canvas').getContext('2d')
  if (!ctx) return text.length * 12.5
  ctx.font = '600 24px -apple-system, BlinkMacSystemFont, system-ui, sans-serif'
  return Math.max(8, ctx.measureText(text).width)
}

function Label({ at, text, color }: { at: P; text: string; color: string }) {
  const w = textWidth(text) + 28
  return (
    <g className="k-pop">
      <rect x={at.x} y={at.y - 22} width={w} height={44} rx={10} fill={color} />
      <text x={at.x + 14} y={at.y} dy="0.36em" fontSize="24" fontWeight="600" fill={color === '#FFD60A' ? '#1C1C1E' : '#fff'}>{text}</text>
    </g>
  )
}

/** One annotation. `drawn` false hides a stroke so it can draw itself in. */
export function ShapeView({ s, px, drawn = true, selected = false, gone = false }: { s: Shape; px?: ImageData | null; drawn?: boolean; selected?: boolean; gone?: boolean }) {
  const src = useShotSrc()
  const common = { 'data-drawn': drawn, 'data-selected': selected || undefined, 'data-gone': gone || undefined, className: `k-shape k-${s.kind}` }
  switch (s.kind) {
    case 'arrow': {
      const { shaft, head } = arrowPath(s.a, s.b)
      return (
        <g {...common} stroke={s.color} strokeWidth={STROKE} fill="none" strokeLinecap="round" strokeLinejoin="round">
          <path className="k-stroke" pathLength={1} d={shaft} />
          <path className="k-stroke k-late" pathLength={1} d={head} />
        </g>
      )
    }
    case 'rect': {
      const r = norm(s.a, s.b)
      return (
        <g {...common}>
          <rect className="k-stroke" pathLength={1} x={r.x} y={r.y} width={r.w} height={r.h} rx={8} fill="none" stroke={s.color} strokeWidth={STROKE} />
        </g>
      )
    }
    case 'oval': {
      const r = norm(s.a, s.b)
      return (
        <g {...common}>
          <ellipse className="k-stroke" pathLength={1} cx={r.x + r.w / 2} cy={r.y + r.h / 2} rx={r.w / 2} ry={r.h / 2} fill="none" stroke={s.color} strokeWidth={STROKE} />
        </g>
      )
    }
    case 'line':
      return (
        <g {...common}>
          <path className="k-stroke" pathLength={1} d={`M${s.a.x},${s.a.y} L${s.b.x},${s.b.y}`} stroke={s.color} strokeWidth={STROKE} strokeLinecap="round" />
        </g>
      )
    case 'highlight':
      return (
        <g {...common}>
          <path className="k-stroke" pathLength={1} d={`M${s.a.x},${s.a.y} L${s.b.x},${s.b.y}`} stroke={s.color} strokeWidth={28} style={{ mixBlendMode: 'multiply' }} />
        </g>
      )
    case 'pen':
      return (
        <g {...common}>
          <path className="k-stroke" pathLength={1} d={s.pts.map((p, i) => `${i ? 'L' : 'M'}${p.x},${p.y}`).join(' ')} stroke={s.color} strokeWidth={STROKE} fill="none" strokeLinecap="round" strokeLinejoin="round" />
        </g>
      )
    case 'counter':
      return <g {...common}><Counter at={s.at} n={s.n} color={s.color} /></g>
    case 'text':
      return <g {...common}><Label at={s.at} text={s.text} color={s.color} /></g>
    case 'redact':
      return <g {...common}><Redact s={s} px={px ?? null} /></g>
    case 'magnifier': {
      const r = norm(s.a, s.b)
      if (r.w < 4 || r.h < 4) return <g {...common}><rect x={r.x} y={r.y} width={r.w} height={r.h} rx={6} fill="none" stroke="#fff" strokeWidth={3} /></g>
      const zoom = 2.6
      const lr = (Math.max(r.w, r.h) * zoom) / 2
      const cx = r.x + r.w + lr * 0.75
      const cy = r.y + r.h + lr * 0.55
      const sx = r.x + r.w / 2
      const sy = r.y + r.h / 2
      return (
        <g {...common}>
          <defs>
            <clipPath id={`lens-${s.id}`}><circle cx={cx} cy={cy} r={lr} /></clipPath>
          </defs>
          <rect x={r.x} y={r.y} width={r.w} height={r.h} rx={6} fill="none" stroke="#fff" strokeWidth={3} filter="drop-shadow(0 1px 3px rgba(0,0,0,.35))" />
          <circle cx={cx} cy={cy} r={lr + 2} fill="#fff" filter="drop-shadow(0 8px 24px rgba(0,0,0,.28))" />
          <g clipPath={`url(#lens-${s.id})`}>
            <image href={src} width={SHOT.w} height={SHOT.h} transform={`translate(${cx - sx * zoom},${cy - sy * zoom}) scale(${zoom})`} />
          </g>
          <circle cx={cx} cy={cy} r={lr} fill="none" stroke="#fff" strokeWidth={5} />
        </g>
      )
    }
    case 'spotlight':
      return null
  }
}

/** Every spotlight dims one shared overlay, so two of them never stack their dim. */
export function Spotlights({ shapes, w, h, pad = 400 }: { shapes: Shape[]; w: number; h: number; pad?: number }) {
  const holes = shapes.filter((s) => s.kind === 'spotlight') as Extract<Shape, { a: P }>[]
  if (!holes.length) return null
  const outer = `M${-pad},${-pad}H${w + pad}V${h + pad}H${-pad}Z`
  const inner = holes
    .map((s) => {
      const r = norm(s.a, s.b)
      const rx = Math.min(14, r.w / 2, r.h / 2)
      return `M${r.x + rx},${r.y}H${r.x + r.w - rx}Q${r.x + r.w},${r.y} ${r.x + r.w},${r.y + rx}V${r.y + r.h - rx}Q${r.x + r.w},${r.y + r.h} ${r.x + r.w - rx},${r.y + r.h}H${r.x + rx}Q${r.x},${r.y + r.h} ${r.x},${r.y + r.h - rx}V${r.y + rx}Q${r.x},${r.y} ${r.x + rx},${r.y}Z`
    })
    .join('')
  return <path className="k-spot" d={outer + inner} fillRule="evenodd" fill="rgba(0,0,0,.5)" />
}

/** The screenshot and its annotations, in the screenshot's own pixels. */
export type View = { x: number; y: number; w: number; h: number }
export const FULL: View = { x: 0, y: 0, w: SHOT.w, h: SHOT.h }

export function Canvas({ shapes, px, drawn, gone, selected, children, className, svgRef, view = FULL, target = 'shot' }: {
  shapes: Shape[]
  px?: ImageData | null
  drawn?: (s: Shape) => boolean
  /** Fading out, for an exit. */
  gone?: boolean
  selected?: string | null
  children?: React.ReactNode
  className?: string
  svgRef?: React.Ref<SVGSVGElement>
  /** The part of the screenshot on show, for a crop. */
  view?: View
  target?: string
}) {
  const src = useShotSrc()
  return (
    <svg ref={svgRef} className={`k-canvas ${className ?? ''}`} viewBox={`${view.x} ${view.y} ${view.w} ${view.h}`} data-target={target} style={view === FULL ? undefined : { overflow: 'hidden' }}>
      <image href={src} width={SHOT.w} height={SHOT.h} />
      {shapes.filter((s) => s.kind === 'redact' || s.kind === 'highlight').map((s) => <ShapeView key={s.id} s={s} px={px} drawn={drawn?.(s) ?? true} gone={gone} selected={selected === s.id} />)}
      <Spotlights shapes={shapes.filter((s) => drawn?.(s) ?? true)} w={SHOT.w} h={SHOT.h} />
      {shapes.filter((s) => s.kind !== 'redact' && s.kind !== 'highlight').map((s) => <ShapeView key={s.id} s={s} px={px} drawn={drawn?.(s) ?? true} gone={gone} selected={selected === s.id} />)}
      {children}
    </svg>
  )
}
