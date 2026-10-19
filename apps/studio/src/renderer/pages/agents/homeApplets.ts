/** Home strip order. The default pin list uses the same ids. */
export const WORKBENCH_APPLET_IDS = ['approval', 'leave', 'travel', 'expense', 'meeting', 'seal'] as const

export type WorkbenchAppletId = (typeof WORKBENCH_APPLET_IDS)[number]

export function isWorkbenchAppletId(value: string | null | undefined): value is WorkbenchAppletId {
  return value != null && (WORKBENCH_APPLET_IDS as readonly string[]).includes(value)
}

export const HOME_APPLET_PINS_KEY = 'a3s.workbench.applet-pins.v1'

/** The home row under the composer holds this many pinned mini apps. */
export const HOME_APPLET_PIN_LIMIT = 6

/** Catalog install slug for each homepage mini app. */
export const HOME_APPLET_SLUG: Record<WorkbenchAppletId, string> = {
  approval: 'a3s-approval',
  leave: 'a3s-leave',
  travel: 'a3s-travel',
  expense: 'a3s-expense',
  meeting: 'a3s-meeting',
  seal: 'a3s-seal'
}

type PinStorage = Pick<Storage, 'getItem' | 'setItem'>

/** Missing storage means the default strip. An empty array means the member unpinned every applet. */
export function readHomeAppletPins(storage: PinStorage | null): WorkbenchAppletId[] {
  if (!storage) return [...WORKBENCH_APPLET_IDS]
  const raw = storage.getItem(HOME_APPLET_PINS_KEY)
  if (raw == null) return [...WORKBENCH_APPLET_IDS]
  try {
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return [...WORKBENCH_APPLET_IDS]
    const ids: WorkbenchAppletId[] = []
    for (const item of parsed) {
      if (!isWorkbenchAppletId(item) || ids.includes(item)) continue
      ids.push(item)
      if (ids.length === HOME_APPLET_PIN_LIMIT) break
    }
    return ids
  } catch {
    return [...WORKBENCH_APPLET_IDS]
  }
}

export function writeHomeAppletPins(storage: PinStorage | null, ids: readonly WorkbenchAppletId[]) {
  const next: WorkbenchAppletId[] = []
  for (const id of ids) {
    if (!isWorkbenchAppletId(id) || next.includes(id)) continue
    next.push(id)
    if (next.length === HOME_APPLET_PIN_LIMIT) break
  }
  storage?.setItem(HOME_APPLET_PINS_KEY, JSON.stringify(next))
}

export function toggleHomeAppletPin(current: readonly WorkbenchAppletId[], id: WorkbenchAppletId): WorkbenchAppletId[] {
  if (current.includes(id)) return current.filter((item) => item !== id)
  if (current.length >= HOME_APPLET_PIN_LIMIT) return current as WorkbenchAppletId[]
  return [...current, id]
}

/** Moves `id` into the slot occupied by `target`. The same array means nothing moved. */
export function reorderHomeAppletPins(
  current: readonly WorkbenchAppletId[],
  id: WorkbenchAppletId,
  target: WorkbenchAppletId
): WorkbenchAppletId[] {
  if (id === target || !current.includes(id) || !current.includes(target)) return current as WorkbenchAppletId[]
  const next = [...current]
  const from = next.indexOf(id)
  const to = next.indexOf(target)
  const [item] = next.splice(from, 1)
  next.splice(to, 0, item)
  return next
}

export function moveHomeAppletPin(
  current: readonly WorkbenchAppletId[],
  id: WorkbenchAppletId,
  direction: -1 | 1
): WorkbenchAppletId[] {
  const index = current.indexOf(id)
  const next = index + direction
  if (index < 0 || next < 0 || next >= current.length) return [...current]
  const copy = [...current]
  const [item] = copy.splice(index, 1)
  copy.splice(next, 0, item)
  return copy
}
