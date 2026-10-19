import { afterEach, describe, expect, it, vi } from 'vitest'

import { currentAppletWindowLabel, runAppletWindowAction } from '../workbenchAppletWindowBridge'

type Internals = {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
  metadata?: { currentWindow?: { label?: string } }
}

function setInternals(value: Internals | undefined) {
  Object.assign(globalThis, { __TAURI_INTERNALS__: value })
}

describe('workbenchAppletWindowBridge', () => {
  afterEach(() => {
    setInternals(undefined)
  })

  it('ignores the main shell window', async () => {
    const invoke = vi.fn()
    setInternals({ invoke, metadata: { currentWindow: { label: 'main' } } })
    expect(currentAppletWindowLabel()).toBeNull()
    expect(await runAppletWindowAction('close')).toBe(false)
    expect(invoke).not.toHaveBeenCalled()
  })

  it('sends native window commands for an applet window', async () => {
    const invoke = vi.fn(() => Promise.resolve(null))
    setInternals({ invoke, metadata: { currentWindow: { label: 'applet-leave' } } })
    expect(currentAppletWindowLabel()).toBe('applet-leave')
    expect(await runAppletWindowAction('minimize')).toBe(true)
    expect(invoke).toHaveBeenCalledWith('plugin:window|minimize', { label: 'applet-leave' })
  })

  it('sends native window commands for an agent encyclopedia window', async () => {
    const invoke = vi.fn(() => Promise.resolve(null))
    setInternals({ invoke, metadata: { currentWindow: { label: 'agent-wiki-0123456789abcdef' } } })
    expect(currentAppletWindowLabel()).toBe('agent-wiki-0123456789abcdef')
    expect(await runAppletWindowAction('close')).toBe(true)
    expect(invoke).toHaveBeenCalledWith('plugin:window|close', { label: 'agent-wiki-0123456789abcdef' })
  })

  it('sends native window commands for an AIX mini app window', async () => {
    const invoke = vi.fn(() => Promise.resolve(null))
    setInternals({ invoke, metadata: { currentWindow: { label: 'aix-applet-0123456789abcdef' } } })
    expect(currentAppletWindowLabel()).toBe('aix-applet-0123456789abcdef')
    expect(await runAppletWindowAction('toggle-maximize')).toBe(true)
    expect(invoke).toHaveBeenCalledWith('plugin:window|toggle_maximize', { label: 'aix-applet-0123456789abcdef' })
  })
})
