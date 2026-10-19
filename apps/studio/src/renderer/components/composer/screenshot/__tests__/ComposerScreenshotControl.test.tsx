import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { toast } from '@renderer/services/toast'

import { ComposerScreenshotControl } from '../ComposerScreenshotControl'
import { SCREEN_CAPTURE_PERMISSION_REQUIRED } from '../screenshotCapture'

const capture = vi.hoisted(() => vi.fn())
const request = vi.hoisted(() => vi.fn())

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'zh-CN' }
  })
}))

vi.mock('@renderer/services/toast', () => ({
  toast: { info: vi.fn(), error: vi.fn() }
}))

vi.mock('@renderer/ipc', () => ({
  ipcApi: { request }
}))

vi.mock('../screenshotCapture', async () => {
  const actual = await vi.importActual<typeof import('../screenshotCapture')>('../screenshotCapture')
  return {
    ...actual,
    captureInteractiveScreenshot: capture,
    attachScreenshotFile: vi.fn(),
    subscribeWorkbenchScreenshots: () => () => undefined
  }
})

describe('ComposerScreenshotControl', () => {
  beforeEach(() => {
    capture.mockReset()
    request.mockReset()
    vi.mocked(toast.info).mockClear()
    vi.mocked(toast.error).mockClear()
  })

  it('opens hide-window from the chevron and leaves settings out of the menu', () => {
    render(<ComposerScreenshotControl onCapture={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.capture_options' }))
    const item = screen.getByRole('menuitem', { name: 'workbench.composer.capture_hide' })
    const menu = screen.getByRole('menu')
    expect(item).toBeInTheDocument()
    expect(menu.parentElement).toBe(document.body)
    expect(screen.queryByRole('menuitem', { name: 'common.settings' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-ui="composer.screenshot.settings"]')).toBeNull()
    expect(capture).not.toHaveBeenCalled()
  })

  it('captures directly from the scissors and attaches the file', async () => {
    const file = new File([Uint8Array.from([1])], 'screenshot.png', { type: 'image/png' })
    capture.mockResolvedValue(file)
    const onCapture = vi.fn()
    render(<ComposerScreenshotControl onCapture={onCapture} />)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.capture' }))
    await waitFor(() => expect(onCapture).toHaveBeenCalledWith(file))
    expect(capture).toHaveBeenCalledWith('direct')
    expect(request).not.toHaveBeenCalled()
  })

  it('hides the window only from the menu item', async () => {
    capture.mockResolvedValue(null)
    render(<ComposerScreenshotControl onCapture={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.capture_options' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'workbench.composer.capture_hide' }))
    await waitFor(() => expect(capture).toHaveBeenCalledWith('hide-window'))
    expect(request).not.toHaveBeenCalled()
  })

  it('opens Screen Recording settings when capture needs that permission', async () => {
    request.mockResolvedValue(undefined)
    capture.mockRejectedValue(new Error(SCREEN_CAPTURE_PERMISSION_REQUIRED))
    render(<ComposerScreenshotControl onCapture={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.capture' }))
    await waitFor(() => expect(request).toHaveBeenCalledWith('system.mac.open_screen_capture_settings'))
    expect(toast.error).toHaveBeenCalledWith('workbench.composer.capture_failed')
  })

  it('keeps settings closed when capture fails for another reason', async () => {
    capture.mockRejectedValue(new Error('The screenshot tool did not return a valid PNG image.'))
    render(<ComposerScreenshotControl onCapture={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.capture' }))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('workbench.composer.capture_failed'))
    expect(request).not.toHaveBeenCalled()
  })

  it('keeps settings closed when the user cancels', async () => {
    capture.mockResolvedValue(null)
    render(<ComposerScreenshotControl onCapture={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.capture' }))
    await waitFor(() => expect(toast.info).toHaveBeenCalledWith('workbench.composer.capture_cancelled'))
    expect(request).not.toHaveBeenCalled()
  })
})
