import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import ChatMarkdown from '../ChatMarkdown'

vi.mock('../ChatMarkdownRuntime', () => ({
  default: ({ block }: { block: { content: string } }) => <p>{block.content}</p>
}))
vi.mock('../OpenUiMessage', () => ({
  OpenUiMessage: ({ source, streaming }: { source: string; streaming: boolean }) => (
    <div data-testid="openui" data-streaming={String(streaming)}>
      {source}
    </div>
  )
}))

describe('OpenUI shared message route', () => {
  it('keeps the same UI mounted when an open fence closes and ordinary prose follows', () => {
    const before = 'Before\n\n```openui\nroot = Card("Budget")\n'
    const view = render(<ChatMarkdown block={{ id: 'part', status: 'streaming', content: before }} />)
    const ui = screen.getByTestId('openui')
    expect(ui).toHaveAttribute('data-streaming', 'true')
    view.rerender(<ChatMarkdown block={{ id: 'part', status: 'success', content: before + '```\n\nAfter' }} />)
    expect(screen.getByTestId('openui')).toBe(ui)
    expect(ui).toHaveAttribute('data-streaming', 'false')
    expect(screen.getByText('After')).toBeInTheDocument()
  })
})
