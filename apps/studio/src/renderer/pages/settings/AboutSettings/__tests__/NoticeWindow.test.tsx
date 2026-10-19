import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key
  })
}))

import { NoticeWindow } from '../NoticeWindow'

function installWindow(label: string, origin: string) {
  document.documentElement.dataset.a3sNoticeOrigin = origin
  const invoke = vi.fn(async () => undefined)
  ;(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {
    invoke,
    metadata: { currentWindow: { label } }
  }
  return invoke
}

describe('NoticeWindow', () => {
  afterEach(() => {
    delete document.documentElement.dataset.a3sNoticeOrigin
    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  it('shows the legal page only inside the matching notice window', () => {
    installWindow('notice-terms', 'http://127.0.0.1:3000')
    const view = render(<NoticeWindow slug="terms" />)
    const frame = screen.getByTitle('settings.about.page.terms')
    expect(frame).toHaveAttribute('src', 'http://127.0.0.1:3000/v1/api/legal/terms')
    expect(frame).toHaveAttribute('sandbox', '')
    expect(frame).toHaveAttribute('referrerpolicy', 'no-referrer')
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    view.unmount()

    installWindow('notice-terms', 'http://evil.test/v1/api/legal/terms')
    render(<NoticeWindow slug="terms" />)
    expect(screen.getByRole('alert')).toHaveTextContent('settings.about.page.open_failed')
    expect(document.querySelector('iframe')).not.toBeInTheDocument()
  })

  it('does not load the page when the window label does not match the slug', () => {
    installWindow('main', 'http://127.0.0.1:3000')
    render(<NoticeWindow slug="terms" />)
    expect(screen.getByRole('alert')).toHaveTextContent('settings.about.page.open_failed')
    expect(document.querySelector('iframe')).not.toBeInTheDocument()
  })

  it('drags and closes only its own notice window', async () => {
    const invoke = installWindow('notice-privacy', 'http://127.0.0.1:3000')
    const user = userEvent.setup()
    render(<NoticeWindow slug="privacy" />)

    fireEvent.mouseDown(screen.getByRole('heading', { name: 'settings.about.page.privacy' }))
    expect(invoke).toHaveBeenCalledWith('plugin:window|start_dragging', { label: 'notice-privacy' })

    await user.click(screen.getByRole('button', { name: 'common.close' }))
    expect(invoke).toHaveBeenCalledWith('plugin:window|close', { label: 'notice-privacy' })
  })
})
