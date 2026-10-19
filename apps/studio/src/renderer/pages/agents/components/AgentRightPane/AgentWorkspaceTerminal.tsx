import { useEffect, useRef, useState, type KeyboardEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { applyTerminalChunk, createTerminalScreen, renderTerminalScreen } from './terminalScreen'

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

function keyToTerminalData(event: KeyboardEvent<HTMLDivElement>): string | undefined {
  if (event.nativeEvent.isComposing || event.key === 'Dead') return undefined
  if (event.metaKey || event.altKey) return undefined
  if (event.ctrlKey) {
    if (event.key === 'c') return '\u0003'
    if (event.key === 'd') return '\u0004'
    if (event.key === 'l') return '\u000c'
    return undefined
  }
  if (event.key === 'Enter') return '\r'
  if (event.key === 'Backspace') return '\u007f'
  if (event.key === 'Tab') return '\t'
  if (event.key === 'ArrowUp') return '\u001b[A'
  if (event.key === 'ArrowDown') return '\u001b[B'
  if (event.key === 'ArrowRight') return '\u001b[C'
  if (event.key === 'ArrowLeft') return '\u001b[D'
  if (event.key.length === 1) return event.key
  return undefined
}

/**
 * Interactive shell for the task workspace. Keystrokes go to a host PTY and
 * the pane pulls output back, so the web view never owns the shell process.
 */
export function AgentWorkspaceTerminal({
  active,
  cwd,
  label,
  sessionId
}: {
  active: boolean
  cwd?: string
  /** Accessible name of this session. Hidden sessions omit it so only the focused shell is named. */
  label?: string
  sessionId?: string
}) {
  const { t } = useTranslation()
  const focusRef = useRef<HTMLDivElement>(null)
  const screenRef = useRef(createTerminalScreen())
  const sessionRef = useRef<string | undefined>(undefined)
  const activeRef = useRef(active)
  const [text, setText] = useState('')
  const [error, setError] = useState<string | null>(null)
  const unavailable = t('agent.right_pane.terminal.unavailable', '终端没有打开')
  const accessibleName = label?.trim() || t('agent.right_pane.tabs.terminal', '终端')
  activeRef.current = active

  useEffect(() => {
    if (!active) return
    focusRef.current?.focus()
  }, [active])

  useEffect(() => {
    // Hidden tabs stay mounted. The shell lives until this session unmounts,
    // so switching tabs does not kill the process or wipe the screen.
    const invoke = tauriInvoke()
    if (!invoke) {
      setError(unavailable)
      return
    }
    const life = { cancelled: false, timer: 0, shellId: '' }
    const directory = cwd && cwd.startsWith('/') ? cwd : null
    const pull = async () => {
      if (!life.shellId || life.cancelled) return
      try {
        const drained = (await invoke('workspace_terminal_drain', { sessionId: life.shellId })) as {
          bytes?: number[]
        }
        const bytes = drained.bytes
        if (!bytes || bytes.length === 0) return
        const chunk = new TextDecoder().decode(Uint8Array.from(bytes))
        screenRef.current = applyTerminalChunk(screenRef.current, chunk)
        setText(renderTerminalScreen(screenRef.current))
        if (activeRef.current) focusRef.current?.scrollTo({ top: focusRef.current.scrollHeight })
      } catch {
        // The next key or the close path reports a dead session.
      }
    }
    void invoke('workspace_terminal_create', { cols: 80, rows: 24, cwd: directory })
      .then((created) => {
        const shellId = (created as { sessionId?: string }).sessionId
        if (!shellId) throw new Error(unavailable)
        if (life.cancelled) {
          void invoke('workspace_terminal_kill', { sessionId: shellId })
          return
        }
        life.shellId = shellId
        sessionRef.current = shellId
        setError(null)
        life.timer = window.setInterval(() => void pull(), 40)
        void pull()
        if (activeRef.current) focusRef.current?.focus()
      })
      .catch(() => {
        if (!life.cancelled) setError(unavailable)
      })
    return () => {
      life.cancelled = true
      window.clearInterval(life.timer)
      const shellId = life.shellId || sessionRef.current
      sessionRef.current = undefined
      if (shellId) void invoke('workspace_terminal_kill', { sessionId: shellId })
    }
  }, [cwd, sessionId, unavailable])

  const write = (data: string) => {
    const invoke = tauriInvoke()
    const shellId = sessionRef.current
    if (!invoke || !shellId || !data) return
    void invoke('workspace_terminal_write', { sessionId: shellId, data }).catch(() => setError(unavailable))
  }

  return (
    <div
      ref={focusRef}
      tabIndex={active ? 0 : -1}
      role="textbox"
      aria-multiline="true"
      aria-hidden={active ? undefined : true}
      aria-label={active ? accessibleName : undefined}
      data-workspace-terminal="true"
      data-workspace-terminal-active={active ? 'true' : 'false'}
      className="h-full min-h-0 overflow-auto bg-[#1e1e1e] px-3 py-2 font-mono text-[13px] leading-5 text-[#e8e8e8] outline-none"
      onKeyDown={(event) => {
        const data = keyToTerminalData(event)
        if (!data) return
        event.preventDefault()
        write(data)
      }}
      onPaste={(event) => {
        const data = event.clipboardData.getData('text')
        if (!data) return
        event.preventDefault()
        write(data)
      }}>
      {error ? <p className="text-[#f0a0a0]">{error}</p> : null}
      <pre className="whitespace-pre-wrap break-all">{text}</pre>
    </div>
  )
}
