// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { PortalContainerProvider } from '../../primitives/portal-container'

import { PopConfirm } from '../PopConfirm'

afterEach(cleanup)
afterEach(() => vi.restoreAllMocks())

describe('PopConfirm actions', () => {
  it('does not consume Escape from a different dialog while clearing a background confirm', () => {
    render(<PopConfirm defaultOpen tone="default" title="Background action" onConfirm={() => undefined}><button type="button">Action</button></PopConfirm>)
    const other = document.createElement('div')
    other.setAttribute('role', 'dialog')
    const button = document.createElement('button')
    other.append(button)
    document.body.append(other)
    const onEscape = vi.fn()
    button.addEventListener('keydown', onEscape)
    fireEvent.keyDown(button, { key: 'Escape' })
    expect(onEscape).toHaveBeenCalledOnce()
    expect(screen.queryByRole('dialog', { name: 'Background action' })).toBeNull()
    other.remove()
  })

  it('keeps a nested confirm inside its owner and aligned with an offset trigger', () => {
    const host = document.createElement('div')
    host.setAttribute('role', 'dialog')
    host.style.position = 'relative'
    document.body.append(host)
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      if (this === host) return new DOMRect(100, 200, 600, 500)
      if (this.classList.contains('confirm-trigger')) return new DOMRect(140, 240, 80, 30)
      return new DOMRect()
    })
    render(
      <PortalContainerProvider container={host}>
        <PopConfirm defaultOpen tone="default" className="confirm-trigger" title="Remove this item?" onConfirm={() => undefined}>
          <button type="button">Remove</button>
        </PopConfirm>
      </PortalContainerProvider>,
      { container: host }
    )
    const panel = screen.getByRole('dialog', { name: 'Remove this item?' })
    expect(host.contains(panel)).toBe(true)
    const viewportTop = Number.parseFloat(panel.style.top) + (panel.style.position === 'fixed' ? 0 : 200)
    expect(viewportTop).toBeGreaterThanOrEqual(270)
    expect(viewportTop).toBeLessThanOrEqual(290)
    fireEvent.click(within(panel).getByRole('button', { name: 'Cancel' }))
    expect(screen.queryByRole('dialog', { name: 'Remove this item?' })).toBeNull()
  })

  it('withdraws confirmation when the owner disables an already open action and still allows cancel', () => {
    let removed = false
    const content = (disabled: boolean) => <PopConfirm defaultOpen disabled={disabled} title="Remove this item?" onConfirm={() => { removed = true }}><button type="button">Remove</button></PopConfirm>
    const view = render(content(false))
    view.rerender(content(true))
    const dialog = screen.getByRole('dialog', { name: 'Remove this item?' })
    const confirm = within(dialog).getByRole<HTMLButtonElement>('button', { name: 'Confirm' })
    expect(confirm.disabled).toBe(true)
    expect(within(dialog).getByRole<HTMLButtonElement>('button', { name: 'Cancel' }).disabled).toBe(false)
    fireEvent.click(confirm)
    expect(removed).toBe(false)
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })
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
