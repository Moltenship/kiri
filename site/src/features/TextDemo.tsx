import { Check } from 'lucide-react'
import { Stage, type Act, type Rect } from '../kit'
import { useDark } from '../theme'
import './features.css'

type S = { dim: boolean; sel: Rect | null; toast: boolean }

const LINES: { t: string; k?: 'cmd' | 'err' | 'dim' | 'gut' }[] = [
  { t: '$ cargo build --release', k: 'cmd' },
  { t: '   Compiling kiri v0.1.0', k: 'dim' },
  { t: 'error[E0425]: cannot find value `zoom`', k: 'err' },
  { t: '   --> src/pin.rs:142:23' },
  { t: '    |', k: 'gut' },
  { t: '142 |     let size = zoom * 2.0;', k: 'gut' },
  { t: '    |                ^^^^ not found', k: 'gut' },
]

const initial: S = { dim: false, sel: null, toast: false }

async function script(a: Act<S>) {
  await a.wait(600)
  await a.key('ctrl+alt+cmd+o')
  await a.set({ dim: true })
  const first = a.rect('line-2')
  const last = a.rect('line-3')
  const from = { x: first.x - 10, y: first.y - 6 }
  await a.move(from)
  await a.drag({ x: first.x + 300, y: last.y + last.height + 6 }, (at) => a.put({ sel: { x: from.x, y: from.y, width: at.x - from.x, height: at.y - from.y } }), { cursor: 'preserve' })
  await a.set({ dim: false, sel: null, toast: true })
  await a.wait(3000)
  await a.set({ toast: false })
  await a.wait(700)
}

export default function TextDemo() {
  const dark = useDark()
  return (
    <Stage
      className="ft ocr"
      width={440}
      height={330}
      initial={initial}
      poster={{ ...initial, toast: true }}
      script={script}
      cursor={{ at: { x: 360, y: 250 } }}
      keys="bottom-right"
      greys="cool"
      theme={dark ? 'dark' : 'light'}
    >
      {(s) => (
        <>
          <div className="ocr-term">
            <div className="ocr-bar">
              <span className="ocr-lights"><i /><i /><i /></span>
              <span className="ocr-name">zsh</span>
            </div>
            <pre className="ocr-out">
              {LINES.map((l, i) => (
                <div key={i} className={`ocr-l${l.k ? ` ocr-l--${l.k}` : ''}`} data-target={`line-${i}`}>
                  {l.t}
                </div>
              ))}
            </pre>
          </div>
          <div className="ocr-dim" data-on={s.dim || undefined} data-cursor={s.dim ? 'crosshair' : undefined}>
            {s.sel && <div className="ocr-sel" style={{ left: s.sel.x, top: s.sel.y, width: Math.max(0, s.sel.width), height: Math.max(0, s.sel.height) }} />}
          </div>
          <div className="ocr-toast" data-on={s.toast || undefined}>
            <Check size={15} strokeWidth={2.6} />
            <span className="ocr-toast-text">Text copied: error[E0425]: cannot find value `zoom` --&gt; src/pin.rs:142:23</span>
            <span className="ocr-toast-btn">Cut breaks</span>
          </div>
        </>
      )}
    </Stage>
  )
}
