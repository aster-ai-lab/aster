/** Bytes for one homepage format job, held by the shell until the PDF window takes them. */

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

interface HandoffFile {
  name: string
  mediaType: string
  bytesBase64: string
}

export interface PdfToolHandoff {
  kind: string
  files: File[]
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

export function pdfHandoffUsesShell(): boolean {
  return Boolean(tauriInvoke())
}

export async function stagePdfToolHandoff(kind: string, files: readonly File[]): Promise<boolean> {
  const invoke = tauriInvoke()
  if (!invoke || files.length === 0) return false
  try {
    const payload: HandoffFile[] = []
    for (const file of files) {
      payload.push({
        name: file.name,
        mediaType: file.type,
        bytesBase64: bytesToBase64(new Uint8Array(await readFileBytes(file)))
      })
    }
    await invoke('stage_pdf_tool_handoff', { kind, files: payload })
    return true
  } catch {
    return false
  }
}

export async function takePdfToolHandoff(): Promise<PdfToolHandoff | null> {
  const invoke = tauriInvoke()
  if (!invoke) return null
  try {
    const job = (await invoke('take_pdf_tool_handoff')) as { kind?: unknown; files?: unknown } | null
    if (!job || typeof job.kind !== 'string' || !Array.isArray(job.files) || job.files.length === 0) return null
    const files = job.files.map((item) => {
      const file = item as Partial<HandoffFile>
      if (typeof file.name !== 'string' || typeof file.bytesBase64 !== 'string') {
        throw new Error('unreadable')
      }
      return new File([bytesFromBase64(file.bytesBase64)], file.name, {
        type: typeof file.mediaType === 'string' ? file.mediaType : ''
      })
    })
    return { kind: job.kind, files }
  } catch {
    return null
  }
}

export async function dropPdfToolHandoff(): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) return
  try {
    await invoke('drop_pdf_tool_handoff')
  } catch {
    // The window did not open. Leave the shell slot for the next drop.
  }
}

function readFileBytes(file: File): Promise<ArrayBuffer> {
  if (typeof file.arrayBuffer === 'function') return file.arrayBuffer()
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as ArrayBuffer)
    reader.onerror = () => reject(reader.error ?? new Error('unreadable'))
    reader.readAsArrayBuffer(file)
  })
}

function bytesToBase64(bytes: Uint8Array): string {
  let text = ''
  const chunk = 0x8000
  for (let index = 0; index < bytes.length; index += chunk) {
    text += String.fromCharCode(...bytes.subarray(index, index + chunk))
  }
  return btoa(text)
}

function bytesFromBase64(value: string): Uint8Array {
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return bytes
}
