import { Ellipsis } from 'lucide-react'
import { useEffect, useId, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

export type ItemActionIcon = {
  key: string
  label: string
  icon: ReactNode
  disabled?: boolean
  testId?: string
  onClick: () => void
}

export type ItemActionMenuEntry =
  | { type: 'separator'; key: string }
  | {
      type: 'item'
      key: string
      label: string
      icon?: ReactNode
      disabled?: boolean
      /** Danger copy stays #b42318. The row is not filled red. */
      tone?: 'danger'
      onClick: () => void
    }

const iconButtonClassName =
  'flex size-7 shrink-0 items-center justify-center rounded-[6px] text-[#5f5f5f] outline-none hover:bg-[#e6e7ea] focus-visible:ring-2 focus-visible:ring-black/10 disabled:pointer-events-none disabled:opacity-40'

type MenuBox = { top?: number; bottom?: number; right: number }

function measureMenu(button: HTMLButtonElement): MenuBox {
  const rect = button.getBoundingClientRect()
  const right = Math.max(8, window.innerWidth - rect.right)
  const spaceBelow = window.innerHeight - rect.bottom
  if (spaceBelow < 220 && rect.top > spaceBelow) {
    return { bottom: window.innerHeight - rect.top + 4, right }
  }
  return { top: rect.bottom + 4, right }
}

/** 名称格右侧留白，避免悬停按钮盖住标题。类名必须写成完整字符串，Tailwind 才会生成。 */
export function itemActionPadClass(count: number): string {
  if (count <= 0) return ''
  if (count === 1) return 'group-hover:pr-8 group-focus-within:pr-8 group-has-[[aria-expanded=true]]:pr-8'
  if (count === 2) return 'group-hover:pr-16 group-focus-within:pr-16 group-has-[[aria-expanded=true]]:pr-16'
  if (count === 3) return 'group-hover:pr-24 group-focus-within:pr-24 group-has-[[aria-expanded=true]]:pr-24'
  if (count === 4) return 'group-hover:pr-32 group-focus-within:pr-32 group-has-[[aria-expanded=true]]:pr-32'
  return 'group-hover:pr-40 group-focus-within:pr-40 group-has-[[aria-expanded=true]]:pr-40'
}

/**
 * 列表项主单元格里的操作栏。默认盖在格子右侧，行悬停、焦点或菜单打开时出现。
 * 行内是图标按钮，最后的更多按钮打开文字下拉。父行需要 `group`，格子需要 `relative`。
 * `placement="inline"` 时不自己定位，由外层决定位置。
 */
export function ItemActionBar({
  inline,
  menu = [],
  moreLabel,
  placement = 'overlay',
  buttonClassName,
  className
}: {
  inline: ItemActionIcon[]
  menu?: ItemActionMenuEntry[]
  moreLabel?: string
  placement?: 'overlay' | 'inline'
  /** Replaces the default 28px icon button. Team tables pass the 24px row action. */
  buttonClassName?: string
  className?: string
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [box, setBox] = useState<MenuBox | null>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const moreRef = useRef<HTMLButtonElement>(null)
  const menuId = useId()
  const label = moreLabel ?? t('common.more')
  const entries: ItemActionMenuEntry[] = []
  for (const entry of menu) {
    if (entry.type === 'separator') {
      if (entries.length === 0 || entries[entries.length - 1]?.type === 'separator') continue
      entries.push(entry)
      continue
    }
    entries.push(entry)
  }
  if (entries[entries.length - 1]?.type === 'separator') entries.pop()
  const showMore = entries.some((entry) => entry.type === 'item')

  useEffect(() => {
    if (!open) return
    const close = (event: MouseEvent) => {
      const target = event.target as Node
      if (rootRef.current?.contains(target) || menuRef.current?.contains(target)) return
      setOpen(false)
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false)
    }
    const dismiss = () => setOpen(false)
    document.addEventListener('mousedown', close)
    document.addEventListener('keydown', onKey)
    window.addEventListener('scroll', dismiss, true)
    window.addEventListener('resize', dismiss)
    return () => {
      document.removeEventListener('mousedown', close)
      document.removeEventListener('keydown', onKey)
      window.removeEventListener('scroll', dismiss, true)
      window.removeEventListener('resize', dismiss)
    }
  }, [open])

  if (inline.length === 0 && !showMore) return null

  const menuNode =
    open && box
      ? createPortal(
          <div
            ref={menuRef}
            id={menuId}
            data-ui="item-action-menu"
            style={{ position: 'fixed', top: box.top, bottom: box.bottom, right: box.right }}
            className="z-50 min-w-[168px] rounded-[10px] border border-[#ebebeb] bg-white py-1 shadow-[0_8px_22px_rgba(20,40,80,0.08)]">
            {entries.map((entry) =>
              entry.type === 'separator' ? (
                <div key={entry.key} className="my-1 h-px bg-[#efefef]" />
              ) : (
                <button
                  key={entry.key}
                  type="button"
                  disabled={entry.disabled}
                  onClick={(event) => {
                    event.stopPropagation()
                    setOpen(false)
                    entry.onClick()
                  }}
                  className={cn(
                    'flex h-8 w-full items-center gap-2 px-3 text-left text-[13px] outline-none hover:bg-[#f5f5f5] focus-visible:bg-[#f5f5f5] disabled:opacity-40',
                    entry.tone === 'danger' ? 'text-[#b42318]' : 'text-[#171717]'
                  )}>
                  {entry.icon ? (
                    <span className="flex size-3.5 shrink-0 items-center justify-center" aria-hidden>
                      {entry.icon}
                    </span>
                  ) : null}
                  {entry.label}
                </button>
              )
            )}
          </div>,
          document.body
        )
      : null

  return (
    <div
      ref={rootRef}
      data-ui="item-action-bar"
      className={cn(
        placement === 'overlay'
          ? 'pointer-events-none absolute top-1/2 right-0 z-10 flex -translate-y-1/2 items-center opacity-0 group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100'
          : 'relative flex items-center',
        open && placement === 'overlay' && 'pointer-events-auto opacity-100',
        className
      )}>
      {inline.map((action) => (
        <button
          key={action.key}
          type="button"
          aria-label={action.label}
          title={action.label}
          data-testid={action.testId}
          disabled={action.disabled}
          onClick={(event) => {
            event.stopPropagation()
            action.onClick()
          }}
          className={buttonClassName ?? iconButtonClassName}>
          {action.icon}
        </button>
      ))}
      {showMore ? (
        <button
          ref={moreRef}
          type="button"
          aria-label={label}
          aria-expanded={open}
          aria-controls={menuId}
          title={label}
          onClick={(event) => {
            event.stopPropagation()
            setOpen((current) => {
              const next = !current
              if (next && moreRef.current) setBox(measureMenu(moreRef.current))
              return next
            })
          }}
          className={cn(
            buttonClassName ?? iconButtonClassName,
            open && (buttonClassName ? 'bg-[#f2f3f5] text-[#171717]' : 'bg-[#e6e7ea] text-[#171717]')
          )}>
          <Ellipsis className={buttonClassName ? 'size-3.5 text-current' : 'size-4 text-current'} strokeWidth={1.7} />
        </button>
      ) : null}
      {menuNode}
    </div>
  )
}
