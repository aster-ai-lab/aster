import { beforeEach, describe, expect, it } from 'vitest'

import {
  defaultHomeShelfLayout,
  readHomeShelfLayout,
  settleHomeShelfLayout,
  writeHomeShelfLayout
} from '../homeShelfLayout'

describe('home shelf layout', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('places the three shelves on one row', () => {
    expect(defaultHomeShelfLayout(['write', 'create', 'process'])).toEqual([
      { i: 'write', x: 0, y: 0, w: 4, h: 10 },
      { i: 'create', x: 4, y: 0, w: 4, h: 10 },
      { i: 'process', x: 8, y: 0, w: 4, h: 10 }
    ])
  })

  it('gives each plugin four columns and stacks a shelf that was left below', () => {
    expect(defaultHomeShelfLayout(['write', 'create'])).toEqual([
      { i: 'write', x: 0, y: 0, w: 4, h: 10 },
      { i: 'create', x: 4, y: 0, w: 4, h: 10 }
    ])
    const settled = settleHomeShelfLayout(
      [
        { i: 'write', x: 0, y: 0, w: 6, h: 10 },
        { i: 'create', x: 6, y: 30, w: 6, h: 10 }
      ],
      'write'
    )
    expect(settled.find((item) => item.i === 'create')?.y).toBe(0)
  })

  it('exchanges places when one shelf is dropped onto the other', () => {
    const settled = settleHomeShelfLayout(
      [
        { i: 'write', x: 6, y: 0, w: 6, h: 10 },
        { i: 'create', x: 6, y: 0, w: 6, h: 10 }
      ],
      'write',
      { i: 'write', x: 0, y: 0, w: 6, h: 10 }
    )
    expect(settled.find((item) => item.i === 'write')).toMatchObject({ x: 6, y: 0 })
    expect(settled.find((item) => item.i === 'create')).toMatchObject({ x: 0, y: 0 })
  })

  it('remembers a custom arrangement and drops a covered shelf below the one that moved', () => {
    writeHomeShelfLayout([
      { i: 'write', x: 4, y: 0, w: 4, h: 8 },
      { i: 'create', x: 4, y: 0, w: 4, h: 8 }
    ])
    const settled = settleHomeShelfLayout(readHomeShelfLayout(['write', 'create']), 'write')
    expect(settled.find((item) => item.i === 'create')?.y).toBe(8)
    writeHomeShelfLayout(settled)
    expect(readHomeShelfLayout(['write', 'create']).find((item) => item.i === 'write')).toMatchObject({ x: 4, y: 0 })
  })
})
