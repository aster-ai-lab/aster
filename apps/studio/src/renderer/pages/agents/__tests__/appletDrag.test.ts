import { describe, expect, it } from 'vitest'

import { appletDragShift, appletDragTarget, appletDragVisualSlot } from '../appletDrag'

const centers = [
  { x: 28, y: 46 },
  { x: 116, y: 46 },
  { x: 204, y: 46 }
]

const rects = [
  { left: 0, top: 10 },
  { left: 88, top: 10 },
  { left: 176, top: 10 }
]

describe('applet drag geometry', () => {
  it('picks the nearest resting center and keeps the earlier slot on a tie', () => {
    expect(appletDragTarget({ x: 28, y: 46 }, centers)).toBe(0)
    expect(appletDragTarget({ x: 200, y: 46 }, centers)).toBe(2)
    expect(appletDragTarget({ x: 72, y: 46 }, centers)).toBe(0)
  })

  it('opens one gap at the pointer and shifts only the items between', () => {
    expect(appletDragVisualSlot(2, 2, 0)).toBe(0)
    expect(appletDragVisualSlot(0, 2, 0)).toBe(1)
    expect(appletDragVisualSlot(1, 2, 0)).toBe(2)
    expect(appletDragShift(2, 2, 0, rects)).toEqual({ x: -176, y: 0 })
    expect(appletDragShift(0, 2, 0, rects)).toEqual({ x: 88, y: 0 })
    expect(appletDragShift(1, 1, 1, rects)).toEqual({ x: 0, y: 0 })
    expect(appletDragShift(0, 0, 1, [])).toEqual({ x: 0, y: 0 })
  })
})
