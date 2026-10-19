import { describe, expect, it } from 'vitest'

import {
  displayedTaskPermissionMode,
  readTaskPermission,
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
  it('keeps plan on the task that chose it and leaves other tasks on ask-every-time', () => {
    const storage = memory()
    writeTaskPermission('draft', { mode: 'plan' }, storage, 'member-a')
    writeTaskPermission('task-a', { mode: 'auto' }, storage, 'member-a')
    writePendingPermissionMode('bypassPermissions', storage, 'member-a')

    expect(readTaskPermission('draft', storage, 'member-a')).toEqual({ mode: 'plan' })
    expect(readTaskPermission('task-a', storage, 'member-b')).toBeNull()
    expect(displayedTaskPermissionMode('task-a', 'member-a', storage)).toBe('auto')
    expect(displayedTaskPermissionMode('task-b', 'member-a', storage)).toBe('default')
    expect(displayedTaskPermissionMode('draft', 'member-a', storage)).toBe('plan')
    expect(displayedTaskPermissionMode('draft', 'member-b', storage)).toBe('default')
    writeTaskPermission('draft', { mode: 'plan' }, storage, '')
    expect(readTaskPermission('draft', storage, '')).toBeNull()
  })
})
