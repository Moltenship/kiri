import { Stage, type Act } from '../kit'
import { Mosaic } from './Mosaic'
import { useDark } from '../theme'
import './features.css'

type S = { blurred: number; draft: number | null }

const LINES = [
  { key: 'APP_ENV', value: 'production' },
  { key: 'GITHUB_TOKEN', value: 'ghp_R7d2Kq9XwLm4Tz8Vn3', secret: 0 },
  { key: 'SMTP_USER', value: 'deploy' },
  { key: 'SMTP_PASSWORD', value: 'x8!fT2qLmv9R', secret: 1 },
]

const initial: S = { blurred: 0, draft: null }

async function script(a: Act<S>) {
  await a.wait(600)
  await a.key('b')
  for (let i = 0; i < 2; i++) {
    const r = a.rect(`secret-${i}`)
    await a.move({ x: r.x - 3, y: r.y - 3 })
    await a.drag({ x: r.x + r.width + 3, y: r.y + r.height + 3 }, (at) => a.put({ draft: Math.max(0, at.x - r.x + 3) }))
    await a.set({ blurred: i + 1, draft: null })
    await a.wait(350)
  }
  await a.wait(2200)
  await a.key('cmd+z')
  await a.set({ blurred: 1 })
  await a.key('cmd+z')
  await a.set({ blurred: 0 })
  await a.wait(500)
}

export default function RedactDemo() {
  const dark = useDark()
  return (
    <Stage
      className="ft rd"
      width={440}
      height={330}
      initial={initial}
      poster={{ blurred: 2, draft: null }}
      script={script}
      cursor={{ at: { x: 300, y: 290 } }}
      keys="bottom-right"
      greys="cool"
      theme={dark ? 'dark' : 'light'}
    >
      {(s) => (
        <div className="rd-pane" data-cursor="crosshair">
          <div className="rd-tabs">
            <span className="rd-tab">.env</span>
            <span className="rd-tab rd-tab--off" />
          </div>
          <div className="rd-code">
            {LINES.map((l, n) => {
              const i = l.secret
              const drafting = i !== undefined && s.draft !== null && s.blurred === i
              const shown = i !== undefined && (s.blurred > i || drafting)
              return (
                <div key={l.key} className="rd-line">
                  <span className="rd-n">{n + 1}</span>
                  <span className="rd-k">{l.key}</span>
                  <span className="rd-eq">=</span>
                  <span className="rd-v" data-target={i !== undefined ? `secret-${i}` : undefined}>
                    {l.value}
                    {i !== undefined && (
                      <span className="rd-blur" data-drafting={drafting || undefined} style={{ width: drafting ? s.draft! : 'calc(100% + 6px)', opacity: shown ? 1 : 0 }}>
                        <Mosaic w={250} h={24} ink="var(--rd-green)" paper="var(--ft-surface)" seed={11 + i * 5} />
                      </span>
                    )}
                  </span>
                </div>
              )
            })}
            <div className="rd-line rd-line--comment">
              <span className="rd-n">5</span>
              <span className="rd-c"># rotate both every 90 days</span>
            </div>
            {[6, 7, 8].map((n) => (
              <div key={n} className="rd-line">
                <span className="rd-n">{n}</span>
                <span className="rd-bar" style={{ width: [120, 168, 96][n - 6] }} />
              </div>
            ))}
          </div>
        </div>
      )}
    </Stage>
  )
}
