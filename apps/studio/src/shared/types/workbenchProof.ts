import * as z from 'zod'

const id = z.string().min(1).max(256)
const digest = z.string().regex(/^sha256:[0-9a-f]{64}$/)

export const workbenchTurnPolicySchema = z.strictObject({ reviewMode: z.boolean(), artifactTrace: z.boolean() })

export const workbenchReviewFindingSchema = z.strictObject({
  id,
  severity: z.enum(['high', 'warning', 'info']),
  category: z.enum(['conclusion', 'citation', 'change', 'tool', 'uncertainty']),
  claim: z.string().min(1).max(2000),
  evidenceQuote: z.string().min(1).max(4000),
  guidance: z.string().min(1).max(2000),
  status: z.enum(['pending', 'addressed', 'accepted', 'waived']),
  reply: z.string().max(16000).optional(),
  receiptDigest: digest.optional()
})

export const workbenchArtifactOriginSchema = z.strictObject({
  kind: z.literal('paperloom-service'),
  memberId: id,
  workspaceRoot: z.string().min(1).max(4096),
  sourcePath: z.string().min(1).max(4096),
  sourceRevision: z.string().regex(/^[0-9a-f]{64}$/),
  sourceDigest: digest,
  savedRevision: z.string().regex(/^[0-9a-f]{64}$/),
  jobId: id,
  jobStatus: z.enum(['queued', 'running', 'succeeded', 'failed', 'canceled']),
  attempt: z.number().int().positive(),
  artifactKey: id,
  manifestDigest: digest,
  connectionRevision: z.string().regex(/^[0-9a-f]{64}$/),
  packageDigest: digest
})

export const workbenchArtifactVersionSchema = z.strictObject({
  origin: workbenchArtifactOriginSchema.optional(),
  id,
  path: z.string().min(1).max(4096),
  name: z.string().min(1).max(512),
  ordinal: z.number().int().positive(),
  bytes: z.number().int().nonnegative(),
  mediaType: z.string().min(1).max(128),
  contentDigest: digest,
  receiptDigest: digest,
  inputDigests: z.array(digest),
  workflowDigest: digest,
  codeDigest: digest,
  environmentDigest: digest,
  environment: z.string(),
  executionId: id,
  toolCallId: id,
  toolName: z.string().min(1).max(512),
  createdAt: z.string().min(1),
  status: z.enum(['traced', 'incomplete', 'failed']),
  error: z.string().max(2000).optional()
})

export const workbenchToolEvidenceSchema = z.strictObject({
  id,
  name: z.string().min(1).max(512),
  state: z.enum(['success', 'error', 'cancelled', 'incomplete']),
  input: z.string(),
  output: z.string()
})

export const workbenchProofSchema = z.strictObject({
  schema: z.literal('a3s.studio.workbench-proof.v1'),
  sessionId: id,
  messageId: id,
  runId: id,
  producerStatus: z.enum(['pending', 'completed', 'failed', 'cancelled']).default('pending'),
  orgId: z.string().optional(),
  context: z
    .strictObject({ prompt: z.string(), components: z.array(z.string()), host: z.unknown().nullable() })
    .optional(),
  policy: workbenchTurnPolicySchema,
  model: z.string().min(1),
  sourceText: z.string(),
  userText: z.string(),
  tools: z.array(workbenchToolEvidenceSchema),
  artifacts: z.array(workbenchArtifactVersionSchema),
  traceError: z.string().max(2000).optional(),
  review: z.strictObject({
    status: z.enum(['off', 'pending', 'running', 'remediating', 'open_queue', 'completed', 'failed', 'cancelled']),
    findings: z.array(workbenchReviewFindingSchema).max(32),
    reviewer: z.literal('independent-model'),
    startedAt: z.string().optional(),
    finishedAt: z.string().optional(),
    error: z.string().max(2000).optional()
  })
})

export const workbenchProofActionSchema = z.discriminatedUnion('action', [
  z.strictObject({ action: z.enum(['accept', 'ignore']), findingId: id }),
  z.strictObject({ action: z.enum(['cancel', 'retry']) })
])

export type WorkbenchTurnPolicy = z.infer<typeof workbenchTurnPolicySchema>
export type WorkbenchProof = z.infer<typeof workbenchProofSchema>
export type WorkbenchReviewFinding = z.infer<typeof workbenchReviewFindingSchema>
export type WorkbenchArtifactVersion = z.infer<typeof workbenchArtifactVersionSchema>
export type WorkbenchProofAction = z.infer<typeof workbenchProofActionSchema>

/** Only the host data part carries authority; ordinary reply and tool text never do. */
export function proofFromParts(
  parts: readonly { type: string; data?: unknown }[],
  messageId?: string
): WorkbenchProof | null {
  const part = parts.find((part) => part.type === 'data-workbench-proof')
  const parsed = workbenchProofSchema.safeParse(part?.data)
  return parsed.success && (!messageId || parsed.data.messageId === messageId) ? parsed.data : null
}
