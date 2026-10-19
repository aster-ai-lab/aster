import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { LucideIcon } from 'lucide-react'
import { Search } from 'lucide-react'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  getSidebarDisplayWidth,
  normalizeSidebarWidth,
  SIDEBAR_FULL_THRESHOLD,
  SIDEBAR_HIDDEN_THRESHOLD,
  SIDEBAR_ICON_WIDTH,
  SIDEBAR_MAX_WIDTH
} from '../constants'
import { Sidebar } from '../Sidebar'
import type { ResolvedSidebarEntry } from '../types'

type AppItem = {
  id: string
  label: string
  icon: LucideIcon
}

vi.mock('@a3s-lab/ui', () => ({
  MenuItem: ({
    icon,
    label,
    onClick,
    onMouseDown,
    onAuxClick,
    className,
    active
  }: {
    icon?: ReactNode
    label: string
    onClick?: () => void
    onMouseDown?: (e: React.MouseEvent) => void
    onAuxClick?: (e: React.MouseEvent) => void
    className?: string
    active?: boolean
  }) => (
    <button
      type="button"
      data-active={active ? 'true' : 'false'}
      className={className}
      onClick={onClick}
      onMouseDown={onMouseDown}
      onAuxClick={onAuxClick}>
      {icon}
      <span>{label}</span>
    </button>
  )
}))

vi.mock('../Tooltip', () => ({
  SidebarTooltip: ({ children }: { children: ReactNode }) => children
}))

vi.mock('@renderer/hooks/useMacTransparentWindow', () => ({
  default: () => false
}))

// Build the type-agnostic resolved entries the real registry would produce, so the
// presentation tests exercise the same shape without depending on app wiring.
const appEntry = (item: AppItem): ResolvedSidebarEntry => ({
  key: `app:${item.id}`,
  label: item.label,
  renderIcon: ({ glyphSize }) => {
    const Icon = item.icon
    return <Icon size={glyphSize} strokeWidth={1.6} />
  },
  isActive: item.id === 'chat',
  onOpen: () => {}
})
const items: AppItem[] = [
  {
    id: 'chat',
    label: 'Chat',
    icon: Search
  }
]
const entries: ResolvedSidebarEntry[] = items.map(appEntry)

const INTERMEDIATE_WIDTH = SIDEBAR_ICON_WIDTH + 30

afterEach(() => {
  vi.useRealTimers()
})

function dragResizeFrom(width: number, moves: number | number[]) {
  const setWidth = vi.fn()
  const onResizePreview = vi.fn()
  const onHoverChange = vi.fn()
  const { container, unmount } = render(
    <Sidebar
      width={width}
      setWidth={setWidth}

      entries={entries}
      onHoverChange={onHoverChange}
      onResizePreview={onResizePreview}
    />
  )
  const resizeHandle = container.querySelector('.cursor-col-resize') as HTMLElement

  fireEvent.mouseDown(resizeHandle, { clientX: width })
  for (const clientX of [moves].flat()) {
    fireEvent.mouseMove(document, { clientX })
  }
  fireEvent.mouseUp(document)

  return { setWidth, onResizePreview, onHoverChange, unmount }
}

describe('Sidebar resize handle', () => {
  it('opts the resize handle out of window drag regions', () => {
    const { container } = render(<Sidebar width={SIDEBAR_FULL_THRESHOLD} setWidth={vi.fn()} entries={entries} />)

    const resizeHandle = container.querySelector('.cursor-col-resize')

    expect(resizeHandle).toBeInTheDocument()
    // Electron drag-region opt-out keeps the resize handle interactive.
    expect(resizeHandle).toHaveClass('[-webkit-app-region:no-drag]')
  })

  it('does not render a drag handle on the ActivityBar', () => {
    const { container } = render(<Sidebar width={SIDEBAR_ICON_WIDTH} setWidth={vi.fn()} entries={entries} />)

    expect(container.querySelector('.cursor-col-resize')).not.toBeInTheDocument()
  })

  it('previews intermediate widths and snaps release by drag direction', () => {
    const cases: Array<[number, number, number]> = [
      [SIDEBAR_FULL_THRESHOLD, INTERMEDIATE_WIDTH, SIDEBAR_ICON_WIDTH],
      [SIDEBAR_FULL_THRESHOLD + 50, SIDEBAR_FULL_THRESHOLD - 10, SIDEBAR_ICON_WIDTH]
    ]

    for (const [start, moveTo, released] of cases) {
      const { setWidth, onResizePreview, unmount } = dragResizeFrom(start, moveTo)

      expect(onResizePreview).toHaveBeenNthCalledWith(1, moveTo)
      expect(onResizePreview).toHaveBeenLastCalledWith(null)
      expect(setWidth).toHaveBeenCalledTimes(1)
      expect(setWidth).toHaveBeenLastCalledWith(released)
      unmount()
    }
  })

  it('keeps non-intermediate drag behavior', () => {
    const cases: Array<[number, number]> = [
      [SIDEBAR_HIDDEN_THRESHOLD - 10, 0],
      [SIDEBAR_HIDDEN_THRESHOLD + 10, SIDEBAR_ICON_WIDTH],
      [SIDEBAR_FULL_THRESHOLD + 10, SIDEBAR_FULL_THRESHOLD + 10],
      [SIDEBAR_MAX_WIDTH + 20, SIDEBAR_MAX_WIDTH]
    ]

    for (const [moveTo, expected] of cases) {
      const { setWidth, unmount } = dragResizeFrom(SIDEBAR_FULL_THRESHOLD, moveTo)

      expect(setWidth).toHaveBeenCalledTimes(1)
      expect(setWidth).toHaveBeenLastCalledWith(expected)
      unmount()
    }
  })

  it('stops tracking the mouse and restores the cursor after release', () => {
    const { setWidth, onResizePreview } = dragResizeFrom(SIDEBAR_FULL_THRESHOLD, SIDEBAR_FULL_THRESHOLD + 10)

    expect(document.body.style.cursor).toBe('')
    expect(document.body.style.userSelect).toBe('')

    const setWidthCalls = setWidth.mock.calls.length
    const previewCalls = onResizePreview.mock.calls.length

    fireEvent.mouseMove(document, { clientX: SIDEBAR_FULL_THRESHOLD + 40 })

    expect(setWidth).toHaveBeenCalledTimes(setWidthCalls)
    expect(onResizePreview).toHaveBeenCalledTimes(previewCalls)
  })

  it('renders intermediate widths with icon layout without menu text', () => {
    const { container, queryByText } = render(
      <Sidebar width={INTERMEDIATE_WIDTH} setWidth={vi.fn()} entries={entries} />
    )

    expect(container.firstElementChild).toHaveStyle({ width: `${INTERMEDIATE_WIDTH}px` })
    expect(queryByText('Chat')).not.toBeInTheDocument()
  })

  it('resolves display widths for CSS variable consumers', () => {
    expect(getSidebarDisplayWidth(SIDEBAR_HIDDEN_THRESHOLD + 10)).toBe(SIDEBAR_ICON_WIDTH)
    expect(getSidebarDisplayWidth(INTERMEDIATE_WIDTH)).toBe(INTERMEDIATE_WIDTH)
    expect(getSidebarDisplayWidth(SIDEBAR_FULL_THRESHOLD)).toBe(SIDEBAR_FULL_THRESHOLD)
  })

  it('normalizes persisted intermediate widths to icon width', () => {
    expect(normalizeSidebarWidth(SIDEBAR_ICON_WIDTH)).toBe(SIDEBAR_ICON_WIDTH)
    expect(normalizeSidebarWidth(INTERMEDIATE_WIDTH)).toBe(SIDEBAR_ICON_WIDTH)
    expect(normalizeSidebarWidth(SIDEBAR_FULL_THRESHOLD)).toBe(SIDEBAR_FULL_THRESHOLD)
  })

  it('keeps the hidden-state hot zone full height without moving the resize binding', () => {
    const { container } = render(
      <Sidebar
        width={SIDEBAR_HIDDEN_THRESHOLD - 10}
        setWidth={vi.fn()}

        entries={entries}
      />
    )

    const resizeHandle = container.querySelector('.cursor-col-resize') as HTMLElement
    const hotZone = resizeHandle.parentElement

    // The hidden sidebar still needs a full-height interactive resize target outside the window drag region.
    expect(resizeHandle).toHaveClass('h-full', 'w-full', 'cursor-col-resize')
    expect(hotZone).toHaveClass('absolute', 'inset-y-0', 'left-0', 'z-50', 'w-4')
    expect(hotZone).toHaveClass('[-webkit-app-region:no-drag]')
  })

  it('restores a hidden ActivityBar by dragging wider from the hot zone', () => {
    const { setWidth, onResizePreview, onHoverChange } = dragResizeFrom(
      SIDEBAR_HIDDEN_THRESHOLD - 10,
      INTERMEDIATE_WIDTH
    )

    expect(onHoverChange).toHaveBeenCalledWith(false)
    expect(onResizePreview).not.toHaveBeenCalledWith(INTERMEDIATE_WIDTH)
    expect(setWidth).toHaveBeenCalled()
    expect(setWidth).toHaveBeenLastCalledWith(SIDEBAR_ICON_WIDTH)
  })

  it('renders the full layout at the full threshold', () => {
    const { container, getByText } = render(
      <Sidebar width={SIDEBAR_FULL_THRESHOLD} setWidth={vi.fn()} entries={entries} />
    )

    expect(container.firstElementChild).toHaveStyle({ width: `${SIDEBAR_FULL_THRESHOLD}px` })
    expect(getByText('Chat')).toBeInTheDocument()
  })

  it('runs the header action when the visible title is clicked', async () => {
    const user = userEvent.setup()
    const onHeaderClick = vi.fn()

    render(
      <Sidebar
        width={SIDEBAR_FULL_THRESHOLD}
        setWidth={vi.fn()}

        entries={entries}
        title="User"
        logo={<span>avatar</span>}
        onHeaderClick={onHeaderClick}
      />
    )

    const headerAction = screen.getByRole('button', { name: /User$/ })
    // Interactive controls must opt out of Electron's window drag region.
    expect(headerAction).toHaveClass('[-webkit-app-region:no-drag]')
    // The sidebar foreground token must win over MenuItem's generic foreground.
    expect(headerAction).toHaveClass('text-sidebar-foreground')

    await user.click(headerAction)

    expect(onHeaderClick).toHaveBeenCalledTimes(1)
  })

  it('keeps the floating sidebar open while a footer overlay is open', () => {
    vi.useFakeTimers()
    const onDismiss = vi.fn()
    let onOverlayOpenChange: ((open: boolean) => void) | undefined

    try {
      const { container } = render(
        <Sidebar
          width={SIDEBAR_FULL_THRESHOLD}
          setWidth={vi.fn()}

          entries={entries}
          actions={(_layout, handleOverlayOpenChange) => {
            onOverlayOpenChange = handleOverlayOpenChange
            return <button type="button">Help</button>
          }}
          isFloating
          onDismiss={onDismiss}
        />
      )

      const panel = container.querySelector('.slide-in-from-left-2') as HTMLElement

      fireEvent.mouseEnter(panel)
      act(() => onOverlayOpenChange?.(true))
      fireEvent.mouseLeave(panel)
      vi.advanceTimersByTime(350)

      expect(onDismiss).not.toHaveBeenCalled()

      act(() => onOverlayOpenChange?.(false))
      vi.advanceTimersByTime(350)

      expect(onDismiss).toHaveBeenCalledTimes(1)
    } finally {
      vi.useRealTimers()
    }
  })

  it('names icon-only resource buttons from their full label', () => {
    render(
      <Sidebar
        width={SIDEBAR_ICON_WIDTH}
        setWidth={vi.fn()}

        entries={[...entries, appEntry({ id: 'custom', label: 'Custom Tool', icon: Search })]}
      />
    )

    expect(screen.getByRole('button', { name: 'Custom Tool' })).toBeInTheDocument()
  })

  it('renders footer actions with the current sidebar layout', () => {
    const renderActions = (layout: 'icon' | 'full') => <button type="button">theme-{layout}</button>

    const { rerender } = render(
      <Sidebar
        width={SIDEBAR_ICON_WIDTH}
        setWidth={vi.fn()}

        entries={entries}
        actions={renderActions}
      />
    )

    expect(document.body).toHaveTextContent('theme-icon')

    rerender(
      <Sidebar
        width={SIDEBAR_FULL_THRESHOLD}
        setWidth={vi.fn()}

        entries={entries}
        actions={renderActions}
      />
    )

    expect(document.body).toHaveTextContent('theme-full')
    expect(document.body).not.toHaveTextContent('theme-icon')
  })

  it('triggers onOpenNewTab on middle-click (auxclick with button 1) in icon layout', () => {
    const onChatOpen = vi.fn()
    const onChatOpenNewTab = vi.fn()
    const testEntries: ResolvedSidebarEntry[] = [
      {
        key: 'app:chat',
        label: 'Chat',
        renderIcon: () => <span>chat-icon</span>,
        isActive: false,
        onOpen: onChatOpen,
        onOpenNewTab: onChatOpenNewTab
      }
    ]

    render(<Sidebar width={SIDEBAR_ICON_WIDTH} setWidth={vi.fn()} entries={testEntries} />)

    const button = screen.getByRole('button', { name: 'Chat' })
    const mouseDown = new MouseEvent('mousedown', { button: 1, bubbles: true, cancelable: true })
    fireEvent(button, mouseDown)
    fireEvent(button, new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true }))

    expect(mouseDown.defaultPrevented).toBe(true)
    expect(onChatOpenNewTab).toHaveBeenCalledTimes(1)
    expect(onChatOpen).not.toHaveBeenCalled()
  })

  it('triggers onOpenNewTab on middle-click (auxclick with button 1) in full layout', () => {
    const onChatOpen = vi.fn()
    const onChatOpenNewTab = vi.fn()
    const testEntries: ResolvedSidebarEntry[] = [
      {
        key: 'app:chat',
        label: 'Chat',
        renderIcon: () => <span>chat-icon</span>,
        isActive: false,
        onOpen: onChatOpen,
        onOpenNewTab: onChatOpenNewTab
      }
    ]

    render(
      <Sidebar
        width={SIDEBAR_FULL_THRESHOLD}
        setWidth={vi.fn()}

        entries={testEntries}
      />
    )

    const button = screen.getByRole('button', { name: /Chat/ })
    const mouseDown = new MouseEvent('mousedown', { button: 1, bubbles: true, cancelable: true })
    fireEvent(button, mouseDown)
    fireEvent(button, new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true }))

    expect(mouseDown.defaultPrevented).toBe(true)
    expect(onChatOpenNewTab).toHaveBeenCalledTimes(1)
    expect(onChatOpen).not.toHaveBeenCalled()
  })

  it('does not trigger onOpenNewTab on auxclick with non-middle button', () => {
    const onChatOpen = vi.fn()
    const onChatOpenNewTab = vi.fn()
    const testEntries: ResolvedSidebarEntry[] = [
      {
        key: 'app:chat',
        label: 'Chat',
        renderIcon: () => <span>chat-icon</span>,
        isActive: false,
        onOpen: onChatOpen,
        onOpenNewTab: onChatOpenNewTab
      }
    ]

    render(<Sidebar width={SIDEBAR_ICON_WIDTH} setWidth={vi.fn()} entries={testEntries} />)

    const button = screen.getByRole('button', { name: 'Chat' })
    const mouseDown = new MouseEvent('mousedown', { button: 2, bubbles: true, cancelable: true })
    fireEvent(button, mouseDown)
    fireEvent(button, new MouseEvent('auxclick', { button: 2, bubbles: true, cancelable: true }))

    expect(mouseDown.defaultPrevented).toBe(false)
    expect(onChatOpenNewTab).not.toHaveBeenCalled()
    expect(onChatOpen).not.toHaveBeenCalled()
  })
})

describe('Sidebar icon presentation', () => {
  it.each([
    [SIDEBAR_FULL_THRESHOLD, { slotSize: 18, glyphSize: 16 }],
    [SIDEBAR_ICON_WIDTH, { slotSize: 22, glyphSize: 22 }]
  ])('uses one fixed icon slot at width %s', (width, expectedPresentation) => {
    const presentations: unknown[] = []
    const mixedEntries: ResolvedSidebarEntry[] = ['glyph', 'avatar'].map((kind) => ({
      key: kind,
      label: kind,
      renderIcon: (presentation: unknown) => {
        presentations.push(presentation)
        return <span>{kind}</span>
      },
      isActive: false,
      onOpen: vi.fn()
    }))

    const { container } = render(<Sidebar width={width} setWidth={vi.fn()} entries={mixedEntries} />)

    expect(presentations).toEqual([expectedPresentation, expectedPresentation])
    const slots = [...container.querySelectorAll('[data-slot="sidebar-entry-icon"]')]
    expect(slots).toHaveLength(2)
    for (const slot of slots) {
      expect(slot).toHaveStyle({
        width: `${expectedPresentation.slotSize}px`,
        height: `${expectedPresentation.slotSize}px`
      })
    }
  })
})

describe('ActivityBar reorder', () => {
  const railEntries = ['工作', '消息', '知识'].map(
    (label): ResolvedSidebarEntry => ({
      key: label,
      label,
      renderIcon: () => <span>{label}</span>,
      isActive: false,
      onOpen: vi.fn()
    })
  )

  it('reorders the upper rail when an icon is dragged and keeps a short press as a click', () => {
    const onReorder = vi.fn()
    const onOpen = vi.fn()
    render(
      <Sidebar
        width={SIDEBAR_ICON_WIDTH}
        setWidth={vi.fn()}
        entries={railEntries.map((entry) => ({ ...entry, onOpen }))}
        onReorder={onReorder}
      />
    )
    const buttons = ['工作', '消息', '知识'].map((name) => screen.getByRole('button', { name }))
    buttons.forEach((button, index) => {
      button.getBoundingClientRect = () =>
        ({
          left: 0,
          top: index * 52,
          width: 36,
          height: 36,
          right: 36,
          bottom: index * 52 + 36,
          x: 0,
          y: index * 52,
          toJSON() {
            return {}
          }
        }) as DOMRect
    })

    fireEvent.click(buttons[0])
    expect(onOpen).toHaveBeenCalledTimes(1)
    expect(onReorder).not.toHaveBeenCalled()
    expect(buttons[0]).toHaveClass('cursor-pointer')
    expect(buttons[0].className).not.toContain('cursor-grab')

    class TestPointerEvent extends MouseEvent {
      pointerId: number
      constructor(type: string, init: MouseEventInit & { pointerId?: number } = {}) {
        super(type, init)
        this.pointerId = init.pointerId ?? 1
      }
    }
    vi.stubGlobal('PointerEvent', TestPointerEvent)

    fireEvent.pointerDown(buttons[2], { button: 0, clientX: 18, clientY: 122, pointerId: 1 })
    fireEvent.pointerMove(buttons[2], { clientX: 18, clientY: 10, pointerId: 1 })
    expect(document.querySelector('[data-ui="activity-bar.drag"]')).toBeTruthy()
    expect(buttons[2]).toHaveClass('cursor-grabbing')
    fireEvent.pointerUp(buttons[2], { clientX: 18, clientY: 10, pointerId: 1 })

    expect(onReorder).toHaveBeenCalledWith(['知识', '工作', '消息'])
    expect(onOpen).toHaveBeenCalledTimes(1)
  })
})
