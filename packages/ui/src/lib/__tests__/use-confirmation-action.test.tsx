// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { useConfirmationAction } from '../use-confirmation-action'

afterEach(cleanup)

describe('confirmation ownership', () => {
  it('dispatches once while pending and closes only after success', async () => {
    let settle!: () => void
    const request = new Promise<void>((resolve) => {
      settle = resolve
    })
    const confirm = vi.fn(() => request)
    const close = vi.fn()
    const { result } = renderHook(() =>
      useConfirmationAction({ open: true, loading: false, onConfirm: confirm, onClose: close })
    )
    let first!: Promise<void>
    act(() => {
      first = result.current.submit()
      void result.current.submit()
    })
    expect(confirm).toHaveBeenCalledOnce()
    expect(result.current.busy).toBe(true)
    expect(close).not.toHaveBeenCalled()
    await act(async () => {
      settle()
      await first
    })
    expect(close).toHaveBeenCalledOnce()
    expect(result.current.busy).toBe(false)
  })

  it('retains a rejected confirmation and allows a reviewed retry', async () => {
    const confirm = vi.fn().mockRejectedValueOnce(new Error('storage failure')).mockResolvedValue(undefined)
    const close = vi.fn()
    const { result } = renderHook(() =>
      useConfirmationAction({ open: true, loading: false, onConfirm: confirm, onClose: close })
    )
    await act(() => result.current.submit())
    expect(result.current.failed).toBe(true)
    expect(close).not.toHaveBeenCalled()
    await act(() => result.current.submit())
    expect(result.current.failed).toBe(false)
    expect(close).toHaveBeenCalledOnce()
  })

  it.each(['replacement', 'reopen', 'unmount'])('does not settle a newer dialog after %s', async (change) => {
    let settle!: () => void
    const request = new Promise<void>((resolve) => {
      settle = resolve
    })
    const close = vi.fn()
    const { result, rerender, unmount } = renderHook(
      ({ open, key }) =>
        useConfirmationAction({
          open,
          operationKey: key,
          loading: false,
          onConfirm: () => request,
          onClose: close
        }),
      { initialProps: { open: true, key: 'record-a:v1' } }
    )
    let pending!: Promise<void>
    act(() => {
      pending = result.current.submit()
    })
    if (change === 'replacement') rerender({ open: true, key: 'record-b:v2' })
    else if (change === 'reopen') {
      rerender({ open: false, key: 'record-a:v1' })
      rerender({ open: true, key: 'record-a:v1' })
    } else unmount()
    await act(async () => {
      settle()
      await pending
    })
    expect(close).not.toHaveBeenCalled()
  })
})
