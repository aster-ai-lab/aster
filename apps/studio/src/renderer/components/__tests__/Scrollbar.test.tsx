import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import Scrollbar from '../Scrollbar'

describe('Scrollbar', () => {
  it('keeps the platform overlay scrollbar and forwards scroll', () => {
    const onScroll = vi.fn()
    render(
      <Scrollbar data-testid="scrollbar" onScroll={onScroll}>
        内容
      </Scrollbar>
    )

    const scrollbar = screen.getByTestId('scrollbar')
    expect(scrollbar).not.toHaveClass('[scrollbar-gutter:stable]')
    expect(scrollbar).not.toHaveAttribute('data-scrollbar', 'overlay')
    expect(scrollbar.style.scrollbarColor).toBe('')
    expect(scrollbar.className).not.toContain('webkit-scrollbar')

    fireEvent.scroll(scrollbar)
    expect(onScroll).toHaveBeenCalledTimes(1)
  })
})
