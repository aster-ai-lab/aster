// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { PopConfirm } from '../PopConfirm'

afterEach(cleanup)

describe('PopConfirm actions', () => {
  it('places the panel without hiding its cancel and confirm buttons', () => {
    render(
      <PopConfirm defaultOpen title="Remove this item?" description="This cannot be undone." onConfirm={() => undefined}>
        <button type="button">Remove</button>
      </PopConfirm>
    )

    const dialog = screen.getByRole('dialog', { name: 'Remove this item?' })
    expect(dialog.style.visibility).not.toBe('hidden')
    expect(dialog.style.top).not.toBe('-9999px')

    const cancel = within(dialog).getByRole('button', { name: 'Cancel' })
    const confirm = within(dialog).getByRole('button', { name: 'Confirm' })
    expect(cancel.style.appearance).toBe('none')
    expect(confirm.style.appearance).toBe('none')
  })

  it('opens when the inner button is activated', () => {
    render(
      <PopConfirm title="归档这个任务？" onConfirm={() => undefined}>
        <button type="button">归档</button>
      </PopConfirm>
    )

    expect(screen.queryByRole('dialog')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: '归档' }))
    expect(screen.getByRole('dialog', { name: '归档这个任务？' })).toBeTruthy()
  })

  it('paints a danger confirm with an explicit face', () => {
    render(
      <PopConfirm
        defaultOpen
        tone="danger"
        title="Remove this item?"
        confirmLabel="Remove"
        onConfirm={() => undefined}
      >
        <button type="button">Remove</button>
      </PopConfirm>
    )

    const dialog = screen.getByRole('dialog', { name: 'Remove this item?' })
    const confirm = within(dialog).getByRole('button', { name: 'Remove' })
    expect(confirm.style.appearance).toBe('none')
    expect(confirm.style.backgroundColor).toBe('#c23b3b')
  })
})
