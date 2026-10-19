import { afterEach, describe, expect, it, vi } from 'vitest'

import { openWorkbenchAppletWindow } from '../openWorkbenchAppletWindow'

function setInvoke(invoke: ((command: string, args?: Record<string, unknown>) => Promise<unknown>) | undefined) {
  Object.assign(globalThis, { __TAURI_INTERNALS__: invoke ? { invoke } : undefined })
}

describe('openWorkbenchAppletWindow', () => {
  afterEach(() => {
    setInvoke(undefined)
  })

  it('asks the shell to open the system window', async () => {
    const invoke = vi.fn(() => Promise.resolve(null))
    setInvoke(invoke)
    expect(await openWorkbenchAppletWindow('leave', '请假')).toBe(true)
    expect(invoke).toHaveBeenCalledWith('open_workbench_applet_window', {
      appletId: 'leave',
      title: '请假',
      channel: 'release'
    })
  })

  it('passes the package window and permissions to the shell', async () => {
    const invoke = vi.fn(() => Promise.resolve(null))
    setInvoke(invoke)
    expect(
      await openWorkbenchAppletWindow('leave', '请假', {
        window: { defaultWidth: 480, defaultHeight: 720, maxWidth: 840, maxHeight: 960 },
        permissions: ['user']
      })
    ).toBe(true)
    expect(invoke).toHaveBeenCalledWith('open_workbench_applet_window', {
      appletId: 'leave',
      title: '请假',
      channel: 'release',
      defaultWidth: 480,
      defaultHeight: 720,
      maxWidth: 840,
      maxHeight: 960,
      permissions: ['user']
    })
  })

  it('stays quiet outside the shell', async () => {
    setInvoke(undefined)
    expect(await openWorkbenchAppletWindow('leave', '请假')).toBe(false)
  })
})
