import {
  Circle, CircleDot, Copy, Download, Ellipsis, Flashlight, GripVertical, Grid3x3, Highlighter, Image, MoveUpRight,
  Pencil, Pin, Redo2, Ruler, Search, Slash, Square, SquareDashedMousePointer, Type, Undo2,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { PALETTE, HIGHLIGHT, type ToolId } from './shapes'
import './editor.css'

export const TOOLS: { id: ToolId | 'ruler'; icon: typeof Copy; key: string; name: string }[] = [
  { id: 'select', icon: SquareDashedMousePointer, key: 'V', name: 'Select and crop' },
  { id: 'arrow', icon: MoveUpRight, key: 'A', name: 'Arrow' },
  { id: 'text', icon: Type, key: 'T', name: 'Text' },
  { id: 'ruler', icon: Ruler, key: '1', name: 'Ruler' },
  { id: 'rect', icon: Square, key: 'R', name: 'Rectangle' },
  { id: 'oval', icon: Circle, key: 'O', name: 'Oval' },
  { id: 'line', icon: Slash, key: 'L', name: 'Line' },
  { id: 'pen', icon: Pencil, key: 'D', name: 'Pen' },
  { id: 'highlight', icon: Highlighter, key: 'H', name: 'Highlighter' },
  { id: 'counter', icon: CircleDot, key: 'C', name: 'Counter' },
  { id: 'redact', icon: Grid3x3, key: 'B', name: 'Blur and erase' },
  { id: 'spotlight', icon: Flashlight, key: 'S', name: 'Spotlight' },
  { id: 'magnifier', icon: Search, key: 'M', name: 'Magnifier' },
]

const SHORT = ['select', 'arrow', 'text', 'rect', 'highlight', 'redact']

const Btn = ({ icon: I, label, on, onClick, target, disabled }: { icon: typeof Copy; label: string; on?: boolean; onClick?: () => void; target?: string; disabled?: boolean }) => (
  <button type="button" className="kw-btn" data-on={on || undefined} aria-label={label} title={label} onClick={onClick} data-target={target} data-cursor="pointer" disabled={disabled}>
    <I size={16} strokeWidth={1.75} />
  </button>
)

export type ChromeProps = {
  tool?: ToolId
  onTool?: (t: ToolId) => void
  onCopy?: () => void
  onUndo?: () => void
  onRedo?: () => void
  canUndo?: boolean
  canRedo?: boolean
  /** The color picker readout on the right, as Kiri shows it. */
  hex: string
  size?: string
  zoom?: string
  color?: string
  onColor?: (c: string) => void
  /** Hides the left and right groups, for a narrow window. */
  compact?: boolean
  className?: string
  canvasProps?: React.HTMLAttributes<HTMLDivElement> & Record<`data-${string}`, unknown>
  children?: ReactNode
  overlay?: ReactNode
}

/** Kiri's editor window: a unified toolbar over a checkered canvas. */
export function Chrome(p: ChromeProps) {
  const palette = p.tool === 'highlight' ? [HIGHLIGHT, '#D4F79C', '#B2F1FC', '#FFCBE4'] : PALETTE
  const styled = p.tool && p.tool !== 'select' && p.tool !== 'magnifier' && p.tool !== 'spotlight' && p.tool !== 'redact'
  return (
    <div className={`kw ${p.className ?? ''}`} data-compact={p.compact || undefined}>
      <div className="kw-bar">
        <span className="kw-lights"><i /><i /><i /></span>
        {!p.compact && (
          <span className="kw-group kw-file">
            <Btn icon={Copy} label="Copy and close (Esc)" onClick={p.onCopy} target="copy" />
            <Btn icon={Download} label="Save" />
            <Btn icon={Pin} label="Pin to screen" />
            <span className="kw-grip"><GripVertical size={16} strokeWidth={1.75} /></span>
          </span>
        )}
        <span className="kw-sep kw-file" />
        <span className="kw-group kw-tools">
          {(p.compact ? TOOLS.filter((t) => SHORT.includes(t.id)) : TOOLS).map((t) => (
            <Btn key={t.id} icon={t.icon} label={`${t.name} (${t.key})`} on={p.tool === t.id} target={`tool-${t.id}`} onClick={t.id === 'ruler' ? undefined : () => p.onTool?.(t.id as ToolId)} />
          ))}
          {!p.compact && <Btn icon={Ellipsis} label="More" />}
        </span>
        {!p.compact && (
          <>
            <span className="kw-sep" />
            <span className="kw-group">
              <Btn icon={Image} label="Backdrop (K)" />
              <Btn icon={Undo2} label="Undo (⌘Z)" onClick={p.onUndo} disabled={p.canUndo === false} target="undo" />
              <Btn icon={Redo2} label="Redo (⇧⌘Z)" onClick={p.onRedo} disabled={p.canRedo === false} />
            </span>
          </>
        )}
        <span className="kw-spacer" />
        {!p.compact && (
          <span className="kw-stats">
            <span className="kw-stat kw-stat--color">
              <i style={{ background: p.hex }} />
              <span><b>{p.hex}</b><small>Tab to copy</small></span>
            </span>
            <span className="kw-stat"><span><b>{p.size ?? '1200×760 px'}</b><small>Image size</small></span></span>
            <span className="kw-stat"><span><b>{p.zoom ?? '100%'}</b><small>Zoom</small></span></span>
          </span>
        )}
      </div>
      <div {...p.canvasProps} className={`kw-canvas ${p.canvasProps?.className ?? ''}`}>
        {styled && p.onColor && (
          <div className="kw-props">
            {palette.map((c) => (
              <button key={c} type="button" className="kw-swatch" aria-label={`Color ${c}`} data-on={p.color === c || undefined} style={{ background: c }} onClick={() => p.onColor?.(c)} />
            ))}
          </div>
        )}
        {p.children}
      </div>
      {p.overlay}
    </div>
  )
}
