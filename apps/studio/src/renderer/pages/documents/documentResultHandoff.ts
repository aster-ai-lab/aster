interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

export function documentResultInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

export function resultBytes(value: string): Uint8Array {
  const binary = globalThis.atob(value)
  return Uint8Array.from(binary, (character) => character.charCodeAt(0))
}

/** The applet requests a save; only the main window chooses and writes the destination. */
export async function requestDocumentResultSave(
  name: string,
  mediaType: string,
  bytes: Uint8Array
): Promise<string | null> {
  const invoke = documentResultInvoke()
  if (!invoke) throw new Error('Document result save unavailable')
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000))
  }
  const id = await invoke('stage_document_result_save', {
    file: { name, mediaType, bytesBase64: globalThis.btoa(binary) }
  })
  if (typeof id !== 'string') throw new Error('Document result save unavailable')
  for (let attempt = 0; attempt < 1200; attempt += 1) {
    const status = (await invoke('document_result_save_status', { id })) as {
      state?: string
      location?: string
    }
    if (status.state === 'saved' && typeof status.location === 'string') return status.location
    if (status.state === 'cancelled') return null
    if (status.state !== 'pending') throw new Error('Document result save unavailable')
    await new Promise<void>((resolve) => globalThis.setTimeout(resolve, 500))
  }
  throw new Error('Document result save unavailable')
}
