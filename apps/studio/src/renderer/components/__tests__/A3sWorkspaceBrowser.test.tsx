// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, render, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { agentBrowserRuntimeService as runtime } from '@renderer/services/AgentBrowserRuntimeService'

import { AgentBrowserRuntimeHost } from '../AgentBrowserRuntimeHost'
import { setNoticeWebviewCover, workspaceBrowserNavigation, workspaceBrowserUrl } from '../workspaceBrowser'

const invoke = vi.hoisted(() => vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>())

vi.mock('@renderer/hooks/tab', () => ({ useTabs: () => ({ tabs: [{ id: 'tab-a' }] }) }))
vi.mock('@renderer/ipc/ipcApi', () => ({
  ipcApi: { on: () => () => {}, request: async () => undefined }
}))

describe('workspace browser on Tauri', () => {
  afterEach(() => {
    cleanup()
    runtime.dispose()
    invoke.mockReset()
    setNoticeWebviewCover(false)
    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  it('accepts only http(s) and file pages', () => {
    expect(workspaceBrowserUrl('https://example.com/docs')).toBe('https://example.com/docs')
    expect(workspaceBrowserUrl('file:///workspace/index.html')).toBe('file:///workspace/index.html')
    expect(workspaceBrowserUrl('about:blank')).toBeUndefined()
    expect(workspaceBrowserUrl('https://user:secret@example.com/')).toBeUndefined()
    expect(workspaceBrowserUrl('javascript:alert(1)')).toBeUndefined()
  })

  it('opens the page inside the pane and hides it when the pane closes', async () => {
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    invoke.mockResolvedValue(undefined)
    const pane = document.createElement('div')
    pane.dataset.rightPaneMode = 'docked'
    const anchor = document.createElement('div')
    pane.append(anchor)
    document.body.append(pane)
    vi.spyOn(anchor, 'getBoundingClientRect').mockReturnValue(new DOMRect(20, 40, 640, 480))
    runtime.declare('session-a', 'tab-a')
    runtime.ensure('session-a', 'https://example.com/docs')
    runtime.update('session-a', { anchor })

    render(<AgentBrowserRuntimeHost />)

    await waitFor(() =>
      expect(anchor.querySelector('[data-workspace-browser="a3s-webview"]')).toHaveAttribute(
        'data-workspace-browser-phase',
        'opened'
      )
    )
    expect(invoke).toHaveBeenCalledOnce()
    expect(invoke).toHaveBeenCalledWith('launch_workspace_browser', {
      sessionId: 'session-a',
      url: 'https://example.com/docs',
      x: 20,
      y: 40,
      width: 640,
      height: 480,
      visible: true,
      reloadKey: 0
    })
    expect(anchor.querySelector('[data-workspace-browser="a3s-webview"]')).toHaveTextContent('')
    expect(document.querySelector('webview')).toBeNull()
    expect(anchor.querySelector('[data-workspace-browser="a3s-webview"]')?.parentElement).toBe(anchor)

    pane.dataset.rightPaneMode = 'closed'
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith(
        'launch_workspace_browser',
        expect.objectContaining({
          sessionId: 'session-a',
          url: 'https://example.com/docs',
          visible: false
        })
      )
    )
    expect(invoke).toHaveBeenCalledTimes(2)
    expect(anchor.querySelector('[data-workspace-browser="a3s-webview"]')).toHaveAttribute(
      'data-workspace-browser-phase',
      'opened'
    )
    expect(document.querySelector('webview')).toBeNull()
    expect(document.body.querySelector('[data-workspace-browser="a3s-webview"]')).toBe(
      anchor.querySelector('[data-workspace-browser="a3s-webview"]')
    )

    pane.remove()
  })

  it('hides the workspace page while a notice dialog is open', async () => {
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    invoke.mockResolvedValue(undefined)
    const pane = document.createElement('div')
    pane.dataset.rightPaneMode = 'docked'
    const anchor = document.createElement('div')
    pane.append(anchor)
    document.body.append(pane)
    vi.spyOn(anchor, 'getBoundingClientRect').mockReturnValue(new DOMRect(20, 40, 640, 480))
    runtime.declare('session-a', 'tab-a')
    runtime.ensure('session-a', 'https://example.com/docs')
    runtime.update('session-a', { anchor })

    render(<AgentBrowserRuntimeHost />)
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1))

    act(() => setNoticeWebviewCover(true))
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith(
        'launch_workspace_browser',
        expect.objectContaining({
          sessionId: 'session-a',
          url: 'https://example.com/docs',
          visible: false
        })
      )
    )

    act(() => setNoticeWebviewCover(false))
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith(
        'launch_workspace_browser',
        expect.objectContaining({
          sessionId: 'session-a',
          url: 'https://example.com/docs',
          visible: true
        })
      )
    )
    pane.remove()
  })

  it('asks the shell to open the next address for the same session', async () => {
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    invoke.mockResolvedValue(undefined)
    const pane = document.createElement('div')
    pane.dataset.rightPaneMode = 'docked'
    const anchor = document.createElement('div')
    pane.append(anchor)
    document.body.append(pane)
    vi.spyOn(anchor, 'getBoundingClientRect').mockReturnValue(new DOMRect(20, 40, 640, 480))
    runtime.declare('session-a', 'tab-a')
    runtime.ensure('session-a', 'https://example.com/docs')
    runtime.update('session-a', { anchor })

    render(<AgentBrowserRuntimeHost />)
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1))
    act(() => {
      runtime.ensure('session-a', 'https://example.com/next')
    })
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith(
        'launch_workspace_browser',
        expect.objectContaining({ sessionId: 'session-a', url: 'https://example.com/next' })
      )
    )
    pane.remove()
  })

  it('keeps the page address when the pane only moves', async () => {
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    invoke.mockResolvedValue(undefined)
    const pane = document.createElement('div')
    pane.dataset.rightPaneMode = 'docked'
    const anchor = document.createElement('div')
    pane.append(anchor)
    document.body.append(pane)
    const rect = vi.spyOn(anchor, 'getBoundingClientRect').mockReturnValue(new DOMRect(20, 40, 640, 480))
    runtime.declare('session-a', 'tab-a')
    runtime.ensure('session-a', 'https://example.com/docs')
    runtime.update('session-a', { anchor })

    render(<AgentBrowserRuntimeHost />)
    await waitFor(() =>
      expect(anchor.querySelector('[data-workspace-browser="a3s-webview"]')).toHaveAttribute(
        'data-workspace-browser-phase',
        'opened'
      )
    )
    act(() => runtime.update('session-a', { url: 'https://example.com/docs/next' }))
    rect.mockReturnValue(new DOMRect(30, 40, 640, 480))
    act(() => window.dispatchEvent(new Event('resize')))
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith(
        'launch_workspace_browser',
        expect.objectContaining({ url: 'https://example.com/docs', x: 30 })
      )
    )
    expect(runtime.get('session-a')?.url).toBe('https://example.com/docs/next')
    expect(runtime.get('session-a')?.sourceUrl).toBe('https://example.com/docs')
    expect(runtime.get('session-a')?.loading).toBe(true)
    pane.remove()
  })

  it('reads embedded history without opening another page', async () => {
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    invoke.mockResolvedValue({ url: 'https://live.test/page', canGoBack: true, canGoForward: false })
    await expect(workspaceBrowserNavigation('back')).resolves.toEqual({
      url: 'https://live.test/page',
      canGoBack: true,
      canGoForward: false,
      loading: false,
      title: null
    })
    expect(invoke).toHaveBeenCalledWith('workspace_browser_navigate', { action: 'back' })
    invoke.mockResolvedValue(undefined)
    await expect(workspaceBrowserNavigation('state')).resolves.toBeNull()
  })

  it('moves the page down while the address menu covers it', async () => {
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    invoke.mockResolvedValue(undefined)
    const pane = document.createElement('div')
    pane.dataset.rightPaneMode = 'docked'
    const anchor = document.createElement('div')
    pane.append(anchor)
    document.body.append(pane)
    vi.spyOn(anchor, 'getBoundingClientRect').mockReturnValue(new DOMRect(20, 40, 640, 480))
    runtime.declare('session-a', 'tab-a')
    runtime.ensure('session-a', 'https://example.com/docs')
    runtime.update('session-a', { anchor })

    render(<AgentBrowserRuntimeHost />)
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1))
    act(() => runtime.update('session-a', { overlayInset: 100 }))
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith(
        'launch_workspace_browser',
        expect.objectContaining({ url: 'https://example.com/docs', y: 140, height: 380 })
      )
    )
    expect(runtime.get('session-a')?.sourceUrl).toBe('https://example.com/docs')
    pane.remove()
  })

  it('does not launch while the workspace pane is collapsed', async () => {
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    invoke.mockResolvedValue(undefined)
    const pane = document.createElement('div')
    pane.dataset.rightPaneMode = 'closed'
    pane.style.opacity = '0'
    const anchor = document.createElement('div')
    pane.append(anchor)
    document.body.append(pane)
    vi.spyOn(anchor, 'getBoundingClientRect').mockReturnValue(new DOMRect(20, 40, 640, 480))
    runtime.declare('session-a', 'tab-a')
    runtime.ensure('session-a', 'https://example.com/docs')
    runtime.update('session-a', { anchor })

    render(<AgentBrowserRuntimeHost />)
    await waitFor(() => expect(anchor.querySelector('[data-workspace-browser="a3s-webview"]')).not.toBeNull())
    expect(invoke).not.toHaveBeenCalled()
    expect(document.querySelector('webview')).toBeNull()
    pane.remove()
  })

  it('reports a failed embed instead of staying on the loading overlay', async () => {
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    invoke.mockRejectedValue(new Error('a3s-webview is not installed on PATH.'))
    const pane = document.createElement('div')
    pane.dataset.rightPaneMode = 'docked'
    const anchor = document.createElement('div')
    pane.append(anchor)
    document.body.append(pane)
    vi.spyOn(anchor, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 0, 320, 240))
    runtime.declare('session-a', 'tab-a')
    runtime.ensure('session-a', 'https://example.com/')
    runtime.update('session-a', { anchor })

    render(<AgentBrowserRuntimeHost />)
    await waitFor(() =>
      expect(anchor.querySelector('[data-workspace-browser="a3s-webview"]')).toHaveAttribute(
        'data-workspace-browser-phase',
        'unavailable'
      )
    )
    expect(anchor.querySelector('[data-workspace-browser="a3s-webview"]')).toHaveAttribute('role', 'alert')
    pane.remove()
  })
})
