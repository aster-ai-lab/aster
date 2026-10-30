// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { Button } from '../button'

describe('Button', () => {
  it('uses variant state feedback instead of an outer focus ring', () => {
    render(<Button variant="link">Learn more</Button>)

    const button = screen.getByRole('button', { name: 'Learn more' })
    expect(button.className).toContain('focus-visible:underline')
    expect(button.className).not.toMatch(/focus-visible:ring-(?!0)/)
    expect(button.className).not.toContain('focus-visible:outline-')
  })

  it('emits data-busy for loading styles and disables the button', () => {
    render(<Button loading>Save</Button>)

    const button = screen.getByRole('button', { name: /save/i })
    expect(button).toHaveAttribute('data-busy', 'true')
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(button).toBeDisabled()
    expect(button.className).toContain('data-[busy=true]:cursor-progress')
    expect(button.className).not.toContain('data-[loading=true]')
  })

  it('uses the navbar spinner size for icon-navbar loading buttons', () => {
    render(
      <Button loading size="icon-navbar" aria-label="Refresh">
        <span />
      </Button>
    )

    expect(screen.getByRole('button', { name: 'Refresh' }).querySelector('svg')).toHaveAttribute('width', '18')
  })

  it('retains the same label node and accessible name throughout a pending exchange', () => {
    const { container, rerender } = render(<Button loading={false}>Save changes</Button>)
    const label = container.querySelector('[data-slot="button-label"]')
    rerender(<Button loading>Save changes</Button>)
    expect(screen.getByRole('button', { name: 'Save changes' })).toBeDisabled()
    expect(container.querySelector('[data-slot="button-label"]')).toBe(label)
    expect(container.querySelector('[data-slot="button-pending"]')).toHaveAttribute('aria-hidden', 'true')
    rerender(<Button loading={false}>Save changes</Button>)
    expect(screen.getByRole('button', { name: 'Save changes' })).toBeEnabled()
    expect(container.querySelector('[data-slot="button-label"]')).toBe(label)
    expect(container.querySelector('[data-slot="button-pending"]')).toBeNull()
  })
})
