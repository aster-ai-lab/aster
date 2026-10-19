import { describe, expect, it } from 'vitest'

import {
  captureTrainingText,
  emptyTraceBook,
  RL_TRAJECTORY_SCHEMA,
  scanSessionRecords,
  type TraceMessage,
  type TraceSessionSource
} from './extractExecutionTrace'

function message(overrides: Partial<TraceMessage> = {}): TraceMessage {
  return {
    id: 'message-1',
    createdAt: '2026-10-07T00:00:00.000Z',
    updatedAt: '2026-10-07T00:00:01.000Z',
    parts: [],
    ...overrides
  }
}

function source(overrides: Partial<TraceSessionSource> = {}): TraceSessionSource {
  return {
    id: 'session-1',
    name: '样本会话',
    updatedAt: '2026-10-07T00:00:00.000Z',
    messages: [message()],
    ...overrides
  }
}

async function sha256Hex(text: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text))
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

describe('scanSessionRecords', () => {
  it('projects a finished tool turn into a3s.rl_trajectory.v1', async () => {
    const result = await scanSessionRecords(emptyTraceBook(), [
      source({
        workspacePath: '/workspace/demo',
        messages: [
          message({
            role: 'assistant',
            status: 'success',
            parts: [
              { type: 'reasoning', text: 'hidden' },
              { type: 'text', text: 'hello' },
              {
                type: 'dynamic-tool',
                toolName: 'bash',
                toolCallId: 'call-1',
                state: 'output-available',
                input: { command: 'printf ok' },
                output: 'ok'
              },
              {
                type: 'tool-edit',
                toolCallId: 'call-2',
                state: 'output-error',
                errorText: 'denied',
                input: { path: 'a.ts' }
              },
              { type: 'data-compaction-anchor', data: { status: 'done', foldedCount: 4 } },
              { type: 'data-compaction-anchor', data: { status: 'skipped' } }
            ]
          })
        ]
      })
    ])

    const records = result.book.sessions[0]?.events.map((event) => event.record) ?? []
    expect(result.book.version).toBe(2)
    expect(result.book.signalRevision).toBe(1)
    expect(result.book.schema).toBe(RL_TRAJECTORY_SCHEMA)
    expect(result.added).toBe(records.length)
    expect(result.skipped).toBe(0)
    expect(records.map((record) => record.event_type)).toEqual([
      'execution_start',
      'llm_response',
      'tool_call',
      'tool_result',
      'tool_call',
      'tool_result',
      'context_compacted',
      'execution_end'
    ])
    expect(
      records.every(
        (record) => record.schema === RL_TRAJECTORY_SCHEMA && record.mode === 'on' && record.session_id === 'session-1'
      )
    ).toBe(true)
    expect(records.map((record) => record.sequence)).toEqual([1, 2, 3, 4, 5, 6, 7, 8])

    const start = records[0]
    expect(start?.payload.workspace).toBe('/workspace/demo')
    expect(start?.payload).not.toHaveProperty('max_tool_rounds')
    expect(start?.payload).not.toHaveProperty('planning_mode')
    expect(start?.payload).not.toHaveProperty('prompt')
    expect(start?.payload).not.toHaveProperty('signals')

    const response = records[1]
    expect(response?.payload).not.toHaveProperty('usage')
    expect(response?.payload).not.toHaveProperty('token_logprobs')
    const responseText = response?.payload.response_text as { text: string }
    const reasoning = response?.payload.reasoning_content as { text: string }
    expect(responseText.text).toBe('hello')
    expect(reasoning.text).toBe('hidden')
    expect(response?.payload.signals).toMatchObject({ decision: 'tool_use' })
    expect(response?.payload).not.toHaveProperty('stop_reason')

    const bash = records[2]
    expect(bash?.payload.tool).toBe('bash')
    expect(bash?.payload.args).toEqual({ command: 'printf ok' })
    expect(bash?.payload.signals).toMatchObject({ decision: 'invoked' })
    const bashResult = records[3]
    expect(bashResult?.payload.success).toBe(true)
    expect(bashResult?.payload.output).toMatchObject({ text: 'ok' })
    expect(bashResult?.payload).not.toHaveProperty('exit_code')
    expect(bashResult?.payload.signals).toMatchObject({ decision: 'succeeded', success: true })

    const editResult = records[5]
    expect(editResult?.payload.tool).toBe('edit')
    expect(editResult?.payload.success).toBe(false)
    expect(editResult?.payload.output).toMatchObject({ text: 'denied' })
    expect(editResult?.payload.signals).toMatchObject({ decision: 'failed', success: false })

    const compact = records[6]
    expect(compact?.payload.before_messages).toBe(4)
    expect(compact?.payload).not.toHaveProperty('percent_before')
    expect(compact?.payload).not.toHaveProperty('signals')
    expect(records[7]?.payload.success).toBe(false)
    expect(records[7]?.payload.tool_calls_count).toBe(2)
    expect(records[7]?.payload.signals).toMatchObject({ decision: 'stopped', success: false })
  })

  it('keeps the full prompt and only the token counts the message stored', async () => {
    const prompt = 'q'.repeat(200)
    const result = await scanSessionRecords(emptyTraceBook(), [
      source({
        messages: [
          message({ id: 'user-1', role: 'user', parts: [{ type: 'text', text: prompt }] }),
          message({
            id: 'assistant-1',
            role: 'assistant',
            status: 'success',
            createdAt: '2026-10-07T00:00:02.000Z',
            updatedAt: '2026-10-07T00:00:03.000Z',
            stats: {
              inputTokens: 3,
              outputTokens: 1,
              totalTokens: 4,
              inputTokenDetails: { cacheReadTokens: 2 },
              timeCompletionMs: 15
            },
            parts: [
              { type: 'text', text: 'done' },
              { type: 'finish', finishReason: 'stop' }
            ]
          })
        ]
      })
    ])
    const records = result.book.sessions[0]?.events.map((event) => event.record) ?? []
    const start = records.find((record) => record.event_type === 'execution_start')
    const promptText = start?.payload.prompt as { text: string; truncated: boolean; byte_len: number }
    expect(promptText.text).toBe(prompt)
    expect(promptText.truncated).toBe(false)
    expect(promptText.byte_len).toBe(200)
    expect(start?.payload).not.toHaveProperty('workspace')
    const response = records.find((record) => record.event_type === 'llm_response')
    expect(response?.payload.usage).toEqual({
      prompt_tokens: 3,
      completion_tokens: 1,
      total_tokens: 4,
      cache_read_tokens: 2
    })
    expect(response?.payload.usage).not.toHaveProperty('cache_write_tokens')
    expect(response?.payload.duration_ms).toBe(15)
    expect(response?.payload.stop_reason).toBe('stop')
    expect(response?.payload.signals).toEqual({ decision: 'reply', duration_ms: 15, stop_reason: 'stop' })
    const end = records.find((record) => record.event_type === 'execution_end')
    expect(end?.payload.success).toBe(true)
    expect(end?.payload.signals).toEqual({ decision: 'finished', success: true })
  })

  it('carries a tool observation into the next model request', async () => {
    const result = await scanSessionRecords(emptyTraceBook(), [
      source({
        messages: [
          message({ id: 'user-1', role: 'user', parts: [{ type: 'text', text: 'hello' }] }),
          message({
            id: 'assistant-1',
            role: 'assistant',
            status: 'success',
            createdAt: '2026-10-07T00:00:02.000Z',
            updatedAt: '2026-10-07T00:00:03.000Z',
            parts: [
              {
                type: 'tool-bash',
                toolCallId: 'call-1',
                state: 'output-available',
                input: { command: 'pwd' },
                output: '/tmp'
              }
            ]
          }),
          message({
            id: 'user-2',
            role: 'user',
            createdAt: '2026-10-07T00:00:04.000Z',
            updatedAt: '2026-10-07T00:00:05.000Z',
            parts: [{ type: 'text', text: 'again' }]
          })
        ]
      })
    ])
    const records = result.book.sessions[0]?.events.map((event) => event.record) ?? []
    const requests = records.filter((record) => record.event_type === 'llm_request')
    const second = requests[1]?.payload.messages as Array<{
      role: string
      content: Array<{ type: string; content?: { text: string } }>
    }>
    const observation = second?.find((item) => item.content.some((block) => block.type === 'tool_result'))
    expect(observation?.role).toBe('user')
    expect(observation?.content[0]?.content?.text).toBe('/tmp')
    expect(requests[1]?.payload.messages_count).toBe(4)
  })

  it('does not extract a record that was already stored', async () => {
    const loaded = source({
      messages: [
        message({
          parts: [
            {
              type: 'tool-bash',
              toolCallId: 'call-1',
              state: 'output-available',
              input: { command: 'pwd' },
              output: 'ok'
            }
          ]
        })
      ]
    })
    const first = await scanSessionRecords(emptyTraceBook(), [loaded])
    const second = await scanSessionRecords(first.book, [loaded])
    expect(second.added).toBe(0)
    expect(second.skipped).toBe(1)
    expect(second.book.sessions[0]?.events).toHaveLength(first.book.sessions[0]?.events.length ?? 0)
    expect(second.book.sessions[0]?.messageWatermark).toBe('2026-10-07T00:00:01.000Z')
    expect(second.book.sessions[0]?.messageCount).toBe(1)
  })

  it('replaces records when a stored message is newer', async () => {
    const first = await scanSessionRecords(emptyTraceBook(), [
      source({
        messages: [
          message({
            parts: [
              {
                type: 'dynamic-tool',
                toolName: 'bash',
                toolCallId: 'call-1',
                state: 'output-available',
                input: { command: 'printf ok' }
              }
            ]
          })
        ]
      })
    ])
    const second = await scanSessionRecords(first.book, [
      source({
        messages: [
          message({
            updatedAt: '2026-10-07T00:00:02.000Z',
            parts: [
              {
                type: 'dynamic-tool',
                toolName: 'read',
                toolCallId: 'call-3',
                state: 'approval-requested',
                input: { path: 'b.ts' }
              }
            ]
          })
        ]
      })
    ])
    const records = second.book.sessions[0]?.events.map((event) => event.record) ?? []
    expect(second.added).toBe(records.length)
    expect(second.skipped).toBe(0)
    expect(records.map((record) => record.event_type)).toEqual(['execution_start', 'llm_response', 'tool_call'])
    expect(records[2]?.payload.tool).toBe('read')
    expect(records[2]?.payload.signals).toEqual({ decision: 'awaiting_user' })
    expect(records.some((record) => record.event_type === 'tool_result' || record.event_type === 'execution_end')).toBe(
      false
    )
  })

  it('stores a denial, its note, and measured durations', async () => {
    const secret = 'sk-abc123456789'
    const result = await scanSessionRecords(emptyTraceBook(), [
      source({
        messages: [
          message({
            role: 'assistant',
            status: 'success',
            stats: {
              runtimeTiming: {
                startedAt: 1_000,
                completedAt: 1_040,
                spans: [
                  { kind: 'approval-wait', toolCallId: 'call-1', startedAt: 1_000, completedAt: 1_012 },
                  { kind: 'tool-execution', toolCallId: 'call-1', startedAt: 1_012, completedAt: 1_030 },
                  { kind: 'approval-wait', toolCallId: 'call-2', startedAt: 1_030, completedAt: 1_036 }
                ]
              }
            },
            parts: [
              {
                type: 'tool-bash',
                toolCallId: 'call-1',
                state: 'output-available',
                input: { command: 'pwd' },
                output: 'ok',
                exitCode: 0,
                approval: { id: 'ap-1', approved: true }
              },
              {
                type: 'tool-read',
                toolCallId: 'call-2',
                state: 'approval-responded',
                input: { path: 'secret.ts' },
                approval: { id: 'ap-2', approved: false, reason: `Use the read-only endpoint ${secret}` },
                error: { name: 'Denied' }
              }
            ]
          })
        ]
      })
    ])
    const records = result.book.sessions[0]?.events.map((event) => event.record) ?? []
    const bashCall = records.find((record) => record.event_type === 'tool_call' && record.payload.tool === 'bash')
    const bashResult = records.find((record) => record.event_type === 'tool_result' && record.payload.tool === 'bash')
    const readCall = records.find((record) => record.event_type === 'tool_call' && record.payload.tool === 'read')
    const readResult = records.find((record) => record.event_type === 'tool_result' && record.payload.tool === 'read')
    const response = records.find((record) => record.event_type === 'llm_response')
    expect(bashCall?.payload.signals).toMatchObject({ decision: 'approved', duration_ms: 12 })
    expect(bashResult?.payload.signals).toMatchObject({
      decision: 'succeeded',
      duration_ms: 18,
      exit_code: 0,
      success: true
    })
    expect(bashResult?.payload.exit_code).toBe(0)
    expect(readCall?.payload.signals).toMatchObject({ decision: 'denied', duration_ms: 6 })
    expect(readCall?.payload.signals).toMatchObject({ feedback: 'Use the read-only endpoint …' })
    expect(readResult?.payload.success).toBe(false)
    expect(readResult?.payload.output).toMatchObject({ text: 'Use the read-only endpoint …' })
    expect(readResult?.payload.error_kind).toBe('Denied')
    expect(readResult?.payload.signals).toMatchObject({
      decision: 'failed',
      error_kind: 'Denied',
      feedback: 'Use the read-only endpoint …'
    })
    expect(response?.payload.duration_ms).toBe(40)
    expect(response?.payload.signals).toMatchObject({ decision: 'tool_use', duration_ms: 40 })
    expect(JSON.stringify(result.book)).not.toContain(secret)
    expect(records.some((record) => record.event_type === 'execution_end' && record.payload.success === false)).toBe(
      true
    )
  })

  it('reprojects a stored book that predates step signals', async () => {
    const loaded = source({
      messages: [
        message({
          parts: [{ type: 'tool-bash', toolCallId: 'call-1', state: 'output-available', output: 'ok' }]
        })
      ]
    })
    const first = await scanSessionRecords(emptyTraceBook(), [loaded])
    const stored = { ...first.book }
    delete stored.signalRevision
    const second = await scanSessionRecords(stored, [loaded])
    expect(second.added).toBe(first.book.sessions[0]?.events.length)
    expect(second.skipped).toBe(0)
    expect(second.book.signalRevision).toBe(1)
    const third = await scanSessionRecords(second.book, [loaded])
    expect(third.added).toBe(0)
    expect(third.skipped).toBe(1)
  })

  it('redacts secrets in stored tool arguments', async () => {
    const secret = 'a'.repeat(32)
    const result = await scanSessionRecords(emptyTraceBook(), [
      source({
        messages: [
          message({
            parts: [
              {
                type: 'tool-bash',
                toolCallId: 'call-1',
                state: 'output-available',
                input: { command: `echo ${secret}` }
              }
            ]
          })
        ]
      })
    ])
    const call = result.book.sessions[0]?.events
      .map((event) => event.record)
      .find((record) => record.event_type === 'tool_call')
    expect(call?.payload.args).toEqual({ command: 'echo …' })
    expect(JSON.stringify(result.book)).not.toContain(secret)
  })

  it('stores a user prompt as training records and skips it on the next scan', async () => {
    const textOnly = source({
      messages: [message({ id: 'message-9', role: 'user', parts: [{ type: 'text', text: 'only prose' }] })]
    })
    const first = await scanSessionRecords(emptyTraceBook(), [textOnly])
    expect(first.book.sessions).toHaveLength(1)
    expect(first.book.sessions[0]?.events.map((event) => event.record.event_type)).toEqual([
      'execution_start',
      'llm_request'
    ])
    expect(first.book.seen['message-9']).toBe('2026-10-07T00:00:01.000Z')
    const second = await scanSessionRecords(first.book, [textOnly])
    expect(second.skipped).toBe(1)
    expect(second.added).toBe(0)
  })

  it('keeps the stored watermark when the caller skipped the message download', async () => {
    const first = await scanSessionRecords(emptyTraceBook(), [
      source({
        messages: [message({ id: 'message-9', role: 'user', parts: [{ type: 'text', text: 'only prose' }] })]
      })
    ])
    const stored = first.book.sessions[0]
    const second = await scanSessionRecords(first.book, [
      source({
        updatedAt: stored?.updatedAt,
        messages: [],
        unchanged: true,
        messageCount: stored?.messageCount
      })
    ])
    expect(second.added).toBe(0)
    expect(second.skipped).toBe(stored?.messageCount)
    expect(second.book.sessions[0]?.messageWatermark).toBe(stored?.messageWatermark)
    expect(second.book.sessions[0]?.messageWatermark).not.toBe('')
    expect(second.book.sessions[0]?.messageCount).toBe(stored?.messageCount)
    expect(second.book.sessions[0]?.events).toEqual(stored?.events)
  })

  it('records an empty watermark when a full read has no messages', async () => {
    const first = await scanSessionRecords(emptyTraceBook(), [
      source({
        messages: [message({ id: 'message-9', role: 'user', parts: [{ type: 'text', text: 'only prose' }] })]
      })
    ])
    const second = await scanSessionRecords(first.book, [source({ messages: [] })])
    expect(second.book.sessions[0]?.messageWatermark).toBe('')
    expect(second.book.sessions[0]?.messageCount).toBe(0)
    expect(second.book.sessions[0]?.events).toEqual(first.book.sessions[0]?.events)
  })
})

describe('captureTrainingText', () => {
  it('hashes the full redacted text and flags a short byte cut', async () => {
    const captured = await captureTrainingText('abcdef', 3)
    expect(captured.text).toBe('abc')
    expect(captured.truncated).toBe(true)
    expect(captured.byte_len).toBe(6)
    expect(captured.sha256).toBe(await sha256Hex('abcdef'))

    const secret = 'sk-abc123456789'
    const redacted = await captureTrainingText(`echo ${secret}`)
    expect(redacted.text).toBe('echo …')
    expect(redacted.sha256).toBe(await sha256Hex('echo …'))

    const split = await captureTrainingText('你a', 2)
    expect(split.text).toBe('')
    expect(split.truncated).toBe(true)
    expect(split.byte_len).toBe(4)
  })
})
