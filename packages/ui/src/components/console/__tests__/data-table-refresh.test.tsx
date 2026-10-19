// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { DataTable } from '../DataTable'

afterEach(() => {
  cleanup()
})

describe('DataTable refresh', () => {
  it('spins and awaits the list reload', async () => {
    let release: () => void = () => undefined
    const onRefresh = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          release = resolve
        }),
    )

    render(
      <DataTable
        columns={[{ id: 'name', header: 'Name', cell: (row: { name: string }) => row.name }]}
        rows={[{ id: '1', name: 'Ada' }]}
        rowKey={(row) => row.id}
        onRefresh={onRefresh}
      />,
    )

    const refresh = screen.getByRole('button', { name: 'Refresh' })
    fireEvent.click(refresh)

    expect(refresh.className).toContain('is-refreshing')
    expect(refresh.getAttribute('aria-busy')).toBe('true')
    await waitFor(() => expect(onRefresh).toHaveBeenCalledTimes(1))
    expect(refresh.className).toContain('is-refreshing')

    release()
    await waitFor(() => expect(refresh.className).not.toContain('is-refreshing'), { timeout: 1500 })
  })
})
