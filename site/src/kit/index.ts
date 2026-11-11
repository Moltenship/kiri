// Components import their own CSS, so no separate style import is required.

// The stage: the canvas, the script, the cursor, measuring and playback
export { Stage } from './stage'
export { useLook } from './looks'
export { useReducedMotion } from './reduced-motion'
export { inset, box, between, contains } from './measure'
export type { StageProps, Play, LookTokens } from './stage'
export type { Act, Script } from './act'
export type { Point, Rect, Target } from './measure'
export type { CursorKind, CursorStyle } from './cursor'
export type { Theme, Palette, Fidelity, Layout } from './looks'
export type { KeysAt } from './keys'
export { clock, installVirtualClock } from './clock'
export type { ClockControl } from './clock'

// Frames: windows, from real geometry
export { Window, Bar } from './window'
export type { WindowProps, BarProps } from './window'

// Motion: behaviors that have to hold up frame by frame (motion/, one file per part)
export { Count, Popover } from './motion'
export type { CountProps, PopoverOrigin } from './motion'
export { EASE, bezier, css as easeCss } from './ease'
export type { EaseName, Bezier } from './ease'

// Small parts: starting points with quiet defaults; restyle them as the product does (controls/)
export { Segmented, Toggle } from './controls'
export { Code } from './code'
export type { CodeLine } from './code'
