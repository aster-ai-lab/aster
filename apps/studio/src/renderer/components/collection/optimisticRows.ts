import { ROW_LEAVE_MS, type RowMotion } from '@a3s-lab/ui'
import { useMemo, useSyncExternalStore } from 'react'

type Change = {
  readonly token: symbol
  /** Hidden from the list (after the leave motion) until released or rolled back. */
  readonly remove?: boolean
  readonly patch?: (row: never) => unknown
  readonly motion?: RowMotion
}
type Changes = ReadonlyMap<string, Change>

/** Rows the reader just removed or changed, keyed by exact record ID. */
export type OptimisticChange<T> = {
  remove?: readonly string[]
  patch?: Readonly<Record<string, (row: T) => T>>
}

export type OptimisticHandle = {
  /** The owner accepted the write: changed rows flash as updated; removed rows stay hidden. */
  confirm: () => void
  /** The owner rejected the write or the outcome is unknown: every row returns and flashes as restored. */
  rollback: () => void
  /** The list now shows owner data that includes the write: stop overlaying it. */
  release: () => void
}

const FLASH_MS = 900
/** A confirmed overlay never outlives this, so a reconcile that never lands cannot pin stale rows. */
const RELEASE_MS = 8000
const EMPTY: Changes = new Map()
const collections = new Map<string, Changes>()
const listeners = new Set<() => void>()

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

function write(
  collection: string,
  ids: readonly string[],
  next: (change: Change | undefined, id: string) => Change | undefined
) {
  if (!ids.length) return
  const current = collections.get(collection) ?? EMPTY
  const draft = new Map(current)
  for (const id of ids) {
    const change = next(current.get(id), id)
    if (change && (change.remove || change.patch || change.motion)) draft.set(id, change)
    else draft.delete(id)
  }
  if (draft.size) collections.set(collection, draft)
  else collections.delete(collection)
  for (const listener of listeners) listener()
}

/**
 * Applies a change to a collection before its owner confirms it. Rows leave or show new values at once;
 * the caller confirms or rolls back with the write's outcome and releases after the list reconciles.
 * A later change to the same row supersedes this one.
 */
export function beginOptimistic<T>(collection: string, change: OptimisticChange<T>): OptimisticHandle {
  const token = Symbol(collection)
  const removed = change.remove ?? []
  const patched = Object.entries(change.patch ?? {})
  const ids = [...removed, ...patched.map(([id]) => id)]
  const own = (next: (change: Change) => Change | undefined) => (current: Change | undefined) =>
    current?.token === token ? next(current) : current
  const later = (ms: number, run: () => void) => void setTimeout(run, ms)
  const patches = new Map(patched)
  write(collection, ids, (_current, id) => {
    const patch = patches.get(id)
    return patch ? { token, patch, motion: 'pending' } : { token, remove: true, motion: 'leaving' }
  })
  later(ROW_LEAVE_MS, () =>
    write(
      collection,
      removed,
      own((current) => (current.motion === 'leaving' ? { ...current, motion: undefined } : current))
    )
  )
  let state: 'pending' | 'confirmed' | 'settled' = 'pending'
  const release = () => {
    if (state === 'settled') return
    state = 'settled'
    write(
      collection,
      ids,
      own((current) => (current.motion === 'updated' ? { token, motion: 'updated' } : undefined))
    )
  }
  return {
    confirm: () => {
      if (state !== 'pending') return
      state = 'confirmed'
      write(
        collection,
        ids,
        own((current) => (current.remove ? current : { ...current, motion: 'updated' }))
      )
      later(FLASH_MS, () =>
        write(
          collection,
          ids,
          own((current) => (current.motion === 'updated' ? { ...current, motion: undefined } : current))
        )
      )
      later(RELEASE_MS, release)
    },
    rollback: () => {
      if (state !== 'pending') return
      state = 'settled'
      write(
        collection,
        ids,
        own(() => ({ token, motion: 'restored' }))
      )
      later(FLASH_MS, () =>
        write(
          collection,
          ids,
          own(() => undefined)
        )
      )
    },
    release
  }
}

type OptimisticRows<T, Rows extends readonly T[]> = {
  rows: Rows
  removed: number
  motion: (id: string) => RowMotion | undefined
}

/**
 * Projects a collection's owner rows through its optimistic changes. `removed` counts owner rows hidden or
 * leaving, so totals can drop with them. Pass a stable `idOf`.
 */
export function useOptimisticRows<T>(collection: string, rows: T[], idOf: (row: T) => string): OptimisticRows<T, T[]>
export function useOptimisticRows<T>(
  collection: string,
  rows: readonly T[],
  idOf: (row: T) => string
): OptimisticRows<T, readonly T[]>
export function useOptimisticRows<T>(
  collection: string,
  rows: readonly T[],
  idOf: (row: T) => string
): OptimisticRows<T, readonly T[]> {
  const changes = useSyncExternalStore(subscribe, () => collections.get(collection) ?? EMPTY)
  return useMemo(() => {
    const motion = (id: string) => changes.get(id)?.motion
    if (!changes.size) return { rows, removed: 0, motion }
    const shown: T[] = []
    let removed = 0
    for (const row of rows) {
      const change = changes.get(idOf(row))
      if (change?.remove) removed += 1
      if (change?.remove && change.motion !== 'leaving') continue
      shown.push(change?.patch ? (change.patch as (row: T) => T)(row) : row)
    }
    return { rows: shown, removed, motion }
  }, [changes, rows, idOf])
}
