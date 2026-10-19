import type { AppletContract } from '../capabilities/capabilitiesData'
import type { WorkbenchWindowAppletId } from './WorkbenchApplets'

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

/** Open the office applet in a native window. Returns false outside the Tauri shell. */
export async function openWorkbenchAppletWindow(
  id: WorkbenchWindowAppletId,
  title: string,
  contract?: AppletContract,
  tool?: string
): Promise<boolean> {
  const invoke = tauriInvoke()
  if (!invoke) return false
  const args: Record<string, unknown> = { appletId: id, title, channel: 'release' }
  if (contract?.window) {
    args.defaultWidth = contract.window.defaultWidth
    args.defaultHeight = contract.window.defaultHeight
    args.maxWidth = contract.window.maxWidth
    args.maxHeight = contract.window.maxHeight
  }
  if (contract && contract.permissions.length > 0) args.permissions = [...contract.permissions]
  if (tool) args.tool = tool
  try {
    await invoke('open_workbench_applet_window', args)
    return true
  } catch {
    return false
  }
}
