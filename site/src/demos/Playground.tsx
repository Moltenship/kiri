import { useCallback, useEffect, useRef, useState } from 'react'
import { Chrome, TOOLS } from '../editor/Chrome'
import { Canvas, HIGHLIGHT, RED, norm, pixelAt, textWidth, useShotPixels, type P, type Shape, type ToolId } from '../editor/shapes'
import { copyPng } from '../editor/exportPng'
import { useDark } from '../theme'
import './playground.css'

let seq = 100
const uid = () => `s${++seq}${Math.floor(Math.random() * 1e5)}`

/** What the page opens on: a finished markup, so the first frame already says what Kiri is for. */
export const STARTER: Shape[] = [
  { id: 's1', kind: 'redact', a: { x: 46, y: 255 }, b: { x: 160, y: 283 }, color: RED },
  { id: 's2', kind: 'highlight', a: { x: 846, y: 241 }, b: { x: 1132, y: 241 }, color: HIGHLIGHT },
  { id: 's3', kind: 'rect', a: { x: 1024, y: 126 }, b: { x: 1180, y: 178 }, color: RED },
  { id: 's4', kind: 'arrow', a: { x: 812, y: 172 }, b: { x: 1012, y: 150 }, color: RED },
  { id: 's5', kind: 'text', at: { x: 672, y: 170 }, text: 'Star it', color: RED },
]

const DRAG_KINDS = ['arrow', 'rect', 'oval', 'line', 'highlight', 'redact', 'spotlight', 'magnifier'] as const
type DragKind = (typeof DRAG_KINDS)[number]

function bbox(s: Shape) {
  if ('a' in s) {
    const r = norm(s.a, s.b)
    return { x: r.x - 12, y: r.y - 12, w: r.w + 24, h: r.h + 24 }
  }
  if (s.kind === 'pen') {
    const xs = s.pts.map((p) => p.x), ys = s.pts.map((p) => p.y)
    return { x: Math.min(...xs) - 12, y: Math.min(...ys) - 12, w: Math.max(...xs) - Math.min(...xs) + 24, h: Math.max(...ys) - Math.min(...ys) + 24 }
  }
  if (s.kind === 'counter') return { x: s.at.x - 28, y: s.at.y - 28, w: 56, h: 56 }
  return { x: s.at.x, y: s.at.y - 22, w: textWidth(s.text) + 28, h: 44 }
}

function moved(s: Shape, d: P): Shape {
  const t = (p: P) => ({ x: p.x + d.x, y: p.y + d.y })
  if ('a' in s) return { ...s, a: t(s.a), b: t(s.b) }
  if (s.kind === 'pen') return { ...s, pts: s.pts.map(t) }
  return { ...s, at: t(s.at) }
}

/** Shift: lines snap to 45°, boxes to squares; a nearly level highlight comes out level. */
function constrain(kind: DragKind, a: P, b: P, shift: boolean): P {
  const dx = b.x - a.x, dy = b.y - a.y
  if (kind === 'highlight' && Math.abs(dy) < Math.abs(dx) * 0.12) return { x: b.x, y: a.y }
  if (!shift) return b
  if (kind === 'rect' || kind === 'oval' || kind === 'redact' || kind === 'spotlight' || kind === 'magnifier') {
    const m = Math.max(Math.abs(dx), Math.abs(dy))
    return { x: a.x + Math.sign(dx || 1) * m, y: a.y + Math.sign(dy || 1) * m }
  }
  const ang = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4)
  const len = Math.hypot(dx, dy)
  return { x: a.x + Math.cos(ang) * len, y: a.y + Math.sin(ang) * len }
}

type Demo = { shape: Shape; tool: ToolId }
const DEMO: Demo[] = [
  { tool: 'rect', shape: STARTER[2] },
  { tool: 'arrow', shape: STARTER[3] },
  { tool: 'text', shape: STARTER[4] },
  { tool: 'highlight', shape: STARTER[1] },
  { tool: 'redact', shape: STARTER[0] },
  { tool: 'counter', shape: { id: 'd6', kind: 'counter', at: { x: 140, y: 448 }, n: 1, color: RED } },
  { tool: 'counter', shape: { id: 'd7', kind: 'counter', at: { x: 190, y: 624 }, n: 2, color: RED } },
  { tool: 'magnifier', shape: { id: 'd8', kind: 'magnifier', a: { x: 870, y: 498 }, b: { x: 950, y: 528 }, color: RED } },
]

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))

/**
 * A working editor on the page. With `autoplay`, it marks the screenshot up on its own
 * until the visitor touches it, then hands over with everything still on the canvas.
 */
export default function Playground({ autoplay = false }: { autoplay?: boolean }) {
  const px = useShotPixels()
  const svg = useRef<SVGSVGElement>(null)
  const wrap = useRef<HTMLDivElement>(null)
  const [shapes, setShapes] = useState<Shape[]>(autoplay ? [] : STARTER)
  const [past, setPast] = useState<Shape[][]>([])
  const [future, setFuture] = useState<Shape[][]>([])
  const [tool, setTool] = useState<ToolId>(autoplay ? 'select' : 'arrow')
  const [colors, setColors] = useState<Record<string, string>>({})
  const [draft, setDraft] = useState<Shape | null>(null)
  const [selected, setSelected] = useState<string | null>(null)
  const [editing, setEditing] = useState<{ at: P; value: string; color: string } | null>(null)
  const [hovered, setHex] = useState<string | null>(null)
  const dark = useDark()
  const hex = hovered ?? (dark ? '#0D1117' : '#FFFFFF')
  const [toast, setToast] = useState<string | null>(null)
  const hot = useRef(false)
  const [playing, setPlaying] = useState(autoplay)
  const [hidden, setHidden] = useState<Set<string>>(new Set())
  const drag = useRef<{ from: P; last: P; kind: 'draw' | 'move'; shift: boolean } | null>(null)

  const color = colors[tool] ?? (tool === 'highlight' ? HIGHLIGHT : RED)

  const commit = useCallback((next: Shape[]) => {
    setPast((p) => [...p.slice(-60), shapes])
    setFuture([])
    setShapes(next)
  }, [shapes])

  const toSvg = (e: { clientX: number; clientY: number }): P => {
    const el = svg.current!
    const pt = new DOMPoint(e.clientX, e.clientY).matrixTransform(el.getScreenCTM()!.inverse())
    return { x: pt.x, y: pt.y }
  }

  const flash = (text: string) => {
    setToast(text)
    window.clearTimeout((flash as any).t)
    ;(flash as any).t = window.setTimeout(() => setToast(null), 1800)
  }

  const takeOver = () => {
    if (!playing) return
    setPlaying(false)
    setHidden(new Set())
    setTool('arrow')
  }

  // The demo: each step picks its tool, then draws.
  useEffect(() => {
    if (!playing) return
    let alive = true
    ;(async () => {
      while (alive) {
        for (const step of DEMO) {
          if (!alive) return
          setTool(step.tool)
          await wait(420)
          if (!alive) return
          setHidden((h) => new Set(h).add(step.shape.id))
          setShapes((s) => [...s, step.shape])
          await wait(40)
          setHidden((h) => { const n = new Set(h); n.delete(step.shape.id); return n })
          await wait(step.tool === 'text' || step.tool === 'counter' ? 650 : 1000)
        }
        setTool('select')
        await wait(2600)
        // Undo, last first, so the loop's reset has a cause.
        for (let i = DEMO.length - 1; i >= 0 && alive; i--) {
          setShapes((s) => s.slice(0, -1))
          await wait(70)
        }
        await wait(700)
      }
    })()
    return () => { alive = false }
  }, [playing])

  const doCopy = useCallback(async () => {
    if (!svg.current) return
    setSelected(null)
    try {
      await copyPng(svg.current)
      flash('Copied. Paste it anywhere.')
    } catch {
      flash('Your browser blocked the clipboard.')
    }
  }, [])

  const undo = useCallback(() => {
    if (!past.length) return
    setFuture((f) => [shapes, ...f])
    setShapes(past[past.length - 1])
    setPast((p) => p.slice(0, -1))
    setSelected(null)
  }, [past, shapes])

  const redo = useCallback(() => {
    if (!future.length) return
    setPast((p) => [...p, shapes])
    setShapes(future[0])
    setFuture((f) => f.slice(1))
  }, [future, shapes])

  const finishText = useCallback(() => {
    if (!editing) return
    const text = editing.value.trim()
    if (text) commit([...shapes, { id: uid(), kind: 'text', at: editing.at, text, color: editing.color }])
    setEditing(null)
  }, [editing, shapes, commit])

  // Keys work while the pointer is over the editor, so the page still scrolls and tabs as usual elsewhere.
  // One listener reads the latest render through a ref, so a key pressed right after the pointer enters still lands.
  const onKey = (e: KeyboardEvent) => {
    if (!hot.current || editing) return
    const k = e.key.toLowerCase()
    if (playing) takeOver()
    if ((e.metaKey || e.ctrlKey) && k === 'z') {
      e.preventDefault()
      e.shiftKey ? redo() : undo()
      return
    }
    if (((e.metaKey || e.ctrlKey) && k === 'c') || k === 'escape') {
      e.preventDefault()
      doCopy()
      return
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return
    if (k === 'tab') {
      e.preventDefault()
      navigator.clipboard?.writeText(hex).then(() => flash(`Copied ${hex}`))
      return
    }
    if ((k === 'backspace' || k === 'delete') && selected) {
      commit(shapes.filter((s) => s.id !== selected))
      setSelected(null)
      return
    }
    const t = TOOLS.find((t) => t.key.toLowerCase() === k && t.id !== 'ruler')
    if (t) {
      setTool(t.id as ToolId)
      setSelected(null)
    }
  }
  const keyRef = useRef(onKey)
  keyRef.current = onKey
  useEffect(() => {
    const on = (e: KeyboardEvent) => keyRef.current(e)
    window.addEventListener('keydown', on)
    return () => window.removeEventListener('keydown', on)
  }, [])

  const onDown = (e: React.PointerEvent) => {
    if (e.button !== 0 || editing) return
    takeOver()
    const p = toSvg(e)
    const t = playing ? 'arrow' : tool
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    if (t === 'select') {
      const hit = [...shapes].reverse().find((s) => {
        const b = bbox(s)
        return p.x >= b.x && p.x <= b.x + b.w && p.y >= b.y && p.y <= b.y + b.h
      })
      setSelected(hit?.id ?? null)
      if (hit) drag.current = { from: p, last: p, kind: 'move', shift: false }
      return
    }
    setSelected(null)
    if (t === 'counter') {
      const n = Math.max(0, ...shapes.map((s) => (s.kind === 'counter' ? s.n : 0))) + 1
      commit([...shapes, { id: uid(), kind: 'counter', at: p, n, color }])
      return
    }
    if (t === 'text') {
      // Keeps the click from moving focus off the field it is about to open.
      e.preventDefault()
      setEditing({ at: p, value: '', color })
      return
    }
    drag.current = { from: p, last: p, kind: 'draw', shift: e.shiftKey }
    setDraft(t === 'pen' ? { id: uid(), kind: 'pen', pts: [p], color } : { id: uid(), kind: t as DragKind, a: p, b: p, color })
  }

  const onMove = (e: React.PointerEvent) => {
    const p = svg.current ? toSvg(e) : null
    if (p) {
      const h = pixelAt(px, p)
      if (h) setHex(h)
    }
    const d = drag.current
    if (!d || !p) return
    if (d.kind === 'move') {
      const delta = { x: p.x - d.last.x, y: p.y - d.last.y }
      d.last = p
      setShapes((all) => all.map((s) => (s.id === selected ? moved(s, delta) : s)))
      return
    }
    setDraft((s) => {
      if (!s) return s
      if (s.kind === 'pen') return { ...s, pts: [...s.pts, p] }
      if ('a' in s) return { ...s, b: constrain(s.kind as DragKind, s.a, p, e.shiftKey) }
      return s
    })
  }

  const onUp = () => {
    const d = drag.current
    drag.current = null
    if (d?.kind === 'move') {
      // The move already happened live; record where it started so undo can return it.
      setPast((p) => [...p, shapes.map((s) => (s.id === selected ? moved(s, { x: d.from.x - d.last.x, y: d.from.y - d.last.y }) : s))])
      setFuture([])
      return
    }
    if (!draft) return
    const big = draft.kind === 'pen' ? draft.pts.length > 2 : 'a' in draft && Math.hypot(draft.b.x - draft.a.x, draft.b.y - draft.a.y) > 8
    if (big) commit([...shapes, draft])
    setDraft(null)
  }

  // The text field sits over the canvas at the click, in the label's own size.
  const fieldAt = (() => {
    if (!editing || !svg.current || !wrap.current) return null
    const m = svg.current.getScreenCTM()!
    const c = new DOMPoint(editing.at.x, editing.at.y).matrixTransform(m)
    const box = wrap.current.getBoundingClientRect()
    return { left: c.x - box.left, top: c.y - box.top, scale: m.a }
  })()

  const all = draft ? [...shapes, draft] : shapes
  const cursor = playing ? 'default' : tool === 'select' ? 'default' : tool === 'text' ? 'text' : 'crosshair'

  return (
    <div className="pg" onPointerEnter={() => (hot.current = true)} onPointerMove={() => (hot.current = true)} onPointerLeave={() => (hot.current = false)} onFocus={() => (hot.current = true)}>
      <Chrome
        tool={tool}
        onTool={(t) => { takeOver(); setTool(t); setSelected(null) }}
        onCopy={doCopy}
        onUndo={undo}
        onRedo={redo}
        canUndo={past.length > 0}
        canRedo={future.length > 0}
        hex={hex}
        color={color}
        onColor={(c) => { setColors((m) => ({ ...m, [tool]: c })); if (selected) commit(shapes.map((s) => (s.id === selected ? { ...s, color: c } : s))) }}
        canvasProps={{ onPointerDown: onDown, onPointerMove: onMove, onPointerUp: onUp, style: { cursor, touchAction: 'none' } }}
        overlay={
          <div className="pg-hint" data-show={toast ? 'toast' : playing ? 'demo' : undefined} aria-live="polite">
            {toast ?? (playing ? 'Playing a demo. Click the screenshot to take over.' : '')}
          </div>
        }
      >
        <div className="pg-stage" ref={wrap}>
          <Canvas svgRef={svg} shapes={all} px={px} selected={selected} drawn={(s) => !hidden.has(s.id)} />
          {editing && fieldAt && (
            <input
              autoFocus
              className="pg-field"
              value={editing.value}
              placeholder="Type"
              style={{ left: fieldAt.left, top: fieldAt.top, fontSize: 24 * fieldAt.scale, height: 44 * fieldAt.scale, borderRadius: 10 * fieldAt.scale, padding: `0 ${14 * fieldAt.scale}px`, background: editing.color }}
              onChange={(e) => setEditing({ ...editing, value: e.target.value })}
              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === 'Escape') finishText() }}
              onBlur={finishText}
            />
          )}
        </div>
      </Chrome>
    </div>
  )
}
