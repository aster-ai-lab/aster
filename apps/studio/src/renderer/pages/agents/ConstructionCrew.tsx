import { useEffect, useRef } from 'react'

/**
 * Original Lottie document: two workers, a striped barrier, and a swinging arm.
 * The player reads the subset this scene uses (position, rotation, rect, ellipse, fill).
 */
const CREW = {
  fr: 30,
  ip: 0,
  op: 90,
  w: 420,
  h: 240,
  layers: [
    layer('sky', [210, 100], 0, rect([0, 0], [420, 200], 0), [0.93, 0.95, 0.98, 1]),
    layer('ground', [210, 216], 0, rect([0, 0], [420, 48], 0), [0.84, 0.88, 0.82, 1]),
    layer(
      'cone',
      [52, 196],
      0,
      [rect([0, 12], [20, 6], 1), rect([0, 3], [12, 14], 1), rect([0, -8], [6, 10], 1)],
      [
        [0.96, 0.54, 0.18, 1],
        [0.96, 0.54, 0.18, 1],
        [0.96, 0.54, 0.18, 1]
      ]
    ),
    layer(
      'walker',
      keys([
        [0, [118, 186]],
        [45, [248, 186]],
        [90, [118, 186]]
      ]),
      0,
      workerShapes(0),
      workerFills()
    ),
    layer('holder', [328, 186], 0, workerShapes(0), workerFills()),
    layer(
      'hammer',
      [346, 158],
      keys([
        [0, -55],
        [12, 28],
        [24, -55],
        [48, 28],
        [60, -55],
        [90, -55]
      ]),
      [rect([18, 0], [40, 6], 3), rect([36, 0], [14, 12], 2)],
      [
        [0.96, 0.54, 0.23, 1],
        [0.35, 0.38, 0.45, 1]
      ]
    ),
    layer(
      'barrier',
      keys([
        [0, [78, 198]],
        [45, [236, 198]],
        [90, [78, 198]]
      ]),
      0,
      [rect([0, 0], [132, 16], 3), rect([-44, 0], [16, 16], 0), rect([0, 0], [16, 16], 0), rect([44, 0], [16, 16], 0)],
      [
        [0.96, 0.75, 0.22, 1],
        [0.09, 0.09, 0.09, 1],
        [0.09, 0.09, 0.09, 1],
        [0.09, 0.09, 0.09, 1]
      ]
    )
  ]
} as const

type StaticProp = { a: 0; k: readonly number[] }
type KeyProp = { a: 1; k: readonly { readonly t: number; readonly s: readonly number[] }[] }
type Prop = StaticProp | KeyProp
type Shape = { ty: 'rc' | 'el'; p: number[]; s: number[]; r: number }

export function ConstructionCrew() {
  const ref = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const canvas = ref.current
    if (!canvas || typeof CanvasRenderingContext2D === 'undefined') return
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
    let frame = 0
    const paint = () => {
      drawCrew(ctx, reduce ? 0 : frame)
      frame = (frame + 1) % CREW.op
    }
    paint()
    if (reduce) return
    const timer = window.setInterval(paint, 1000 / CREW.fr)
    return () => window.clearInterval(timer)
  }, [])

  return (
    <canvas
      ref={ref}
      width={CREW.w}
      height={CREW.h}
      aria-hidden
      data-lottie="construction-crew"
      className="h-[168px] w-[294px]"
    />
  )
}

function drawCrew(ctx: CanvasRenderingContext2D, frame: number) {
  ctx.clearRect(0, 0, CREW.w, CREW.h)
  for (const item of CREW.layers) {
    const position = sample(item.p, frame)
    const rotation = sample(item.r, frame)[0] ?? 0
    ctx.save()
    ctx.translate(position[0] ?? 0, position[1] ?? 0)
    ctx.rotate((rotation * Math.PI) / 180)
    item.shapes.forEach((shape, index) => {
      const fill = item.fills[index]
      if (!fill) return
      ctx.fillStyle = `rgba(${Math.round(fill[0] * 255)}, ${Math.round(fill[1] * 255)}, ${Math.round(fill[2] * 255)}, ${fill[3]})`
      ctx.beginPath()
      if (shape.ty === 'el') {
        ctx.ellipse(shape.p[0], shape.p[1], shape.s[0] / 2, shape.s[1] / 2, 0, 0, Math.PI * 2)
      } else {
        roundRect(ctx, shape.p[0] - shape.s[0] / 2, shape.p[1] - shape.s[1] / 2, shape.s[0], shape.s[1], shape.r)
      }
      ctx.fill()
    })
    ctx.restore()
  }
}

function sample(prop: Prop, frame: number): number[] {
  if (prop.a === 0) return [...prop.k]
  const keys = prop.k
  const first = keys[0]
  const last = keys[keys.length - 1]
  if (!first || !last || frame <= first.t) return first ? [...first.s] : [0]
  if (frame >= last.t) return [...last.s]
  for (let index = 0; index < keys.length - 1; index += 1) {
    const from = keys[index]
    const to = keys[index + 1]
    if (!from || !to || frame < from.t || frame > to.t) continue
    const span = to.t - from.t || 1
    const mix = (frame - from.t) / span
    return from.s.map((value, axis) => value + ((to.s[axis] ?? value) - value) * mix)
  }
  return [...last.s]
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, width: number, height: number, radius: number) {
  const r = Math.min(radius, width / 2, height / 2)
  ctx.moveTo(x + r, y)
  ctx.arcTo(x + width, y, x + width, y + height, r)
  ctx.arcTo(x + width, y + height, x, y + height, r)
  ctx.arcTo(x, y + height, x, y, r)
  ctx.arcTo(x, y, x + width, y, r)
}

function still(value: number[]): StaticProp {
  return { a: 0, k: value }
}

function keys(points: Array<[number, number[] | number]>): KeyProp {
  return {
    a: 1,
    k: points.map(([t, s]) => ({ t, s: typeof s === 'number' ? [s] : s }))
  }
}

function rect(p: number[], s: number[], r: number): Shape {
  return { ty: 'rc', p, s, r }
}

function ellipse(p: number[], s: number[]): Shape {
  return { ty: 'el', p, s, r: 0 }
}

function workerShapes(shift: number): Shape[] {
  return [
    rect([shift - 6, 18], [7, 26], 2),
    rect([shift + 6, 18], [7, 26], 2),
    rect([shift, -2], [24, 30], 6),
    ellipse([shift, -28], [16, 16]),
    rect([shift, -38], [20, 8], 3)
  ]
}

function workerFills(): number[][] {
  return [
    [0.23, 0.47, 0.94, 1],
    [0.23, 0.47, 0.94, 1],
    [0.94, 0.54, 0.23, 1],
    [0.95, 0.78, 0.64, 1],
    [0.96, 0.77, 0.26, 1]
  ]
}

function layer(
  name: string,
  p: number[] | KeyProp,
  r: number | KeyProp,
  shapes: Shape | Shape[],
  fills: number[] | number[][]
) {
  return {
    nm: name,
    p: Array.isArray(p) ? still(p) : p,
    r: typeof r === 'number' ? still([r]) : r,
    shapes: Array.isArray(shapes) ? shapes : [shapes],
    fills: typeof fills[0] === 'number' ? [fills as number[]] : (fills as number[][])
  }
}
