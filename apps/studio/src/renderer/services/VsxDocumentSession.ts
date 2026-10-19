import { loggerService } from '@logger'
import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import type { VsxDocumentHostMessage, VsxDocumentPublication } from '@shared/types/vsxDocument'
import type { WorkspaceDiagnosticPublication } from '@shared/types/workspaceDocument'

import type { PluginDocumentClient, WorkspaceDocumentClient } from './WorkspaceDocumentService'

const logger = loggerService.withContext('VsxDocumentSession')
const PROTOCOL = 'a3s.vsx.document/1'
type Payload<T> = T extends unknown ? Omit<T, 'protocol' | 'sessionId'> : never

export type VsxDocumentPort = Pick<
  globalThis.MessagePort,
  'postMessage' | 'addEventListener' | 'removeEventListener' | 'start' | 'close'
>

/** Connect only a port transferred to a separately admitted compatible host. */
export class VsxDocumentSession {
  private readonly sessionId = globalThis.crypto.randomUUID()
  private stopped = false
  private busy = false
  private syncing = false
  private closing?: Promise<void>
  private timer?: ReturnType<typeof globalThis.setInterval>
  private sentVersion = 0
  private accepted?: { sequence: number; publication: WorkspaceDiagnosticPublication; input: string }

  private constructor(
    private readonly client: PluginDocumentClient,
    private readonly context: PluginActivationContext,
    private readonly port: VsxDocumentPort
  ) {}

  static async connect(
    owner: WorkspaceDocumentClient,
    context: PluginActivationContext,
    port: VsxDocumentPort
  ): Promise<VsxDocumentSession> {
    const client = await owner.delegateDiagnostics(context).catch((error: unknown) => {
      port.close()
      throw error
    })
    const session = new VsxDocumentSession(client, context, port)
    try {
      context.defer(() => session.close())
      port.addEventListener('message', session.onMessage)
      port.addEventListener('messageerror', session.onMessageError)
      port.start()
      await session.sync()
      if (session.stopped || !context.isCurrent()) throw new Error('VSX document owner closed')
      session.timer = globalThis.setInterval(() => {
        void session.sync().catch(() => {
          void session
            .close()
            .catch((error: unknown) =>
              logger.error('VSX document close failed', error instanceof Error ? error : new Error(String(error)))
            )
        })
      }, 250)
      return session
    } catch (error) {
      await session.close()
      throw error
    }
  }

  async sync(): Promise<void> {
    if (this.stopped || !this.context.isCurrent()) throw new Error('VSX document owner closed')
    if (this.syncing || this.busy) return
    this.syncing = true
    try {
      const snapshot = await this.client.read()
      if (new globalThis.TextEncoder().encode(snapshot.text).length > 64 * 1024)
        throw new Error('VSX document exceeds limit')
      if (this.stopped || !this.context.isCurrent()) throw new Error('VSX document owner closed')
      if (snapshot.version !== this.sentVersion) {
        this.send({ type: 'document', snapshot })
        this.sentVersion = snapshot.version
      }
    } finally {
      this.syncing = false
    }
  }

  close(): Promise<void> {
    if (this.closing) return this.closing
    this.stopped = true
    globalThis.clearInterval(this.timer)
    this.port.removeEventListener('message', this.onMessage)
    this.port.removeEventListener('messageerror', this.onMessageError)
    this.port.close()
    this.closing = this.client.close()
    return this.closing
  }

  private readonly onMessageError = (): void => {
    void this.close().catch((error: unknown) =>
      logger.error('VSX document close failed', error instanceof Error ? error : new Error(String(error)))
    )
  }

  private readonly onMessage = (event: globalThis.MessageEvent<unknown>): void => {
    if (this.stopped || !this.context.isCurrent()) return
    const input = publication(event.data)
    if (!input || input.sessionId !== this.sessionId) {
      this.send({ type: 'refused', reason: 'invalid' })
      return
    }
    if (this.busy) {
      this.send({ type: 'refused', reason: 'busy' })
      return
    }
    this.busy = true
    void this.publish(input)
      .catch(() => this.send({ type: 'refused', reason: 'unavailable' }))
      .finally(() => {
        this.busy = false
      })
  }

  private async publish(input: VsxDocumentPublication): Promise<void> {
    const snapshot = await this.client.read()
    if (
      input.documentId !== snapshot.documentId ||
      input.version !== snapshot.version ||
      input.version !== this.sentVersion
    ) {
      this.send({ type: 'refused', reason: 'stale' })
      return
    }
    const serialized = JSON.stringify(input.items)
    if (input.sequence === this.accepted?.sequence) {
      if (serialized !== this.accepted.input || input.version !== this.accepted.publication.version) {
        this.send({ type: 'refused', reason: 'invalid' })
        return
      }
    } else {
      if (input.sequence !== (this.accepted?.sequence ?? 0) + 1) {
        this.send({ type: 'refused', reason: 'invalid' })
        return
      }
      const result = await this.client.publishDiagnostics(snapshot, input.items)
      this.accepted = { sequence: input.sequence, publication: result, input: serialized }
    }
    this.send({ type: 'accepted', sequence: input.sequence, version: input.version })
  }

  private send(message: Payload<VsxDocumentHostMessage>): void {
    if (this.stopped || !this.context.isCurrent()) return
    this.port.postMessage({ ...message, protocol: PROTOCOL, sessionId: this.sessionId })
  }
}

function publication(value: unknown): VsxDocumentPublication | null {
  if (!value || typeof value !== 'object') return null
  const input = value as VsxDocumentPublication
  if (
    input.protocol !== PROTOCOL ||
    input.type !== 'diagnostics' ||
    typeof input.sessionId !== 'string' ||
    input.sessionId.length > 64 ||
    typeof input.documentId !== 'string' ||
    input.documentId.length > 64 ||
    !Number.isSafeInteger(input.version) ||
    input.version < 1 ||
    !Number.isSafeInteger(input.sequence) ||
    input.sequence < 1 ||
    !Array.isArray(input.items) ||
    input.items.length > 128 ||
    input.items.some(
      (item) =>
        !item ||
        typeof item.message !== 'string' ||
        item.message.length > 2048 ||
        !item.start ||
        !item.end ||
        typeof item.start.line !== 'number' ||
        typeof item.start.character !== 'number' ||
        typeof item.end.line !== 'number' ||
        typeof item.end.character !== 'number' ||
        Object.keys(item).sort().join(',') !== 'end,message,severity,start' ||
        Object.keys(item.start).sort().join(',') !== 'character,line' ||
        Object.keys(item.end).sort().join(',') !== 'character,line' ||
        !['error', 'warning', 'information', 'hint'].includes(item.severity)
    ) ||
    Object.keys(input).sort().join(',') !== 'documentId,items,protocol,sequence,sessionId,type,version' ||
    new globalThis.TextEncoder().encode(JSON.stringify(input.items)).length > 16 * 1024
  )
    return null
  return {
    ...input,
    items: input.items.map((item) => ({
      start: { line: item.start.line, character: item.start.character },
      end: { line: item.end.line, character: item.end.character },
      severity: item.severity,
      message: item.message
    }))
  }
}
