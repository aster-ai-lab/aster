import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { FileTreeSearchField } from '../FileTreeSearchField'

describe('FileTreeSearchField', () => {
  it('keeps refresh inside the field unless a caller moves it out', () => {
    const { rerender } = render(
      <FileTreeSearchField
        value=""
        onChange={() => undefined}
        placeholder="搜索"
        searchLabel="搜索"
        refreshLabel="刷新"
        onRefresh={() => undefined}
      />
    )
    const search = screen.getByRole('searchbox', { name: '搜索' })
    const field = search.closest('label')?.parentElement as HTMLElement
    expect(field).toContainElement(screen.getByRole('button', { name: '刷新' }))

    rerender(
      <FileTreeSearchField
        value=""
        onChange={() => undefined}
        placeholder="搜索"
        searchLabel="搜索"
        refreshLabel="刷新"
        showRefresh={false}
        onRefresh={() => undefined}
      />
    )
    expect(screen.queryByRole('button', { name: '刷新' })).not.toBeInTheDocument()
  })
})
