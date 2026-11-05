import ScrollDemo from './ScrollDemo'
import RedactDemo from './RedactDemo'
import MeasureDemo from './MeasureDemo'
import TextDemo from './TextDemo'
import PinDemo from './PinDemo'
import BackdropDemo from './BackdropDemo'

const DEMOS = { scroll: ScrollDemo, redact: RedactDemo, measure: MeasureDemo, text: TextDemo, pin: PinDemo, backdrop: BackdropDemo }
export type DemoName = keyof typeof DEMOS

export default function FeatureDemo({ name }: { name: DemoName }) {
  const Demo = DEMOS[name]
  return <Demo />
}
