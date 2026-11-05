import { Check } from 'lucide-react'
import { Stage, type Act } from '../kit'
import { useDark } from '../theme'
import './features.css'

type S = { v: boolean; h: boolean; pinned: boolean; color: boolean }

const BTN = { x: 52, y: 148, w: 212, h: 48 }
const AT = { x: BTN.x + 196, y: BTN.y + 38 }

const initial: S = { v: false, h: false, pinned: false, color: false }

async function script(a: Act<S>) {
  await a.wait(500)
  await a.move(AT)
  await a.wait(200)
  await a.key('1')
  await a.set({ v: true })
  await a.wait(500)
  await a.key('2')
  await a.set({ h: true })
  await a.wait(500)
  await a.click(AT)
  await a.set({ pinned: true })
  await a.wait(700)
  await a.key('tab')
  await a.set({ color: true })
  await a.wait(2400)
  await a.set({ color: false })
  await a.key('cmd+z')
  await a.set({ v: false, h: false, pinned: false })
  await a.wait(700)
}

/** A measurement between two edges, Kiri pink, with its length on a pill. */
function Span({ axis, on, label }: { axis: 'v' | 'h'; on: boolean; label: string }) {
  const style = axis === 'v' ? { left: AT.x, top: BTN.y, height: BTN.h } : { left: BTN.x, top: AT.y, width: BTN.w }
  return (
    <div className={`ms-span ms-span--${axis}`} data-on={on || undefined} style={style}>
      <span className="ms-pill">{label}</span>
    </div>
  )
}

export default function MeasureDemo() {
  const dark = useDark()
  return (
    <Stage
      className="ft ms"
      width={440}
      height={392}
      initial={initial}
      poster={{ v: true, h: true, pinned: true, color: true }}
      script={script}
      cursor={{ at: { x: 330, y: 300 } }}
      keys="bottom-right"
      greys="cool"
      theme={dark ? 'dark' : 'light'}
    >
      {(s) => (
        <>
          <div className="ms-shot" data-cursor="crosshair">
            <span className="ms-title" />
            <span className="ms-title ms-title--2" />
            <span className="ms-lede" />
            <span className="ms-lede ms-lede--2" />
            <div className="ms-btn" style={{ left: BTN.x - 28, top: BTN.y - 28, width: BTN.w, height: BTN.h }}>Download for macOS</div>
            <div className="ms-btn ms-btn--ghost" style={{ left: BTN.x - 28 + BTN.w + 12, top: BTN.y - 28, height: BTN.h }}>Star on GitHub</div>
            <span className="ms-foot" />
            <div className="ms-cards">
              {[0, 1, 2].map((i) => (
                <span key={i} className="ms-card">
                  <i />
                  <b />
                  <b />
                </span>
              ))}
            </div>
          </div>
          <Span axis="v" on={s.v} label="48" />
          <Span axis="h" on={s.h} label="212" />
          <div className="ms-color" data-on={s.color || undefined} style={{ left: AT.x - 150, top: BTN.y + BTN.h + 44 }}>
            <span className="ms-swatch" />
            {dark ? '#F2F2F3' : '#161616'}
            <Check size={14} strokeWidth={2.6} />
          </div>
        </>
      )}
    </Stage>
  )
}
