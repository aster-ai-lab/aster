// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ConfirmDialog } from '../index'

afterEach(cleanup)

describe('ConfirmDialog', () => {
  it('guards a disabled confirmation without blocking cancellation', async () => {
    const user = userEvent.setup()
    const onConfirm = vi.fn()
    const onOpenChange = vi.fn()

    render(
      <ConfirmDialog
        open
        confirmDisabled
        title="Confirm action"
        confirmText="Confirm"
        cancelText="Cancel"
        onConfirm={onConfirm}
        onOpenChange={onOpenChange}
      />
    )

    const confirmButton = screen.getByRole('button', { name: 'Confirm' })
    expect(confirmButton).toBeDisabled()

    await user.click(confirmButton)

    expect(onConfirm).not.toHaveBeenCalled()
    expect(onOpenChange).not.toHaveBeenCalled()

    const cancelButton = screen.getByRole('button', { name: 'Cancel' })
    expect(cancelButton).toBeEnabled()

    await user.click(cancelButton)

    expect(onOpenChange).toHaveBeenCalledTimes(1)
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it('disables cancellation when requested', async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()

    render(<ConfirmDialog open cancelDisabled title="Confirm action" cancelText="Cancel" onOpenChange={onOpenChange} />)

    const cancelButton = screen.getByRole('button', { name: 'Cancel' })
    expect(cancelButton).toBeDisabled()

    await user.click(cancelButton)

    expect(onOpenChange).not.toHaveBeenCalled()
  })

  it('keeps the corner close control off until a caller asks for it', () => {
    const { rerender } = render(<ConfirmDialog open title="Confirm action" onOpenChange={() => {}} />)

    expect(screen.queryByRole('button', { name: 'Close' })).not.toBeInTheDocument()

    rerender(<ConfirmDialog open showCloseButton closeLabel="关闭" title="Confirm action" onOpenChange={() => {}} />)

    expect(screen.getByRole('button', { name: '关闭' })).toBeInTheDocument()
  })

  it('uses fade-scale motion without directional translation', () => {
    render(
      <ConfirmDialog
        open
        title="Confirm action"
        description="This action needs your confirmation."
        onOpenChange={() => {}}
      />
    )

    const dialog = screen.getByRole('dialog', { name: 'Confirm action' })

    expect(dialog.className).not.toMatch(/slide-in-from-bottom|slide-out-to-bottom|zoom-in-|animate-in/)
    expect(dialog).toHaveClass('motion-reduce:animate-none')
  })

  it('stays open when confirmation resolves false', async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    const onConfirm = vi.fn().mockResolvedValue(false)
    render(<ConfirmDialog open title="Confirm action" onOpenChange={onOpenChange} onConfirm={onConfirm} />)

    await user.click(screen.getByRole('button', { name: 'Confirm' }))

    expect(onConfirm).toHaveBeenCalledOnce()
    expect(onOpenChange).not.toHaveBeenCalledWith(false)
  })

  it('shows a localized failure without closing and allows retry', async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    const onConfirm = vi.fn().mockRejectedValueOnce(new Error('request failed')).mockResolvedValue(true)
    render(
      <ConfirmDialog
        open
        title="Delete record"
        failureText="删除失败，请重试"
        onOpenChange={onOpenChange}
        onConfirm={onConfirm}
      />
    )
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(screen.getByRole('alert')).toHaveTextContent('删除失败，请重试')
    expect(onOpenChange).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it.each([
    ['void', () => undefined],
    ['true', () => true]
  ])('closes when confirmation returns %s', async (_label, onConfirm) => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    render(<ConfirmDialog open title="Confirm action" onOpenChange={onOpenChange} onConfirm={onConfirm} />)

    await user.click(screen.getByRole('button', { name: 'Confirm' }))

    expect(onOpenChange).toHaveBeenCalledWith(false)
  })
})
