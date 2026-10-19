import { harnessExecutionDocument, type HarnessExecutionEdge, type HarnessExecutionStep } from './harnessExecutionFlow'

export const BOARD_W = 1160
export const BOARD_H = 860

export type FigureShape = 'card' | 'store'

export interface FigureBox {
  id: string
  x: number
  y: number
  w: number
  h: number
  shape: FigureShape
}

export interface FigureFrame {
  id: string
  x: number
  y: number
  w: number
  h: number
  labelKey: string
}

export interface FigureCaption {
  x: number
  y: number
  labelKey: string
}

export interface FigureEdgeGeometry {
  id: string
  labelKey?: string
  d: string
  labelX: number
  labelY: number
}

export interface FigureBeat {
  edgeId: string
  light: readonly string[]
}

export interface FigureStory {
  id: string
  labelKey: string
  beats: readonly FigureBeat[]
}

interface Point {
  x: number
  y: number
}

const card = (id: string, x: number, y: number, w: number, h: number): FigureBox => ({
  id,
  x,
  y,
  w,
  h,
  shape: 'card'
})

const SCHEDULER_X = 276
const SCHEDULER_W = 188
const SCHEDULER_H = 52
const SCHEDULER_Y = [48, 132, 216, 300, 384, 468, 552, 636, 720]

const KERNEL_W = 176
const KERNEL_H = 50
const KERNEL_LEFT_X = 622
const KERNEL_RIGHT_X = 948
const KERNEL_Y = [252, 330, 408, 486, 564, 642, 720]

const STORE_W = 114
const STORE_H = 72
const STORE_X = [622, 748, 874, 1000]

/** Empty lanes used by return edges. Cards stay out of these x positions. */
const RAIL = {
  schedulerLeft: 246,
  schedulerRight: 498,
  channelTool: 554,
  channelReturn: 582,
  gutterQueue: 830,
  gutterConfirm: 872,
  gutterDeny: 914,
  header: 198,
  bottom: 820
}

export const FIGURE_FRAMES: readonly FigureFrame[] = [
  { id: 'scheduler', x: 212, y: 16, w: 318, h: 812, labelKey: 'settings.evolution.phases.scheduler' },
  { id: 'stock', x: 602, y: 16, w: 542, h: 136, labelKey: 'settings.evolution.phases.stock' },
  { id: 'kernel', x: 602, y: 164, w: 542, h: 676, labelKey: 'settings.evolution.phases.kernel' }
]

export const FIGURE_CAPTIONS: readonly FigureCaption[] = [
  { x: 626, y: 224, labelKey: 'settings.evolution.figure.groups.call' },
  { x: 952, y: 224, labelKey: 'settings.evolution.figure.groups.close' }
]

export const TURN_BOX = { x: 16, y: 268, w: 184, h: 300 }

export const FIGURE_BOXES: readonly FigureBox[] = [
  card('user-message', SCHEDULER_X, SCHEDULER_Y[0], SCHEDULER_W, SCHEDULER_H),
  card('compact-choice', SCHEDULER_X, SCHEDULER_Y[1], SCHEDULER_W, SCHEDULER_H),
  card('summarize', SCHEDULER_X, SCHEDULER_Y[2], SCHEDULER_W, SCHEDULER_H),
  card('infer', SCHEDULER_X, SCHEDULER_Y[3], SCHEDULER_W, SCHEDULER_H),
  card('text-choice', SCHEDULER_X, SCHEDULER_Y[4], SCHEDULER_W, SCHEDULER_H),
  card('assistant-text', SCHEDULER_X, SCHEDULER_Y[5], SCHEDULER_W, SCHEDULER_H),
  card('question-choice', SCHEDULER_X, SCHEDULER_Y[6], SCHEDULER_W, SCHEDULER_H),
  card('question', SCHEDULER_X, SCHEDULER_Y[7], SCHEDULER_W, SCHEDULER_H),
  card('answer', SCHEDULER_X, SCHEDULER_Y[8], SCHEDULER_W, SCHEDULER_H),
  { id: 'system', x: STORE_X[0], y: 46, w: STORE_W, h: STORE_H, shape: 'store' },
  { id: 'tools', x: STORE_X[1], y: 46, w: STORE_W, h: STORE_H, shape: 'store' },
  { id: 'budget', x: STORE_X[2], y: 46, w: STORE_W, h: STORE_H, shape: 'store' },
  { id: 'compact', x: STORE_X[3], y: 46, w: STORE_W, h: STORE_H, shape: 'store' },
  card('budget-choice', KERNEL_LEFT_X, KERNEL_Y[0], KERNEL_W, KERNEL_H),
  card('permission-projection', KERNEL_LEFT_X, KERNEL_Y[1], KERNEL_W, KERNEL_H),
  card('tool-execution', KERNEL_LEFT_X, KERNEL_Y[2], KERNEL_W, KERNEL_H),
  card('queue-choice', KERNEL_LEFT_X, KERNEL_Y[3], KERNEL_W, KERNEL_H),
  card('mutating-choice', KERNEL_LEFT_X, KERNEL_Y[4], KERNEL_W, KERNEL_H),
  card('completion-gate', KERNEL_LEFT_X, KERNEL_Y[5], KERNEL_W, KERNEL_H),
  card('turn-done', KERNEL_LEFT_X, KERNEL_Y[6], KERNEL_W, KERNEL_H),
  card('budget-denied', KERNEL_RIGHT_X, KERNEL_Y[0], KERNEL_W, KERNEL_H),
  card('confirm-choice', KERNEL_RIGHT_X, KERNEL_Y[1], KERNEL_W, KERNEL_H),
  card('confirm', KERNEL_RIGHT_X, KERNEL_Y[2], KERNEL_W, KERNEL_H),
  card('approved-choice', KERNEL_RIGHT_X, KERNEL_Y[3], KERNEL_W, KERNEL_H),
  card('denied', KERNEL_RIGHT_X, KERNEL_Y[4], KERNEL_W, KERNEL_H),
  card('deny-queue', KERNEL_RIGHT_X, KERNEL_Y[5], KERNEL_W, KERNEL_H),
  card('incomplete', KERNEL_RIGHT_X, KERNEL_Y[6], KERNEL_W, KERNEL_H)
]

const beat = (edgeId: string, light: readonly string[]): FigureBeat => ({ edgeId, light })

export const FIGURE_STORIES: readonly FigureStory[] = [
  {
    id: 'text',
    labelKey: 'settings.evolution.figure.stories.text',
    beats: [
      beat('user-message__compact-choice', ['user-message', 'compact-choice']),
      beat('compact-choice__infer', ['compact-choice', 'infer']),
      beat('infer__text-choice', ['infer', 'text-choice']),
      beat('text-choice__assistant-text', ['text-choice', 'assistant-text'])
    ]
  },
  {
    id: 'compact',
    labelKey: 'settings.evolution.figure.stories.compact',
    beats: [
      beat('user-message__compact-choice', ['user-message', 'compact-choice', 'compact']),
      beat('compact-choice__summarize', ['compact-choice', 'summarize', 'compact']),
      beat('summarize__infer', ['summarize', 'infer']),
      beat('infer__text-choice', ['infer', 'text-choice']),
      beat('text-choice__assistant-text', ['text-choice', 'assistant-text'])
    ]
  },
  {
    id: 'question',
    labelKey: 'settings.evolution.figure.stories.question',
    beats: [
      beat('compact-choice__infer', ['compact-choice', 'infer']),
      beat('infer__text-choice', ['infer', 'text-choice']),
      beat('text-choice__question-choice', ['text-choice', 'question-choice']),
      beat('question-choice__question', ['question-choice', 'question']),
      beat('question__answer', ['question', 'answer']),
      beat('answer__compact-choice', ['answer', 'compact-choice'])
    ]
  },
  {
    id: 'tool',
    labelKey: 'settings.evolution.figure.stories.tool',
    beats: [
      beat('question-choice__budget-choice', ['question-choice', 'budget-choice', 'budget', 'tools']),
      beat('budget__budget-choice', ['budget', 'budget-choice']),
      beat('budget-choice__permission-projection', ['budget-choice', 'permission-projection']),
      beat('permission-projection__tool-execution', ['permission-projection', 'tool-execution', 'tools']),
      beat('tool-execution__queue-choice', ['tool-execution', 'queue-choice']),
      beat('queue-choice__mutating-choice', ['queue-choice', 'mutating-choice']),
      beat('mutating-choice__compact-choice', ['mutating-choice', 'compact-choice'])
    ]
  },
  {
    id: 'confirm',
    labelKey: 'settings.evolution.figure.stories.confirm',
    beats: [
      beat('permission-projection__confirm-choice', ['permission-projection', 'confirm-choice']),
      beat('confirm-choice__confirm', ['confirm-choice', 'confirm']),
      beat('confirm__approved-choice', ['confirm', 'approved-choice']),
      beat('approved-choice__tool-execution', ['approved-choice', 'tool-execution'])
    ]
  },
  {
    id: 'deny',
    labelKey: 'settings.evolution.figure.stories.deny',
    beats: [
      beat('confirm-choice__denied', ['confirm-choice', 'denied']),
      beat('denied__deny-queue', ['denied', 'deny-queue']),
      beat('deny-queue__turn-done', ['deny-queue', 'turn-done'])
    ]
  },
  {
    id: 'budget',
    labelKey: 'settings.evolution.figure.stories.budget',
    beats: [beat('budget-choice__budget-denied', ['budget-choice', 'budget-denied', 'budget'])]
  },
  {
    id: 'gate',
    labelKey: 'settings.evolution.figure.stories.gate',
    beats: [
      beat('mutating-choice__completion-gate', ['mutating-choice', 'completion-gate']),
      beat('completion-gate__turn-done', ['completion-gate', 'turn-done'])
    ]
  },
  {
    id: 'open',
    labelKey: 'settings.evolution.figure.stories.open',
    beats: [
      beat('completion-gate__incomplete', ['completion-gate', 'incomplete']),
      beat('incomplete__infer', ['incomplete', 'infer'])
    ]
  }
]

const BOXES = new Map(FIGURE_BOXES.map((box) => [box.id, box]))

function midY(box: FigureBox): number {
  return box.y + box.h / 2
}

function midX(box: FigureBox): number {
  return box.x + box.w / 2
}

function rightOf(box: FigureBox): Point {
  return { x: box.x + box.w, y: midY(box) }
}

function leftOf(box: FigureBox): Point {
  return { x: box.x, y: midY(box) }
}

function bottomOf(box: FigureBox): Point {
  return { x: midX(box), y: box.y + box.h }
}

function topOf(box: FigureBox): Point {
  return { x: midX(box), y: box.y }
}

function vertical(from: FigureBox, to: FigureBox): Point[] {
  const downward = midY(to) >= midY(from)
  return [downward ? bottomOf(from) : topOf(from), downward ? topOf(to) : bottomOf(to)]
}

function horizontal(from: FigureBox, to: FigureBox): Point[] {
  const rightward = midX(to) >= midX(from)
  return [rightward ? rightOf(from) : leftOf(from), rightward ? leftOf(to) : rightOf(to)]
}

function routeAround(
  from: FigureBox,
  to: FigureBox,
  rail: number,
  leave: 'left' | 'right',
  enter: 'left' | 'right'
): Point[] {
  const start = leave === 'right' ? rightOf(from) : leftOf(from)
  const end = enter === 'right' ? rightOf(to) : leftOf(to)
  return [start, { x: rail, y: start.y }, { x: rail, y: end.y }, end]
}

function waypoints(id: string, from: FigureBox, to: FigureBox): Point[] {
  switch (id) {
    case 'compact-choice__infer':
    case 'text-choice__question-choice':
      return routeAround(from, to, RAIL.schedulerRight, 'right', 'right')
    case 'answer__compact-choice':
      return routeAround(from, to, RAIL.schedulerLeft, 'left', 'left')
    case 'question-choice__budget-choice':
      return routeAround(from, to, RAIL.channelTool, 'right', 'left')
    case 'budget__budget-choice':
      return [bottomOf(from), { x: midX(from), y: RAIL.header }, { x: midX(to), y: RAIL.header }, topOf(to)]
    case 'approved-choice__tool-execution':
      return routeAround(from, to, RAIL.gutterConfirm, 'left', 'right')
    case 'confirm-choice__denied':
      return routeAround(from, to, RAIL.gutterConfirm, 'left', 'left')
    case 'queue-choice__permission-projection':
      return routeAround(from, to, RAIL.gutterQueue, 'right', 'right')
    case 'deny-queue__permission-projection':
      return routeAround(from, to, RAIL.gutterDeny, 'left', 'right')
    case 'deny-queue__turn-done':
      return [
        leftOf(from),
        { x: RAIL.gutterDeny, y: midY(from) },
        { x: RAIL.gutterDeny, y: RAIL.bottom - 24 },
        { x: midX(to), y: RAIL.bottom - 24 },
        bottomOf(to)
      ]
    case 'completion-gate__incomplete':
      return routeAround(from, to, RAIL.gutterQueue, 'right', 'left')
    case 'mutating-choice__compact-choice':
      return routeAround(from, to, RAIL.channelReturn, 'left', 'right')
    case 'incomplete__infer':
      return [
        bottomOf(from),
        { x: midX(from), y: RAIL.bottom + 14 },
        { x: RAIL.channelTool, y: RAIL.bottom + 14 },
        { x: RAIL.channelTool, y: midY(to) },
        rightOf(to)
      ]
    default: {
      const sameColumn = Math.abs(midX(from) - midX(to)) < 24
      const sameRow = Math.abs(midY(from) - midY(to)) < 24
      if (sameColumn) return vertical(from, to)
      if (sameRow) return horizontal(from, to)
      return [rightOf(from), leftOf(to)]
    }
  }
}

const LABEL_AT: Record<string, Point> = {
  'compact-choice__infer': { x: RAIL.schedulerRight, y: SCHEDULER_Y[2] + SCHEDULER_H / 2 },
  'text-choice__question-choice': { x: RAIL.schedulerRight, y: SCHEDULER_Y[5] + SCHEDULER_H / 2 },
  'answer__compact-choice': { x: RAIL.schedulerLeft, y: (SCHEDULER_Y[8] + SCHEDULER_Y[1]) / 2 },
  'question-choice__budget-choice': { x: RAIL.channelTool, y: 500 },
  'mutating-choice__compact-choice': { x: RAIL.channelReturn, y: 250 },
  incomplete__infer: { x: 700, y: RAIL.bottom + 14 },
  'queue-choice__permission-projection': { x: RAIL.gutterQueue, y: 390 },
  'deny-queue__permission-projection': { x: RAIL.gutterDeny, y: 560 },
  'confirm-choice__denied': { x: RAIL.gutterConfirm, y: 470 },
  'approved-choice__tool-execution': { x: 910, y: KERNEL_Y[3] + KERNEL_H / 2 },
  'completion-gate__incomplete': { x: RAIL.gutterQueue, y: 706 },
  'deny-queue__turn-done': { x: 820, y: RAIL.bottom - 24 },
  'budget__budget-choice': { x: 820, y: RAIL.header }
}

function longestMid(points: readonly Point[]): Point {
  let bestLen = -1
  let best = points[0] ?? { x: 0, y: 0 }
  for (let index = 0; index < points.length - 1; index += 1) {
    const start = points[index]
    const end = points[index + 1]
    const len = Math.hypot(end.x - start.x, end.y - start.y)
    if (len > bestLen) {
      bestLen = len
      best = { x: (start.x + end.x) / 2, y: (start.y + end.y) / 2 }
    }
  }
  return best
}

function fillet(points: readonly Point[], radius = 12): string {
  const clean: Point[] = []
  for (const point of points) {
    const prev = clean[clean.length - 1]
    if (!prev || prev.x !== point.x || prev.y !== point.y) clean.push(point)
  }
  if (clean.length === 0) return ''
  if (clean.length === 1) return `M ${clean[0].x} ${clean[0].y}`
  if (clean.length === 2) {
    return `M ${clean[0].x} ${clean[0].y} L ${clean[1].x} ${clean[1].y}`
  }
  let path = `M ${clean[0].x} ${clean[0].y}`
  for (let index = 1; index < clean.length - 1; index += 1) {
    const prev = clean[index - 1]
    const current = clean[index]
    const next = clean[index + 1]
    const inLen = Math.hypot(current.x - prev.x, current.y - prev.y)
    const outLen = Math.hypot(next.x - current.x, next.y - current.y)
    if (inLen === 0 || outLen === 0) continue
    const curve = Math.min(radius, inLen / 2, outLen / 2)
    const inX = (current.x - prev.x) / inLen
    const inY = (current.y - prev.y) / inLen
    const outX = (next.x - current.x) / outLen
    const outY = (next.y - current.y) / outLen
    const enter = { x: current.x - inX * curve, y: current.y - inY * curve }
    const leave = { x: current.x + outX * curve, y: current.y + outY * curve }
    path += ` L ${enter.x} ${enter.y} Q ${current.x} ${current.y} ${leave.x} ${leave.y}`
  }
  const last = clean[clean.length - 1]
  path += ` L ${last.x} ${last.y}`
  return path
}

function edgeLabelKey(edge: HarnessExecutionEdge, steps: readonly HarnessExecutionStep[]): string | undefined {
  if (edge.labelKey) return edge.labelKey
  const source = steps.find((step) => step.id === edge.source)
  if (!source) return undefined
  if (edge.sourceHandle === 'matched') return source.matchedLabelKey
  if (edge.sourceHandle === 'otherwise') return source.otherwiseLabelKey
  return undefined
}

export function figureEdges(): readonly FigureEdgeGeometry[] {
  const { edges, steps } = harnessExecutionDocument()
  return edges.map((edge) => {
    const from = BOXES.get(edge.source)
    const to = BOXES.get(edge.target)
    if (!from || !to) return { id: edge.id, d: '', labelX: 0, labelY: 0 }
    const points = waypoints(edge.id, from, to)
    const label = LABEL_AT[edge.id] ?? longestMid(points)
    return {
      id: edge.id,
      labelKey: edgeLabelKey(edge, steps),
      d: fillet(points),
      labelX: label.x,
      labelY: label.y
    }
  })
}

export function figureStep(id: string): HarnessExecutionStep | undefined {
  return harnessExecutionDocument().steps.find((step) => step.id === id)
}
