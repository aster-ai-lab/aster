// @vitest-environment happy-dom
import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import { Checkbox } from '../checkbox'

describe('Checkbox', () => {
  it('toggles with the keyboard and exposes its state', async () => {
    const user = userEvent.setup()
    render(<Checkbox aria-label="Accept" />)
    const box = screen.getByRole('checkbox', { name: 'Accept' })
    await user.tab()
    expect(box).toHaveFocus()
    await user.keyboard(' ')
    expect(box).toBeChecked()
    expect(box.querySelector('svg')?.getAttribute('stroke-width')).toBe('1.7')
    await user.keyboard(' ')
    expect(box).not.toBeChecked()
  })
  it('does not toggle while disabled', async () => {
    const user = userEvent.setup()
    render(<Checkbox disabled aria-label="Accept" />)
    const box = screen.getByRole('checkbox', { name: 'Accept' })
    await user.click(box)
    expect(box).not.toBeChecked()
  })
})
