// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { getToastUtilities, type ToastLabels, ToastProvider, ToastViewport, useToasts } from '../toast'

const toast = getToastUtilities()

describe('Toast', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  afterEach(() => {
    act(() => {
      toast.closeAll()
    })
    vi.useRealTimers()
  })

  it('keeps the toast card on the compact floating radius', () => {
    const toastCss = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), '../../arc/toast/toast.module.css'),
      'utf8'
    )
    expect(toastCss).toContain('border-radius: var(--radius-panel)')
    expect(toastCss).not.toMatch(/border-radius:\s*16px/)
  })

  it('keeps older actions inaccessible until the stack expands, then restores the chosen item', async () => {
    const user = userEvent.setup()
    const archived = new Set(['First', 'Second', 'Third', 'Fourth'])
    render(<ToastViewport />)
    act(() => {
      for (const title of archived) {
        toast.success({
          title,
          action: {
            label: `Restore ${title}`,
            onClick: () => {
              archived.delete(title)
            }
          }
        })
      }
    })
    expect(screen.getByText('First').closest('[inert]')).not.toBeNull()
    expect(screen.getByText('Fourth').closest('[inert]')).toBeNull()
    const region = screen.getByRole('region', { name: 'notifications' })
    fireEvent.mouseEnter(region)
    expect(screen.getByText('First').closest('[inert]')).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Restore Second' }))
    expect([...archived]).toEqual(['First', 'Third', 'Fourth'])
    expect(screen.queryByRole('button', { name: 'Restore Second' })).not.toBeInTheDocument()
    const exiting = screen.queryByText('Second')
    if (exiting) expect(exiting.closest('[inert]')).not.toBeNull()
    fireEvent.mouseLeave(region)
    expect(screen.getByText('First').closest('[inert]')).not.toBeNull()
  })

  it('pauses existing and newly arriving notifications while expanded and resumes the remaining timeout', () => {
    vi.useFakeTimers()
    render(<ToastViewport />)
    act(() => {
      toast.info({ title: 'First', timeout: 1000 })
    })
    act(() => {
      vi.advanceTimersByTime(400)
    })
    const region = screen.getByRole('region', { name: 'notifications' })
    fireEvent.mouseEnter(region)
    act(() => {
      toast.info({ title: 'Second', timeout: 1000 })
    })
    act(() => {
      vi.advanceTimersByTime(5000)
    })
    expect(screen.getByText('First')).toBeInTheDocument()
    expect(screen.getByText('Second')).toBeInTheDocument()
    fireEvent.mouseLeave(region)
    act(() => {
      vi.advanceTimersByTime(600)
    })
    const exiting = screen.queryByText('First')
    if (exiting) expect(exiting.closest('[inert]')).not.toBeNull()
    expect(screen.getByText('Second')).toBeInTheDocument()
    act(() => {
      vi.advanceTimersByTime(400)
    })
    expect(screen.queryByRole('region')).not.toBeInTheDocument()
  })

  it('expands for keyboard focus and stays expanded when the pointer leaves', () => {
    render(<ToastViewport />)
    act(() => {
      toast.info('First')
      toast.info('Second')
    })
    const region = screen.getByRole('region', { name: 'notifications' })
    fireEvent.focus(screen.getAllByRole('button', { name: 'Close' })[0])
    fireEvent.mouseLeave(region)
    expect(screen.getByText('First').closest('[inert]')).toBeNull()
    fireEvent.blur(region, { relatedTarget: document.body })
    expect(screen.getByText('First').closest('[inert]')).not.toBeNull()
  })

  it('opens a conversation notification with the keyboard without triggering secondary actions', async () => {
    const user = userEvent.setup()
    const open = vi.fn()
    render(<ToastViewport labels={{ notifications: '通知' }} />)
    act(() => {
      toast.info({ title: 'Task finished', description: 'Open its conversation', onClick: open })
    })
    const content = screen.getByRole('button', { name: 'Task finished Open its conversation' })
    content.focus()
    await user.keyboard('{Enter}')
    expect(open).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('region', { name: '通知' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Close' }))
    expect(open).toHaveBeenCalledTimes(1)
  })

  it('preserves reading time while the window is hidden', () => {
    vi.useFakeTimers()
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(false)
    render(<ToastViewport />)
    act(() => {
      toast.info('Read this notification')
      vi.advanceTimersByTime(1000)
    })
    act(() => {
      hidden.mockReturnValue(true)
      document.dispatchEvent(new Event('visibilitychange'))
      vi.advanceTimersByTime(10000)
    })
    expect(screen.getByText('Read this notification')).toBeVisible()
    act(() => {
      hidden.mockReturnValue(false)
      document.dispatchEvent(new Event('visibilitychange'))
      vi.advanceTimersByTime(3999)
    })
    expect(screen.getByText('Read this notification')).toBeVisible()
    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
    hidden.mockRestore()
  })

  it('renders a string toast in the viewport', () => {
    render(<ToastViewport />)

    act(() => {
      toast.success('Saved')
    })

    expect(screen.getByText('Saved')).toBeInTheDocument()
    expect(screen.getByRole('region', { name: 'notifications' })).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveAttribute('aria-live', 'polite')
  })

  it('dismisses an actionable toast before running its action without triggering the toast click', async () => {
    const user = userEvent.setup()
    const onToastClick = vi.fn()
    const onAction = vi.fn(() => {
      expect(toast.getToastQueue().toasts).toHaveLength(0)
    })

    render(<ToastViewport />)

    act(() => {
      toast.success({
        action: { label: 'Undo', onClick: onAction },
        key: 'deleted-item',
        onClick: onToastClick,
        title: 'Item deleted'
      })
    })

    await user.click(screen.getByRole('button', { name: 'Undo' }))

    expect(screen.queryByText('Item deleted')).not.toBeInTheDocument()
    expect(onToastClick).not.toHaveBeenCalled()
    expect(onAction).toHaveBeenCalledTimes(1)
  })

  it('aligns actionable toast controls to the primary content row', () => {
    render(<ToastViewport />)

    act(() => {
      toast.info({
        action: { label: 'Undo', onClick: vi.fn() },
        description: 'The item can be restored for 30 days.',
        title: 'Item deleted'
      })
    })

    const actionButton = screen.getByRole('button', { name: 'Undo' })
    const closeButton = screen.getByRole('button', { name: 'Close' })
    const title = screen.getByText('Item deleted')

    expect(actionButton).toHaveAttribute('data-variant', 'outline')
    expect(screen.getByRole('status')).toHaveClass('items-start')
    expect(actionButton).toBeEnabled()
    expect(title).toBeVisible()
    expect(closeButton).toHaveAccessibleName('Close')
  })

  it('marks toast items as no-drag so they stay clickable over titlebar drag regions', () => {
    render(<ToastViewport />)

    act(() => {
      toast.success('Saved')
    })

    expect(screen.getByRole('status')).toHaveClass('[-webkit-app-region:no-drag]')
  })

  it('auto-dismisses non-sticky toasts after their timeout', () => {
    vi.useFakeTimers()
    render(<ToastViewport />)

    act(() => {
      toast.info({ key: 'sync-started', timeout: 1000, title: 'Sync started' })
    })

    expect(screen.getByText('Sync started')).toBeInTheDocument()

    act(() => {
      vi.advanceTimersByTime(1000)
    })

    expect(screen.queryByText('Sync started')).not.toBeInTheDocument()
  })

  it('keeps timeout 0 toasts visible until explicitly closed', () => {
    vi.useFakeTimers()
    render(<ToastViewport />)

    act(() => {
      toast.warning({ key: 'manual-warning', timeout: 0, title: 'Manual warning' })
    })

    expect(screen.getByRole('alert')).toHaveTextContent('Manual warning')

    act(() => {
      vi.advanceTimersByTime(30_000)
    })

    expect(screen.getByText('Manual warning')).toBeInTheDocument()

    act(() => {
      toast.closeToast('manual-warning')
    })

    expect(screen.queryByText('Manual warning')).not.toBeInTheDocument()
  })

  it('closes keyed toasts and calls onClose', () => {
    const onClose = vi.fn()

    render(<ToastViewport />)

    act(() => {
      toast.error({
        description: 'The operation failed',
        key: 'operation-error',
        onClose,
        title: 'Failed'
      })
    })

    expect(screen.getByText('Failed')).toBeInTheDocument()
    expect(screen.getByText('The operation failed')).toBeInTheDocument()

    act(() => {
      toast.closeToast('operation-error')
    })

    expect(screen.queryByText('Failed')).not.toBeInTheDocument()
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('uses configured labels for fallback loading states and close button', async () => {
    const labels: Partial<ToastLabels> = {
      close: 'Dismiss',
      error: 'Localized error',
      errorDescription: 'Localized fallback error',
      loading: 'Localized loading',
      success: 'Localized success'
    }
    const localizedToast = getToastUtilities(labels)

    render(<ToastViewport labels={labels} />)

    await act(async () => {
      localizedToast.loading({ promise: Promise.resolve() })
    })

    expect(screen.getByText('Localized success')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Dismiss' })).toBeInTheDocument()
  })

  it('keeps loading rejection errors sticky and calls onError', async () => {
    const onError = vi.fn()
    render(<ToastViewport />)

    await act(async () => {
      toast.loading({
        key: 'failed-task',
        onError,
        promise: Promise.reject(new Error('Network failed'))
      })
    })

    expect(onError).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('alert')).toHaveTextContent('Network failed')
    expect(toast.getToastQueue().toasts.find((item) => item.key === 'failed-task')?.timeout).toBe(0)
  })

  it('upserts loading success by key and resets the timer', async () => {
    vi.useFakeTimers()
    const promise = Promise.resolve()
    render(<ToastViewport />)

    await act(async () => {
      toast.loading({ key: 'sync-task', promise })
      await promise
    })

    expect(toast.getToastQueue().toasts).toHaveLength(1)
    expect(screen.getByText('Success')).toBeInTheDocument()

    act(() => {
      vi.advanceTimersByTime(1999)
    })
    expect(screen.getByText('Success')).toBeInTheDocument()

    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(screen.queryByText('Success')).not.toBeInTheDocument()
  })

  it('does not revive a loading toast after it was closed', async () => {
    let resolvePromise: () => void = () => {}
    const promise = new Promise<void>((resolve) => {
      resolvePromise = resolve
    })

    render(<ToastViewport />)

    act(() => {
      toast.loading({ key: 'sync-task', promise, title: 'Syncing' })
    })

    expect(screen.getByText('Syncing')).toBeInTheDocument()

    act(() => {
      toast.closeToast('sync-task')
    })

    await act(async () => {
      resolvePromise()
      await promise
    })

    expect(screen.queryByText('Success')).not.toBeInTheDocument()
  })

  it('shares the single default store across ToastProvider instances', () => {
    // ToastProvider no longer forks a per-instance store: every provider and its
    // viewport read defaultToastStore, so utilities obtained from different
    // providers observe one shared queue (fixes the command-entry-vs-viewport
    // store split that black-holed window.toast in viewport-less windows).
    const providerToastA = vi.fn<(toastApi: ReturnType<typeof useToasts>) => void>()
    const providerToastB = vi.fn<(toastApi: ReturnType<typeof useToasts>) => void>()

    function Producer({ onReady }: { onReady: (toastApi: ReturnType<typeof useToasts>) => void }) {
      const toastApi = useToasts()
      onReady(toastApi)
      return null
    }

    render(
      <>
        <ToastProvider>
          <Producer onReady={providerToastA} />
        </ToastProvider>
        <ToastProvider>
          <Producer onReady={providerToastB} />
        </ToastProvider>
      </>
    )

    const toastA = providerToastA.mock.calls.at(-1)?.[0]
    const toastB = providerToastB.mock.calls.at(-1)?.[0]

    expect(toastA).toBeDefined()
    expect(toastB).toBeDefined()

    act(() => {
      toastA?.success('Shared')
    })

    expect(toastA?.getToastQueue().toasts).toHaveLength(1)
    expect(toastB?.getToastQueue().toasts).toHaveLength(1)
  })

  it('resolves a labels getter each time a toast fires', () => {
    // A getter lets callers read i18n lazily: each fire re-invokes it, so a
    // language switch between two toasts is reflected without re-creating the
    // utilities object.
    let currentLoading = 'Loading EN'
    const labelsGetter = (): Partial<ToastLabels> => ({ loading: currentLoading })
    const localizedToast = getToastUtilities(labelsGetter)

    render(<ToastViewport labels={labelsGetter} />)

    act(() => {
      localizedToast.loading({ key: 'getter-a', promise: new Promise<void>(() => {}) })
    })
    expect(screen.getByText('Loading EN')).toBeInTheDocument()

    currentLoading = 'Loading ZH'
    act(() => {
      localizedToast.loading({ key: 'getter-b', promise: new Promise<void>(() => {}) })
    })
    expect(screen.getByText('Loading ZH')).toBeInTheDocument()
  })

  it('closeAll removes every toast and invokes each onClose', () => {
    const firstClose = vi.fn()
    const secondClose = vi.fn()

    render(<ToastViewport />)

    act(() => {
      toast.info({ key: 'first', onClose: firstClose, title: 'First' })
      toast.error({ key: 'second', onClose: secondClose, title: 'Second' })
    })

    act(() => {
      toast.closeAll()
    })

    expect(firstClose).toHaveBeenCalledTimes(1)
    expect(secondClose).toHaveBeenCalledTimes(1)
    expect(screen.queryByText('First')).not.toBeInTheDocument()
    expect(screen.queryByText('Second')).not.toBeInTheDocument()
  })
})
