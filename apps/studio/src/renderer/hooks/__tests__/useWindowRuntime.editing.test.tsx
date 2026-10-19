import { cleanup, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const ipcState = vi.hoisted(() => {
  const state = { fullscreen: false }
  const request = vi.fn(async (route: string) => {
    if (route === 'window.is_full_screen') return state.fullscreen
    if (route === 'app.get_info') return { homePath: '/tmp', resourcesPath: '/tmp' }
    return undefined
  })
  return { state, request }
})

vi.mock('@renderer/ipc', () => ({
  ipcApi: { request: (route: string, input?: unknown) => ipcState.request(route, input) },
  useIpcOn: () => undefined
}))

const { useWindowRuntime } = await import('../useWindowRuntime')

describe('useWindowRuntime editing and fullscreen', () => {
  beforeEach(() => {
    ipcState.state.fullscreen = false
    ipcState.request.mockClear()
    window.root = document.createElement('div')
    document.body.innerHTML = ''
  })

  afterEach(() => {
    cleanup()
  })

  it('installs the editing guard for the chat textarea', () => {
    renderHook(() => useWindowRuntime())
    const textarea = document.createElement('textarea')
    textarea.value = 'hello\nworld'
    document.body.appendChild(textarea)
    textarea.setSelectionRange(8, 8)

    textarea.dispatchEvent(new KeyboardEvent('keydown', { key: 'Home', bubbles: true, cancelable: true }))

    expect(textarea.selectionStart).toBe(6)
    expect(textarea.value).toBe('hello\nworld')
  })

  it('asks whether the window is fullscreen before changing it', async () => {
    renderHook(() => useWindowRuntime())
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))

    await vi.waitFor(() => expect(ipcState.request).toHaveBeenCalledWith('window.is_full_screen', undefined))
    expect(ipcState.request).not.toHaveBeenCalledWith('window.set_full_screen', false)

    ipcState.state.fullscreen = true
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await vi.waitFor(() => expect(ipcState.request).toHaveBeenCalledWith('window.set_full_screen', false))
  })

  it('does not query fullscreen while Settings is open', () => {
    const overlay = document.createElement('div')
    overlay.dataset.ui = 'settings.overlay'
    overlay.dataset.state = 'open'
    document.body.appendChild(overlay)
    renderHook(() => useWindowRuntime())

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    const routes = ipcState.request.mock.calls.map((call) => call[0])
    expect(routes).not.toContain('window.is_full_screen')
    expect(routes).not.toContain('window.set_full_screen')
  })
})
