import { afterEach, describe, expect, it, vi } from 'vitest'

import { exportTraceDocument, saveTraceFile } from './exportTraceBook'
import { emptyTraceBook, RL_TRAJECTORY_SCHEMA, type TraceBook, type TrainingRecord } from './extractExecutionTrace'

function record(payload: Record<string, unknown>): { messageId: string; record: TrainingRecord } {
  return {
    messageId: 'message-1',
    record: {
      schema: RL_TRAJECTORY_SCHEMA,
      sequence: 1,
      timestamp_ms: 1,
      event_type: 'tool_result',
      session_id: 'session-1',
      mode: 'on',
      context: {},
      payload
    }
  }
}

function book(events: TraceBook['sessions'][number]['events']): TraceBook {
  return {
    ...emptyTraceBook(),
    sessions: [
      {
        sessionId: 'session-1',
        name: '样本',
        updatedAt: '2026-10-07T00:00:00.000Z',
        events
      }
    ]
  }
}

describe('exportTraceDocument', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('drops a missing duration and keeps a measured zero', () => {
    const stored = book([
      record({ tool: 'bash', signals: { decision: 'succeeded' } }),
      record({ tool: 'read', duration_ms: 0, signals: { decision: 'approved', duration_ms: 0 } }),
      record({ tool: 'edit', duration_ms: Number.NaN, signals: { duration_ms: Number.POSITIVE_INFINITY } })
    ])
    const result = exportTraceDocument(stored)
    const lines = result.text
      .trim()
      .split('\n')
      .map((line) => JSON.parse(line) as TrainingRecord)

    expect(result.measured).toBe(1)
    expect(result.text.endsWith('\n')).toBe(true)
    expect(lines).toHaveLength(3)
    const [missing, zero, infinite] = lines
    const duration = (line: TrainingRecord | undefined) =>
      (line?.payload.signals as { duration_ms?: unknown } | undefined)?.duration_ms
    expect(missing?.payload.duration_ms).toBeUndefined()
    expect(duration(missing)).toBeUndefined()
    expect(zero?.payload.duration_ms).toBe(0)
    expect(duration(zero)).toBe(0)
    expect(infinite?.payload.duration_ms).toBeUndefined()
    expect(duration(infinite)).toBeUndefined()
    expect(stored.sessions[0]?.events[2]?.record.payload.duration_ms).toBeNaN()
  })

  it('returns empty text when the book has no records', () => {
    expect(exportTraceDocument(emptyTraceBook())).toEqual({ text: '', measured: 0 })
  })

  it('downloads the text and ignores a missing URL helper', () => {
    const createObjectURL = vi.fn(() => 'blob:trace')
    const revokeObjectURL = vi.fn()
    vi.stubGlobal('URL', { createObjectURL, revokeObjectURL })
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {})

    saveTraceFile('a3s.rl_trajectory.v1.jsonl', '{"a":1}\n')

    expect(createObjectURL).toHaveBeenCalledTimes(1)
    const blob = createObjectURL.mock.calls[0]?.[0] as Blob
    expect(blob.type).toBe('application/x-ndjson')
    expect(click).toHaveBeenCalledTimes(1)
    expect(document.querySelector('a[download="a3s.rl_trajectory.v1.jsonl"]')).toBeNull()
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:trace')

    vi.stubGlobal('URL', {})
    expect(() => saveTraceFile('a3s.rl_trajectory.v1.jsonl', '')).not.toThrow()
  })
})
