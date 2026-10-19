import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

vi.mock('@renderer/services/popup', async (importOriginal) => await importOriginal())
vi.mock('@a3s-lab/ui', () => vi.importActual('@a3s-lab/ui'))
vi.mock('@renderer/hooks/useProvider', () => ({
  useProviders: () => ({ providers: [] })
}))
vi.mock('@data/hooks/useDataApi', () => ({
  useQuery: () => ({ data: undefined, isLoading: false })
}))
vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, values?: { provider?: string }) => (values?.provider ? `${key}:${values.provider}` : key)
  })
}))

import { PopupHost } from '@renderer/components/PopupHost'
import { POPUP_EXIT_MS, popupService } from '@renderer/services/popup'

import UrlSchemaInfoPopup from '../UrlSchemaInfoPopup'

function openPopup() {
  render(<PopupHost />)
  act(() => {
    void UrlSchemaInfoPopup.show({
      id: 'example',
      apiKey: 'sk-example',
      baseUrl: 'https://api.example.com',
      name: 'Example'
    })
  })
}

afterEach(() => {
  cleanup()
  vi.useFakeTimers()
  for (const entry of [...popupService.getSnapshot()]) {
    popupService.settle(entry.instanceId, { updatedProvider: undefined, isNew: false, displayName: '' })
  }
  vi.advanceTimersByTime(POPUP_EXIT_MS)
  vi.useRealTimers()
})

describe('UrlSchemaInfoPopup', () => {
  it('opens on the shared 12px dialog panel', () => {
    openPopup()

    const panel = screen.getByRole('dialog')
    expect(panel).toHaveAttribute('data-slot', 'dialog-content')
    expect(panel).toHaveClass('rounded-[12px]')
    expect(panel).not.toHaveClass('rounded-2xl')
  })
})
