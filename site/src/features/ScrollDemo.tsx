import { Count, Stage, type Act, type Rect } from '../kit'
import { useDark } from '../theme'
import './features.css'

type Phase = 'idle' | 'scroll' | 'done'
type S = { phase: Phase; sel: Rect | null; scroll: number; gone: boolean }

const GEO = {
  wide: { page: 440, sel: { x: 56, y: 40, width: 320, height: 220 }, card: { x: 492, w: 176 } },
  compact: { page: 276, sel: { x: 18, y: 28, width: 236, height: 196 }, card: { x: 290, w: 132 } },
}
const STEP = 130
const STEPS = 4

const RELEASES = [
  { v: '0.1.0-beta.10', lines: [92, 78, 54], shot: false },
  { v: '0.1.0-beta.9', lines: [84, 60], shot: true },
  { v: '0.1.0-beta.8', lines: [88, 70, 46], shot: false },
  { v: '0.1.0-beta.7', lines: [80, 64], shot: true },
  { v: '0.1.0-beta.6', lines: [90, 58, 72], shot: false },
]

/** The page column inside the selection; the result card draws the same column, scaled. */
function Column() {
  return (
    <div className="sc-col">
      <div className="sc-h1">Changelog</div>
      {RELEASES.map((r) => (
        <div key={r.v} className="sc-rel">
          <div className="sc-v">{r.v}</div>
          {r.shot && <div className="sc-shot" />}
          {r.lines.map((w, i) => (
            <span key={i} className="sc-line" style={{ width: `${w}%` }} />
          ))}
        </div>
      ))}
    </div>
  )
}

const initial: S = { phase: 'idle', sel: null, scroll: 0, gone: false }

async function script(a: Act<S>) {
  const { sel } = GEO[a.layout]
  await a.wait(500)
  const from = { x: sel.x, y: sel.y }
  await a.move(from)
  await a.drag({ x: sel.x + sel.width, y: sel.y + sel.height }, (at) => a.put({ sel: { x: from.x, y: from.y, width: at.x - from.x, height: at.y - from.y } }), { cursor: 'preserve' })
  await a.set({ sel, phase: 'scroll' })
  await a.wait(500)
  for (let i = 1; i <= STEPS; i++) {
    await a.set({ scroll: i * STEP })
    await a.wait(750)
  }
  await a.key('esc')
  await a.set({ phase: 'done' })
  await a.wait(2400)
  await a.set({ gone: true })
  await a.wait(450)
  await a.snap({ ...initial })
  await a.wait(300)
}

export default function ScrollDemo() {
  const dark = useDark()
  return (
    <Stage
      className="ft sc"
      width={720}
      height={340}
      compact={{ below: 560, width: 440, height: 330 }}
      initial={initial}
      poster={{ phase: 'done', sel: GEO.wide.sel, scroll: STEP * STEPS, gone: false }}
      script={script}
      cursor={{ at: { wide: { x: 300, y: 300 }, compact: { x: 200, y: 290 } } }}
      keys={{ wide: 'bottom-right', compact: 'bottom-right' }}
      greys="cool"
      theme={dark ? 'dark' : 'light'}
    >
      {(s, _rect, layout) => {
        const g = GEO[layout]
        const k = g.card.w / g.sel.width
        const sel = s.sel ?? g.sel
        const capturing = s.phase === 'scroll'
        return (
          <>
            <div className="sc-page" style={{ width: g.page }} data-cursor="crosshair">
              <div className="sc-scroll" style={{ transform: `translateY(${-s.scroll}px)` }}>
                <div className="sc-nav" />
                <div style={{ position: 'absolute', left: g.sel.x, top: g.sel.y, width: g.sel.width }}>
                  <Column />
                </div>
                <div className="sc-side" style={{ left: g.sel.x + g.sel.width + 16 }} />
              </div>
            </div>
            <div className="sc-dim" data-on={(s.sel && s.phase !== 'done') || undefined} style={{ width: g.page }}>
              <div
                className="sc-sel"
                data-live={capturing || undefined}
                style={{ left: sel.x, top: sel.y, width: Math.max(0, sel.width), height: Math.max(0, sel.height) }}
              />
            </div>
            <div className="sc-hud" data-on={capturing || undefined} style={{ left: g.sel.x + g.sel.width / 2, top: g.sel.y + g.sel.height + 14 }}>
              Scrolling… <Count value={(g.sel.height + s.scroll) * 2} /> px
            </div>
            <div
              className="sc-card"
              data-on={(s.phase !== 'idle' && !s.gone) || undefined}
              data-done={s.phase === 'done' || undefined}
              style={{ left: g.card.x, top: g.sel.y, width: g.card.w, height: (g.sel.height + s.scroll) * k }}
            >
              <div style={{ width: g.sel.width, transform: `scale(${k})`, transformOrigin: '0 0' }}>
                <Column />
              </div>
            </div>
          </>
        )
      }}
    </Stage>
  )
}
