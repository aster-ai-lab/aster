import { act, cleanup, render, screen } from '@testing-library/react'
import type * as ReactModule from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('@renderer/services/popup', async (importOriginal) => await importOriginal())
vi.mock('@a3s-lab/ui', () => vi.importActual('@a3s-lab/ui'))
vi.mock('@renderer/components/GlobalSearch/GlobalSearchPanel', async () => {
  const React = await vi.importActual<typeof ReactModule>('react')
  return {
    GlobalSearchPanel: () => {
      const inputRef = React.useRef<HTMLInputElement>(null)
      React.useEffect(() => {
        inputRef.current?.focus()
      }, [])
      return <input ref={inputRef} aria-label="Search input" />
    }
  }
})
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => (key === 'globalSearch.open' ? 'Open global search' : key)
  })
}))

import { PopupHost } from '@renderer/components/PopupHost'
import { POPUP_EXIT_MS, popupService } from '@renderer/services/popup'

import GlobalSearchPopup from '../GlobalSearchPopup'

function openGlobalSearchPopup() {
  render(<PopupHost />)
  act(() => {
    void GlobalSearchPopup.show()
  })
}

afterEach(() => {
  cleanup()
  vi.useFakeTimers()
  for (const entry of [...popupService.getSnapshot()]) {
    popupService.settle(entry.instanceId, {})
  }
  vi.advanceTimersByTime(POPUP_EXIT_MS)
  vi.useRealTimers()
})

describe('GlobalSearchPopup panel radius', () => {
  it('opens on the shared 12px dialog panel', async () => {
    openGlobalSearchPopup()

    const panel = await screen.findByRole('dialog')
    expect(panel).toHaveAttribute('data-slot', 'dialog-content')
    expect(panel).toHaveClass('rounded-[12px]')
    expect(panel).not.toHaveClass('rounded-4xl')
  })
})
