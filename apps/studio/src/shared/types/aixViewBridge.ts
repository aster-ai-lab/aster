import * as z from 'zod'

export const AIX_VIEW_BRIDGE_VERSION = 1
export const AIX_VIEW_STATE_BYTES = 64 * 1024
export const AIX_VIEW_BRIDGE_MESSAGES = 4096
export const AIX_DOCUMENT_PROTOCOL_VERSION = 1
export const AIX_DOCUMENT_ATTEMPT_LIMIT = 16

export const aixEditorRecoveryStatusSchema = z
  .object({
    protocolVersion: z.literal(1),
    type: z.literal('aix.view.recovery'),
    dirty: z.boolean()
  })
  .strict()
export const aixEditorCheckpointResultSchema = z
  .object({
    protocolVersion: z.literal(1),
    type: z.literal('aix.view.checkpoint.result'),
    id: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
    ok: z.boolean(),
    dirty: z.boolean()
  })
  .strict()
export const aixEditorCheckpointRequestSchema = z
  .object({
    protocolVersion: z.literal(1),
    type: z.literal('aix.view.checkpoint'),
    id: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
    purpose: z.enum(['leave', 'renew'])
  })
  .strict()

/** A document supplies its own port; the host never hands state to a WindowProxy. */
export const aixDocumentReadySchema = z
  .object({
    protocolVersion: z.literal(AIX_DOCUMENT_PROTOCOL_VERSION),
    type: z.literal('aix.document.ready'),
    documentId: z.string().regex(/^[a-f0-9]{64}$/)
  })
  .strict()

export const AIX_VIEW_THEME_TOKENS = [
  '--background',
  '--foreground',
  '--card',
  '--card-foreground',
  '--popover',
  '--popover-foreground',
  '--primary',
  '--primary-foreground',
  '--secondary',
  '--secondary-foreground',
  '--muted',
  '--muted-foreground',
  '--accent',
  '--accent-foreground',
  '--destructive',
  '--border',
  '--input',
  '--ring',
  '--radius',
  '--font-family',
  '--code-font-family'
] as const

/** Readonly appearance observations; they confer no native resource authority. */
export const aixViewEnvironmentSchema = z
  .object({
    theme: z.enum(['light', 'dark']),
    locale: z
      .string()
      .max(64)
      .regex(/^[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*$/),
    direction: z.enum(['ltr', 'rtl']),
    tokens: z.partialRecord(z.enum(AIX_VIEW_THEME_TOKENS), z.string().max(256))
  })
  .strict()

export type AixViewEnvironment = z.infer<typeof aixViewEnvironmentSchema>

/** Serialized UI state is data, never native configuration or executable code. */
export function isAixViewState(value: string | null): boolean {
  if (value === null) return true
  if (value.length > AIX_VIEW_STATE_BYTES || new TextEncoder().encode(value).length > AIX_VIEW_STATE_BYTES) return false
  try {
    const pending: { value: unknown; depth: number }[] = [{ value: JSON.parse(value), depth: 0 }]
    let nodes = 0
    while (pending.length) {
      const current = pending.pop()!
      if (++nodes > 4096 || current.depth > 16) return false
      if (current.value && typeof current.value === 'object') {
        for (const child of Object.values(current.value)) pending.push({ value: child, depth: current.depth + 1 })
      } else if (typeof current.value === 'number' && !Number.isFinite(current.value)) return false
    }
    return true
  } catch {
    return false
  }
}

export const aixViewStateRequestSchema = z
  .object({
    protocolVersion: z.literal(AIX_VIEW_BRIDGE_VERSION),
    type: z.literal('aix.view.store'),
    id: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
    stateJSON: z.string().nullable().refine(isAixViewState)
  })
  .strict()

export interface AixViewConnect {
  readonly protocolVersion: typeof AIX_VIEW_BRIDGE_VERSION
  readonly type: 'aix.view.connect'
  readonly surface: string
  readonly stateJSON: string | null
  readonly environment?: AixViewEnvironment
}

export interface AixViewEnvironmentChanged {
  readonly protocolVersion: typeof AIX_VIEW_BRIDGE_VERSION
  readonly type: 'aix.view.environment'
  readonly environment: AixViewEnvironment
}

export interface AixViewStored {
  readonly protocolVersion: typeof AIX_VIEW_BRIDGE_VERSION
  readonly type: 'aix.view.stored'
  readonly id: number
}
