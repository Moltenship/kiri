import { Bar, Segmented, Stage, Toggle, Window, type Act } from '../kit'
import { useDark } from '../theme'
import './features.css'

type S = { on: boolean; aspect: number; palette: number }

// Kiri's backdrop palettes (src/backdrop.rs), drawn as a mesh: one color per corner.
const PALETTES = [
  ['#8EC5FC', '#E0C3FC', '#7FD1FF', '#C7B8FF'],
  ['#FF9A8B', '#FF6A88', '#FFC796', '#FF99AC'],
  ['#84FAB0', '#8FD3F4', '#C2FFD8', '#6FC3FF'],
]
const mesh = ([a, b, c, d]: string[]) =>
  `radial-gradient(at 0% 0%, ${a} 0, transparent 70%), radial-gradient(at 100% 0%, ${b} 0, transparent 70%), radial-gradient(at 0% 100%, ${c} 0, transparent 70%), radial-gradient(at 100% 100%, ${d} 0, transparent 70%), ${a}`

const ASPECTS = ['Auto', '1:1', '4:3', '16:9']
const SHOT = { w: 224, h: 136 }
const CENTER = { x: 220, y: 132 }
const PAD = 26

function plate(aspect: number) {
  const h = SHOT.h + PAD * 2
  const w = aspect === 3 ? (h * 16) / 9 : SHOT.w + PAD * 2
  return { left: CENTER.x - w / 2, top: CENTER.y - h / 2, width: w, height: h }
}

const initial: S = { on: false, aspect: 0, palette: 0 }

async function script(a: Act<S>) {
  await a.wait(600)
  await a.key('k')
  await a.set({ on: true })
  await a.wait(700)
  await a.click('aspect-3')
  await a.set({ aspect: 3 })
  await a.wait(600)
  await a.click('palette-1')
  await a.set({ palette: 1 })
  await a.wait(2400)
  await a.click('backdrop-remove')
  await a.set({ on: false })
  await a.wait(500)
  await a.snap({ aspect: 0, palette: 0 })
  await a.wait(300)
}

export default function BackdropDemo() {
  const dark = useDark()
  return (
    <Stage
      className="ft bd"
      width={440}
      height={330}
      initial={initial}
      poster={{ on: true, aspect: 3, palette: 1 }}
      script={script}
      cursor={{ at: { x: 360, y: 250 } }}
      keys="top"
      greys="cool"
      theme={dark ? 'dark' : 'light'}
    >
      {(s) => (
        <>
          <div className="bd-plate" data-on={s.on || undefined} style={plate(s.on ? s.aspect : 0)}>
            {PALETTES.map((p, i) => (
              <span key={i} style={{ background: mesh(p), opacity: s.palette === i ? 1 : 0 }} />
            ))}
          </div>
          <Window title="Capture" className="bd-shot" data-on={s.on || undefined} style={{ left: CENTER.x - SHOT.w / 2, top: CENTER.y - SHOT.h / 2, width: SHOT.w, height: SHOT.h }}>
            <div className="bd-rows">
              <div className="bd-row">
                <span>Freeze the screen while selecting</span>
                <Toggle on />
              </div>
              <div className="bd-row">
                <Bar w={96} h={6} tone={2} />
                <Toggle on={false} />
              </div>
            </div>
          </Window>
          <div className="bd-panel" data-on={s.on || undefined}>
            <Segmented items={ASPECTS} value={s.aspect} targets={ASPECTS.map((_, i) => `aspect-${i}`)} cursor="pointer" className="bd-seg" />
            <span className="bd-swatches">
              {PALETTES.map((p, i) => (
                <span key={i} className="bd-swatch" data-on={s.palette === i || undefined} data-target={`palette-${i}`} data-cursor="pointer" style={{ background: mesh(p) }} />
              ))}
            </span>
            <span className="bd-remove" data-target="backdrop-remove" data-cursor="pointer">Remove</span>
          </div>
        </>
      )}
    </Stage>
  )
}
