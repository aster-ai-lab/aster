// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest'

import { EmptyState } from '../index'

beforeAll(() => vi.stubGlobal('PointerEvent', MouseEvent))
afterAll(() => vi.unstubAllGlobals())
afterEach(cleanup)

describe('EmptyState', () => {
  it('offers the actual recovery action to keyboard users', async () => {
    const user = userEvent.setup()
    const clear = vi.fn()
    render(
      <EmptyState
        illustration="search"
        title="No results for roadmap"
        description="Two filters are active."
        actionLabel="Clear filters"
        onAction={clear}
      />
    )
    expect(screen.getByRole('heading', { name: 'No results for roadmap' })).toBeVisible()
    expect(screen.getByText('Two filters are active.')).toBeVisible()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Clear filters' })).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(clear).toHaveBeenCalledTimes(1)
  })

  it('keeps primary and secondary outcomes independent', async () => {
    const user = userEvent.setup()
    const create = vi.fn(),
      browse = vi.fn()
    render(
      <EmptyState
        title="No files"
        actionLabel="Import"
        onAction={create}
        secondaryLabel="Browse"
        onSecondary={browse}
      />
    )
    await user.click(screen.getByRole('button', { name: 'Browse' }))
    expect(browse).toHaveBeenCalledOnce()
    expect(create).not.toHaveBeenCalled()
  })

  it('does not offer an action without an executable callback', () => {
    render(<EmptyState title="No results" actionLabel="Create" secondaryLabel="Browse" />)
    expect(screen.queryByRole('button')).toBeNull()
  })

  it('preserves action focus while the search result copy changes', async () => {
    const user = userEvent.setup()
    const clear = vi.fn()
    const view = render(<EmptyState title="No results for plan" actionLabel="Clear" onAction={clear} />)
    await user.tab()
    view.rerender(<EmptyState title="No results for roadmap" actionLabel="Clear" onAction={clear} />)
    expect(screen.getByRole('button', { name: 'Clear' })).toHaveFocus()
    expect(screen.getByRole('heading', { name: 'No results for roadmap' })).toBeVisible()
  })

  it('keeps the illustration decorative for assistive technology', () => {
    render(<EmptyState illustration="search" title="No results" />)
    expect(screen.queryByRole('img')).toBeNull()
    expect(screen.getByRole('heading', { name: 'No results' })).toBeVisible()
  })
})
