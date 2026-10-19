import { beforeEach, describe, expect, it, vi } from 'vitest'

const ipc = vi.hoisted(() => ({
  on: vi.fn<(channel: string, callback: (payload: unknown) => void) => () => void>(() => vi.fn()),
  request: vi.fn()
}))

vi.mock('@renderer/ipc/ipcApi', () => ({ ipcApi: ipc }))

import {
  applyTobotChunk,
  approvalFromLines,
  overlayAgentLines,
  overlayPendingReply,
  resetTobotStreamForTests,
  subscribeTobotStream,
  type TobotLive
} from '@renderer/services/tobotStream'

const chunk = (delta: string, topicId = 'agent-session:tobot', anchorMessageId = 'assistant-1') =>
  ({
    topicId,
    anchorMessageId,
    chunk: { type: 'text-delta', delta }
  }) as Parameters<typeof applyTobotChunk>[1]

describe('tobotStream', () => {
  beforeEach(() => {
    resetTobotStreamForTests()
    ipc.on.mockClear()
    ipc.request.mockReset()
  })

  it('appends text deltas for the ToBot topic and ignores other topics', () => {
    const first = applyTobotChunk(null, chunk('你是 '))
    expect(first).toEqual({
      anchorId: 'assistant-1',
      text: '你是 ',
      parts: [{ type: 'text', text: '你是 ', state: 'streaming' }]
    })
    const second = applyTobotChunk(first, chunk('ToBot'))
    expect(second).toEqual({
      anchorId: 'assistant-1',
      text: '你是 ToBot',
      parts: [{ type: 'text', text: '你是 ToBot', state: 'streaming' }]
    })
    expect(applyTobotChunk(second, chunk('nope', 'agent-session:task'))).toBe(second)
    expect(applyTobotChunk(second, chunk('', 'agent-session:tobot', 'assistant-2'))).toBe(second)
  })

  it('keeps legacy form history readable with typed read-only answers', () => {
    const requested = applyTobotChunk(null, {
      topicId: 'agent-session:tobot',
      anchorMessageId: 'assistant-1',
      chunk: { type: 'form-request', formId: 'form-1', format: 'openui', spec: '<Input name="count" label="Count" />' }
    })
    const answered = applyTobotChunk(requested, {
      topicId: 'agent-session:tobot',
      anchorMessageId: 'assistant-1',
      chunk: { type: 'form-answered', formId: 'form-1', values: { count: 3, agreed: false } }
    })
    expect(answered?.parts).toContainEqual({
      type: 'data-form',
      data: {
        id: 'form-1',
        format: 'openui',
        spec: '<Input name="count" label="Count" />',
        state: 'answered',
        values: { count: 3, agreed: false }
      }
    })
  })

  it('preserves standard SDK form snapshots and refuses stale owners, versions and answer replays', () => {
    const binding = {
      protocolVersion: 1,
      requestId: 'a'.repeat(64),
      revision: `sha256:${'b'.repeat(64)}`,
      owner: {
        sessionId: 'tobot',
        executionId: 'model',
        attemptId: 1,
        anchorMessageId: 'assistant-1',
        acpSessionId: 'acp',
        promptId: 'prompt'
      }
    }
    const spec = 'root = Card("Details", [])'
    const envelope = {
      topicId: 'agent-session:tobot',
      anchorMessageId: 'assistant-1',
      executionId: 'model',
      attemptId: 1
    }
    const request = {
      ...envelope,
      chunk: {
        type: 'data-form',
        id: binding.requestId,
        data: { id: binding.requestId, binding, format: 'openui', spec, state: 'form-requested' }
      }
    } as Parameters<typeof applyTobotChunk>[1]
    const live = applyTobotChunk(null, request)
    expect(live?.parts[0]?.data?.binding).toEqual(binding)
    const answer = {
      ...envelope,
      chunk: {
        type: 'data-form',
        id: binding.requestId,
        data: {
          id: binding.requestId,
          binding,
          format: 'openui',
          spec,
          state: 'answered',
          values: { count: 0, agreed: false }
        }
      }
    } as Parameters<typeof applyTobotChunk>[1]
    expect(applyTobotChunk(live, { ...answer, attemptId: 2 })).toBe(live)
    expect(applyTobotChunk(live, { ...answer, executionId: 'other' as never })).toBe(live)
    const stale = globalThis.structuredClone(answer)
    Object.assign((stale.chunk as { data: { binding: typeof binding } }).data.binding, {
      revision: `sha256:${'c'.repeat(64)}`
    })
    expect(applyTobotChunk(live, stale)).toBe(live)
    const changed = globalThis.structuredClone(request)
    Object.assign((changed.chunk as { data: { spec: string } }).data, { spec: 'root = Card("Changed", [])' })
    expect(applyTobotChunk(live, changed)).toBe(live)
    const answered = applyTobotChunk(live, answer)
    expect(answered?.parts[0]?.data).toEqual({
      id: binding.requestId,
      binding,
      format: 'openui',
      spec,
      state: 'answered',
      values: { count: 0, agreed: false }
    })
    expect(applyTobotChunk(answered, answer)).toBe(answered)
    expect(applyTobotChunk(answered, request)).toBe(answered)
  })

  it('keeps reasoning and tool rows on the live turn', () => {
    const text = applyTobotChunk(null, chunk('先看一下。'))
    const reasoning = applyTobotChunk(text, {
      topicId: 'agent-session:tobot',
      anchorMessageId: 'assistant-1',
      chunk: { type: 'reasoning-delta', id: 'r', delta: '记忆已经在约束里。' }
    })
    const tool = applyTobotChunk(reasoning, {
      topicId: 'agent-session:tobot',
      anchorMessageId: 'assistant-1',
      chunk: {
        type: 'tool-input-available',
        toolCallId: 'call-1',
        toolName: 'bash',
        input: { command: 'find . -maxdepth 4' }
      }
    })
    const done = applyTobotChunk(tool, {
      topicId: 'agent-session:tobot',
      anchorMessageId: 'assistant-1',
      chunk: { type: 'tool-output-available', toolCallId: 'call-1', output: 'Directory: .\n' }
    })
    expect(done?.text).toBe('先看一下。')
    expect(done?.parts.map((part) => part.type)).toEqual(['text', 'reasoning', 'dynamic-tool'])
    expect(done?.parts[1]).toMatchObject({ text: '记忆已经在约束里。', state: 'streaming' })
    expect(done?.parts[2]).toMatchObject({
      toolName: 'bash',
      state: 'output-available',
      output: 'Directory: .\n'
    })
    const pending = overlayAgentLines(
      [{ id: 'assistant-1', role: 'assistant', text: '', status: 'pending', parts: [] }],
      done ?? null,
      (id, text, parts) => ({ id, role: 'assistant', text, status: 'pending', parts })
    )
    expect(pending[0]?.parts).toHaveLength(3)
    expect(pending[0]?.text).toBe('先看一下。')
  })

  it('keeps a finished server line and grows a pending one', () => {
    const live: TobotLive = {
      anchorId: 'assistant-1',
      text: '## 标题',
      parts: [{ type: 'text', text: '## 标题', state: 'streaming' }]
    }
    const pending = overlayAgentLines(
      [{ id: 'assistant-1', role: 'assistant', text: '', status: 'pending' }],
      live,
      (id, text) => ({ id, role: 'assistant', text, status: 'pending' })
    )
    expect(pending[0]?.text).toBe('## 标题')
    const settled = overlayAgentLines(
      [{ id: 'assistant-1', role: 'assistant', text: 'saved', status: 'success' }],
      live,
      (id, text) => ({ id, role: 'assistant', text, status: 'pending' })
    )
    expect(settled[0]?.text).toBe('saved')
  })

  it('replays the open turn unless the listener asks for live chunks only', async () => {
    const buffered = chunk('旧回复')
    ipc.request.mockImplementation(async (route: string) =>
      route === 'ai.stream.attach' ? { status: 'attached', bufferedChunks: [buffered] } : undefined
    )
    const seen: string[] = []
    const take = (payload: Parameters<typeof applyTobotChunk>[1]) => {
      if (payload.chunk.type === 'text-delta' && 'delta' in payload.chunk) seen.push(payload.chunk.delta)
    }
    const stopLive = subscribeTobotStream(take, () => undefined, { replay: false })
    await ipc.request.mock.results.at(-1)?.value
    await Promise.resolve()
    expect(seen).toEqual([])
    stopLive()

    const stopReplay = subscribeTobotStream(take, () => undefined)
    await ipc.request.mock.results.at(-1)?.value
    await Promise.resolve()
    expect(seen).toEqual(['旧回复'])
    stopReplay()
  })

  it('keeps the stream attached while another listener is still listening', async () => {
    ipc.request.mockImplementation(async (route: string) =>
      route === 'ai.stream.attach' ? { status: 'attached', bufferedChunks: [chunk('旧')] } : undefined
    )
    const first: string[] = []
    const second: string[] = []
    const take = (into: string[]) => (payload: Parameters<typeof applyTobotChunk>[1]) => {
      if (payload.chunk.type === 'text-delta' && 'delta' in payload.chunk) into.push(payload.chunk.delta)
    }
    const stopFirst = subscribeTobotStream(take(first), () => undefined, { replay: false })
    const stopSecond = subscribeTobotStream(take(second), () => undefined)
    await ipc.request.mock.results.at(-1)?.value
    await Promise.resolve()
    expect(first).toEqual([])
    expect(second).toEqual(['旧'])
    expect(ipc.request.mock.calls.filter((call) => call[0] === 'ai.stream.attach')).toHaveLength(2)
    stopSecond()
    expect(ipc.request.mock.calls.filter((call) => call[0] === 'ai.stream.detach')).toHaveLength(0)
    const onChunk = ipc.on.mock.calls.find((call) => call[0] === 'ai.stream.chunk')?.[1] as
      | ((payload: ReturnType<typeof chunk>) => void)
      | undefined
    onChunk?.(chunk('新'))
    expect(first).toEqual(['新'])
    expect(second).toEqual(['旧'])
    stopFirst()
    expect(ipc.request.mock.calls.filter((call) => call[0] === 'ai.stream.detach')).toHaveLength(1)
  })

  it('does not deliver chunks from another ToBot session', async () => {
    const memberA = 'tobot-11111111-1111-4111-8111-111111111111'
    const memberB = 'tobot-22222222-2222-4222-8222-222222222222'
    ipc.request.mockImplementation(async (route: string, input?: { topicId?: string }) => {
      if (route !== 'ai.stream.attach') return undefined
      const topicId = input?.topicId ?? ''
      return {
        status: 'attached',
        bufferedChunks: [chunk(topicId.endsWith(memberA) ? 'only-a' : 'only-b', topicId)]
      }
    })
    const seenA: string[] = []
    const seenB: string[] = []
    const take = (into: string[]) => (payload: Parameters<typeof applyTobotChunk>[1]) => {
      if (payload.chunk.type === 'text-delta' && 'delta' in payload.chunk) into.push(payload.chunk.delta)
    }
    const stopA = subscribeTobotStream(take(seenA), () => undefined, { sessionId: memberA })
    const stopB = subscribeTobotStream(take(seenB), () => undefined, { sessionId: memberB })
    await ipc.request.mock.results.at(-1)?.value
    await ipc.request.mock.results.at(-2)?.value
    await Promise.resolve()
    expect(seenA).toEqual(['only-a'])
    expect(seenB).toEqual(['only-b'])
    const onChunk = ipc.on.mock.calls.find((call) => call[0] === 'ai.stream.chunk')?.[1] as
      | ((payload: ReturnType<typeof chunk>) => void)
      | undefined
    onChunk?.(chunk('leak', `agent-session:${memberA}`))
    expect(seenA).toEqual(['only-a', 'leak'])
    expect(seenB).toEqual(['only-b'])
    stopA()
    onChunk?.(chunk('late', `agent-session:${memberA}`))
    expect(seenB).toEqual(['only-b'])
    const detached = ipc.request.mock.calls.filter((call) => call[0] === 'ai.stream.detach')
    expect(detached.some((call) => call[1]?.topicId === `agent-session:${memberA}`)).toBe(true)
    expect(detached.some((call) => call[1]?.topicId === `agent-session:${memberB}`)).toBe(false)
    stopB()
  })

  it('writes the live reply onto the pending group line', () => {
    const live: TobotLive = {
      anchorId: 'assistant-1',
      text: '正在写',
      parts: [{ type: 'text', text: '正在写', state: 'streaming' }]
    }
    const shown = overlayPendingReply(
      [
        { text: '', status: 'pending' },
        { text: 'old', status: 'success' }
      ],
      live
    )
    expect(shown[0]?.text).toBe('正在写')
    expect(shown[1]?.text).toBe('old')
  })

  it('restores a waiting tool approval from the newest pending assistant', () => {
    const minutes = {
      type: 'dynamic-tool',
      toolName: 'meeting-minutes',
      state: 'approval-requested',
      approval: { id: 'a1' }
    }
    expect(approvalFromLines([{ role: 'assistant', status: 'pending', parts: [minutes] }])).toEqual({
      approvalId: 'a1',
      toolName: 'meeting-minutes'
    })
    expect(approvalFromLines([{ role: 'assistant', status: 'success', parts: [minutes] }])).toBeNull()
    expect(
      approvalFromLines([{ role: 'assistant', status: 'pending', parts: [{ type: 'text', text: 'hi' }] }])
    ).toBeNull()
    expect(
      approvalFromLines([
        { role: 'assistant', status: 'pending', parts: [minutes] },
        {
          role: 'assistant',
          status: 'pending',
          parts: [{ type: 'dynamic-tool', toolName: 'read', state: 'output-available' }]
        }
      ])
    ).toBeNull()
    expect(
      approvalFromLines([
        {
          role: 'assistant',
          status: 'pending',
          parts: [
            { type: 'dynamic-tool', toolName: 'read', state: 'output-available' },
            minutes,
            {
              type: 'dynamic-tool',
              toolName: 'calendar',
              state: 'approval-requested',
              approval: { id: 'a2' }
            }
          ]
        }
      ])
    ).toEqual({ approvalId: 'a1', toolName: 'meeting-minutes' })
  })
})
