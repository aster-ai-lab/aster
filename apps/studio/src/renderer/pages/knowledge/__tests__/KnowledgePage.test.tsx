import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

vi.mock('../KnowledgeNotesPage', () => ({
  KnowledgeNotesPage: () => <div>notes-encyclopedia</div>
}))

import KnowledgePage from '../KnowledgePage'

describe('KnowledgePage', () => {
  it('shows the later encyclopedia notes page', () => {
    render(<KnowledgePage />)
    expect(screen.getByText('notes-encyclopedia')).toBeInTheDocument()
    expect(screen.queryByTestId('segmented-control')).not.toBeInTheDocument()
  })
})
