import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import {
  AIX_COMMAND_MESSAGE_LIMIT,
  AIX_COMMAND_PROTOCOL_VERSION,
  aixCommandProviderMessageSchema,
  type AixCommandConnect,
  type AixCommandExecute,
  type AixCommandSnapshot
} from '@shared/types/aixCommand'

interface PendingCommand {
  readonly id: number
  readonly command: string
  readonly signal: globalThis.AbortSignal
  readonly cancel: () => void
  resolve(): void
  reject(error: Error): void
}

/** One admitted document routes only its declared operations on its private port. */
export class AixCommandProvider {
  private port?: globalThis.MessagePort
  private pending?: PendingCommand
  private sequence = 0
  private messages = 0
  private timer?: ReturnType<typeof globalThis.setTimeout>
  private readonly abort = new globalThis.AbortController()
  private readonly commands: ReadonlySet<string>
  private readonly listeners = new Set<() => void>()
  private snapshot: AixCommandSnapshot = { status: 'loading', busy: false }
  readonly signal = this.abort.signal

  constructor(
    private readonly context: PluginActivationContext,
    private readonly surface: string,
    commands: readonly string[],
    private readonly refused: () => void,
    private readonly timeoutMs = 5000
  ) {
    this.commands = new Set(commands)
    context.signal.addEventListener('abort', this.close, { once: true })
  }

  readonly isCurrent = (): boolean => !this.signal.aborted && this.context.isCurrent()
  readonly getSnapshot = (): AixCommandSnapshot => this.snapshot
  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  private publish(snapshot: AixCommandSnapshot): void {
    this.snapshot = snapshot
    for (const listener of this.listeners) listener()
  }

  private deadline(): void {
    globalThis.clearTimeout(this.timer)
    this.timer = globalThis.setTimeout(() => this.finish('failed'), this.timeoutMs)
  }

  waitForDocument(): void {
    if (this.isCurrent() && !this.port) this.deadline()
  }

  connect(port: globalThis.MessagePort): void {
    if (!this.isCurrent() || this.port || this.snapshot.status !== 'loading')
      throw new Error('AIX command document connection refused')
    this.port = port
    this.deadline()
    try {
      port.postMessage({
        protocolVersion: AIX_COMMAND_PROTOCOL_VERSION,
        type: 'aix.command.connect',
        surface: this.surface,
        commands: [...this.commands].sort()
      } satisfies AixCommandConnect)
    } catch (error) {
      this.finish('failed')
      throw error
    }
  }

  receive(data: unknown): void {
    if (!this.isCurrent() || !this.port) return
    const parsed = aixCommandProviderMessageSchema.safeParse(data)
    if (++this.messages > AIX_COMMAND_MESSAGE_LIMIT || !parsed.success) return this.finish('failed')
    const message = parsed.data
    if (message.type === 'aix.command.ready') {
      if (this.snapshot.status !== 'loading') return this.finish('failed')
      globalThis.clearTimeout(this.timer)
      this.publish({ status: 'ready', busy: false })
      return
    }
    const pending = this.pending
    if (!pending || message.id !== pending.id || message.command !== pending.command) return this.finish('failed')
    globalThis.clearTimeout(this.timer)
    this.pending = undefined
    pending.signal.removeEventListener('abort', pending.cancel)
    this.publish({ status: 'ready', busy: false })
    if (message.type === 'aix.command.completed' && this.isCurrent() && !pending.signal.aborted) pending.resolve()
    else pending.reject(new Error('AIX view command refused'))
  }

  canExecute(command: string): boolean {
    return (
      this.isCurrent() &&
      !!this.port &&
      this.snapshot.status === 'ready' &&
      !this.pending &&
      this.messages < AIX_COMMAND_MESSAGE_LIMIT &&
      this.commands.has(command)
    )
  }

  execute(command: string, signal: globalThis.AbortSignal): Promise<void> {
    if (!this.canExecute(command) || signal.aborted) return Promise.reject(new Error('AIX view command unavailable'))
    return new Promise<void>((resolve, reject) => {
      const id = ++this.sequence
      const cancel = () => this.finish('failed')
      this.pending = { id, command, signal, cancel, resolve, reject }
      signal.addEventListener('abort', cancel, { once: true })
      this.publish({ status: 'ready', busy: true })
      if (!this.pending || !this.isCurrent()) return
      this.deadline()
      try {
        this.port?.postMessage({
          protocolVersion: AIX_COMMAND_PROTOCOL_VERSION,
          type: 'aix.command.execute',
          id,
          command
        } satisfies AixCommandExecute)
      } catch {
        this.finish('failed')
      }
    })
  }

  private finish(status: 'failed' | 'closed'): void {
    if (this.signal.aborted) return
    globalThis.clearTimeout(this.timer)
    const pending = this.pending
    this.pending = undefined
    this.port = undefined
    this.context.signal.removeEventListener('abort', this.close)
    this.abort.abort()
    pending?.signal.removeEventListener('abort', pending.cancel)
    pending?.reject(new Error('AIX command document closed'))
    this.publish({ status, busy: false })
    if (status === 'failed') this.refused()
  }

  readonly close = (): void => this.finish('closed')
}
