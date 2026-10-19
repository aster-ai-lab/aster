import * as z from 'zod'

export const AGENT_FORM_PROTOCOL_VERSION = 1
export const AGENT_FORM_SPEC_MAX_BYTES = 65_536

const bytes = (value: string) => new globalThis.TextEncoder().encode(value).byteLength
const identity = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[^\p{Cc}]+$/u)
  .refine((value) => value.trim().length > 0 && bytes(value) <= 128)
const requestId = z.string().regex(/^[a-f0-9]{64}$/)

export const agentFormBindingSchema = z
  .object({
    protocolVersion: z.literal(AGENT_FORM_PROTOCOL_VERSION),
    requestId,
    owner: z
      .object({
        sessionId: identity,
        executionId: identity,
        attemptId: z.number().int().min(1).max(4_294_967_295),
        anchorMessageId: identity,
        acpSessionId: identity,
        promptId: identity,
        run: z
          .object({
            coreSessionId: identity,
            runId: identity,
            modelTurn: z.number().int().min(1).max(Number.MAX_SAFE_INTEGER).nullable()
          })
          .strict()
          .optional()
      })
      .strict(),
    revision: z.string().regex(/^sha256:[a-f0-9]{64}$/)
  })
  .strict()

export const agentFormValuesSchema = z
  .record(
    identity,
    z.union([
      z
        .string()
        .max(4096)
        .refine((value) => bytes(value) <= 4096),
      z.number().min(-Number.MAX_SAFE_INTEGER).max(Number.MAX_SAFE_INTEGER),
      z.boolean()
    ])
  )
  .refine((values) => Object.keys(values).length <= 64 && bytes(JSON.stringify(values)) <= 65_536)

export const agentFormSubmissionSchema = z
  .object({ binding: agentFormBindingSchema, values: agentFormValuesSchema })
  .strict()

export const agentQuestionSchema = z
  .object({
    question: z
      .string()
      .min(1)
      .refine((text) => text.trim().length > 0 && bytes(text) <= 4096),
    choices: z
      .array(
        z
          .object({
            id: identity,
            label: z
              .string()
              .min(1)
              .refine((text) => text.trim().length > 0 && bytes(text) <= 4096)
          })
          .strict()
      )
      .max(8),
    allowFreeText: z.boolean()
  })
  .strict()
  .refine(
    (question) =>
      (question.choices.length > 0 || question.allowFreeText) &&
      new Set(question.choices.map((choice) => choice.id)).size === question.choices.length
  )
export type AgentQuestion = z.infer<typeof agentQuestionSchema>
export const agentFormReceiptSchema = z
  .object({
    protocolVersion: z.literal(AGENT_FORM_PROTOCOL_VERSION),
    requestId,
    ok: z.boolean()
  })
  .strict()

const part = {
  id: requestId,
  binding: agentFormBindingSchema,
  format: z.enum(['openui', 'question']),
  spec: z
    .string()
    .min(1)
    .max(AGENT_FORM_SPEC_MAX_BYTES)
    .refine((value) => bytes(value) <= AGENT_FORM_SPEC_MAX_BYTES)
}
export const agentFormPartSchema = z
  .discriminatedUnion('state', [
    z.object({ ...part, state: z.literal('form-requested') }).strict(),
    z.object({ ...part, state: z.literal('closed') }).strict(),
    z.object({ ...part, state: z.literal('answered'), values: agentFormValuesSchema }).strict()
  ])
  .refine((part) => part.id === part.binding.requestId)

export type AgentFormBinding = z.infer<typeof agentFormBindingSchema>
export type AgentFormValues = z.infer<typeof agentFormValuesSchema>
export type AgentFormSubmission = z.infer<typeof agentFormSubmissionSchema>
