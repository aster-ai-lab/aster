import * as z from 'zod'

import type { PendingToolApproval } from '@shared/ai/transport'

const identity = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[^\p{Cc}]+$/u)
  .refine((value) => value.trim().length > 0 && new globalThis.TextEncoder().encode(value).byteLength <= 128)
const ordinal = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)

export const studioActiveExecutionSchema = z.strictObject({
  executionId: identity,
  attemptId: ordinal,
  anchorMessageId: identity
})

export const pendingToolApprovalsSchema = z
  .array(
    z
      .object({
        executionId: identity,
        anchorMessageId: identity,
        messageId: identity,
        approvalId: identity,
        toolCallId: identity
      })
      .strict() satisfies z.ZodType<PendingToolApproval>
  )
  .max(256)
  .refine((approvals) => new Set(approvals.map((approval) => approval.approvalId)).size === approvals.length)

/** Readonly native observation; modelTurn is a model-call ordinal, not a Tool Turn grant. */
export const studioExecutionObservationSchema = z
  .object({
    protocolVersion: z.literal(1),
    promptId: z
      .string()
      .min(1)
      .max(128)
      .regex(/^[a-zA-Z0-9_.-]+$/),
    coreSessionId: identity,
    runId: identity,
    sequence: ordinal,
    modelTurn: ordinal.nullable()
  })
  .strict()

const studioLiveStatusSchema = z
  .object({
    topicId: z
      .string()
      .max(256)
      .regex(/^agent-session:[^\p{Cc}]+$/u),
    status: z.record(z.string(), z.unknown())
  })
  .strict()

export const studioExecutionSnapshotSchema = z
  .object({
    protocolVersion: z.literal(1),
    topics: z.array(
      z
        .object({
          topicId: z
            .string()
            .max(256)
            .regex(/^agent-session:[^\p{Cc}]+$/u),
          execution: studioExecutionObservationSchema
        })
        .strict()
    ),
    liveStatuses: z.array(studioLiveStatusSchema).optional()
  })
  .strict()

export type StudioExecutionObservation = Readonly<z.infer<typeof studioExecutionObservationSchema>>
export type StudioExecutionObservations = Readonly<Record<string, StudioExecutionObservation>>
