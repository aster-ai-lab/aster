import type { MailComposeSeed } from './mailCompose'

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

export function mailComposeWindowAvailable(): boolean {
  return Boolean(tauriInvoke())
}

/** Open the reused compose window. Returns false outside the Tauri shell. */
export async function openMailComposeWindow(seed: MailComposeSeed): Promise<boolean> {
  const invoke = tauriInvoke()
  if (!invoke) return false
  try {
    await invoke('open_mail_compose_window', { draft: JSON.stringify(seed) })
    return true
  } catch {
    return false
  }
}

export async function publishMailComposeResult(folder: 'sent' | 'drafts'): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) return
  try {
    await invoke('publish_mail_compose_result', { folder })
  } catch {
    // The letter is already stored. The main window reloads on its next visit.
  }
}
