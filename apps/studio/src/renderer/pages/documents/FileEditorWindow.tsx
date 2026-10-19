import { Copy, Minus, Square, X } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { WorkbenchFileEditor } from '@renderer/pages/agents/WorkbenchFileEditor'
import { formatSavedClock } from '@renderer/pages/documents/DocumentOpenView'

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
  if (!internals?.invoke || !label?.startsWith('file-editor-')) return
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

/** Full-window body of a native file editor. The title drags the system window. */
export function FileEditorWindow({ root, path, readOnly = false }: { root: string; path: string; readOnly?: boolean }) {
  const { t } = useTranslation()
  const name = path.split(/[/\\]/u).pop() || path
  const [zoomed, setZoomed] = useState(false)
  const [dirty, setDirty] = useState(false)
  const [savedAt, setSavedAt] = useState<Date | null>(null)
  const unsaved = t('agent.preview_pane.edit.unsaved')

  useEffect(() => {
    document.getElementById('spinner')?.remove()
    const previous = document.title
    document.title = dirty ? `${name} — ${unsaved}` : name
    return () => {
      document.title = previous
    }
  }, [dirty, name, unsaved])

  useEffect(() => {
    if (!dirty) return undefined
    const warn = (event: BeforeUnloadEvent) => {
      event.preventDefault()
    }
    window.addEventListener('beforeunload', warn)
    return () => window.removeEventListener('beforeunload', warn)
  }, [dirty])

  useEffect(() => {
    setSavedAt(null)
  }, [path])

  const zoom = () => {
    setZoomed((value) => !value)
    runWindowAction('toggle-maximize')
  }

  return (
    <section
      aria-label={name}
      data-ui="documents.file-editor"
      data-readonly={readOnly ? 'true' : undefined}
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
          <h1 className="m-0 flex min-w-0 items-center gap-1.5 truncate text-[14px] leading-tight font-medium text-[#171717]">
            {dirty ? <span aria-hidden className="size-1.5 shrink-0 rounded-full bg-[#171717]" /> : null}
            <span className="min-w-0 truncate">{name}</span>
          </h1>
        </div>
        {savedAt ? (
          <time
            dateTime={savedAt.toISOString()}
            className="shrink-0 px-2 text-[12px] leading-none text-[#8a8a8a] tabular-nums">
            {t('knowledge.notes.saved_at', { time: formatSavedClock(savedAt) })}
          </time>
        ) : null}
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
      <div className="min-h-0 flex-1">
        <WorkbenchFileEditor
          root={root}
          path={path}
          readOnly={readOnly}
          onDirtyChange={readOnly ? undefined : setDirty}
          onSavedAtChange={readOnly ? undefined : setSavedAt}
        />
      </div>
    </section>
  )
}
