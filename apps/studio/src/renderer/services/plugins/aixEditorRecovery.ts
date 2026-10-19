import { loggerService } from '@logger'
import {
  aixEditorCheckpointRequestSchema,
  aixEditorCheckpointResultSchema,
  aixEditorRecoveryStatusSchema
} from '@shared/types/aixViewBridge'

const logger = loggerService.withContext('AixEditorRecovery')
function invoke(command: string): Promise<unknown> {
  const host = (
    globalThis as typeof globalThis & {
      __TAURI_INTERNALS__?: {
        invoke?: (command: string) => Promise<unknown>
        metadata?: { currentWindow?: { label?: string } }
      }
    }
  ).__TAURI_INTERNALS__
  if (!host?.metadata?.currentWindow?.label?.startsWith('aix-applet-')) return Promise.resolve()
  return host?.invoke?.(command) ?? Promise.resolve()
}

export async function closeRecoveredAixWindow(): Promise<void> {
  await invoke('close_aix_applet_window')
}

interface RecoveryDocument {
  port: MessagePort
  next: number
  pending?: { id: number; resolve: (dirty: boolean) => void; reject: (error: Error) => void }
}
const documents = new Map<string, RecoveryDocument>()
const key = (slug: string, view: string) => `${slug}\0${view}`

/** A private document port reports only recovery readiness, never its protected content. */
export function bindAixEditorRecovery(slug: string, view: string, port: MessagePort): () => void {
  const id = key(slug, view)
  const document: RecoveryDocument = { port, next: 0 }
  const receive = ({ data }: MessageEvent) => {
    const status = aixEditorRecoveryStatusSchema.safeParse(data)
    if (status.success) {
      if (!documents.has(id)) {
        documents.set(id, document)
        void invoke('protect_aix_applet_window').catch((error: unknown) =>
          logger.error('Native recovery guard unavailable', error instanceof Error ? error : new Error(String(error)))
        )
      }
      return
    }
    const result = aixEditorCheckpointResultSchema.safeParse(data)
    const pending = document.pending
    if (!result.success || !pending || documents.get(id) !== document || result.data.id !== pending.id) return
    document.pending = undefined
    if (result.data.ok) pending.resolve(result.data.dirty)
    else pending.reject(new Error('Editor checkpoint unavailable'))
  }
  port.addEventListener('message', receive)
  return () => {
    port.removeEventListener('message', receive)
    document.pending?.reject(new Error('Editor document closed'))
    if (documents.get(id) === document) documents.delete(id)
  }
}

export function hasAixEditorRecovery(slug: string, view: string): boolean {
  return documents.has(key(slug, view))
}

export async function checkpointAixEditor(
  slug: string,
  view: string,
  purpose: 'leave' | 'renew' = 'leave'
): Promise<boolean> {
  const document = documents.get(key(slug, view))
  if (!document) return false
  if (document.pending) throw new Error('Editor checkpoint in progress')
  return new Promise<boolean>((resolve, reject) => {
    const id = ++document.next
    const timer = globalThis.setTimeout(() => {
      if (document.pending?.id === id) {
        document.pending = undefined
        reject(new Error('Editor checkpoint timed out'))
      }
    }, 5000)
    document.pending = {
      id,
      resolve: (dirty) => {
        globalThis.clearTimeout(timer)
        resolve(dirty)
      },
      reject: (error) => {
        globalThis.clearTimeout(timer)
        reject(error)
      }
    }
    try {
      document.port.postMessage(
        aixEditorCheckpointRequestSchema.parse({ protocolVersion: 1, type: 'aix.view.checkpoint', id, purpose })
      )
    } catch (error) {
      document.pending = undefined
      globalThis.clearTimeout(timer)
      reject(error)
    }
  })
}
