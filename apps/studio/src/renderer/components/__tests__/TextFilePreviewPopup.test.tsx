import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('@renderer/services/popup', async (importOriginal) => await importOriginal())
vi.mock('@a3s-lab/ui', () => vi.importActual('@a3s-lab/ui'))
vi.mock('@data/hooks/usePreference', () => ({
  usePreference: () => [14, vi.fn()]
}))
vi.mock('@renderer/hooks/useCodeStyle', () => ({
  useCmTheme: () => 'light'
}))

import { PopupHost } from '@renderer/components/PopupHost'
import { POPUP_EXIT_MS, popupService } from '@renderer/services/popup'

import TextFilePreviewPopup from '../TextFilePreviewPopup'

function openPreview() {
  render(<PopupHost />)
  act(() => {
    void TextFilePreviewPopup.show('plain text', 'Notes')
  })
}

afterEach(() => {
  cleanup()
  vi.useFakeTimers()
  for (const entry of [...popupService.getSnapshot()]) {
    popupService.settle(entry.instanceId, undefined)
  }
  vi.advanceTimersByTime(POPUP_EXIT_MS)
  vi.useRealTimers()
})

describe('TextFilePreviewPopup', () => {
  it('opens on the shared 12px dialog panel', () => {
    openPreview()

    const panel = screen.getByRole('dialog')
    expect(panel).toHaveAttribute('data-slot', 'dialog-content')
    expect(panel).toHaveClass('rounded-[12px]')
    expect(panel).not.toHaveClass('rounded-[20px]')
    expect(screen.getByRole('heading', { name: 'Notes' })).toBeInTheDocument()
  })
})
