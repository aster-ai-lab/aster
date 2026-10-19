import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import {
  AIX_TREE_MESSAGE_LIMIT,
  AIX_TREE_PROTOCOL_VERSION,
  aixTreeProviderMessageSchema,
  type AixTreeAction,
  type AixTreeRequest,
  type AixTreeSnapshot
} from '@shared/types/aixTree'

/** Package code stays in its isolated frame; only typed tree data reaches native UI. */
export class AixTreeProvider {
  private port?: globalThis.MessagePort
  private ready = false
  private sequence = 0
  private pending = 0
  private messages = 0
  private revision = 0
  private refreshQueued = false
  private timer?: ReturnType<typeof globalThis.setTimeout>
  private snapshot: AixTreeSnapshot = { status: 'loading', busy: true, nodes: [] }
  private readonly listeners = new Set<() => void>()

  constructor(
    private readonly context: PluginActivationContext,
    private readonly surface: string,
    private readonly timeoutMs = 5000,
    private readonly closed?: () => void
  ) {}

  readonly getSnapshot = (): AixTreeSnapshot => this.snapshot
  readonly subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  private publish(snapshot: AixTreeSnapshot): void {
    this.snapshot = snapshot
    for (const listener of this.listeners) listener()
  }

  private deadline(): void {
    globalThis.clearTimeout(this.timer)
    this.timer = globalThis.setTimeout(() => this.finish('failed'), this.timeoutMs)
  }

  waitForDocument(): void {
    if (!this.port && this.context.isCurrent()) this.deadline()
  }

  connect(port: globalThis.MessagePort): void {
    this.finish('closed')
    if (!this.context.isCurrent()) {
      port.close()
      return
    }
    this.sequence = this.messages = this.revision = 0
    this.refreshQueued = false
    this.port = port
    this.publish({ status: 'loading', busy: true, nodes: [] })
    this.deadline()
    port.onmessage = ({ data }) => {
      if (this.port !== port || !this.context.isCurrent()) return
      if (++this.messages > AIX_TREE_MESSAGE_LIMIT) return this.finish('failed')
      const parsed = aixTreeProviderMessageSchema.safeParse(data)
      if (!parsed.success) return this.finish('failed')
      const message = parsed.data
      if (message.type === 'aix.tree.ready') {
        if (this.ready) return this.finish('failed')
        this.ready = true
        this.request('refresh', null)
      } else if (!this.ready) this.finish('failed')
      else if (message.type === 'aix.tree.changed') {
        if (message.revision <= this.revision) return this.finish('failed')
        this.revision = message.revision
        if (this.pending) this.refreshQueued = true
        else this.request('refresh', null)
      } else {
        if (!this.pending || message.id !== this.pending) return this.finish('failed')
        this.pending = 0
        globalThis.clearTimeout(this.timer)
        this.publish({ status: 'ready', busy: false, nodes: message.nodes })
        if (this.refreshQueued) {
          this.refreshQueued = false
          this.request('refresh', null)
        }
      }
    }
    port.onmessageerror = () => {
      if (this.port === port) this.finish('failed')
    }
    port.start()
    try {
      port.postMessage({ protocolVersion: AIX_TREE_PROTOCOL_VERSION, type: 'aix.tree.connect', surface: this.surface })
    } catch (error) {
      this.finish('failed')
      throw error
    }
  }

  request(action: AixTreeAction, nodeId: string | null): boolean {
    const port = this.port
    if (!port || !this.ready || this.pending || !this.context.isCurrent()) return false
    const node = this.snapshot.nodes.find((item) => item.id === nodeId)
    if (action !== 'refresh' && (!node || ((action === 'expand' || action === 'collapse') && !node.hasChildren)))
      return false
    if (action === 'invoke' && !node?.command) return false
    if (this.sequence >= AIX_TREE_MESSAGE_LIMIT) {
      this.finish('failed')
      return false
    }
    this.pending = ++this.sequence
    this.publish({ ...this.snapshot, busy: true })
    this.deadline()
    port.postMessage({
      protocolVersion: AIX_TREE_PROTOCOL_VERSION,
      type: 'aix.tree.request',
      id: this.pending,
      action,
      nodeId: action === 'refresh' ? null : nodeId,
      ...(action === 'invoke' ? { command: node?.command } : {})
    } satisfies AixTreeRequest)
    return true
  }

  private finish(status: 'failed' | 'closed'): void {
    globalThis.clearTimeout(this.timer)
    this.pending = 0
    this.ready = false
    if (this.port) {
      this.port.onmessage = null
      this.port.onmessageerror = null
      this.port.close()
      this.port = undefined
      this.closed?.()
    }
    this.publish({ status, busy: false, nodes: [] })
  }

  close(): void {
    this.finish('closed')
  }
}
