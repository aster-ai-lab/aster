// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest'

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuCheckboxItem,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
  isTriggerPress
} from '../dropdown-menu'
import { PortalContainerProvider } from '../portal-container'

beforeAll(() => {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

afterEach(() => {
  cleanup()
})

describe('DropdownMenuContent', () => {
  it('uses the provider portal container by default', () => {
    const portalContainer = document.createElement('div')
    document.body.appendChild(portalContainer)

    try {
      render(
        <PortalContainerProvider container={portalContainer}>
          <DropdownMenu open>
            <DropdownMenuTrigger>Open</DropdownMenuTrigger>
            <DropdownMenuContent>
              <DropdownMenuItem>Item</DropdownMenuItem>
              <DropdownMenuSub open>
                <DropdownMenuSubTrigger>More</DropdownMenuSubTrigger>
                <DropdownMenuSubContent>
                  <DropdownMenuItem>Sub item</DropdownMenuItem>
                </DropdownMenuSubContent>
              </DropdownMenuSub>
            </DropdownMenuContent>
          </DropdownMenu>
        </PortalContainerProvider>
      )

      expect(portalContainer).toContainElement(screen.getByText('Item'))
      const subContent = screen.getByText('Sub item').closest('[data-slot="dropdown-menu-sub-content"]')
      expect(portalContainer).toContainElement(subContent)
      expect(screen.getByText('Item').closest('[data-slot="dropdown-menu-content"]')).not.toContainElement(subContent)
    } finally {
      portalContainer.remove()
    }
  })

  it('allows keyboard selection of a checked menu item', async () => {
    const user = userEvent.setup()
    const onCheckedChange = vi.fn()
    render(
      <DropdownMenu open>
        <DropdownMenuTrigger>Open</DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuCheckboxItem checked onCheckedChange={onCheckedChange}>Notifications</DropdownMenuCheckboxItem>
        </DropdownMenuContent>
      </DropdownMenu>
    )
    const item = screen.getByRole('menuitemcheckbox', { name: 'Notifications' })
    expect(item).toHaveAttribute('aria-checked', 'true')
    item.focus()
    await user.keyboard('{Enter}')
    expect(onCheckedChange).toHaveBeenCalledWith(false)
  })

  it('marks content and sub-content as no-drag so they stay clickable over titlebar drag regions', () => {
    render(
      <DropdownMenu open>
        <DropdownMenuTrigger>Open</DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem>Item</DropdownMenuItem>
          <DropdownMenuSub open>
            <DropdownMenuSubTrigger>More</DropdownMenuSubTrigger>
            <DropdownMenuSubContent>
              <DropdownMenuItem>Sub item</DropdownMenuItem>
            </DropdownMenuSubContent>
          </DropdownMenuSub>
        </DropdownMenuContent>
      </DropdownMenu>
    )

    expect(screen.getByText('Item').closest('[data-slot="dropdown-menu-content"]')).toHaveClass(
      '[-webkit-app-region:no-drag]'
    )
    expect(screen.getByText('Sub item').closest('[data-slot="dropdown-menu-sub-content"]')).toHaveClass(
      '[-webkit-app-region:no-drag]'
    )
  })

  // A destructive item's icon can be nested (e.g. wrapped in a layout span by the caller), so the
  // override must be a descendant selector (`[&_svg…]`); the broken direct-child form (`*:[svg]`)
  // never reaches a nested icon and leaves it muted-gray. Pin the descendant form, ban the old one.
  it('scopes the destructive icon color with a descendant selector', () => {
    render(
      <DropdownMenu open>
        <DropdownMenuTrigger>Open</DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem variant="destructive">
            <span>
              <svg aria-label="trash" />
            </span>
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    )

    const item = screen.getByText('Delete').closest('[data-slot="dropdown-menu-item"]')
    expect(item?.className.replaceAll('"', "'")).toContain("data-[variant=destructive]:[&_svg:not([class*='text-'])]:text-destructive!")
    expect(item).not.toHaveClass('data-[variant=destructive]:*:[svg]:text-destructive!')
  })

  /**
   * The content stays mounted for its 200ms leave animation, and the dying layer
   * still counts as a dismissable layer. A press on the trigger in that window is
   * an "outside" press for it, so without the guard it closes the menu the press
   * just opened and the trigger looks dead — reported as issue #14, where a user
   * could not reopen the permission menu at all.
   *
   * jsdom cannot deliver the pointerdown that Radix turns into this callback, so
   * the decision the callback makes is pinned directly; the integration is
   * covered by the composer's permission-posture end-to-end spec.
   */
  it('treats a press inside its own trigger as a trigger press', () => {
    render(
      <DropdownMenu open>
        <DropdownMenuTrigger>
          <span>Open</span>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem>Item</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    )

    const label = screen.getByText('Open')
    expect(isTriggerPress({ target: label })).toBe(true)
    expect(isTriggerPress({ target: screen.getByText('Item') })).toBe(false)
    expect(isTriggerPress({ target: document.body })).toBe(false)
    expect(isTriggerPress({ target: null })).toBe(false)
  })

  it('leaves the animating-out content inert so it cannot swallow a press', () => {
    render(
      <DropdownMenu open>
        <DropdownMenuTrigger>Open</DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem>Item</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    )

    expect(screen.getByText('Item').closest('[data-slot="dropdown-menu-content"]')).toHaveClass(
      'data-[state=closed]:pointer-events-none'
    )
  })
})
