import { describe, expect, it } from 'vitest'

import {
  agentSessionArchiveRequestSchema,
  agentSessionArchiveReceiptSchema,
  agentSessionArchiveLookupSchema
} from '../agentSessionArchive'

const operationId = '12345678-1234-4234-8234-123456789012'
const epoch = '12345678-1234-4234-8234-123456789013'

describe('archive operation wire contract', () => {
  it('requires a bounded, unique selection tied to one operation and native instance', () => {
    expect(agentSessionArchiveRequestSchema.safeParse({ operationId, epoch, sessionIds: ['one', 'two'] }).success).toBe(
      true
    )
    for (const input of [
      { operationId, sessionIds: ['one'] },
      { operationId, epoch, sessionIds: [] },
      { operationId, epoch, sessionIds: ['one', 'one'] },
      { operationId, epoch, sessionIds: Array.from({ length: 201 }, (_, i) => String(i)) },
      { operationId, epoch, sessionIds: ['one'], permanent: true }
    ])
      expect(agentSessionArchiveRequestSchema.safeParse(input).success).toBe(false)
  })

  it('refuses a receipt that labels a failed or duplicated item as archived', () => {
    const receipt = {
      operationId,
      epoch,
      deletedIds: ['one'],
      results: [
        { sessionId: 'one', status: 'archived' },
        { sessionId: 'two', status: 'failed' }
      ]
    }
    expect(agentSessionArchiveReceiptSchema.parse(receipt).results[1].status).toBe('failed')
    expect(agentSessionArchiveReceiptSchema.safeParse({ ...receipt, deletedIds: ['two'] }).success).toBe(false)
    expect(
      agentSessionArchiveReceiptSchema.safeParse({ ...receipt, results: [receipt.results[0], receipt.results[0]] })
        .success
    ).toBe(false)
    expect(
      agentSessionArchiveReceiptSchema.safeParse({ ...receipt, results: [{ sessionId: 'one', status: 'success' }] })
        .success
    ).toBe(false)
    expect(agentSessionArchiveLookupSchema.safeParse({ epoch: operationId, receipt }).success).toBe(false)
    expect(agentSessionArchiveLookupSchema.parse({ epoch, receipt: null }).receipt).toBeNull()
  })
})
