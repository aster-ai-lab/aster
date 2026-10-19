import * as z from 'zod'

export const AIX_AGENT_PROTOCOL_VERSION = 1
export const AIX_AGENT_MESSAGE_LIMIT = 4096
export const AIX_AGENT_PROMPT_BYTES = 16 * 1024
export const AIX_AGENT_PARAMETERS_BYTES = 64 * 1024
export const AIX_AGENT_IMAGES = 2
/** Decoded bytes of one image. The native host checks the type and the size again. */
export const AIX_AGENT_IMAGE_BYTES = 256 * 1024
export const AIX_AGENT_REFUSALS = [
  'busy',
  'capacity',
  'not_found',
  'invalid_request',
  'agent_failed',
  'closed'
] as const
export type AixAgentRefusal = (typeof AIX_AGENT_REFUSALS)[number]

const encoder = new TextEncoder()
const identity = z.number().int().positive().max(Number.MAX_SAFE_INTEGER)
const IMAGE_DATA = /^data:image\/(?:jpeg|png);base64,[A-Za-z0-9+/]+={0,2}$/

/** One camera frame for one turn. The box sees only its name; the host attaches the bytes. */
export const aixAgentImageSchema = z
  .object({
    name: z.string().regex(/^[a-z][a-z0-9_-]{0,31}$/),
    data: z
      .string()
      .max(Math.ceil(AIX_AGENT_IMAGE_BYTES / 3) * 4 + 32)
      .regex(IMAGE_DATA)
  })
  .strict()

export type AixAgentImage = z.infer<typeof aixAgentImageSchema>

/** A view asks one edge agent of its own package for one turn. */
export const aixAgentInvokeSchema = z
  .object({
    protocolVersion: z.literal(AIX_AGENT_PROTOCOL_VERSION),
    type: z.literal('aix.agent.invoke'),
    id: identity,
    agent: z.string().regex(/^[a-z0-9][a-z0-9-]{0,63}$/),
    prompt: z
      .string()
      .min(1)
      .refine((prompt) => prompt.trim().length > 0 && encoder.encode(prompt).length <= AIX_AGENT_PROMPT_BYTES),
    parameters: z
      .record(z.string(), z.unknown())
      .refine((parameters) => encoder.encode(JSON.stringify(parameters)).length <= AIX_AGENT_PARAMETERS_BYTES)
      .optional(),
    images: z
      .array(aixAgentImageSchema)
      .min(1)
      .max(AIX_AGENT_IMAGES)
      .refine((images) => new Set(images.map((image) => image.name)).size === images.length)
      .optional()
  })
  .strict()

export type AixAgentInvoke = z.infer<typeof aixAgentInvokeSchema>

export const aixAgentTurnSchema = z.object({ text: z.string() }).strict()

export interface AixAgentResult {
  readonly protocolVersion: typeof AIX_AGENT_PROTOCOL_VERSION
  readonly type: 'aix.agent.result'
  readonly id: number
  readonly text: string
}

export interface AixAgentRefused {
  readonly protocolVersion: typeof AIX_AGENT_PROTOCOL_VERSION
  readonly type: 'aix.agent.refused'
  readonly id: number
  readonly reason: AixAgentRefusal
}

export class AixAgentRefusedError extends Error {
  constructor(readonly reason: AixAgentRefusal) {
    super(`AIX view agent refused: ${reason}`)
  }
}
