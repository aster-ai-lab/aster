// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest'

import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '../context-menu'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '../dropdown-menu'
import { HoverCard, HoverCardContent, HoverCardTrigger } from '../hover-card'
import { Popover, PopoverContent, PopoverTrigger } from '../popover'

beforeAll(() => {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  HTMLElement.prototype.hasPointerCapture ??= () => false
  HTMLElement.prototype.setPointerCapture ??= () => undefined
  HTMLElement.prototype.releasePointerCapture ??= () => undefined
})
afterEach(cleanup)

function Popup({ kind }: { kind: 'menu' | 'popover' }) {
  return kind === 'menu' ? (
    <DropdownMenu>
      <DropdownMenuTrigger>Open</DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem>Inside</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  ) : (
    <Popover>
      <PopoverTrigger>Open</PopoverTrigger>
      <PopoverContent>
        <button>Inside</button>
      </PopoverContent>
    </Popover>
  )
}

describe.each(['menu', 'popover'] as const)('%s dismissal contract', (kind) => {
  it('closes on a native background click directed to the window', async () => {
    const user = userEvent.setup()
    render(<Popup kind={kind} />)
    await user.click(screen.getByText('Open'))
    expect(screen.getByText('Inside')).toBeVisible()
    fireEvent.click(window)
    await waitFor(() => expect(screen.queryByText('Inside')).not.toBeInTheDocument())
  })

  it('closes without consuming the first outside click or stealing its focus', async () => {
    const user = userEvent.setup()
    const clicked = vi.fn()
    render(
      <>
        <Popup kind={kind} />
        <input aria-label="Outside" onPointerDown={(e) => e.stopPropagation()} onClick={clicked} />
      </>
    )
    await user.click(screen.getByText('Open'))
    await user.click(screen.getByRole('textbox', { name: 'Outside' }))
    await waitFor(() => expect(screen.queryByText('Inside')).not.toBeInTheDocument())
    expect(clicked).toHaveBeenCalledOnce()
    expect(screen.getByRole('textbox', { name: 'Outside' })).toHaveFocus()
  })

  it('closes when the application loses focus', async () => {
    const user = userEvent.setup()
    render(<Popup kind={kind} />)
    await user.click(screen.getByText('Open'))
    fireEvent.blur(window)
    await waitFor(() => expect(screen.queryByText('Inside')).not.toBeInTheDocument())
  })
})

it('keeps a parent popover open while using a nested portalled menu', async () => {
  const user = userEvent.setup()
  render(
    <Popover>
      <PopoverTrigger>Parent</PopoverTrigger>
      <PopoverContent>
        <span>Parent content</span>
        <Popup kind="menu" />
      </PopoverContent>
    </Popover>
  )
  await user.click(screen.getByText('Parent'))
  await user.click(screen.getByText('Open'))
  await user.click(screen.getByText('Inside'))
  expect(screen.getByText('Parent content')).toBeVisible()
})

it('lets a context menu dismiss and focus the first outside click', async () => {
  const user = userEvent.setup()
  const clicked = vi.fn()
  render(
    <>
      <ContextMenu>
        <ContextMenuTrigger>Context</ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem>Inside</ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
      <input aria-label="Outside" onClick={clicked} />
    </>
  )
  await user.pointer({ keys: '[MouseRight]', target: screen.getByText('Context') })
  expect(screen.getByText('Inside')).toBeVisible()
  await user.click(screen.getByRole('textbox', { name: 'Outside' }))
  await waitFor(() => expect(screen.queryByText('Inside')).not.toBeInTheDocument())
  expect(clicked).toHaveBeenCalledOnce()
  expect(screen.getByRole('textbox', { name: 'Outside' })).toHaveFocus()
})

it.each(['background', 'blur'])('closes a context menu on native %s interaction', async (action) => {
  const user = userEvent.setup()
  render(
    <ContextMenu>
      <ContextMenuTrigger>Context</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem>Inside</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
  await user.pointer({ keys: '[MouseRight]', target: screen.getByText('Context') })
  expect(screen.getByText('Inside')).toBeVisible()
  if (action === 'background') fireEvent.click(window)
  else fireEvent.blur(window)
  await waitFor(() => expect(screen.queryByText('Inside')).not.toBeInTheDocument())
})

it.each(['background', 'blur'])('closes a hover card on %s interaction', async (action) => {
  render(
    <HoverCard defaultOpen>
      <HoverCardTrigger>Hover</HoverCardTrigger>
      <HoverCardContent>Inside</HoverCardContent>
    </HoverCard>
  )
  if (action === 'background') fireEvent.click(window)
  else fireEvent.blur(window)
  await waitFor(() => expect(screen.queryByText('Inside')).not.toBeInTheDocument())
})
