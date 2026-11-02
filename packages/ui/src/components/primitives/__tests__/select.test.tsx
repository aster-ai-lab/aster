// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeAll, describe, expect, it } from 'vitest'

import { PortalContainerProvider } from '../portal-container'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../select'

beforeAll(() => {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  if (!HTMLElement.prototype.hasPointerCapture) {
    HTMLElement.prototype.hasPointerCapture = () => false
  }
  if (!HTMLElement.prototype.releasePointerCapture) {
    HTMLElement.prototype.releasePointerCapture = () => {}
  }
  if (!HTMLElement.prototype.setPointerCapture) {
    HTMLElement.prototype.setPointerCapture = () => {}
  }
  HTMLElement.prototype.scrollIntoView = () => {}
})

afterEach(() => {
  cleanup()
})

describe('SelectContent', () => {
  it('can be reached by keyboard and exposes its current value', async () => {
    const user = userEvent.setup()
    render(
      <Select defaultValue="alpha">
        <SelectTrigger aria-label="Mode">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="alpha">Alpha</SelectItem>
        </SelectContent>
      </Select>
    )

    const trigger = screen.getByRole('combobox', { name: 'Mode' })
    await user.tab()
    expect(trigger).toHaveFocus()
    expect(trigger).toHaveTextContent('Alpha')
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
  })

  it('allows consumers to supply the full-width layout', () => {
    render(
      <Select defaultValue="alpha">
        <SelectTrigger aria-label="Mode" size="sm" className="w-full h-8"><SelectValue /></SelectTrigger>
        <SelectContent><SelectItem value="alpha">Alpha</SelectItem></SelectContent>
      </Select>
    )
    const trigger = screen.getByRole('combobox', { name: 'Mode' })
    expect(trigger).toHaveClass('w-full', 'h-8')
    expect(trigger).not.toHaveClass('w-fit!')
  })

  it('does not show the error state when aria-invalid is the string false', () => {
    render(
      <Select defaultValue="alpha">
        <SelectTrigger aria-label="Mode" aria-invalid="false"><SelectValue /></SelectTrigger>
        <SelectContent><SelectItem value="alpha">Alpha</SelectItem></SelectContent>
      </Select>
    )
    expect(screen.getByRole('combobox', { name: 'Mode' })).not.toHaveClass('border-destructive!')
  })

  it('opens and closes without controlled open props', async () => {
    render(
      <Select defaultValue="alpha">
        <SelectTrigger aria-label="Mode">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="alpha">Alpha</SelectItem>
          <SelectItem value="beta">Beta</SelectItem>
        </SelectContent>
      </Select>
    )

    fireEvent.pointerDown(screen.getByRole('combobox', { name: 'Mode' }))
    fireEvent.click(screen.getByRole('combobox', { name: 'Mode' }))

    expect(await screen.findByRole('option', { name: 'Beta' })).toBeInTheDocument()

    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' })

    await waitFor(() => expect(screen.queryByRole('option', { name: 'Beta' })).not.toBeInTheDocument())
  })

  it('uses the provider portal container by default', () => {
    const portalContainer = document.createElement('div')
    document.body.appendChild(portalContainer)

    try {
      render(
        <PortalContainerProvider container={portalContainer}>
          <Select open value="alpha">
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent data-testid="content">
              <SelectItem value="alpha">Alpha</SelectItem>
            </SelectContent>
          </Select>
        </PortalContainerProvider>
      )

      expect(portalContainer).toContainElement(screen.getByTestId('content'))
    } finally {
      portalContainer.remove()
    }
  })

  it('marks content as no-drag so it stays clickable over titlebar drag regions', () => {
    render(
      <Select open value="alpha">
        <SelectTrigger>
          <SelectValue />
        </SelectTrigger>
        <SelectContent data-testid="content">
          <SelectItem value="alpha">Alpha</SelectItem>
        </SelectContent>
      </Select>
    )

    expect(screen.getByTestId('content')).toHaveClass('[-webkit-app-region:no-drag]')
  })
})
