import {
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  type RefObject
} from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import { ArrowRight, Files, PanelBottomClose, X } from 'lucide-react'

export type WorkbenchTabContextAction = 'close' | 'close-others' | 'close-right' | 'close-all'

export interface WorkbenchTabContextMenuState {
  tabKey: string
  x: number
  y: number
}

function nextMenuIndex(itemCount: number, currentIndex: number, delta: 1 | -1): number {
  if (itemCount <= 0) return -1
  const start = currentIndex >= 0 && currentIndex < itemCount ? currentIndex : delta === 1 ? -1 : 0
  return (start + delta + itemCount) % itemCount
}

/**
 * Cursor-style tab menu used by the task workspace and its terminal strip.
 * The menu is portaled so a clipped pane cannot hide it.
 */
export function WorkbenchTabContextMenu({
  canCloseOthers,
  canCloseRight,
  id,
  label,
  menuRef,
  onAction,
  onKeyDown,
  x,
  y
}: {
  canCloseOthers: boolean
  canCloseRight: boolean
  id: string
  label: string
  menuRef: RefObject<HTMLDivElement | null>
  onAction: (action: WorkbenchTabContextAction) => void
  onKeyDown: (event: ReactKeyboardEvent<HTMLDivElement>) => void
  x: number
  y: number
}) {
  const { t } = useTranslation()
  const menuLabel = t('workbench.workspace.context.label', '标签页操作')
  const item = (
    action: WorkbenchTabContextAction,
    text: string,
    icon: ReactNode,
    enabled = true
  ) => (
    <button
      type="button"
      role="menuitem"
      disabled={!enabled}
      data-chrome-tab-context-action={action}
      className="flex min-h-8 w-full items-center gap-2 rounded-[7px] px-2 text-left text-[12px] text-[#171717] hover:bg-[#f3f3f3] focus-visible:bg-[#f3f3f3] focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
      onClick={() => {
        if (!enabled) return
        onAction(action)
      }}>
      {icon}
      <span>{text}</span>
    </button>
  )
  const menu = (
    <div
      ref={menuRef}
      id={id}
      role="menu"
      aria-label={`${label} · ${menuLabel}`}
      data-chrome-tab-context-menu="true"
      className="fixed z-[200] grid w-52 gap-0.5 rounded-[12px] border border-[#d8d8d8] bg-white p-1.5 text-[#171717] shadow-[0_12px_32px_rgb(0_0_0/0.16)]"
      style={
        {
          top: `min(${y}px, calc(100dvh - 17rem))`,
          left: `min(${x}px, calc(100vw - 14rem))`
        } as CSSProperties
      }
      onKeyDown={onKeyDown}
      onPointerDown={(event) => event.stopPropagation()}>
      {item('close', t('workbench.workspace.context.close', '关闭标签页'), <X size={14} strokeWidth={1.7} />)}
      {item(
        'close-others',
        t('workbench.workspace.context.close_others', '关闭其他标签页'),
        <Files size={14} strokeWidth={1.7} />,
        canCloseOthers
      )}
      {item(
        'close-right',
        t('workbench.workspace.context.close_right', '关闭右侧标签页'),
        <ArrowRight size={14} strokeWidth={1.7} />,
        canCloseRight
      )}
      {item(
        'close-all',
        t('workbench.workspace.context.close_all', '关闭所有标签页'),
        <PanelBottomClose size={14} strokeWidth={1.7} />
      )}
    </div>
  )
  if (typeof document === 'undefined') return menu
  return createPortal(menu, document.body)
}

export function useWorkbenchTabContextMenu(tabKeys: readonly string[]) {
  const componentId = useId().replaceAll(':', '')
  const menuId = `chrome-tab-context-menu-${componentId}`
  const menuRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLElement | undefined>(undefined)
  const [menu, setMenu] = useState<WorkbenchTabContextMenuState>()

  const dismiss = (restoreFocus = false) => {
    const trigger = triggerRef.current
    triggerRef.current = undefined
    setMenu(undefined)
    if (restoreFocus && trigger) {
      requestAnimationFrame(() => {
        if (trigger.isConnected) trigger.focus()
      })
    }
  }

  const open = (tabKey: string, x: number, y: number, trigger: HTMLElement) => {
    triggerRef.current = trigger
    setMenu({ tabKey, x, y })
  }

  useEffect(() => {
    if (!menu) return
    if (!tabKeys.includes(menu.tabKey)) {
      dismiss()
      return
    }
    const frame = requestAnimationFrame(() => {
      menuRef.current?.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')?.focus()
    })
    const dismissFromOutside = (event: PointerEvent) => {
      const target = event.target
      if (target instanceof Node && menuRef.current?.contains(target)) return
      dismiss()
    }
    const dismissFromViewportChange = () => dismiss()
    document.addEventListener('pointerdown', dismissFromOutside)
    window.addEventListener('blur', dismissFromViewportChange)
    window.addEventListener('resize', dismissFromViewportChange)
    window.addEventListener('scroll', dismissFromViewportChange, true)
    return () => {
      cancelAnimationFrame(frame)
      document.removeEventListener('pointerdown', dismissFromOutside)
      window.removeEventListener('blur', dismissFromViewportChange)
      window.removeEventListener('resize', dismissFromViewportChange)
      window.removeEventListener('scroll', dismissFromViewportChange, true)
    }
  }, [menu, tabKeys])

  const onMenuKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const items = [...(menuRef.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? [])]
    if (event.key === 'Escape' || event.key === 'Tab') {
      event.preventDefault()
      dismiss(true)
      return
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    event.preventDefault()
    const currentIndex = items.findIndex((item) => item === document.activeElement)
    const nextIndex = nextMenuIndex(items.length, currentIndex, event.key === 'ArrowDown' ? 1 : -1)
    items[nextIndex]?.focus()
  }

  const openFromContextMenu = (tabKey: string, event: ReactMouseEvent<HTMLElement>) => {
    event.preventDefault()
    event.stopPropagation()
    const trigger = event.currentTarget.querySelector<HTMLElement>('[role="tab"]') ?? event.currentTarget
    open(tabKey, event.clientX, event.clientY, trigger)
  }

  const openFromKeyboard = (tabKey: string, event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return false
    event.preventDefault()
    event.stopPropagation()
    const rect = event.currentTarget.getBoundingClientRect()
    open(tabKey, rect.left + 8, rect.bottom + 4, event.currentTarget)
    return true
  }

  return { dismiss, menu, menuId, menuRef, onMenuKeyDown, openFromContextMenu, openFromKeyboard }
}
