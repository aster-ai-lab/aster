import { act, fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { RESTING_DIALOG_LEAVE_MS, useRestingDialog } from '../restingDialog'

function Probe({ open }: { open: boolean }) {
  const presence = useRestingDialog(open)
  if (!presence.present) return null
  return <div role="dialog" data-state={presence.state} />
}

afterEach(() => {
  vi.useRealTimers()
})

describe('useRestingDialog', () => {
  it('keeps the dialog mounted through the leave, then removes it', () => {
    vi.useFakeTimers()
    function Host() {
      const [open, setOpen] = useState(true)
      return (
        <>
          <button type="button" onClick={() => setOpen(false)}>
            Close
          </button>
          <Probe open={open} />
        </>
      )
    }
    render(<Host />)
    expect(screen.getByRole('dialog')).toHaveAttribute('data-state', 'open')
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(screen.getByRole('dialog')).toHaveAttribute('data-state', 'closed')
    act(() => {
      vi.advanceTimersByTime(RESTING_DIALOG_LEAVE_MS)
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('drops the dialog immediately when motion is reduced', () => {
    const original = window.matchMedia
    window.matchMedia = ((query: string) => ({
      matches: query.includes('reduce'),
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
      onchange: null
    })) as typeof window.matchMedia
    function Host() {
      const [open, setOpen] = useState(true)
      return (
        <>
          <button type="button" onClick={() => setOpen(false)}>
            Close
          </button>
          <Probe open={open} />
        </>
      )
    }
    render(<Host />)
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    window.matchMedia = original
  })
})
