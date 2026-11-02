// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { Switch } from '../switch'

afterEach(() => {
  cleanup()
})

describe('Switch', () => {
  it('toggles aria-checked when clicked', async () => {
    const user = userEvent.setup()
    render(<Switch />)

    const root = screen.getByRole('switch')

    expect(root).toHaveAttribute('aria-checked', 'false')
    await user.click(root)
    expect(root).toHaveAttribute('aria-checked', 'true')
  })

  it('does not toggle when disabled', async () => {
    const user = userEvent.setup()
    render(<Switch disabled />)

    const root = screen.getByRole('switch')

    expect(root).toHaveAttribute('aria-checked', 'false')
    await user.click(root)
    expect(root).toHaveAttribute('aria-checked', 'false')
  })

  it('prevents repeat changes while loading, then allows toggling again', async () => {
    const user = userEvent.setup()
    const onCheckedChange = vi.fn()
    const { rerender } = render(<Switch aria-label="Service" loading onCheckedChange={onCheckedChange} />)
    const root = screen.getByRole('switch', { name: 'Service' })
    expect(root).toBeDisabled()
    expect(root).toHaveAttribute('aria-busy', 'true')
    await user.click(root)
    expect(onCheckedChange).not.toHaveBeenCalled()
    rerender(<Switch aria-label="Service" onCheckedChange={onCheckedChange} />)
    expect(root).toBeEnabled()
    expect(root).not.toHaveAttribute('aria-busy')
    await user.click(root)
    expect(onCheckedChange).toHaveBeenCalledOnce()
  })

  it('renders the thumb svg without the invalid "inherit" dimension attributes', () => {
    const { container } = render(<Switch loading />)

    const svg = container.querySelector('[data-slot="switch-thumb"] svg')

    expect(svg).not.toBeNull()
    // "inherit" is a CSS keyword and is invalid as an SVG width/height attribute,
    // which makes React throw "<svg> attribute width: Expected length, "inherit"".
    expect(svg).not.toHaveAttribute('width', 'inherit')
    expect(svg).not.toHaveAttribute('height', 'inherit')
    expect(svg).toHaveAttribute('viewBox', '0 0 20 20')
  })
})
