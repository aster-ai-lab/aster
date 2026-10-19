import * as z from 'zod'

export const AIX_RESOURCE_PROTOCOL_VERSION = 1
export const AIX_RESOURCE_MESSAGE_LIMIT = 4096
export const AIX_RESOURCE_TIMEOUT_MS = 60_000

const uuid = z
  .string()
  .uuid()
  .refine((value) => value === value.toLowerCase() && value !== '00000000-0000-0000-0000-000000000000')
const key = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[A-Za-z0-9_.-]+$/)
  .refine((value) => value !== '.' && value !== '..')
const digest = z.string().regex(/^sha256:[a-f0-9]{64}$/)
const revision = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER)
export const AIX_DRAFT_BYTES = 64 * 1024 * 1024
const draftContent = z
  .string()
  .refine((value) => new TextEncoder().encode(value).byteLength <= AIX_DRAFT_BYTES)
  .nullable()
export const aixDraftSnapshotSchema = z.object({ revision, content: draftContent }).strict()
const fields = {
  lease: uuid,
  key,
  operationId: uuid,
  expectedRevision: revision.nullable(),
  content: z.string().max(16 * 1024 * 1024),
  assets: z.array(digest).max(64)
}

export const aixResourceOperationSchema = z.discriminatedUnion('operation', [
  z.object({ operation: z.literal('workspaces') }).strict(),
  z.object({ operation: z.literal('read-draft'), key }).strict(),
  z
    .object({
      operation: z.literal('save-draft'),
      key,
      expectedRevision: revision,
      operationId: uuid,
      content: draftContent
    })
    .strict(),
  z
    .object({
      operation: z.literal('projects'),
      workspaceId: uuid,
      page: z.number().int().min(1).max(100_001),
      limit: z.number().int().min(1).max(100)
    })
    .strict(),
  z
    .object({
      operation: z.literal('create-project'),
      workspaceId: uuid,
      id: uuid,
      title: z.string().trim().min(1).max(128)
    })
    .strict(),
  z
    .object({
      operation: z.literal('open'),
      workspaceId: uuid,
      projectId: uuid,
      resources: z
        .array(key)
        .min(1)
        .max(16)
        .refine((values) => new Set(values).size === values.length),
      access: z.enum(['read', 'read-write'])
    })
    .strict(),
  z.object({ operation: z.literal('close'), lease: uuid }).strict(),
  z.object({ operation: z.literal('read'), lease: uuid, key, revision: revision.nullable() }).strict(),
  z.object({ operation: z.literal('read-receipt'), lease: uuid, operationId: uuid }).strict(),
  z.object({ operation: z.literal('save'), ...fields }).strict(),
  z.object({ operation: z.literal('read-asset'), lease: uuid, digest }).strict(),
  z
    .object({
      operation: z.literal('store-asset'),
      lease: uuid,
      digest,
      kind: z.enum(['glb', 'recording', 'artifact']),
      bytesBase64: z.string().max(Math.ceil((100 * 1024 * 1024) / 3) * 4)
    })
    .strict()
])

export const aixResourceRequestSchema = z
  .object({
    protocolVersion: z.literal(AIX_RESOURCE_PROTOCOL_VERSION),
    type: z.literal('aix.resource.request'),
    id: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
    request: aixResourceOperationSchema
  })
  .strict()

export const AIX_RESOURCE_REFUSALS = [
  'invalid_request',
  'not_found',
  'conflict',
  'busy',
  'unavailable',
  'outcome_unknown',
  'closed'
] as const
export type AixResourceRefusal = (typeof AIX_RESOURCE_REFUSALS)[number]
export type AixResourceOperation = z.infer<typeof aixResourceOperationSchema>

export class AixResourceRefusedError extends Error {
  constructor(readonly reason: AixResourceRefusal) {
    super(`AIX resource refused: ${reason}`)
  }
}

export const aixDocumentSnapshotSchema = z
  .object({
    key,
    revision,
    content: fields.content,
    digest,
    assets: fields.assets
  })
  .strict()

export const aixDocumentReceiptSchema = z
  .object({
    operationId: uuid,
    document: aixDocumentSnapshotSchema
  })
  .strict()

export const aixDocumentProjectSchema = z.object({ id: uuid, title: z.string().min(1).max(128) }).strict()
export const aixDocumentProjectPageSchema = z
  .object({
    items: z.array(aixDocumentProjectSchema).max(100),
    total: revision,
    page: z.number().int().positive(),
    limit: z.number().int().min(1).max(100)
  })
  .strict()
  .refine(
    (value) =>
      value.page <= Math.max(1, Math.ceil(value.total / value.limit)) &&
      value.items.length === Math.min(value.limit, Math.max(0, value.total - (value.page - 1) * value.limit))
  )

export const aixResourceLeaseSchema = z
  .object({
    protocolVersion: z.literal(1),
    lease: uuid,
    workspaceId: uuid,
    projectId: uuid,
    resources: z.array(key).min(1).max(16),
    access: z.enum(['read', 'read-write']),
    identity: z
      .object({
        packageId: z.string().min(1).max(256),
        generation: z.number().int().positive(),
        revision: digest,
        activationId: z.string().regex(/^[a-f0-9]{64}$/),
        instanceId: z.string().regex(/^[a-f0-9]{64}$/),
        canRead: z.boolean(),
        canWrite: z.boolean()
      })
      .strict()
  })
  .strict()
