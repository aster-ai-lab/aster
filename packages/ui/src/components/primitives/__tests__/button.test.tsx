// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { Button } from '../button'

describe('Button', () => {
  it('keeps a visible keyboard focus outline on link variants', () => {
    render(<Button variant="link">Learn more</Button>)

    const button = screen.getByRole('button', { name: 'Learn more' })
    expect(button.className).toContain('focus-visible:underline')
    expect(button.className).not.toMatch(/focus-visible:ring-(?!0)/)
    expect(button.className).toContain('focus-visible:outline-2')
  })

  it('prevents repeated activation while loading', async () => {
    const onClick = vi.fn()
    const user = userEvent.setup()
    render(<Button loading onClick={onClick}>Save</Button>)

    const button = screen.getByRole('button', { name: /save/i })
    expect(button).toHaveAttribute('data-busy', 'true')
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(button).toBeDisabled()
    await user.click(button)
    expect(onClick).not.toHaveBeenCalled()
  })

  it.each(['default', 'icon-sm'] as const)(
    'keeps an aria-disabled %s button focusable without activating it',
    async (size) => {
      const user = userEvent.setup()
      const onClick = vi.fn()
      render(<Button size={size} aria-disabled onClick={onClick}>Unavailable</Button>)
      const button = screen.getByRole('button', { name: 'Unavailable' })
      await user.tab()
      expect(button).toHaveFocus()
      await user.keyboard('{Enter} ')
      await user.click(button)
      expect(onClick).not.toHaveBeenCalled()
    }
  )

  it('blocks a loading link without losing its label or focus target', async () => {
    const click = vi.fn()
    const user = userEvent.setup()
    render(<Button href="#blocked" loading loadingIcon={<span>Pending</span>} onClick={click}>Open file</Button>)
    const link = screen.getByRole('link', { name: 'Open file' })
    expect(link).toHaveAttribute('aria-disabled', 'true')
    expect(link).toHaveAttribute('aria-busy', 'true')
    await user.click(link)
    expect(click).not.toHaveBeenCalled()
  })

  it('blocks the child action of an aria-disabled slotted link', async () => {
    const click = vi.fn()
    const user = userEvent.setup()
    render(<Button asChild aria-disabled><a href="#blocked" onClick={click}>Unavailable link</a></Button>)
    const link = screen.getByRole('link', { name: 'Unavailable link' })
    await user.tab()
    expect(link).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(click).not.toHaveBeenCalled()
  })

  it('uses the navbar spinner size for icon-navbar loading buttons', () => {
    render(
      <Button loading size="icon-navbar" aria-label="Refresh">
        <span />
      </Button>
    )

    expect(screen.getByRole('button', { name: 'Refresh' }).querySelector('[data-slot=button-pending]')).toBeInTheDocument()
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
    expect(screen.getByRole('button', { name: 'Save changes' })).not.toHaveAttribute('aria-busy')
  })
})
