import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { beginOptimistic, useOptimisticRows } from '../optimisticRows'

type Row = { id: string; name: string }
const idOf = (row: Row) => row.id
const rows: Row[] = [
  { id: 'a', name: 'Alpha' },
  { id: 'b', name: 'Beta' },
  { id: 'c', name: 'Gamma' }
]
let collection = ''
let serial = 0

beforeEach(() => {
  vi.useFakeTimers()
  collection = `test/${serial++}`
})
afterEach(() => {
  vi.useRealTimers()
})

describe('optimistic collection rows', () => {
  // Catches a removal that waits for the owner, skips its exit motion, or leaves the total unchanged.
  it('plays the exit, then hides a removed row until release', () => {
    const { result } = renderHook(() => useOptimisticRows(collection, rows, idOf))
    let change!: ReturnType<typeof beginOptimistic>
    act(() => {
      change = beginOptimistic<Row>(collection, { remove: ['b'] })
    })
    expect(result.current.rows.map(idOf)).toEqual(['a', 'b', 'c'])
    expect(result.current.motion('b')).toBe('leaving')
    expect(result.current.removed).toBe(1)
    act(() => {
      vi.advanceTimersByTime(200)
    })
    expect(result.current.rows.map(idOf)).toEqual(['a', 'c'])
    act(() => change.confirm())
    expect(result.current.rows.map(idOf)).toEqual(['a', 'c'])
    act(() => change.release())
    expect(result.current.rows.map(idOf)).toEqual(['a', 'b', 'c'])
  })

  // Catches a failed write that keeps the reader's change or brings the row back without a signal.
  it('restores every row of a rejected change with a restored motion', () => {
    const { result } = renderHook(() => useOptimisticRows(collection, rows, idOf))
    act(() => {
      const change = beginOptimistic<Row>(collection, {
        remove: ['a'],
        patch: { c: (row) => ({ ...row, name: 'Renamed' }) }
      })
      vi.advanceTimersByTime(200)
      change.rollback()
    })
    expect(result.current.rows).toEqual(rows)
    expect(result.current.motion('a')).toBe('restored')
    expect(result.current.motion('c')).toBe('restored')
    act(() => {
      vi.advanceTimersByTime(900)
    })
    expect(result.current.motion('a')).toBeUndefined()
  })

  // Catches a confirmed edit snapping back to old values before the list has reconciled.
  it('keeps a confirmed patch until release and flashes it as updated', () => {
    const { result } = renderHook(() => useOptimisticRows(collection, rows, idOf))
    let change!: ReturnType<typeof beginOptimistic>
    act(() => {
      change = beginOptimistic<Row>(collection, { patch: { a: (row) => ({ ...row, name: 'Renamed' }) } })
    })
    expect(result.current.rows[0]).toEqual({ id: 'a', name: 'Renamed' })
    expect(result.current.motion('a')).toBe('pending')
    act(() => change.confirm())
    expect(result.current.motion('a')).toBe('updated')
    act(() => change.release())
    expect(result.current.rows[0]).toEqual({ id: 'a', name: 'Alpha' })
    expect(result.current.motion('a')).toBe('updated')
    act(() => {
      vi.advanceTimersByTime(900)
    })
    expect(result.current.motion('a')).toBeUndefined()
  })

  // Catches an older write's rollback undoing a newer change to the same row.
  it('lets a later change to the same row supersede an earlier one', () => {
    const { result } = renderHook(() => useOptimisticRows(collection, rows, idOf))
    act(() => {
      const first = beginOptimistic<Row>(collection, { patch: { a: (row) => ({ ...row, name: 'First' }) } })
      beginOptimistic<Row>(collection, { patch: { a: (row) => ({ ...row, name: 'Second' }) } })
      first.rollback()
    })
    expect(result.current.rows[0].name).toBe('Second')
    expect(result.current.motion('a')).toBe('pending')
  })

  // Catches an overlay pinned forever when the reconcile after a confirmed write never lands.
  it('releases a confirmed change after the reconcile window', () => {
    const { result } = renderHook(() => useOptimisticRows(collection, rows, idOf))
    act(() => {
      beginOptimistic<Row>(collection, { remove: ['a'] }).confirm()
      vi.advanceTimersByTime(8000)
    })
    expect(result.current.rows.map(idOf)).toEqual(['a', 'b', 'c'])
  })
})
