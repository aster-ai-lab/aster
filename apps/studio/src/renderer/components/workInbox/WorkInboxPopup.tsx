import { Bell } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { useRestingDialog } from '@renderer/components/restingDialog'
import { SidebarTooltip } from '@renderer/components/Sidebar/Tooltip'
import { useMailUnread } from '@renderer/mail/mailUnread'
import { openWorkbenchAppletWindow } from '@renderer/pages/agents/openWorkbenchAppletWindow'
import { INBOX_APPLET_ID } from '@renderer/pages/agents/WorkbenchApplets'
import { loadWorkSummary, WORK_INBOX_CHANGED } from '@renderer/workInbox/client'

import { InboxHome } from './InboxHome'
import { type WorkInboxViewProps } from './WorkInboxView'

const PANEL_WIDTH = 1040
const PANEL_HEIGHT = 720

const INBOX_WINDOW = {
  window: { defaultWidth: 1040, defaultHeight: 720, maxWidth: 1440, maxHeight: 1200 },
  permissions: [] as Array<'user' | 'model' | 'knowledge'>
}

type WorkInboxPopupProps = WorkInboxViewProps & {
  open: boolean
  anchor: HTMLElement | null
  onOpenChange: (open: boolean) => void
}

export function WorkInboxPopup({ open, anchor, onOpenChange, ...view }: WorkInboxPopupProps) {
  const { t } = useTranslation()
  const panelRef = useRef<HTMLDivElement>(null)
  const presence = useRestingDialog(open)
  const [frame, setFrame] = useState({ left: 16, top: 16, width: PANEL_WIDTH, height: PANEL_HEIGHT })

  const place = useCallback(() => {
    const width = Math.min(PANEL_WIDTH, window.innerWidth - 24)
    const height = Math.min(PANEL_HEIGHT, window.innerHeight - 24)
    const rect = anchor?.getBoundingClientRect()
    let left = rect ? rect.right + 8 : 16
    let top = rect ? rect.bottom - height : 16
    if (left + width > window.innerWidth - 8) left = Math.max(8, window.innerWidth - width - 8)
    if (top < 8) top = 8
    setFrame({ left, top, width, height })
  }, [anchor])

  useEffect(() => {
    if (!open) return
    place()
    window.addEventListener('resize', place)
    return () => window.removeEventListener('resize', place)
  }, [open, place])

  useEffect(() => {
    if (!open) return
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onOpenChange(false)
    }
    const onPointer = (event: MouseEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      if (target instanceof Element && target.closest('.ui-popconfirm')) return
      if (panelRef.current?.contains(target) || anchor?.contains(target)) return
      onOpenChange(false)
    }
    document.addEventListener('keydown', onKey)
    document.addEventListener('mousedown', onPointer)
    return () => {
      document.removeEventListener('keydown', onKey)
      document.removeEventListener('mousedown', onPointer)
    }
  }, [anchor, onOpenChange, open])

  if (!presence.present) return null

  return createPortal(
    <div
      ref={panelRef}
      role="dialog"
      aria-modal="true"
      aria-label={t('work.inbox.entry')}
      data-ui="work.inbox.panel"
      data-state={presence.state}
      className={`a3s-popup-rise fixed z-[80] flex flex-col overflow-hidden rounded-[12px] border border-[#efefef] bg-white shadow-[0_12px_32px_rgba(0,0,0,0.12)]${
        presence.state === 'closed' ? ' pointer-events-none' : ''
      }`}
      style={{ left: frame.left, top: frame.top, width: frame.width, height: frame.height }}>
      <InboxHome {...view} />
    </div>,
    document.body
  )
}

export async function openInboxWindow(title: string): Promise<boolean> {
  return openWorkbenchAppletWindow(INBOX_APPLET_ID, title, INBOX_WINDOW)
}

export function WorkInboxButton({
  layout,
  loadSummary = loadWorkSummary
}: {
  layout: 'icon' | 'full'
  loadSummary?: typeof loadWorkSummary
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [anchor, setAnchor] = useState<HTMLElement | null>(null)
  const [todo, setTodo] = useState(0)
  const mailUnread = useMailUnread()
  const buttonRef = useRef<HTMLButtonElement>(null)

  const refreshDot = useCallback(() => {
    void loadSummary().then((summary) => setTodo(summary.todo))
  }, [loadSummary])

  useEffect(() => {
    refreshDot()
    const timer = window.setInterval(refreshDot, 30_000)
    window.addEventListener(WORK_INBOX_CHANGED, refreshDot)
    return () => {
      window.clearInterval(timer)
      window.removeEventListener(WORK_INBOX_CHANGED, refreshDot)
    }
  }, [refreshDot])

  const label = t('work.inbox.entry')
  const dot = todo > 0 || mailUnread > 0

  const button = (
    <button
      ref={buttonRef}
      type="button"
      data-ui="work.inbox.open"
      aria-label={label}
      aria-expanded={open}
      onClick={(event) => {
        const target = event.currentTarget
        void openInboxWindow(label).then((opened) => {
          if (opened) {
            setOpen(false)
            return
          }
          setAnchor(target)
          setOpen((current) => !current)
        })
      }}
      className={
        layout === 'icon'
          ? 'activity-bar-item relative grid size-9 place-items-center bg-transparent text-muted-foreground'
          : 'flex w-full items-center justify-start gap-2.5 rounded-lg px-2.5 py-1.75 text-[13px] text-foreground hover:bg-accent/60'
      }>
      <Bell size={layout === 'icon' ? 22 : 16} strokeWidth={layout === 'icon' ? 1.6 : 1.7} />
      {layout === 'full' ? <span>{label}</span> : null}
      {dot ? (
        <span
          data-ui="work.inbox.dot"
          className={
            layout === 'icon'
              ? 'absolute top-1 right-1 size-2 rounded-full bg-[#E23B3B]'
              : 'ml-auto size-2 rounded-full bg-[#E23B3B]'
          }
        />
      ) : null}
    </button>
  )

  return (
    <>
      {layout === 'icon' ? <SidebarTooltip content={label}>{button}</SidebarTooltip> : button}
      <WorkInboxPopup
        open={open}
        anchor={anchor ?? buttonRef.current}
        onOpenChange={setOpen}
        loadSummary={loadSummary}
      />
    </>
  )
}
