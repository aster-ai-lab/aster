// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { PopConfirm } from '../PopConfirm'

beforeEach(() => {
  document.documentElement.removeAttribute('data-motion-paused')
})

afterEach(cleanup)

describe('PopConfirm exit presence', () => {
  it('keeps the panel mounted with data-state=closed through the shared 200ms exit', async () => {
    render(
      <PopConfirm defaultOpen title="Remove this item?" onConfirm={() => undefined}>
        <button type="button">Remove</button>
      </PopConfirm>
    )
    const panel = () => screen.queryByRole('dialog', { name: 'Remove this item?' })
    expect(panel()?.getAttribute('data-state')).toBe('open')
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(panel()?.getAttribute('data-state')).toBe('closed')
    expect(panel()?.hasAttribute('inert')).toBe(true)
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 220))
    })
    expect(panel()).toBeNull()
  })
})
