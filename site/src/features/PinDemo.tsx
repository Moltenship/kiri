import { Check } from 'lucide-react'
import { Code, Popover, Stage, type Act, type Point } from '../kit'
import { useDark } from '../theme'
import './features.css'

type S = { pin: 'off' | 'on' | 'closing'; at: Point; menu: Point | null; translucent: boolean }

const START = { x: 150, y: 120 }
const PLACE = { x: 226, y: 30 }
const SIZE = { w: 190, h: 156 }

const CSS = [
  '.card {',
  '  display: grid;',
  '  gap: 12px;',
  '  padding: 20px;',
  '  border-radius: 16px;',
  '  background: #fff;',
  '}',
  '.card .cta {',
  '  height: 36px;',
  '  border-radius: 10px;',
  '  background: #0a84ff;',
  '}',
]

const initial: S = { pin: 'off', at: START, menu: null, translucent: false }

async function script(a: Act<S>) {
  const grab = (p: Point) => ({ x: p.x + 70, y: p.y + 40 })
  await a.wait(500)
  await a.key('cmd+p')
  await a.set({ pin: 'on' })
  await a.wait(500)
  await a.move(grab(START))
  await a.drag(grab(PLACE), (at) => a.put({ at: { x: at.x - 70, y: at.y - 40 } }))
  await a.set({ at: PLACE })
  await a.wait(300)

  const menuAt = () => {
    const c = grab(PLACE)
    return { x: c.x - 250, y: c.y + 4 }
  }
  await a.click(grab(PLACE), { right: true })
  await a.set({ menu: menuAt() })
  await a.wait(300)
  await a.click('pin-translucent')
  await a.set({ translucent: true })
  await a.set({ menu: null })
  await a.wait(2400)

  await a.click(grab(PLACE), { right: true })
  await a.set({ menu: menuAt() })
  await a.wait(300)
  await a.click('pin-close')
  await a.set({ menu: null })
  await a.set({ pin: 'closing' })
  await a.wait(450)
  await a.snap({ ...initial })
  await a.wait(300)
}

export default function PinDemo() {
  const dark = useDark()
  return (
    <Stage
      className="ft pn"
      width={440}
      height={330}
      initial={initial}
      poster={{ pin: 'on', at: PLACE, menu: null, translucent: true }}
      script={script}
      cursor={{ at: { x: 330, y: 280 } }}
      keys="bottom-right"
      greys="cool"
      theme={dark ? 'dark' : 'light'}
    >
      {(s) => (
        <>
          <div className="pn-editor">
            <div className="pn-tabs">
              <span className="rd-tab">card.css</span>
              <span className="rd-tab rd-tab--off" />
            </div>
            <Code lines={CSS} className="pn-code" />
          </div>
          <div
            className="pn-pin"
            data-state={s.pin}
            data-translucent={s.translucent || undefined}
            data-cursor="open"
            style={{ left: s.at.x, top: s.at.y, width: SIZE.w, height: SIZE.h }}
          >
            <span className="pn-cover" />
            <span className="pn-t" />
            <span className="pn-l" />
            <span className="pn-l pn-l--2" />
            <span className="pn-cta" />
          </div>
          <Popover show={!!s.menu} className="pn-menu" origin="top right" style={s.menu ? { left: s.menu.x, top: s.menu.y } : undefined}>
            <div className="pn-item">Edit</div>
            <div className="pn-item">Copy</div>
            <hr />
            <div className="pn-item">50%</div>
            <div className="pn-item"><Check size={12} strokeWidth={3} className="pn-check" />100%</div>
            <div className="pn-item">200%</div>
            <hr />
            <div className="pn-item" data-target="pin-translucent">
              {s.translucent && <Check size={12} strokeWidth={3} className="pn-check" />}Semi-transparent
            </div>
            <div className="pn-item">Click-through (hold ⌥ to reach it)</div>
            <hr />
            <div className="pn-item" data-target="pin-close">Close</div>
          </Popover>
        </>
      )}
    </Stage>
  )
}
