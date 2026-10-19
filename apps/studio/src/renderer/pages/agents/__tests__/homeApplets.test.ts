import { describe, expect, it } from 'vitest'

import {
  HOME_APPLET_PIN_LIMIT,
  HOME_APPLET_PINS_KEY,
  moveHomeAppletPin,
  readHomeAppletPins,
  reorderHomeAppletPins,
  toggleHomeAppletPin,
  WORKBENCH_APPLET_IDS,
  writeHomeAppletPins,
  type WorkbenchAppletId
} from '../homeApplets'

function memory() {
  const values = new Map<string, string>()
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value)
    }
  }
}

describe('home applet pins', () => {
  it('starts with the office strip and keeps a custom order', () => {
    const storage = memory()
    expect(readHomeAppletPins(storage)[0]).toBe('approval')
    expect(readHomeAppletPins(storage)).toHaveLength(6)
    const unpinned = toggleHomeAppletPin(readHomeAppletPins(storage), 'approval')
    expect(unpinned[0]).toBe('leave')
    const moved = moveHomeAppletPin(unpinned, 'leave', 1)
    writeHomeAppletPins(storage, moved)
    expect(storage.getItem(HOME_APPLET_PINS_KEY)).toContain('leave')
    expect(readHomeAppletPins(storage)).toEqual(moved)
    writeHomeAppletPins(storage, [])
    expect(readHomeAppletPins(storage)).toEqual([])
  })

  it('keeps the home row at six pins and reorders by dropping on another pin', () => {
    const full = [...WORKBENCH_APPLET_IDS]
    expect(full).toHaveLength(HOME_APPLET_PIN_LIMIT)
    expect(toggleHomeAppletPin(full, 'pdf' as WorkbenchAppletId)).toBe(full)
    const six = toggleHomeAppletPin(full, 'seal')
    expect(toggleHomeAppletPin(six, 'seal')).toEqual([...six, 'seal'])
    const moved = reorderHomeAppletPins(full, 'seal', 'approval')
    expect(moved[0]).toBe('seal')
    expect(moved[1]).toBe('approval')
    expect(reorderHomeAppletPins(full, 'approval', 'approval')).toBe(full)
  })
})
