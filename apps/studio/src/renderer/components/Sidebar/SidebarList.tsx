import { MenuItem } from '@a3s-lab/ui'
import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { createPortal } from 'react-dom'

import { appletDragShift, appletDragTarget, type AppletDragRect } from '@renderer/pages/agents/appletDrag'

import { SidebarTooltip } from './Tooltip'
import type { ResolvedSidebarEntry, SidebarIconPresentation, SidebarVisibleLayout } from './types'

const FULL_ICON_PRESENTATION = { slotSize: 18, glyphSize: 16 } as const
const ICON_ICON_PRESENTATION = { slotSize: 22, glyphSize: 22 } as const

export interface SidebarListProps {
  layout: SidebarVisibleLayout
  entries: ResolvedSidebarEntry[]
  onReorder?: (keys: string[]) => void
}

/** Icon rail order follows a drag. The labeled list stays a click menu. */
export function SidebarList({ layout, ...props }: SidebarListProps) {
  if (layout === 'icon') return <IconList {...props} />
  return <FullList entries={props.entries} />
}

type ListProps = Omit<SidebarListProps, 'layout'>

function createAuxClickHandler(entry: ResolvedSidebarEntry) {
  if (entry.disabled || !entry.onOpenNewTab) return undefined
  return (e: React.MouseEvent) => {
    if (e.button === 1) {
      e.preventDefault()
      entry.onOpenNewTab?.()
    }
  }
}

function preventMiddleClickAutoscroll(e: React.MouseEvent) {
  if (e.button === 1) e.preventDefault()
}

function SidebarEntryIcon({
  entry,
  presentation
}: {
  entry: ResolvedSidebarEntry
  presentation: SidebarIconPresentation
}) {
  return (
    <span
      data-slot="sidebar-entry-icon"
      aria-hidden="true"
      className="relative flex shrink-0 items-center justify-center"
      style={{ width: presentation.slotSize, height: presentation.slotSize }}>
      {entry.renderIcon(presentation)}
      {entry.badgeDot ? (
        <span data-ui="sidebar.entry-dot" className="absolute top-0.5 right-0.5 size-2 rounded-full bg-[#E23B3B]" />
      ) : null}
      {entry.badge ? (
        <span
          data-ui="sidebar.entry-badge"
          className="absolute -top-1.5 -right-2 flex h-4 min-w-4 items-center justify-center rounded-full bg-[#F04438] px-1 text-[10px] leading-none font-medium text-white ring-2 ring-background">
          {entry.badge > 99 ? '99+' : entry.badge}
        </span>
      ) : null}
    </span>
  )
}

const DRAG_THRESHOLD_PX = 8

type ActivitySlot = AppletDragRect & { width: number; height: number }

type ActivityDrag = {
  from: number
  to: number
  pointerX: number
  pointerY: number
  offsetX: number
  offsetY: number
  width: number
  height: number
  rects: AppletDragRect[]
}

type ActivitySession = {
  pointerId: number
  startX: number
  startY: number
  from: number
  active: boolean
  slots: ActivitySlot[]
}

function IconList({ entries, onReorder }: ListProps) {
  const buttons = useRef<Array<HTMLButtonElement | null>>([])
  const session = useRef<ActivitySession | null>(null)
  const dragRef = useRef<ActivityDrag | null>(null)
  const suppressClick = useRef(false)
  const [drag, setDrag] = useState<ActivityDrag | null>(null)
  const reduced =
    typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true

  const publish = (next: ActivityDrag | null) => {
    dragRef.current = next
    setDrag(next)
  }

  const finish = (commit: boolean) => {
    const current = session.current
    const active = dragRef.current
    session.current = null
    publish(null)
    if (!commit || !current?.active || !active || active.to === active.from || !onReorder) return
    const keys = entries.map((entry) => entry.key)
    const [moved] = keys.splice(active.from, 1)
    if (moved) keys.splice(active.to, 0, moved)
    onReorder(keys)
  }

  const finishRef = useRef(finish)
  finishRef.current = finish
  const dragging = drag !== null
  useEffect(() => {
    if (!dragging) return
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      finishRef.current(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [dragging])

  const onPointerDown = (index: number) => (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (!onReorder || event.button !== 0) return
    session.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      from: index,
      active: false,
      slots: []
    }
    event.currentTarget.setPointerCapture?.(event.pointerId)
  }

  const onPointerMove = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const current = session.current
    if (!current || event.pointerId !== current.pointerId) return
    const dx = event.clientX - current.startX
    const dy = event.clientY - current.startY
    if (!current.active) {
      if (dx * dx + dy * dy < DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX) return
      current.active = true
      suppressClick.current = true
      current.slots = buttons.current.map((node) => {
        const rect = node?.getBoundingClientRect()
        return { left: rect?.left ?? 0, top: rect?.top ?? 0, width: rect?.width ?? 0, height: rect?.height ?? 0 }
      })
    }
    const origin = current.slots[current.from]
    const centers = current.slots.map((slot) => ({ x: slot.left + slot.width / 2, y: slot.top + slot.height / 2 }))
    publish({
      from: current.from,
      to: appletDragTarget({ x: event.clientX, y: event.clientY }, centers),
      pointerX: event.clientX,
      pointerY: event.clientY,
      offsetX: origin ? current.startX - origin.left : 0,
      offsetY: origin ? current.startY - origin.top : 0,
      width: origin?.width || 36,
      height: origin?.height || 36,
      rects: current.slots
    })
  }

  const onClick = (entry: ResolvedSidebarEntry) => (event: React.MouseEvent) => {
    if (suppressClick.current) {
      suppressClick.current = false
      event.preventDefault()
      event.stopPropagation()
      return
    }
    if (!entry.disabled) entry.onOpen()
  }

  const held = drag ? entries[drag.from] : null

  return (
    <div
      className={`flex flex-col items-center gap-4 px-0 pt-2 [-webkit-app-region:no-drag] ${drag ? 'cursor-grabbing' : ''}`}
      onKeyDown={(event) => {
        if (event.key === 'Escape' && session.current?.active) {
          event.preventDefault()
          finish(false)
        }
      }}>
      {entries.map((entry, index) => {
        const isActive = !entry.disabled && entry.isActive
        const shift = drag ? appletDragShift(index, drag.from, drag.to, drag.rects) : { x: 0, y: 0 }
        const heldHere = drag?.from === index
        const moving = Boolean(drag) && (shift.x !== 0 || shift.y !== 0)

        return (
          <SidebarTooltip
            key={entry.key}
            content={entry.statusLabel ? `${entry.label} — ${entry.statusLabel}` : entry.label}>
            <button
              ref={(node) => {
                buttons.current[index] = node
              }}
              type="button"
              aria-label={entry.label}
              aria-current={isActive ? 'page' : undefined}
              aria-description={entry.statusLabel}
              aria-disabled={entry.disabled || undefined}
              data-activity-order={index}
              onClick={onClick(entry)}
              onPointerDown={onPointerDown(index)}
              onPointerMove={onPointerMove}
              onPointerUp={() => finish(true)}
              onPointerCancel={() => finish(false)}
              onMouseDown={preventMiddleClickAutoscroll}
              onAuxClick={createAuxClickHandler(entry)}
              className={`activity-bar-item activity-bar-item--nav grid size-9 place-items-center bg-transparent transition-colors duration-150 ${
                entry.disabled ? 'cursor-not-allowed opacity-55' : drag ? 'cursor-grabbing' : 'cursor-pointer'
              } ${heldHere ? 'opacity-0 outline outline-1 outline-foreground/30 outline-dashed' : ''}`}
              style={
                moving
                  ? {
                      transform: `translate(${shift.x}px, ${shift.y}px)`,
                      transition: reduced ? undefined : 'transform 180ms cubic-bezier(0.2, 0, 0, 1)'
                    }
                  : undefined
              }>
              <SidebarEntryIcon entry={entry} presentation={ICON_ICON_PRESENTATION} />
            </button>
          </SidebarTooltip>
        )
      })}
      {drag && held
        ? createPortal(
            <div
              data-ui="activity-bar.drag"
              className="pointer-events-none fixed z-[80] grid size-9 place-items-center rounded-xl bg-[#d4d4d4] text-[#171717] shadow-lg"
              style={{
                left: drag.pointerX - drag.offsetX,
                top: drag.pointerY - drag.offsetY,
                width: drag.width,
                height: drag.height
              }}>
              <SidebarEntryIcon entry={held} presentation={ICON_ICON_PRESENTATION} />
            </div>,
            document.body
          )
        : null}
    </div>
  )
}

function FullList({ entries }: ListProps) {
  return (
    <div className="space-y-0.5 px-2 [-webkit-app-region:no-drag]">
      {entries.map((entry) => {
        const isActive = !entry.disabled && entry.isActive

        return (
          <div key={entry.key} className="sidebar-full-item relative" data-active={isActive ? 'true' : undefined}>
            <MenuItem
              variant="ghost"
              icon={<SidebarEntryIcon entry={entry} presentation={FULL_ICON_PRESENTATION} />}
              label={entry.label}
              aria-label={entry.label}
              aria-current={isActive ? 'page' : undefined}
              aria-description={entry.statusLabel}
              title={entry.statusLabel}
              active={isActive}
              aria-disabled={entry.disabled || undefined}
              onClick={entry.disabled ? undefined : entry.onOpen}
              onMouseDown={preventMiddleClickAutoscroll}
              onAuxClick={createAuxClickHandler(entry)}
              className="rounded-[0.625rem] aria-disabled:cursor-not-allowed aria-disabled:text-muted-foreground aria-disabled:opacity-55 aria-disabled:hover:bg-transparent aria-disabled:hover:text-muted-foreground data-[active=true]:bg-accent data-[active=true]:text-foreground"
            />
          </div>
        )
      })}
    </div>
  )
}
