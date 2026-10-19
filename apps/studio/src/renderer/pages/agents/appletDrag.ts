/** Resting-slot geometry for an icon drag. The list order changes on drop. */

export type AppletDragPoint = { x: number; y: number }

export type AppletDragRect = { left: number; top: number }

/** Index of the resting center closest to the pointer. A tie keeps the earlier slot. */
export function appletDragTarget(pointer: AppletDragPoint, centers: readonly AppletDragPoint[]): number {
  let best = 0
  let bestDistance = Number.POSITIVE_INFINITY
  for (let index = 0; index < centers.length; index += 1) {
    const dx = pointer.x - centers[index].x
    const dy = pointer.y - centers[index].y
    const distance = dx * dx + dy * dy
    if (distance < bestDistance) {
      bestDistance = distance
      best = index
    }
  }
  return best
}

/** Slot the item at `index` shows while the item at `from` is held over `to`. */
export function appletDragVisualSlot(index: number, from: number, to: number): number {
  if (index === from) return to
  if (from < to && index > from && index <= to) return index - 1
  if (to < from && index >= to && index < from) return index + 1
  return index
}

export function appletDragShift(
  index: number,
  from: number,
  to: number,
  rects: readonly AppletDragRect[]
): AppletDragPoint {
  const origin = rects[index]
  const dest = rects[appletDragVisualSlot(index, from, to)]
  if (!origin || !dest) return { x: 0, y: 0 }
  return { x: dest.left - origin.left, y: dest.top - origin.top }
}
