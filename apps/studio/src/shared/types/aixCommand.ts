import * as z from 'zod'

export const AIX_COMMAND_PROTOCOL_VERSION = 1
export const AIX_COMMAND_MESSAGE_LIMIT = 4096
const identity = z.number().int().positive().max(Number.MAX_SAFE_INTEGER)
const command = z.string().regex(/^[a-z][a-z0-9-]{0,127}$/)
export const aixCommandProviderMessageSchema = z.discriminatedUnion('type', [
  z.object({ protocolVersion: z.literal(AIX_COMMAND_PROTOCOL_VERSION), type: z.literal('aix.command.ready') }).strict(),
  z
    .object({
      protocolVersion: z.literal(AIX_COMMAND_PROTOCOL_VERSION),
      type: z.enum(['aix.command.completed', 'aix.command.refused']),
      id: identity,
      command
    })
    .strict()
])

export interface AixCommandConnect {
  readonly protocolVersion: typeof AIX_COMMAND_PROTOCOL_VERSION
  readonly type: 'aix.command.connect'
  readonly surface: string
  readonly commands: readonly string[]
}

export interface AixCommandExecute {
  readonly protocolVersion: typeof AIX_COMMAND_PROTOCOL_VERSION
  readonly type: 'aix.command.execute'
  readonly id: number
  readonly command: string
}

export interface AixCommandSnapshot {
  readonly status: 'loading' | 'ready' | 'failed' | 'closed'
  readonly busy: boolean
}
