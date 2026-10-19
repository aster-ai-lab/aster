import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import {
  AIX_RESOURCE_MESSAGE_LIMIT,
  AIX_RESOURCE_PROTOCOL_VERSION,
  AIX_RESOURCE_TIMEOUT_MS,
  AixResourceRefusedError,
  aixResourceRequestSchema,
  type AixResourceOperation
} from '@shared/types/aixResource'

export type AixResourceCall = (operation: AixResourceOperation, signal: AbortSignal) => Promise<unknown>

/** A live document requests only typed resource operations; native leases own authorization. */
export class AixResourceProvider {
  private port?: MessagePort
  private sequence = 0
  private messages = 0
  private stopped = false
  private pending?: AbortController

  constructor(
    private readonly context: PluginActivationContext,
    private readonly call: AixResourceCall,
    private readonly refused: () => void
  ) {}

  connect(port: MessagePort): void {
    if (this.stopped || this.port || !this.context.isCurrent()) throw new Error('AIX resource document unavailable')
    this.port = port
  }

  receive(data: unknown): void {
    if (this.stopped || !this.port || !this.context.isCurrent()) return
    const parsed = aixResourceRequestSchema.safeParse(data)
    if (++this.messages > AIX_RESOURCE_MESSAGE_LIMIT || !parsed.success || parsed.data.id <= this.sequence) {
      this.close()
      this.refused()
      return
    }
    this.sequence = parsed.data.id
    if (this.pending) {
      this.post({ id: parsed.data.id, reason: 'busy' })
      return
    }
    const abort = new AbortController()
    this.pending = abort
    void this.run(parsed.data.id, parsed.data.request, abort)
  }

  private async run(id: number, operation: AixResourceOperation, abort: AbortController): Promise<void> {
    const timeout = setTimeout(() => abort.abort(), AIX_RESOURCE_TIMEOUT_MS)
    let response: { id: number; value?: unknown; reason?: string }
    try {
      const value = await Promise.race([
        this.call(operation, abort.signal),
        new Promise<never>((_resolve, reject) => {
          abort.signal.addEventListener('abort', () => reject(new AixResourceRefusedError('outcome_unknown')), {
            once: true
          })
        })
      ])
      response = { id, value }
    } catch (error) {
      response = { id, reason: error instanceof AixResourceRefusedError ? error.reason : 'unavailable' }
    } finally {
      clearTimeout(timeout)
    }
    if (this.pending !== abort) return
    this.pending = undefined
    this.post(response)
  }

  private post(response: { id: number; value?: unknown; reason?: string }): void {
    if (this.stopped || !this.context.isCurrent()) return
    try {
      this.port?.postMessage({
        protocolVersion: AIX_RESOURCE_PROTOCOL_VERSION,
        type: response.reason ? 'aix.resource.refused' : 'aix.resource.result',
        ...response
      })
    } catch {
      this.close()
      this.refused()
    }
  }

  close(): void {
    this.stopped = true
    this.pending?.abort()
    this.pending = undefined
    this.port = undefined
  }
}
