import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import {
  AIX_AGENT_MESSAGE_LIMIT,
  AIX_AGENT_PROTOCOL_VERSION,
  AixAgentRefusedError,
  aixAgentInvokeSchema,
  type AixAgentImage,
  type AixAgentInvoke,
  type AixAgentRefusal,
  type AixAgentRefused,
  type AixAgentResult
} from '@shared/types/aixAgent'

export type AixAgentTurn = (
  agent: string,
  prompt: string,
  parameters: Record<string, unknown> | undefined,
  images: readonly AixAgentImage[] | undefined,
  signal: globalThis.AbortSignal
) => Promise<string>

/** One admitted document calls its own package's edge agents, one turn at a time, on its private port. */
export class AixAgentProvider {
  private port?: globalThis.MessagePort
  private pending?: { readonly id: number; readonly abort: globalThis.AbortController }
  private messages = 0
  private closed = false

  constructor(
    private readonly context: PluginActivationContext,
    private readonly turn: AixAgentTurn,
    private readonly refused: () => void
  ) {
    context.signal.addEventListener('abort', this.close, { once: true })
  }

  private readonly isCurrent = (): boolean => !this.closed && this.context.isCurrent()

  connect(port: globalThis.MessagePort): void {
    if (!this.isCurrent() || this.port) throw new Error('AIX agent document connection refused')
    this.port = port
  }

  receive(data: unknown): void {
    if (!this.isCurrent() || !this.port) return
    const parsed = aixAgentInvokeSchema.safeParse(data)
    if (++this.messages > AIX_AGENT_MESSAGE_LIMIT || !parsed.success) return this.fail()
    const message = parsed.data
    if (this.pending) return this.refuse(message.id, 'busy')
    const abort = new globalThis.AbortController()
    this.pending = { id: message.id, abort }
    void this.run(message, abort)
  }

  private async run(message: AixAgentInvoke, abort: globalThis.AbortController): Promise<void> {
    let reply: AixAgentResult | AixAgentRefused
    try {
      const text = await this.turn(message.agent, message.prompt, message.parameters, message.images, abort.signal)
      reply = { protocolVersion: AIX_AGENT_PROTOCOL_VERSION, type: 'aix.agent.result', id: message.id, text }
    } catch (error) {
      const reason: AixAgentRefusal = error instanceof AixAgentRefusedError ? error.reason : 'agent_failed'
      reply = { protocolVersion: AIX_AGENT_PROTOCOL_VERSION, type: 'aix.agent.refused', id: message.id, reason }
    }
    if (this.pending?.abort !== abort) return
    this.pending = undefined
    if (abort.signal.aborted || !this.isCurrent()) return
    this.post(reply)
  }

  private refuse(id: number, reason: AixAgentRefusal): void {
    this.post({ protocolVersion: AIX_AGENT_PROTOCOL_VERSION, type: 'aix.agent.refused', id, reason })
  }

  private post(message: AixAgentResult | AixAgentRefused): void {
    try {
      this.port?.postMessage(message)
    } catch {
      this.fail()
    }
  }

  private fail(): void {
    if (this.closed) return
    this.close()
    this.refused()
  }

  readonly close = (): void => {
    if (this.closed) return
    this.closed = true
    this.context.signal.removeEventListener('abort', this.close)
    this.pending?.abort.abort()
    this.pending = undefined
    this.port = undefined
  }
}
