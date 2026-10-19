import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import {
  AIX_VIEW_BRIDGE_MESSAGES,
  AIX_VIEW_BRIDGE_VERSION,
  aixViewStateRequestSchema,
  aixViewEnvironmentSchema,
  type AixViewEnvironment,
  type AixViewEnvironmentChanged,
  type AixViewConnect,
  type AixViewStored
} from '@shared/types/aixViewBridge'

/** One executing frame gets a revocable UI state/appearance port. */
export class AixViewBridge {
  private port?: globalThis.MessagePort
  private environment?: AixViewEnvironment
  private environmentJSON?: string

  constructor(
    private readonly context: PluginActivationContext,
    private readonly surface: string,
    private readonly state: { read(): string | null; write(value: string | null): boolean },
    private readonly closed?: () => void
  ) {}

  connect(port: globalThis.MessagePort): void {
    this.close()
    if (!this.context.isCurrent()) {
      port.close()
      return
    }
    this.port = port
    let sequence = 0
    let messages = 0
    port.onmessage = ({ data }) => {
      if (this.port !== port || !this.context.isCurrent()) return
      // Deck facts and recovery notices are not stored document state. A strict
      // store parse closes the port, and the outline invoke after a homepage
      // seed never reaches the host.
      if (
        data?.protocolVersion === AIX_VIEW_BRIDGE_VERSION &&
        (data?.type === 'aix.view.home' ||
          data?.type === 'aix.view.deck-snapshot' ||
          data?.type === 'aix.view.deck-result' ||
          data?.type === 'aix.view.recovery' ||
          data?.type === 'aix.view.checkpoint.result')
      )
        return
      const parsed = aixViewStateRequestSchema.safeParse(data)
      if (++messages > AIX_VIEW_BRIDGE_MESSAGES || !parsed.success || parsed.data.id <= sequence) {
        this.close()
        return
      }
      sequence = parsed.data.id
      if (!this.state.write(parsed.data.stateJSON)) {
        this.close()
        return
      }
      port.postMessage({
        protocolVersion: AIX_VIEW_BRIDGE_VERSION,
        type: 'aix.view.stored',
        id: sequence
      } satisfies AixViewStored)
    }
    port.onmessageerror = () => {
      if (this.port === port) this.close()
    }
    port.start()
    try {
      port.postMessage({
        protocolVersion: AIX_VIEW_BRIDGE_VERSION,
        type: 'aix.view.connect',
        surface: this.surface,
        stateJSON: this.state.read(),
        ...(this.environment ? { environment: this.environment } : {})
      } satisfies AixViewConnect)
    } catch (error) {
      this.close()
      throw error
    }
  }

  setEnvironment(value: AixViewEnvironment): void {
    if (!this.context.isCurrent()) return
    const environment = aixViewEnvironmentSchema.parse(value)
    const serialized = JSON.stringify(environment)
    if (serialized === this.environmentJSON) return
    this.environment = environment
    this.environmentJSON = serialized
    this.port?.postMessage({
      protocolVersion: AIX_VIEW_BRIDGE_VERSION,
      type: 'aix.view.environment',
      environment
    } satisfies AixViewEnvironmentChanged)
  }

  close(): void {
    if (this.port) {
      this.port.onmessage = null
      this.port.onmessageerror = null
      this.port.close()
      this.port = undefined
      this.closed?.()
    }
  }
}
