export type AppletWindowAction = 'close' | 'minimize' | 'toggle-maximize' | 'start-dragging'

const COMMANDS: Record<AppletWindowAction, string> = {
  close: 'plugin:window|close',
  minimize: 'plugin:window|minimize',
  'toggle-maximize': 'plugin:window|toggle_maximize',
  'start-dragging': 'plugin:window|start_dragging'
}

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
  metadata?: { currentWindow?: { label?: string } }
}

function internals(): TauriInternals | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__
}

/** Label of this applet, agent-wiki, or AIX mini app system window, or null on the main shell. */
export function currentAppletWindowLabel(): string | null {
  const label = internals()?.metadata?.currentWindow?.label
  if (typeof label !== 'string') return null
  if (label.startsWith('applet-') || label.startsWith('agent-wiki-') || label.startsWith('aix-applet-')) return label
  return null
}

/** Native close / minimize / zoom / drag. No-op unless this page owns an applet window. */
export async function runAppletWindowAction(action: AppletWindowAction): Promise<boolean> {
  const label = currentAppletWindowLabel()
  const invoke = internals()?.invoke
  if (!label || !invoke) return false
  try {
    await invoke(COMMANDS[action], { label })
    return true
  } catch {
    return false
  }
}
