import * as z from 'zod'

export const agentSessionArchiveIdentitySchema = z.strictObject({ operationId: z.uuid(), epoch: z.uuid() })

export const agentSessionArchiveRequestSchema = agentSessionArchiveIdentitySchema
  .extend({
    sessionIds: z.array(z.string().min(1)).min(1).max(200)
  })
  .refine((request) => new Set(request.sessionIds).size === request.sessionIds.length, 'Archive IDs must be unique')

export const agentSessionArchiveItemSchema = z.strictObject({
  sessionId: z.string().min(1),
  status: z.enum(['archived', 'already-archived', 'not-found', 'failed'])
})

export const agentSessionArchiveReceiptSchema = z
  .strictObject({
    operationId: z.uuid(),
    epoch: z.uuid(),
    deletedIds: z.array(z.string().min(1)).max(200),
    results: z.array(agentSessionArchiveItemSchema).min(1).max(200)
  })
  .refine((receipt) => {
    const archived = new Set(receipt.results.filter((item) => item.status === 'archived').map((item) => item.sessionId))
    return (
      new Set(receipt.results.map((item) => item.sessionId)).size === receipt.results.length &&
      new Set(receipt.deletedIds).size === receipt.deletedIds.length &&
      archived.size === receipt.deletedIds.length &&
      receipt.deletedIds.every((id) => archived.has(id))
    )
  }, 'Archive receipt has inconsistent item results')

export const agentSessionArchiveLookupSchema = z
  .strictObject({
    epoch: z.uuid(),
    receipt: agentSessionArchiveReceiptSchema.nullable()
  })
  .refine(
    (lookup) => lookup.receipt === null || lookup.receipt.epoch === lookup.epoch,
    'Archive receipt belongs to another instance'
  )

export type AgentSessionArchiveRequest = z.infer<typeof agentSessionArchiveRequestSchema>
export type AgentSessionArchiveReceipt = z.infer<typeof agentSessionArchiveReceiptSchema>
export type AgentSessionArchiveItem = z.infer<typeof agentSessionArchiveItemSchema>
