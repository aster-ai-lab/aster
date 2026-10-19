import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { QuickPanelRow } from '../list'

describe('QuickPanelRow', () => {
  it('puts the description after the label and keeps the suffix at the end', () => {
    render(
      <QuickPanelRow
        active={false}
        item={{
          id: 'slash-command:/long-command',
          label: '/long-command',
          description: 'A description long enough to exercise the single-line overflow contract.',
          icon: 'icon',
          suffix: 'source'
        }}
        onSelect={vi.fn()}
      />
    )

    const label = screen.getByText('/long-command')
    const description = screen.getByText(/A description long enough/)
    const suffix = screen.getByText('source')

    expect(label).toHaveClass('max-w-[46%]', 'shrink-0', 'truncate')
    expect(label.parentElement).toBe(description.parentElement)
    expect(description).toHaveClass('min-w-0', 'flex-1', 'truncate', 'text-left')
    expect(suffix).toHaveClass('max-w-full', 'truncate')
    expect(suffix.parentElement?.parentElement).not.toBe(label.parentElement)
  })

  it('lets labels use the remaining width when there is no description', () => {
    render(
      <QuickPanelRow
        active={false}
        item={{
          id: 'prompt-load-error',
          label: 'Unable to load prompts because the remote service returned a detailed error message.',
          icon: 'icon',
          disabled: true
        }}
        onSelect={vi.fn()}
      />
    )

    const label = screen.getByText(/Unable to load prompts/)
    const primary = label.parentElement
    const metadata = primary?.nextElementSibling

    expect(primary).toHaveClass('min-w-0', 'flex-1')
    expect(primary).not.toHaveClass('max-w-[40%]')
    expect(primary).not.toHaveClass('shrink-0')
    expect(metadata).toHaveClass('shrink-0')
    expect(metadata).not.toHaveClass('flex-1')
  })
})
