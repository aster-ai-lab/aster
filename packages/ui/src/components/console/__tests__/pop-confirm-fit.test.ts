import { describe, expect, it } from 'vitest'

import { fitConfirmPanel } from '../PopConfirm'

const viewport = { width: 1440, height: 900 }

describe('fitConfirmPanel', () => {
  it('shrinks a confirm panel so it stays inside a narrow sidebar', () => {
    const placed = fitConfirmPanel(
      { top: 400, bottom: 424, left: 280, right: 304, width: 24 },
      { width: 296, height: 120 },
      { left: 62, right: 322, top: 0, bottom: 900 },
      viewport
    )

    expect(placed.width).toBe(244)
    expect(placed.left).toBe(70)
    expect(placed.left + placed.width).toBe(314)
    expect(placed.placement).toBe('below')
    expect(placed.top).toBe(434)
  })

  it('keeps the natural width when no boundary is given', () => {
    const placed = fitConfirmPanel(
      { top: 100, bottom: 124, left: 16, right: 40, width: 24 },
      { width: 296, height: 120 },
      null,
      viewport
    )

    expect(placed.width).toBe(296)
    expect(placed.left).toBe(8)
  })

  it('leaves a zero measurement untouched so callers can keep the 18.5rem fallback', () => {
    const placed = fitConfirmPanel(
      { top: 100, bottom: 124, left: 280, right: 304, width: 24 },
      { width: 0, height: 0 },
      { left: 62, right: 322, top: 0, bottom: 900 },
      viewport
    )

    expect(placed.width).toBe(0)
  })
})
