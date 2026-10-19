import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { publishWikiRetrieval, resetWikiRetrievalForTests } from '../wikiRetrievalPhase'
import { WikiRetrievalStatus } from '../WikiRetrievalStatus'

describe('wiki retrieval status', () => {
  afterEach(() => {
    cleanup()
    resetWikiRetrievalForTests()
    vi.useRealTimers()
  })

  it('stays blank until preparation lasts, then clears when the index is published', () => {
    vi.useFakeTimers()
    publishWikiRetrieval('preparing')
    render(<WikiRetrievalStatus />)
    expect(screen.queryByRole('status')).toBeNull()

    act(() => {
      vi.advanceTimersByTime(400)
    })
    const status = screen.getByRole('status')
    expect(status).toHaveAttribute('data-ui', 'knowledge.notes.retrieval')
    expect(status).toHaveAttribute('aria-live', 'polite')
    expect(status).toHaveTextContent(/正在准备笔记检索|Preparing note search/)
    expect(status).toHaveClass('text-[12px]', 'leading-5', 'text-[#8a8a8a]')

    act(() => {
      publishWikiRetrieval('idle')
    })
    expect(screen.queryByRole('status')).toBeNull()
  })

  it('keeps a failed index visible', () => {
    publishWikiRetrieval('failed')
    render(<WikiRetrievalStatus />)
    const status = screen.getByRole('status')
    expect(status).toHaveTextContent(/笔记检索未就绪|Note search is not ready/)
    expect(status).toHaveClass('text-[12px]', 'text-[#8a8a8a]')
  })
})
