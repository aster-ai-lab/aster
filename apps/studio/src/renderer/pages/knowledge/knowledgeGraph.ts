import { findNote, noteLinks } from './knowledgeLinks'
import { noteDisplayName } from './noteReading'

export interface GraphPoint {
  path: string
  title: string
  x: number
  y: number
}

export interface GraphLink {
  from: string
  to: string
}

export interface NoteGraph {
  nodes: GraphPoint[]
  edges: GraphLink[]
}

export interface GraphNote {
  name: string
  path: string
  markdown: string | null
}

/** Which titles to draw. A dense graph stays dots until zoom or hover, so labels do not stack into a ring. */
export function graphShowsLabel(options: { count: number; zoom: number; hovered: boolean; center: boolean }): boolean {
  if (options.hovered || options.center) return true
  if (options.count <= 18) return true
  return options.zoom >= 1.6
}

/** Shorten a title for a dot. The accessible name stays the full title. */
export function graphLabel(title: string, count: number): string {
  const max = count <= 8 ? 16 : count <= 18 ? 10 : 6
  const chars = Array.from(title)
  if (chars.length <= max) return title
  return `${chars.slice(0, max - 1).join('')}…`
}

export function graphLabelSize(count: number): number {
  if (count <= 8) return 3
  if (count <= 18) return 2.2
  return 1.8
}

/**
 * Local link graph.
 * With a center note, only that note and its resolved neighbors are placed.
 * Otherwise unread notes and orphans stay off the drawing unless orphans are requested.
 * Positions come from a fixed simulation so titles are not laid around one circle.
 */
export function noteGraph(
  notes: readonly GraphNote[],
  options?: { centerPath?: string | null; orphans?: boolean }
): NoteGraph {
  const edges: GraphLink[] = []
  const seen = new Set<string>()
  for (const note of notes) {
    if (note.markdown == null) continue
    for (const link of noteLinks(note.markdown)) {
      const target = findNote(notes, link.target)
      if (!target || target.path === note.path) continue
      const key = `${note.path}\0${target.path}`
      if (seen.has(key)) continue
      seen.add(key)
      edges.push({ from: note.path, to: target.path })
    }
  }
  const linked = new Set<string>()
  for (const edge of edges) {
    linked.add(edge.from)
    linked.add(edge.to)
  }
  const centerPath = options?.centerPath || null
  const orphans = options?.orphans ?? false
  const selected = notes.filter((note) => {
    if (centerPath) {
      if (note.path === centerPath) return true
      return edges.some(
        (edge) =>
          (edge.from === centerPath && edge.to === note.path) || (edge.to === centerPath && edge.from === note.path)
      )
    }
    if (note.markdown == null) return false
    if (linked.has(note.path)) return true
    return orphans
  })
  const selectedPaths = new Set(selected.map((note) => note.path))
  const visibleEdges = edges.filter((edge) => selectedPaths.has(edge.from) && selectedPaths.has(edge.to))
  const placed = layout(
    selected.map((note) => note.path),
    visibleEdges
  )
  return {
    nodes: selected.map((note) => {
      const point = placed.get(note.path) ?? { x: 50, y: 50 }
      return { path: note.path, title: noteDisplayName(note.name, note.markdown), x: point.x, y: point.y }
    }),
    edges: visibleEdges
  }
}

function layout(paths: readonly string[], edges: readonly GraphLink[]): Map<string, { x: number; y: number }> {
  const placed = new Map<string, { x: number; y: number }>()
  const count = paths.length
  if (count === 0) return placed
  if (count === 1) {
    placed.set(paths[0] ?? '', { x: 50, y: 50 })
    return placed
  }
  const golden = Math.PI * (3 - Math.sqrt(5))
  const points = paths.map((_, index) => {
    const radius = 28 * Math.sqrt((index + 0.5) / count)
    const angle = index * golden
    return { x: 50 + radius * Math.cos(angle), y: 50 + radius * Math.sin(angle) }
  })
  const indexOf = new Map(paths.map((path, index) => [path, index]))
  for (let step = 0; step < 40; step += 1) {
    const force = points.map(() => ({ x: 0, y: 0 }))
    for (let i = 0; i < count; i += 1) {
      force[i].x += (50 - points[i].x) * 0.02
      force[i].y += (50 - points[i].y) * 0.02
      for (let j = i + 1; j < count; j += 1) {
        const dx = points[i].x - points[j].x
        const dy = points[i].y - points[j].y
        const dist = Math.hypot(dx, dy) || 0.01
        const push = 42 / (dist * dist)
        force[i].x += (dx / dist) * push
        force[i].y += (dy / dist) * push
        force[j].x -= (dx / dist) * push
        force[j].y -= (dy / dist) * push
      }
    }
    for (const edge of edges) {
      const i = indexOf.get(edge.from)
      const j = indexOf.get(edge.to)
      if (i == null || j == null) continue
      const dx = points[j].x - points[i].x
      const dy = points[j].y - points[i].y
      const dist = Math.hypot(dx, dy) || 0.01
      const pull = (dist - 18) * 0.04
      force[i].x += (dx / dist) * pull
      force[i].y += (dy / dist) * pull
      force[j].x -= (dx / dist) * pull
      force[j].y -= (dy / dist) * pull
    }
    for (let i = 0; i < count; i += 1) {
      points[i].x += Math.max(-1.5, Math.min(1.5, force[i].x))
      points[i].y += Math.max(-1.5, Math.min(1.5, force[i].y))
    }
  }
  let minX = Infinity
  let maxX = -Infinity
  let minY = Infinity
  let maxY = -Infinity
  for (const point of points) {
    minX = Math.min(minX, point.x)
    maxX = Math.max(maxX, point.x)
    minY = Math.min(minY, point.y)
    maxY = Math.max(maxY, point.y)
  }
  const spanX = maxX - minX || 1
  const spanY = maxY - minY || 1
  const round = (value: number) => Math.round(value * 100) / 100
  paths.forEach((path, index) => {
    const point = points[index]
    if (!point) return
    placed.set(path, {
      x: round(12 + ((point.x - minX) / spanX) * 76),
      y: round(12 + ((point.y - minY) / spanY) * 76)
    })
  })
  return placed
}
