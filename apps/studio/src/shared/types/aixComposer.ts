import * as z from 'zod'

export const AIX_COMPOSER_PROTOCOL_VERSION = 1
export const AIX_COMPOSER_TEXT_BYTES = 64 * 1024
export const AIX_COMPOSER_TIMEOUT_MS = 5000

export const aixComposerTextSchema = z
  .string()
  .refine(
    (value) =>
      value.length <= AIX_COMPOSER_TEXT_BYTES && new TextEncoder().encode(value).length <= AIX_COMPOSER_TEXT_BYTES,
    'AIX composer text exceeds 64 KiB'
  )
const identity = {
  protocolVersion: z.literal(AIX_COMPOSER_PROTOCOL_VERSION),
  id: z.literal(1),
  revision: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER)
}
export const aixComposerProviderMessageSchema = z.discriminatedUnion('type', [
  z
    .object({ protocolVersion: z.literal(AIX_COMPOSER_PROTOCOL_VERSION), type: z.literal('aix.composer.ready') })
    .strict(),
  z.object({ ...identity, type: z.literal('aix.composer.result'), text: aixComposerTextSchema }).strict(),
  z.object({ ...identity, type: z.literal('aix.composer.refused') }).strict()
])

export interface AixComposerRequest {
  readonly protocolVersion: typeof AIX_COMPOSER_PROTOCOL_VERSION
  readonly type: 'aix.composer.request'
  readonly id: 1
  readonly revision: number
  readonly text: string
}
