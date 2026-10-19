import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { SidebarList } from '../SidebarList'

vi.unmock('@a3s-lab/ui')

describe('unavailable sidebar shortcuts', () => {
  it.each(['icon', 'full'] as const)('stays focusable but cannot activate in %s layout', async (layout) => {
    const onOpen = vi.fn()
    const onOpenNewTab = vi.fn()
    const user = userEvent.setup()
    render(
      <SidebarList
        layout={layout}

        entries={[
          {
            key: 'unavailable',
            label: 'Unavailable resource',
            renderIcon: () => null,
            isActive: false,
            disabled: true,
            statusLabel: 'Resource temporarily unavailable',
            onOpen,
            onOpenNewTab
          }
        ]}
      />
    )
    const button = screen.getByRole('button', { name: 'Unavailable resource' })
    expect(button).toHaveAttribute('aria-disabled', 'true')
    expect(button).toHaveAccessibleDescription('Resource temporarily unavailable')
    await user.click(button)
    expect(button).toHaveFocus()
    await user.keyboard('{Enter} ')
    fireEvent(button, new MouseEvent('auxclick', { bubbles: true, button: 1 }))
    expect(onOpen).not.toHaveBeenCalled()
    expect(onOpenNewTab).not.toHaveBeenCalled()
  })
})

describe('sidebar entry badge', () => {
  it.each(['icon', 'full'] as const)('shows the unread count in %s layout and hides zero', (layout) => {
    const base = { renderIcon: () => null, isActive: false, onOpen: vi.fn() }
    render(
      <SidebarList
        layout={layout}
        entries={[
          { ...base, key: 'messages', label: 'Messages', badge: 120 },
          { ...base, key: 'files', label: 'Files', badge: 0 }
        ]}
      />
    )
    const badges = document.querySelectorAll('[data-ui="sidebar.entry-badge"]')
    expect(badges).toHaveLength(1)
    expect(badges[0]).toHaveTextContent('99+')
    expect(screen.getByRole('button', { name: 'Messages' })).toContainElement(badges[0] as HTMLElement)
  })

  it.each(['icon', 'full'] as const)('draws a red dot without a count in %s layout', (layout) => {
    const base = { renderIcon: () => null, isActive: false, onOpen: vi.fn() }
    render(
      <SidebarList
        layout={layout}
        entries={[
          { ...base, key: 'messages', label: 'Messages', badgeDot: true },
          { ...base, key: 'work', label: 'Work', badgeDot: false }
        ]}
      />
    )
    const dots = document.querySelectorAll('[data-ui="sidebar.entry-dot"]')
    expect(dots).toHaveLength(1)
    expect(dots[0]).toBeEmptyDOMElement()
    expect(dots[0]).toHaveClass('bg-[#E23B3B]')
    expect(screen.getByRole('button', { name: 'Messages' })).toContainElement(dots[0] as HTMLElement)
    expect(document.querySelector('[data-ui="sidebar.entry-badge"]')).toBeNull()
  })
})
