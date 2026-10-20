import { describe, expect, it } from 'vitest'

import {
  displayedTaskPermissionMode,
  readTaskPermission,
  readPendingPermissionMode,
  subscribeTaskPermission,
  readTaskPermissionRevision,
  writePendingPermissionMode,
  writeTaskPermission
} from '../workbenchPermissionPostures'

function memory() {
  const values = new Map<string, string>()
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    removeItem: (key: string) => void values.delete(key)
  }
}

describe('task permission', () => {
  it('refreshes an open task control when another window changes its stored mode', () => {
    let called = 0
    const before = readTaskPermissionRevision()
    const unsubscribe = subscribeTaskPermission(() => {
      called += 1
    })
    window.dispatchEvent(new StorageEvent('storage', { key: 'workbench.task.permission:v1:member-a' }))
    expect(called).toBe(1)
    expect(readTaskPermissionRevision()).toBeGreaterThan(before)
    unsubscribe()
  })

  it('stores plan-first independently from the task approval mode', () => {
    const storage = memory()
    writeTaskPermission('task-a', { mode: 'auto', planFirst: true }, storage, 'member-a')
    expect(readTaskPermission('task-a', storage, 'member-a')).toEqual({ mode: 'auto', planFirst: true })
    expect(readTaskPermission('task-b', storage, 'member-a')).toBeNull()
    expect(readTaskPermission('task-a', storage, 'member-b')).toBeNull()
  })

  it('does not promote local approval preferences into a signed-in member', () => {
    const storage = memory()
    writePendingPermissionMode('bypassPermissions', storage, null)
    expect(readPendingPermissionMode(storage, 'member-a')).toBeNull()
    expect(readPendingPermissionMode(storage, 'local')).toBeNull()
    writePendingPermissionMode('invalid-mode', storage, 'member-b')
    expect(readPendingPermissionMode(storage, 'member-b')).toBeNull()
  })

  it('keeps plan on the task that chose it and leaves other tasks on ask-every-time', () => {
    const storage = memory()
    writeTaskPermission('draft', { mode: 'plan' }, storage, 'member-a')
    writeTaskPermission('task-a', { mode: 'auto' }, storage, 'member-a')
    writePendingPermissionMode('bypassPermissions', storage, 'member-a')

    expect(readTaskPermission('draft', storage, 'member-a')).toEqual({ mode: 'plan', planFirst: false })
    expect(readTaskPermission('task-a', storage, 'member-b')).toBeNull()
    expect(displayedTaskPermissionMode('task-a', 'member-a', storage)).toBe('auto')
    expect(displayedTaskPermissionMode('task-b', 'member-a', storage)).toBe('default')
    expect(displayedTaskPermissionMode('draft', 'member-a', storage)).toBe('plan')
    expect(displayedTaskPermissionMode('draft', 'member-b', storage)).toBe('default')
    writeTaskPermission('draft', { mode: 'plan' }, storage, '')
    expect(readTaskPermission('draft', storage, '')).toBeNull()
  })
})
