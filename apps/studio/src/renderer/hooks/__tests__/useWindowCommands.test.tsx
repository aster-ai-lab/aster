import { mockPreferenceState } from '@test-mocks/renderer/PreferenceService'
import { cleanup, render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CommandContextKeyProvider } from '@renderer/components/command/CommandContextKeyProvider'
import { CommandProvider } from '@renderer/components/command/CommandProvider'
import { nextZoomFactor, WindowCommandBindings } from '@renderer/hooks/useWindowCommands'
import { ipcApi } from '@renderer/ipc'
import { setSettingsSurfaceActive } from '@renderer/pages/settings/settingsSurface'
import { toast } from '@renderer/services/toast'

const capture = vi.hoisted(() => vi.fn())
const deliver = vi.hoisted(() => vi.fn(async () => undefined))
const openSettings = vi.hoisted(() => vi.fn(async () => undefined))

vi.mock('@renderer/ipc', () => ({
  ipcApi: { request: vi.fn(async () => undefined) },
  useIpcOn: () => undefined
}))

vi.mock('@renderer/components/composer/screenshot/screenshotCapture', () => ({
  captureInteractiveScreenshot: capture,
  deliverCapturedScreenshot: deliver,
  isScreenCapturePermissionError: (error: unknown) =>
    String(error instanceof Error ? error.message : error).includes('screen-capture-permission-required'),
  openScreenCaptureSettings: openSettings
}))

function dispatch(init: KeyboardEventInit): void {
  window.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }))
}

function renderBindings() {
  return render(
    <CommandContextKeyProvider>
      <CommandProvider>
        <WindowCommandBindings />
      </CommandProvider>
    </CommandContextKeyProvider>
  )
}

describe('useWindowCommands', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockPreferenceState.clear()
    setSettingsSurfaceActive(false)
    vi.spyOn(window, 'print').mockImplementation(() => undefined)
  })

  afterEach(() => {
    setSettingsSurfaceActive(false)
    cleanup()
  })

  it('steps zoom by 0.2 and applies the stored factor once', () => {
    expect(nextZoomFactor(1, 1)).toBe(1.2)
    expect(nextZoomFactor(0.2, -1)).toBe(0.2)
    expect(nextZoomFactor(10, 1)).toBe(10)
    expect(nextZoomFactor(1.16, 1)).toBe(1.4)

    const view = renderBindings()
    expect(ipcApi.request).not.toHaveBeenCalled()

    dispatch({ key: '=', code: 'Equal', ctrlKey: true })
    expect(mockPreferenceState.get('app.zoom_factor')).toBe(1.2)

    view.rerender(
      <CommandContextKeyProvider>
        <CommandProvider>
          <WindowCommandBindings />
        </CommandProvider>
      </CommandContextKeyProvider>
    )
    expect(ipcApi.request).toHaveBeenCalledWith('window.set_zoom', { factor: 1.2 })
  })

  it('closes the window and zooms while Settings is open', () => {
    setSettingsSurfaceActive(true)
    renderBindings()

    dispatch({ key: 'w', code: 'KeyW', ctrlKey: true, shiftKey: true })
    expect(ipcApi.request).toHaveBeenCalledWith('window.close')

    dispatch({ key: '=', code: 'Equal', ctrlKey: true })
    expect(mockPreferenceState.get('app.zoom_factor')).toBe(1.2)
  })

  it('prints from the window fallback', () => {
    renderBindings()
    dispatch({ key: 'p', code: 'KeyP', ctrlKey: true })
    expect(window.print).toHaveBeenCalledOnce()
  })

  it('captures a screenshot only after the feature is on, and opens settings when permission is missing', async () => {
    renderBindings()
    dispatch({ key: 'a', code: 'KeyA', ctrlKey: true, shiftKey: true })
    expect(capture).not.toHaveBeenCalled()

    cleanup()
    mockPreferenceState.set('feature.screenshot.enabled', true)
    capture.mockResolvedValueOnce(null)
    renderBindings()
    dispatch({ key: 'a', code: 'KeyA', ctrlKey: true, shiftKey: true })
    await vi.waitFor(() => expect(capture).toHaveBeenCalledWith('direct'))
    expect(deliver).not.toHaveBeenCalled()

    capture.mockRejectedValueOnce(new Error('screen-capture-permission-required'))
    dispatch({ key: 'a', code: 'KeyA', ctrlKey: true, shiftKey: true })
    await vi.waitFor(() => expect(openSettings).toHaveBeenCalledOnce())
    expect(toast.error).toHaveBeenCalledWith('截图失败。请在系统设置中允许录制屏幕。')
  })
})
