import { describe, expect, it } from 'vitest'

import { emptyTraceBook, type SessionTrace, type TraceBook } from './extractExecutionTrace'
import { canSkipSessionMessages } from './loadLocalSessionRecords'

const prior: SessionTrace = {
  sessionId: 'session-1',
  name: '样本',
  updatedAt: '2026-10-07T00:00:02.000Z',
  events: [],
  messageWatermark: '2026-10-07T00:00:02.000Z',
  messageCount: 3
}

function book(overrides: Partial<TraceBook> = {}): TraceBook {
  return { ...emptyTraceBook(), ...overrides }
}

describe('canSkipSessionMessages', () => {
  it('skips a download only when the listed time still covers the watermark', () => {
    expect(canSkipSessionMessages(book(), prior.updatedAt, prior)).toBe(true)
    expect(canSkipSessionMessages(book(), '2026-10-07T00:00:03.000Z', prior)).toBe(false)
    expect(
      canSkipSessionMessages(book(), '2026-10-07T00:00:01.000Z', {
        ...prior,
        updatedAt: '2026-10-07T00:00:01.000Z'
      })
    ).toBe(false)
    expect(canSkipSessionMessages(book({ signalRevision: 0 }), prior.updatedAt, prior)).toBe(false)
    expect(canSkipSessionMessages(undefined, prior.updatedAt, prior)).toBe(false)
    expect(canSkipSessionMessages(book(), prior.updatedAt, { ...prior, messageWatermark: '' })).toBe(false)
    expect(canSkipSessionMessages(book(), prior.updatedAt, { ...prior, messageWatermark: undefined })).toBe(false)
    expect(canSkipSessionMessages(book(), prior.updatedAt, { ...prior, messageCount: undefined })).toBe(false)
    expect(canSkipSessionMessages(book(), '', prior)).toBe(false)
    expect(canSkipSessionMessages(book(), prior.updatedAt, undefined)).toBe(false)
  })
})
