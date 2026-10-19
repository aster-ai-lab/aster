export const HOME_SHELF_COLUMNS = 12
export const HOME_SHELF_ROW_PX = 32
const STORAGE_KEY = 'workbench.home.shelf-layout:v1'

const SHELF_SLUG = /^[a-z][a-z0-9-]{0,63}$/

/** One pinned plugin. The id is the install slug. */
export type HomeShelfId = string

export type HomeShelfPlacement = {
  i: HomeShelfId
  x: number
  y: number
  w: number
  h: number
}

const MIN_W = 3
const MIN_H = 6
const DEFAULT_H = 10

export function hasSavedHomeShelfLayout() {
  try {
    return localStorage.getItem(STORAGE_KEY) != null
  } catch {
    return false
  }
}

export function defaultHomeShelfLayout(ids: readonly HomeShelfId[]): HomeShelfPlacement[] {
  const width = 4
  return ids.map((id, index) => ({
    i: id,
    x: (index % 3) * width,
    y: Math.floor(index / 3) * DEFAULT_H,
    w: width,
    h: DEFAULT_H
  }))
}

export function stackedHomeShelfLayout(ids: readonly HomeShelfId[]): HomeShelfPlacement[] {
  return ids.map((id, index) => ({
    i: id,
    x: 0,
    y: index * DEFAULT_H,
    w: HOME_SHELF_COLUMNS,
    h: DEFAULT_H
  }))
}

function placementOk(value: unknown): value is HomeShelfPlacement {
  if (!value || typeof value !== 'object') return false
  const row = value as Record<string, unknown>
  return (
    typeof row.i === 'string' &&
    SHELF_SLUG.test(row.i) &&
    typeof row.x === 'number' &&
    typeof row.y === 'number' &&
    typeof row.w === 'number' &&
    typeof row.h === 'number'
  )
}

function finite(value: number, fallback: number) {
  return Number.isFinite(value) ? value : fallback
}

function clamp(item: HomeShelfPlacement): HomeShelfPlacement {
  const w = Math.min(HOME_SHELF_COLUMNS, Math.max(MIN_W, Math.round(finite(item.w, 4))))
  const h = Math.max(MIN_H, Math.round(finite(item.h, 8)))
  const x = Math.min(HOME_SHELF_COLUMNS - w, Math.max(0, Math.round(finite(item.x, 0))))
  const y = Math.max(0, Math.round(finite(item.y, 0)))
  return { i: item.i, x, y, w, h }
}

export function readHomeShelfLayout(ids: readonly HomeShelfId[]): HomeShelfPlacement[] {
  const fallback = defaultHomeShelfLayout(ids)
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    const saved = raw ? (JSON.parse(raw) as unknown) : []
    if (!Array.isArray(saved)) return fallback
    const merged = ids.map((id, index) => {
      const found = saved.find((item) => placementOk(item) && item.i === id)
      return found ? clamp(found) : fallback[index]
    })
    return hasOverlap(merged) ? settleHomeShelfLayout(merged, merged[0].i) : merged
  } catch {
    return fallback
  }
}

export function writeHomeShelfLayout(layout: readonly HomeShelfPlacement[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(layout.map(clamp)))
  } catch {
    // Private browsing and a full disk both refuse the write.
  }
}

function overlaps(left: HomeShelfPlacement, right: HomeShelfPlacement) {
  if (left.i === right.i) return false
  return (
    left.x < right.x + right.w && left.x + left.w > right.x && left.y < right.y + right.h && left.y + left.h > right.y
  )
}

function hasOverlap(layout: readonly HomeShelfPlacement[]) {
  return layout.some((item, index) => layout.slice(index + 1).some((other) => overlaps(item, other)))
}

/**
 * Keep the shelf the user is holding. A covered shelf takes the vacated cell when it fits,
 * otherwise it moves below. Every other shelf then rises into the first open row.
 */
export function settleHomeShelfLayout(
  layout: readonly HomeShelfPlacement[],
  moved: HomeShelfId,
  previous?: HomeShelfPlacement
): HomeShelfPlacement[] {
  const next = layout.map((item) => clamp(item))
  const actor = next.find((item) => item.i === moved)
  if (!actor) return next
  const vacated = previous ? clamp(previous) : null
  for (const other of next) {
    if (other.i === moved || !overlaps(actor, other)) continue
    if (vacated) {
      const candidate = clamp({ ...other, x: vacated.x, y: vacated.y })
      const blocked =
        overlaps(candidate, actor) ||
        next.some((item) => item.i !== other.i && item.i !== moved && overlaps(candidate, item))
      if (!blocked) {
        other.x = candidate.x
        other.y = candidate.y
        continue
      }
    }
    other.y = actor.y + actor.h
  }
  const placed: HomeShelfPlacement[] = [actor]
  const others = next.filter((item) => item.i !== moved).sort((left, right) => left.y - right.y || left.x - right.x)
  for (const item of others) {
    let y = item.y
    while (y > 0 && !placed.some((existing) => overlaps({ ...item, y: y - 1 }, existing))) y -= 1
    const raised = { ...item, y }
    placed.push(raised)
  }
  return next.map((item) => placed.find((raised) => raised.i === item.i) ?? item)
}
