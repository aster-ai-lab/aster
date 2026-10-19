import { Copy, Minus, Square, X } from 'lucide-react'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { readCenterId } from '@renderer/data/memberOrgs'
import type { MailFile } from '@renderer/mail/mailApi'

import {
  EMPTY_COMPOSE,
  teamMailOrgId,
  tokensFrom,
  type ComposeFocus,
  type ComposeKind,
  type ComposeState
} from './mailCompose'
import { MailComposeSurface } from './MailComposeSurface'
import { publishMailComposeResult } from './openMailComposeWindow'

type WindowAction = 'close' | 'minimize' | 'toggle-maximize' | 'start-dragging'

const COMMANDS: Record<WindowAction, string> = {
  close: 'plugin:window|close',
  minimize: 'plugin:window|minimize',
  'toggle-maximize': 'plugin:window|toggle_maximize',
  'start-dragging': 'plugin:window|start_dragging'
}

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
  metadata?: { currentWindow?: { label?: string } }
}

function runWindowAction(action: WindowAction): void {
  const internals = (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__
  const label = internals?.metadata?.currentWindow?.label
  if (!internals?.invoke || label !== 'mail-compose') return
  void internals.invoke(COMMANDS[action], { label }).catch(() => undefined)
}

function WindowControl({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-label={label}
      onMouseDown={(event) => event.stopPropagation()}
      onClick={onClick}
      className="grid size-6 place-items-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
      {children}
    </button>
  )
}

type BootSeed = {
  kind?: string
  to?: string[]
  cc?: string[]
  subject?: string
  body?: string
  threadId?: string
  draftId?: string
  focus?: string
  from?: string
  title?: string
  attachments?: MailFile[]
  orgId?: string
}

function filesOf(value: unknown): MailFile[] {
  if (!Array.isArray(value)) return []
  return value.flatMap((item) => {
    if (!item || typeof item !== 'object') return []
    const row = item as Record<string, unknown>
    if (typeof row.name !== 'string' || typeof row.data !== 'string') return []
    return [
      { name: row.name, mime: typeof row.mime === 'string' ? row.mime : 'application/octet-stream', data: row.data }
    ]
  })
}

function stringsOf(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value.flatMap((item) => (typeof item === 'string' ? [item] : []))
}

export function seedFromBoot(
  value: BootSeed | null | undefined,
  fallbackCenter = ''
): { state: ComposeState; focus: ComposeFocus; from: string; orgId?: string } {
  const kind: ComposeKind = value?.kind === 'reply' || value?.kind === 'draft' ? value.kind : 'new'
  const orgSource = value && Object.prototype.hasOwnProperty.call(value, 'orgId') ? value.orgId : fallbackCenter
  return {
    from: value?.from ?? '',
    focus: value?.focus === 'body' ? 'body' : 'to',
    orgId: teamMailOrgId(typeof orgSource === 'string' ? orgSource : ''),
    state: {
      ...EMPTY_COMPOSE,
      kind,
      to: tokensFrom(stringsOf(value?.to)),
      cc: tokensFrom(stringsOf(value?.cc)),
      subject: value?.subject ?? '',
      body: value?.body ?? '',
      attachments: filesOf(value?.attachments),
      threadId: value?.threadId || undefined,
      draftId: value?.draftId || undefined
    }
  }
}

function readBoot(): { state: ComposeState; focus: ComposeFocus; from: string; orgId?: string } {
  const boot = (globalThis as typeof globalThis & { __A3S_MAIL_COMPOSE__?: BootSeed }).__A3S_MAIL_COMPOSE__
  return seedFromBoot(boot, readCenterId())
}

/** Full-window body of the mail composer. The title drags the system window. */
export function MailComposeWindow() {
  const { t } = useTranslation()
  const [session, setSession] = useState(readBoot)
  const [epoch, setEpoch] = useState(0)
  const [zoomed, setZoomed] = useState(false)
  const [dirty, setDirty] = useState(false)
  const dismissRef = useRef<() => void>(() => runWindowAction('close'))
  const title =
    session.state.kind === 'reply'
      ? t('mail.reply')
      : session.state.kind === 'draft'
        ? t('mail.compose.continue')
        : t('mail.compose')

  useEffect(() => {
    document.getElementById('spinner')?.remove()
    const previous = document.title
    document.title = title
    return () => {
      document.title = previous
    }
  }, [title])

  useEffect(() => {
    if (!dirty) return undefined
    const warn = (event: BeforeUnloadEvent) => {
      event.preventDefault()
    }
    window.addEventListener('beforeunload', warn)
    return () => window.removeEventListener('beforeunload', warn)
  }, [dirty])

  useEffect(() => {
    const onSeed = (event: Event) => {
      const detail = (event as CustomEvent<BootSeed>).detail
      setSession(seedFromBoot(detail, readCenterId()))
      setEpoch((value) => value + 1)
    }
    window.addEventListener('a3s:mail-compose', onSeed)
    return () => window.removeEventListener('a3s:mail-compose', onSeed)
  }, [])

  const zoom = () => {
    setZoomed((value) => !value)
    runWindowAction('toggle-maximize')
  }

  return (
    <section
      aria-label={title}
      data-ui="mail.compose.window"
      className="fixed inset-0 flex flex-col bg-white text-[#171717]">
      <header className="flex h-8 shrink-0 items-center border-b border-[#efefef] bg-white pr-1.5 pl-3">
        <div
          className="flex min-w-0 flex-1 items-center"
          onMouseDown={(event) => {
            if (event.button !== 0) return
            runWindowAction('start-dragging')
          }}
          onDoubleClick={(event) => {
            if (event.button !== 0) return
            event.preventDefault()
            zoom()
          }}>
          <h1 className="m-0 min-w-0 truncate text-[14px] leading-tight font-medium text-[#171717]">{title}</h1>
        </div>
        <div
          role="group"
          aria-label={t('workbench.applet.window_controls')}
          className="flex shrink-0 items-center gap-0.5">
          <WindowControl label={t('workbench.applet.minimize')} onClick={() => runWindowAction('minimize')}>
            <Minus size={14} strokeWidth={1.7} aria-hidden />
          </WindowControl>
          <WindowControl label={zoomed ? t('workbench.applet.restore') : t('workbench.applet.maximize')} onClick={zoom}>
            {zoomed ? (
              <Copy size={14} strokeWidth={1.7} aria-hidden />
            ) : (
              <Square size={14} strokeWidth={1.7} aria-hidden />
            )}
          </WindowControl>
          <WindowControl label={t('common.close')} onClick={() => dismissRef.current()}>
            <X size={14} strokeWidth={1.7} aria-hidden />
          </WindowControl>
        </div>
      </header>
      <MailComposeSurface
        key={epoch}
        address={session.from}
        initial={session.state}
        focusField={session.focus}
        showTitle={false}
        teamOrgId={session.orgId}
        onDirtyChange={setDirty}
        onRegisterDismiss={(dismiss) => {
          dismissRef.current = dismiss
        }}
        onClose={() => runWindowAction('close')}
        onSent={(_message, draft) => {
          const folder = draft ? 'drafts' : 'sent'
          void publishMailComposeResult(folder).finally(() => runWindowAction('close'))
        }}
      />
    </section>
  )
}
