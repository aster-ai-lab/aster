export const CREATE_BOARD_STORAGE_KEY = 'workbench.create.boards.v1'

export const CREATE_KINDS = ['center', 'ppt', 'site', 'video', 'image'] as const

export type CreateKind = (typeof CREATE_KINDS)[number]

export type BoardNodeKind = 'sticky' | 'text' | 'shape' | 'frame' | 'pen' | 'connector'

export type CreateTool = 'select' | 'hand' | 'sticky' | 'text' | 'shape' | 'frame' | 'pen' | 'connector'

export type BoardPoint = [number, number]

export type BoardNode = {
  id: string
  kind: BoardNodeKind
  x: number
  y: number
  w: number
  h: number
  text: string
  literal: boolean
  fill: string
  points: BoardPoint[]
}

export type Camera = { panX: number; panY: number; zoom: number }

export type BoardDoc = {
  version: 1
  title: string
  titleLiteral: boolean
  nodes: BoardNode[]
  camera: Camera
}

export type ResizeCorner = 'nw' | 'ne' | 'se' | 'sw'

export const STICKY_COLORS = [
  { id: 'yellow', value: '#fff4c2' },
  { id: 'rose', value: '#ffd6e0' },
  { id: 'blue', value: '#d9e8ff' },
  { id: 'green', value: '#d8f3df' },
  { id: 'peach', value: '#ffe3cc' }
] as const

export const MIN_ZOOM = 0.25
export const MAX_ZOOM = 2.5

const TITLE_KEY: Record<CreateKind, string> = {
  center: 'workbench.feature.create',
  ppt: 'workbench.feature.create.ppt',
  site: 'workbench.feature.create.site',
  video: 'workbench.feature.create.video',
  image: 'workbench.feature.create.image'
}

const NODE_KINDS = new Set<BoardNodeKind>(['sticky', 'text', 'shape', 'frame', 'pen', 'connector'])

export function clampZoom(zoom: number) {
  if (!Number.isFinite(zoom)) return 1
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom))
}

export function screenToWorld(camera: Camera, sx: number, sy: number) {
  return { x: (sx - camera.panX) / camera.zoom, y: (sy - camera.panY) / camera.zoom }
}

/** Keep the world point under the cursor fixed while the zoom changes. */
export function zoomAt(camera: Camera, anchorX: number, anchorY: number, nextZoom: number): Camera {
  const zoom = clampZoom(nextZoom)
  const world = screenToWorld(camera, anchorX, anchorY)
  return { zoom, panX: anchorX - world.x * zoom, panY: anchorY - world.y * zoom }
}

export function fitCamera(nodes: BoardNode[], viewW: number, viewH: number): Camera | null {
  if (viewW < 32 || viewH < 32 || nodes.length === 0) return null
  let minX = Infinity
  let minY = Infinity
  let maxX = -Infinity
  let maxY = -Infinity
  for (const node of nodes) {
    minX = Math.min(minX, node.x)
    minY = Math.min(minY, node.y)
    maxX = Math.max(maxX, node.x + Math.max(node.w, 1))
    maxY = Math.max(maxY, node.y + Math.max(node.h, 1))
  }
  const pad = 72
  const bw = Math.max(1, maxX - minX)
  const bh = Math.max(1, maxY - minY)
  const zoom = clampZoom(Math.min((viewW - pad * 2) / bw, (viewH - pad * 2) / bh))
  return {
    zoom,
    panX: (viewW - bw * zoom) / 2 - minX * zoom,
    panY: (viewH - bh * zoom) / 2 - minY * zoom
  }
}

export function hitNode(nodes: BoardNode[], x: number, y: number): BoardNode | null {
  for (let index = nodes.length - 1; index >= 0; index -= 1) {
    const node = nodes[index]
    if (!node) continue
    if (x >= node.x && y >= node.y && x <= node.x + node.w && y <= node.y + node.h) return node
  }
  return null
}

export function resizeNode(origin: BoardNode, corner: ResizeCorner, dx: number, dy: number) {
  const minW = origin.kind === 'frame' ? 140 : 72
  const minH = origin.kind === 'frame' ? 88 : 48
  let { x, y, w, h } = origin
  if (corner === 'se' || corner === 'ne') w = Math.max(minW, origin.w + dx)
  if (corner === 'sw' || corner === 'nw') {
    w = Math.max(minW, origin.w - dx)
    x = origin.x + origin.w - w
  }
  if (corner === 'se' || corner === 'sw') h = Math.max(minH, origin.h + dy)
  if (corner === 'ne' || corner === 'nw') {
    h = Math.max(minH, origin.h - dy)
    y = origin.y + origin.h - h
  }
  return { x, y, w, h }
}

export function boundsOf(points: BoardPoint[]) {
  const xs = points.map((point) => point[0])
  const ys = points.map((point) => point[1])
  const x = Math.min(...xs)
  const y = Math.min(...ys)
  return {
    x,
    y,
    w: Math.max(12, Math.max(...xs) - x),
    h: Math.max(12, Math.max(...ys) - y),
    points: points.map(([px, py]) => [px - x, py - y] as BoardPoint)
  }
}

export function createNodeId() {
  return `n-${Math.random().toString(36).slice(2, 10)}`
}

function node(
  partial: Pick<BoardNode, 'id' | 'kind' | 'x' | 'y' | 'w' | 'h' | 'text'> & Partial<BoardNode>
): BoardNode {
  return {
    literal: false,
    fill: 'transparent',
    points: [],
    ...partial
  }
}

export function placeNode(tool: 'sticky' | 'text' | 'shape' | 'frame', x: number, y: number): BoardNode {
  if (tool === 'sticky') {
    return node({
      id: createNodeId(),
      kind: 'sticky',
      x: x - 88,
      y: y - 74,
      w: 176,
      h: 148,
      text: '',
      literal: true,
      fill: STICKY_COLORS[0].value
    })
  }
  if (tool === 'text') {
    return node({ id: createNodeId(), kind: 'text', x, y, w: 200, h: 40, text: '', literal: true })
  }
  if (tool === 'shape') {
    return node({
      id: createNodeId(),
      kind: 'shape',
      x: x - 84,
      y: y - 52,
      w: 168,
      h: 104,
      text: '',
      literal: true,
      fill: '#ffffff'
    })
  }
  return node({
    id: createNodeId(),
    kind: 'frame',
    x: x - 180,
    y: y - 110,
    w: 360,
    h: 220,
    text: 'workbench.create.frame',
    fill: 'transparent'
  })
}

function frame(id: string, x: number, y: number, w: number, h: number, text: string) {
  return node({ id, kind: 'frame', x, y, w, h, text })
}

function sticky(id: string, x: number, y: number, text: string) {
  return node({ id, kind: 'sticky', x, y, w: 168, h: 124, text, fill: STICKY_COLORS[0].value })
}

function shape(id: string, x: number, y: number, w: number, h: number, text: string, fill: string) {
  return node({ id, kind: 'shape', x, y, w, h, text, fill })
}

export function seedNodes(kind: CreateKind): BoardNode[] {
  if (kind === 'ppt') {
    return [
      frame('ppt-1', 0, 36, 320, 180, 'workbench.create.seed.ppt.1'),
      frame('ppt-2', 356, 36, 320, 180, 'workbench.create.seed.ppt.2'),
      frame('ppt-3', 712, 36, 320, 180, 'workbench.create.seed.ppt.3'),
      frame('ppt-4', 1068, 36, 320, 180, 'workbench.create.seed.ppt.4'),
      sticky('ppt-note', 20, 64, 'workbench.create.seed.ppt.note')
    ]
  }
  if (kind === 'site') {
    return [
      frame('site-frame', 0, 28, 760, 460, 'workbench.create.seed.site.frame'),
      shape('site-nav', 28, 78, 704, 48, 'workbench.create.seed.site.nav', '#f5f5f5'),
      shape('site-hero', 28, 142, 704, 156, 'workbench.create.seed.site.hero', '#e7ecff'),
      sticky('site-note', 48, 176, 'workbench.create.seed.site.note')
    ]
  }
  if (kind === 'video') {
    return [
      frame('video-1', 0, 48, 240, 136, 'workbench.create.seed.video.1'),
      frame('video-2', 268, 48, 240, 136, 'workbench.create.seed.video.2'),
      frame('video-3', 536, 48, 240, 136, 'workbench.create.seed.video.3'),
      frame('video-4', 804, 48, 240, 136, 'workbench.create.seed.video.4'),
      sticky('video-note', 16, 72, 'workbench.create.seed.video.note')
    ]
  }
  if (kind === 'image') {
    return [
      frame('image-frame', 0, 32, 560, 380, 'workbench.create.seed.image.frame'),
      shape('image-a', 36, 92, 72, 72, '', '#e07848'),
      shape('image-b', 120, 92, 72, 72, '', '#4c6fff'),
      shape('image-c', 204, 92, 72, 72, '', '#69b9c9'),
      sticky('image-note', 300, 150, 'workbench.create.seed.image.note')
    ]
  }
  return []
}

export function freshBoard(kind: CreateKind): BoardDoc {
  return {
    version: 1,
    title: TITLE_KEY[kind],
    titleLiteral: false,
    nodes: seedNodes(kind),
    camera: { panX: 64, panY: 56, zoom: 1 }
  }
}

function isPoint(value: unknown): value is BoardPoint {
  return (
    Array.isArray(value) &&
    value.length === 2 &&
    value.every((part) => typeof part === 'number' && Number.isFinite(part))
  )
}

function isNode(value: unknown): value is BoardNode {
  if (!value || typeof value !== 'object') return false
  const node = value as BoardNode
  return (
    typeof node.id === 'string' &&
    NODE_KINDS.has(node.kind) &&
    [node.x, node.y, node.w, node.h].every((part) => typeof part === 'number' && Number.isFinite(part)) &&
    typeof node.text === 'string' &&
    typeof node.literal === 'boolean' &&
    typeof node.fill === 'string' &&
    Array.isArray(node.points) &&
    node.points.every(isPoint)
  )
}

function isDoc(value: unknown): value is BoardDoc {
  if (!value || typeof value !== 'object') return false
  const doc = value as BoardDoc
  return (
    doc.version === 1 &&
    typeof doc.title === 'string' &&
    typeof doc.titleLiteral === 'boolean' &&
    Array.isArray(doc.nodes) &&
    doc.nodes.every(isNode) &&
    !!doc.camera &&
    [doc.camera.panX, doc.camera.panY, doc.camera.zoom].every(
      (part) => typeof part === 'number' && Number.isFinite(part)
    )
  )
}

type Store = Partial<Record<CreateKind, BoardDoc>>

function readStore(storage: Pick<Storage, 'getItem'> | null): Store {
  if (!storage) return {}
  try {
    const raw = storage.getItem(CREATE_BOARD_STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw) as unknown
    if (!parsed || typeof parsed !== 'object') return {}
    const store: Store = {}
    for (const kind of CREATE_KINDS) {
      const doc = (parsed as Store)[kind]
      if (isDoc(doc)) store[kind] = { ...doc, camera: { ...doc.camera, zoom: clampZoom(doc.camera.zoom) } }
    }
    return store
  } catch {
    return {}
  }
}

export function boardFor(storage: Pick<Storage, 'getItem'> | null, kind: CreateKind): BoardDoc {
  return readStore(storage)[kind] ?? freshBoard(kind)
}

export function writeBoard(storage: Pick<Storage, 'getItem' | 'setItem'> | null, kind: CreateKind, doc: BoardDoc) {
  if (!storage || !isDoc(doc)) return
  try {
    const store = readStore(storage)
    store[kind] = doc
    storage.setItem(CREATE_BOARD_STORAGE_KEY, JSON.stringify(store))
  } catch {
    // A full or blocked store keeps the in-memory board for this visit.
  }
}
