import type { PluginActivationContext, PluginComposerDraft } from '@renderer/types/workbenchPlugin'
import {
  AIX_COMPOSER_PROTOCOL_VERSION,
  AIX_COMPOSER_TIMEOUT_MS,
  aixComposerProviderMessageSchema,
  aixComposerTextSchema,
  type AixComposerRequest
} from '@shared/types/aixComposer'

/** One explicit user action receives one bounded draft and may propose one replacement. */
export class AixComposerProvider {
  private port?: MessagePort
  private ready = false
  private stopped = false
  private pending?: { revision: number; text: string; resolve(text: string): void; reject(error: Error): void }
  private timer?: ReturnType<typeof setTimeout>

  constructor(
    private readonly context: PluginActivationContext,
    private readonly surface: string,
    private readonly timeoutMs = AIX_COMPOSER_TIMEOUT_MS
  ) {}

  invoke(draft: PluginComposerDraft): Promise<string> {
    if (this.stopped || this.pending || !this.context.isCurrent())
      return Promise.reject(new Error('AIX composer unavailable'))
    if (
      !Number.isSafeInteger(draft.revision) ||
      draft.revision < 0 ||
      !aixComposerTextSchema.safeParse(draft.text).success
    )
      return Promise.reject(new Error('Invalid AIX composer draft'))
    return new Promise((resolve, reject) => {
      this.pending = { revision: draft.revision, text: draft.text, resolve, reject }
      this.timer = setTimeout(() => this.close(), this.timeoutMs)
      this.request()
    })
  }

  connect(port: MessagePort): void {
    if (this.port || this.stopped || !this.context.isCurrent()) {
      port.close()
      return
    }
    this.port = port
    port.onmessage = ({ data }) => {
      if (this.port !== port || this.stopped || !this.context.isCurrent()) return this.close()
      const parsed = aixComposerProviderMessageSchema.safeParse(data)
      if (!parsed.success) return this.close()
      const message = parsed.data
      if (message.type === 'aix.composer.ready') {
        if (this.ready) return this.close()
        this.ready = true
        this.request()
        return
      }
      const pending = this.pending
      if (!this.ready || !pending || message.revision !== pending.revision) return this.close()
      this.pending = undefined
      this.close()
      if (message.type === 'aix.composer.result') pending.resolve(message.text)
      else pending.reject(new Error('AIX composer declined the edit'))
    }
    port.onmessageerror = () => this.close()
    port.start()
    try {
      port.postMessage({
        protocolVersion: AIX_COMPOSER_PROTOCOL_VERSION,
        type: 'aix.composer.connect',
        surface: this.surface
      })
    } catch (error) {
      this.close()
      throw error
    }
  }

  private request(): void {
    if (!this.ready || !this.pending || !this.port) return
    try {
      this.port.postMessage({
        protocolVersion: AIX_COMPOSER_PROTOCOL_VERSION,
        type: 'aix.composer.request',
        id: 1,
        revision: this.pending.revision,
        text: this.pending.text
      } satisfies AixComposerRequest)
    } catch {
      this.close()
    }
  }

  close(): void {
    this.stopped = true
    clearTimeout(this.timer)
    if (this.port) {
      this.port.onmessage = null
      this.port.onmessageerror = null
      this.port.close()
      this.port = undefined
    }
    this.pending?.reject(new Error('AIX composer closed or timed out'))
    this.pending = undefined
  }
}
