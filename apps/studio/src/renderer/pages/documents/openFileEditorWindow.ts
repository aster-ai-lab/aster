interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

/** Open one workspace file in a native editor window. Returns false outside the Tauri shell. */
export async function openFileEditorWindow(input: {
  root: string
  path: string
  title: string
  readOnly?: boolean
  fileId?: string
}): Promise<boolean> {
  const invoke = tauriInvoke()
  if (!invoke) return false
  try {
    await invoke('open_file_editor_window', {
      root: input.root,
      path: input.path,
      title: input.title,
      ...(input.readOnly ? { readOnly: true } : {}),
      ...(input.fileId ? { fileId: input.fileId } : {})
    })
    return true
  } catch {
    return false
  }
}
