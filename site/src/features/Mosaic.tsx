import { useMemo } from 'react'
import { rng, splits } from '../editor/shapes'

/** Kiri's blur: cells of random size, each the average of the text and paper under it. */
export function Mosaic({ w, h, ink, paper = '#ffffff', seed = 7 }: { w: number; h: number; ink: string; paper?: string; seed?: number }) {
  const cells = useMemo(() => {
    const rand = rng(seed)
    const out: { x: number; y: number; w: number; h: number; fill: string }[] = []
    const rows = splits(0, h, rand)
    for (let r = 0; r < rows.length - 1; r++) {
      const cols = splits(0, w, rand)
      for (let c = 0; c < cols.length - 1; c++) {
        const t = 0.06 + rand() * 0.42
        out.push({ x: cols[c], y: rows[r], w: cols[c + 1] - cols[c], h: rows[r + 1] - rows[r], fill: `color-mix(in srgb, ${ink} ${Math.round(t * 100)}%, ${paper})` })
      }
    }
    return out
  }, [w, h, ink, paper, seed])
  return (
    <svg className="ft-mosaic" width={w} height={h} viewBox={`0 0 ${w} ${h}`} aria-hidden>
      {cells.map((c, i) => (
        <rect key={i} x={c.x} y={c.y} width={c.w + 0.5} height={c.h + 0.5} style={{ fill: c.fill }} />
      ))}
    </svg>
  )
}
