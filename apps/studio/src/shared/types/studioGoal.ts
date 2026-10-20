import * as z from 'zod'

const ordinal = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
const binding = {
  sessionId: z.string().min(1),
  acpSessionId: z.string().min(1).nullable(),
  executionId: z.string().min(1).nullable()
}
const goal = z
  .strictObject({
    goalId: z.string().min(1),
    goalRevision: ordinal,
    planRevision: ordinal.nullable(),
    goalSequence: ordinal,
    state: z.enum(['active', 'paused', 'waiting', 'completed', 'cancelled']),
    objective: z.string(),
    tick: z.enum(['advance', 'wait', 'needs_input', 'paused', 'budget_exhausted', 'reconcile', 'complete']).nullable()
  })
  .refine((value) => value.tick !== null || value.state === 'cancelled')

/** Native current-session observation, never reconstructed from transcript commands or prose. */
export const studioGoalSnapshotSchema = z.discriminatedUnion('status', [
  z
    .strictObject({ ...binding, status: z.literal('known'), goal: goal.nullable() })
    .refine((value) => value.goal === null || value.acpSessionId !== null),
  z.strictObject({
    ...binding,
    status: z.literal('unavailable'),
    goal: z.null(),
    reason: z.enum(['runtime_unavailable', 'unsupported_agent', 'invalid_snapshot', 'request_failed', 'stale_binding'])
  })
])

export type StudioGoalSnapshot = z.infer<typeof studioGoalSnapshotSchema>
export type StudioGoal = NonNullable<Extract<StudioGoalSnapshot, { status: 'known' }>['goal']>

export function unavailableStudioGoal(
  sessionId: string,
  reason: Extract<StudioGoalSnapshot, { status: 'unavailable' }>['reason']
): StudioGoalSnapshot {
  return { status: 'unavailable', sessionId, acpSessionId: null, executionId: null, goal: null, reason }
}

export function parseStudioGoalSnapshot(value: unknown, sessionId: string): StudioGoalSnapshot {
  const parsed = studioGoalSnapshotSchema.safeParse(value)
  return parsed.success && parsed.data.sessionId === sessionId
    ? parsed.data
    : unavailableStudioGoal(sessionId, 'invalid_snapshot')
}
