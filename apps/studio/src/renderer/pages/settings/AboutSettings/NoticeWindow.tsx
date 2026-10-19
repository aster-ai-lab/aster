import { Copy, Minus, Square, X } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import type { NoticeSlug } from './publicationLink'

type WindowAction = 'close' | 'minimize' | 'toggle-maximize' | 'start-dragging'

const COMMANDS: Record<WindowAction, string> = {
  close: 'plugin:window|close',
  minimize: 'plugin:window|minimize',
  'toggle-maximize': 'plugin:window|toggle_maximize',
  'start-dragging': 'plugin:window|start_dragging'
}

const NOTICE_LABEL = /^notice-(terms|privacy|copyright|support)$/

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
  metadata?: { currentWindow?: { label?: string } }
}

function windowInternals(): TauriInternals | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__
}

function runWindowAction(action: WindowAction): void {
  const internals = windowInternals()
  const label = internals?.metadata?.currentWindow?.label
  if (!internals?.invoke || !label || !NOTICE_LABEL.test(label)) return
  void internals.invoke(COMMANDS[action], { label }).catch(() => undefined)
}

function noticeDocumentUrl(slug: NoticeSlug): string | null {
  const label = windowInternals()?.metadata?.currentWindow?.label
  if (label !== `notice-${slug}`) return null
  const raw = document.documentElement.dataset.a3sNoticeOrigin ?? ''
  let origin: URL
  try {
    origin = new URL(raw)
  } catch {
    return null
  }
  if (origin.protocol !== 'http:' && origin.protocol !== 'https:') return null
  if (origin.username || origin.password) return null
  if (origin.pathname !== '/' || origin.search || origin.hash) return null
  return `${origin.origin}/v1/api/legal/${slug}`
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

/** Full-window body of a published notice. The title drags the system window. */
export function NoticeWindow({ slug }: { slug: NoticeSlug }) {
  const { t } = useTranslation()
  const title = t(`settings.about.page.${slug}`)
  const pageUrl = noticeDocumentUrl(slug)
  const [zoomed, setZoomed] = useState(false)

  useEffect(() => {
    document.getElementById('spinner')?.remove()
    const previous = document.title
    document.title = title
    return () => {
      document.title = previous
    }
  }, [title])

  const zoom = () => {
    setZoomed((value) => !value)
    runWindowAction('toggle-maximize')
  }

  return (
    <section
      aria-label={title}
      data-ui="settings.notice-window"
      data-notice-slug={slug}
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
          <WindowControl label={t('common.close')} onClick={() => runWindowAction('close')}>
            <X size={14} strokeWidth={1.7} aria-hidden />
          </WindowControl>
        </div>
      </header>
      {pageUrl ? (
        <iframe
          title={title}
          src={pageUrl}
          sandbox=""
          referrerPolicy="no-referrer"
          className="min-h-0 w-full flex-1 border-0 bg-white"
        />
      ) : (
        <p role="alert" className="grid min-h-0 flex-1 place-items-center px-6 text-center text-[13px] text-[#60646c]">
          {t('settings.about.page.open_failed')}
        </p>
      )}
    </section>
  )
}
