import type { CommandId } from '@shared/utils/command'

export type ExtensionCommandId = `extension:${string}`
export type RendererCommandId = CommandId | ExtensionCommandId
export type CommandHandler = () => void | Promise<void>

export interface CommandHandlerOptions {
  enabled?: boolean
}

/** Renderer registration ownership grants no native or Code resource authority. */
export interface ExtensionCommandOwner {
  readonly signal: globalThis.AbortSignal
  isCurrent(): boolean
}

export interface ExtensionCommandDefinition {
  readonly id: ExtensionCommandId
  readonly title: string
  readonly category?: string
  readonly iconKey?: string
}

export interface ExtensionCommandBehavior {
  enabled(): boolean
  execute(signal: globalThis.AbortSignal): void | Promise<void>
}

export interface ExtensionCommandRegistration {
  dispose(): void
  refresh(): void
}

/** The captured operation belongs to this registration, even after ID reuse. */
export interface ResolvedExtensionCommand extends ExtensionCommandDefinition {
  readonly enabled: boolean
  execute(): Promise<boolean>
}

export interface CommandRuntime {
  execute(command: RendererCommandId): void
  hasHandler(command: RendererCommandId): boolean
  registerHandler(command: CommandId, handler: CommandHandler, options?: CommandHandlerOptions): () => void
  registerCommand(
    definition: ExtensionCommandDefinition,
    owner: ExtensionCommandOwner,
    behavior: ExtensionCommandBehavior
  ): ExtensionCommandRegistration
  resolveCommand(command: ExtensionCommandId): ResolvedExtensionCommand | undefined
  subscribe(listener: () => void): () => void
  getSnapshot(): number
}
