import { loggerService } from '@logger'
import type {
  CommandHandler,
  CommandHandlerOptions,
  ExtensionCommandBehavior,
  ExtensionCommandDefinition,
  ExtensionCommandId,
  ExtensionCommandOwner,
  ExtensionCommandRegistration,
  RendererCommandId,
  ResolvedExtensionCommand
} from '@renderer/types/command'
import type { ContextReader } from '@shared/types/command'
import { type CommandId, evaluateContextExpr, findCommandDefinition } from '@shared/utils/command'

const logger = loggerService.withContext('RendererCommandService')
const ID = /^extension:[a-z][a-z0-9_.-]{0,254}:[a-f0-9]{64}:[a-z][a-z0-9-]{0,127}$/
const MAX_COMMANDS = 1024
const MAX_OWNER_COMMANDS = 32

interface HandlerEntry {
  readonly handler: CommandHandler
  readonly enabled: boolean
}

interface ExtensionEntry {
  readonly definition: ExtensionCommandDefinition
  readonly owner: ExtensionCommandOwner
  readonly behavior: ExtensionCommandBehavior
  readonly abort: globalThis.AbortController
  readonly dispose: () => void
  readonly execute: () => Promise<boolean>
  busy: boolean
}

export function extensionCommandId(namespace: string, documentId: string, command: string): ExtensionCommandId {
  const id: ExtensionCommandId = `extension:${namespace}:${documentId}:${command}`
  if (!ID.test(id)) throw new Error('Invalid extension command identity')
  return id
}

/** Each renderer provider owns one dispatcher; extension handlers never replace builtins. */
export class RendererCommandService {
  private readonly handlers = new Map<CommandId, HandlerEntry[]>()
  private readonly commands = new Map<ExtensionCommandId, ExtensionEntry>()
  private readonly ownerCounts = new WeakMap<ExtensionCommandOwner, number>()
  private readonly listeners = new Set<() => void>()
  private revision = 0
  private context: ContextReader = {}
  private active = true

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  readonly getSnapshot = (): number => this.revision

  private changed(): void {
    this.revision++
    for (const listener of this.listeners) listener()
  }

  setContext(context: ContextReader): void {
    if (this.context === context) return
    this.context = context
    this.changed()
  }

  open(): void {
    this.active = true
  }

  readonly registerHandler = (
    command: CommandId,
    handler: CommandHandler,
    options?: CommandHandlerOptions
  ): (() => void) => {
    if (!this.active) throw new Error('Renderer command dispatcher closed')
    if (!findCommandDefinition(command)) throw new Error('Unknown builtin command')
    const entry = { handler, enabled: options?.enabled !== false }
    const handlers = this.handlers.get(command) ?? []
    this.handlers.set(command, [...handlers, entry])
    this.changed()
    return () => {
      const current = this.handlers.get(command)
      if (!current?.includes(entry)) return
      const remaining = current.filter((item) => item !== entry)
      if (remaining.length) this.handlers.set(command, remaining)
      else this.handlers.delete(command)
      this.changed()
    }
  }

  private activeHandler(command: CommandId): HandlerEntry | undefined {
    const definition = findCommandDefinition(command)
    if (!this.active || !definition || !evaluateContextExpr(definition.enablement, this.context)) return
    return this.handlers.get(command)?.findLast((entry) => entry.enabled)
  }

  private enabled(entry: ExtensionEntry, acceptingResult = false): boolean {
    if (
      !this.active ||
      this.commands.get(entry.definition.id) !== entry ||
      entry.abort.signal.aborted ||
      (entry.busy && !acceptingResult)
    )
      return false
    try {
      return entry.owner.isCurrent() && entry.behavior.enabled()
    } catch (error) {
      logger.error('Extension command eligibility failed', error as Error)
      return false
    }
  }

  readonly registerCommand = (
    definition: ExtensionCommandDefinition,
    owner: ExtensionCommandOwner,
    behavior: ExtensionCommandBehavior
  ): ExtensionCommandRegistration => {
    const text = (value: unknown, limit: number): boolean =>
      typeof value === 'string' && value.trim().length > 0 && [...value].length <= limit
    if (
      !ID.test(definition.id) ||
      !text(definition.title, 64) ||
      (definition.category !== undefined && !text(definition.category, 64)) ||
      (definition.iconKey !== undefined && !/^[a-z][a-z0-9-]{0,127}$/.test(definition.iconKey)) ||
      Object.keys(definition).some((key) => !['id', 'title', 'category', 'iconKey'].includes(key)) ||
      typeof behavior.enabled !== 'function' ||
      typeof behavior.execute !== 'function'
    )
      throw new Error('Invalid extension command definition')
    if (!this.active) throw new Error('Renderer command dispatcher closed')
    if (owner.signal.aborted || !owner.isCurrent()) throw new Error('Extension command owner closed')
    if (this.commands.has(definition.id)) throw new Error('Duplicate extension command identity')
    if (this.commands.size >= MAX_COMMANDS || (this.ownerCounts.get(owner) ?? 0) >= MAX_OWNER_COMMANDS)
      throw new Error('Extension command admission budget exhausted')

    const abort = new globalThis.AbortController()
    const entry: ExtensionEntry = {
      definition: Object.freeze({ ...definition }),
      owner,
      behavior: { enabled: behavior.enabled.bind(behavior), execute: behavior.execute.bind(behavior) },
      abort,
      busy: false,
      dispose: () => {
        if (this.commands.get(entry.definition.id) !== entry) return
        this.commands.delete(entry.definition.id)
        this.ownerCounts.set(owner, (this.ownerCounts.get(owner) ?? 1) - 1)
        owner.signal.removeEventListener('abort', entry.dispose)
        abort.abort()
        this.changed()
      },
      execute: async () => {
        if (!this.enabled(entry)) return false
        entry.busy = true
        this.changed()
        try {
          if (!this.enabled(entry, true)) return false
          await entry.behavior.execute(abort.signal)
          return this.enabled(entry, true)
        } catch (error) {
          logger.error('Extension command execution failed', error as Error)
          return false
        } finally {
          entry.busy = false
          if (this.commands.get(entry.definition.id) === entry) this.changed()
        }
      }
    }
    this.commands.set(definition.id, entry)
    this.ownerCounts.set(owner, (this.ownerCounts.get(owner) ?? 0) + 1)
    owner.signal.addEventListener('abort', entry.dispose, { once: true })
    this.changed()
    return Object.freeze({
      dispose: entry.dispose,
      refresh: () => {
        if (this.commands.get(entry.definition.id) === entry) this.changed()
      }
    })
  }

  readonly resolveCommand = (command: ExtensionCommandId): ResolvedExtensionCommand | undefined => {
    const entry = this.commands.get(command)
    if (!this.active || !entry || entry.abort.signal.aborted || !entry.owner.isCurrent()) return
    return Object.freeze({ ...entry.definition, enabled: this.enabled(entry), execute: entry.execute })
  }

  readonly hasHandler = (command: RendererCommandId): boolean =>
    command.startsWith('extension:')
      ? Boolean(this.resolveCommand(command as ExtensionCommandId)?.enabled)
      : Boolean(this.activeHandler(command as CommandId))

  readonly execute = (command: RendererCommandId): void => {
    if (command.startsWith('extension:')) {
      void this.resolveCommand(command as ExtensionCommandId)?.execute()
      return
    }
    const handler = this.activeHandler(command as CommandId)?.handler
    if (!handler) {
      logger.warn(`No renderer command handler registered: ${command}`)
      return
    }
    try {
      void Promise.resolve(handler()).catch((error) =>
        logger.error(`Renderer command handler failed: ${command}`, error)
      )
    } catch (error) {
      logger.error(`Renderer command handler failed: ${command}`, error as Error)
    }
  }

  close(): void {
    this.active = false
    for (const entry of [...this.commands.values()]) entry.dispose()
    if (this.handlers.size) {
      this.handlers.clear()
      this.changed()
    }
  }
}
