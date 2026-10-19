import { readFileSync } from 'node:fs'
import process from 'node:process'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { AgentFormSubmission } from '@shared/types/agentForm'

import { cacheService } from '../CacheService'
import {
  answerStudioAgentRequest,
  installStudioAgentIpc,
  resetStudioAgentPlaneForTests,
  submitAgentForm
} from '../studioAgentPlane'

const token = 'b'.repeat(64)
const runtime = { apiBaseUrl: 'http://127.0.0.1:9', token }
const plane = 'http://127.0.0.1:7'

type Call = {
  url: string
  method: string
  body: unknown
  auth: string | null
  signal?: globalThis.AbortSignal | null
  cache?: RequestCache
}

class FakeEventSource {
  static last: FakeEventSource | null = null
  onmessage: ((message: { data: string }) => void) | null = null
  onopen: (() => void) | null = null
  onerror: (() => void) | null = null
  constructor(readonly url: string) {
    FakeEventSource.last = this
  }
  close() {}
  open() {
    this.onopen?.()
  }
  disconnect() {
    this.onerror?.()
  }
  send(event: unknown) {
    this.onmessage?.({ data: JSON.stringify(event) })
  }
}

function json(value: unknown, status = 200) {
  return new globalThis.Response(JSON.stringify(value), { status, headers: { 'content-type': 'application/json' } })
}

function stubFetch(answer: (call: Call) => globalThis.Response | undefined) {
  const calls: Call[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: globalThis.RequestInit) => {
      const headers = new globalThis.Headers(init?.headers)
      if (url === `${runtime.apiBaseUrl}/api/agent-plane`) return json({ baseUrl: plane })
      const call = {
        url,
        method: init?.method ?? 'GET',
        body: init?.body ? JSON.parse(String(init.body)) : undefined,
        auth: headers.get('authorization'),
        signal: init?.signal,
        cache: init?.cache
      }
      calls.push(call)
      return answer(call) ?? json({ message: 'nope' }, 404)
    })
  )
  return calls
}

function installBridge() {
  const original = {
    request: vi.fn(async () => ({ ok: true, data: 'original' })),
    on: vi.fn(() => () => {})
  }
  const dataApi = {
    request: vi.fn(async (request: { path: string }) =>
      request.path === '/agent-workspaces/w1'
        ? { id: 'x', status: 200, data: { id: 'w1', name: 'proj', path: '/tmp/proj', type: 'user' } }
        : { id: 'x', status: 404, error: { code: 'NOT_FOUND', message: 'not found', status: 404 } }
    ),
    onDataChanged: vi.fn(() => () => {})
  }
  vi.stubGlobal('api', { ipcApi: { ...original }, dataApi })
  return { original, dataApi }
}

async function connectEvents() {
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
  installBridge()
  stubFetch(() => undefined)
  installStudioAgentIpc()
  globalThis.api!.ipcApi.on('ai.stream.chunk', () => {})
  await vi.waitFor(() => expect(FakeEventSource.last).not.toBeNull())
  const source = FakeEventSource.last!
  source.open()
  return source
}

const observed = (runId: string, sequence = 1, modelTurn: number | null = null) => ({
  protocolVersion: 1,
  promptId: 'prompt-1',
  coreSessionId: 'core-1',
  runId,
  sequence,
  modelTurn
})
const executionCache = 'studio.execution.observations' as const

describe('studioAgentPlane', () => {
  beforeEach(() => {
    vi.stubGlobal('EventSource', FakeEventSource)
    globalThis.localStorage.removeItem('a3s.studio.harness-assemble.v1')
  })

  afterEach(() => {
    resetStudioAgentPlaneForTests()
    FakeEventSource.last = null
    Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
    vi.unstubAllGlobals()
  })

  it('leaves agent routes to the Electron bridge when no studio runtime is present', async () => {
    await expect(answerStudioAgentRequest({ id: 'r', method: 'GET', path: '/agents' } as never)).resolves.toBeNull()
  })

  it('routes only the scoped message DELETE to the native plane and preserves its errors', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const calls = stubFetch((call) =>
      call.method === 'DELETE' && call.url.endsWith('/messages/owned')
        ? json(null)
        : json({ message: 'message not found' }, 404)
    )
    const deleted = await answerStudioAgentRequest({
      id: 'delete',
      method: 'DELETE',
      path: '/agent-sessions/s1/messages/owned'
    } as never)
    expect(deleted?.status).toBe(200)
    expect(calls[0]).toMatchObject({
      url: `${plane}/api/agent-sessions/s1/messages/owned`,
      method: 'DELETE',
      auth: `Bearer ${token}`
    })
    const missing = await answerStudioAgentRequest({
      id: 'missing',
      method: 'DELETE',
      path: '/agent-sessions/s2/messages/foreign'
    } as never)
    expect(missing).toMatchObject({ status: 404, error: { code: 'NOT_FOUND' } })
    const unsupported = await answerStudioAgentRequest({
      id: 'deny',
      method: 'DELETE',
      path: '/agent-sessions/s1'
    } as never)
    expect(unsupported).toMatchObject({ status: 405 })
  })

  it('posts the exact form binding and accepts only a correlated versioned native receipt', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const submission: AgentFormSubmission = {
      binding: {
        protocolVersion: 1,
        requestId: 'a'.repeat(64),
        revision: `sha256:${'b'.repeat(64)}`,
        owner: {
          sessionId: 's1',
          executionId: 'model',
          attemptId: 1,
          anchorMessageId: 'assistant',
          acpSessionId: 'acp',
          promptId: 'prompt'
        }
      },
      values: { count: 0, ready: false, topic: 'Work' }
    }
    let receipt: unknown = { protocolVersion: 1, requestId: submission.binding.requestId, ok: true }
    const calls = stubFetch(() => json(receipt))
    const signal = new globalThis.AbortController().signal
    await expect(submitAgentForm(submission, signal)).resolves.toBe(true)
    expect(calls[0]).toEqual({
      url: `${plane}/api/agent-forms/${submission.binding.requestId}`,
      method: 'POST',
      body: submission,
      auth: `Bearer ${token}`,
      signal,
      cache: 'no-store'
    })
    receipt = { protocolVersion: 1, requestId: 'c'.repeat(64), ok: true }
    await expect(submitAgentForm(submission, signal)).resolves.toBe(false)
    receipt = { ok: true }
    await expect(submitAgentForm(submission, signal)).resolves.toBe(false)
    receipt = { protocolVersion: 1, requestId: submission.binding.requestId, ok: false }
    await expect(submitAgentForm(submission, signal)).resolves.toBe(false)
  })

  it('does not send malformed or cancelled form submissions', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const calls = stubFetch(() => json({ ok: true }))
    const signal = new globalThis.AbortController().signal
    await expect(submitAgentForm({ values: { count: 3 } } as unknown as AgentFormSubmission, signal)).resolves.toBe(
      false
    )
    const abort = new globalThis.AbortController()
    abort.abort()
    await expect(
      submitAgentForm(
        {
          binding: {
            protocolVersion: 1,
            requestId: 'a'.repeat(64),
            revision: `sha256:${'b'.repeat(64)}`,
            owner: {
              sessionId: 's1',
              executionId: 'model',
              attemptId: 1,
              anchorMessageId: 'assistant',
              acpSessionId: 'acp',
              promptId: 'prompt'
            }
          },
          values: {}
        },
        abort.signal
      )
    ).resolves.toBe(false)
    expect(calls).toEqual([])
  })

  it('preserves captured team and recipe despite a later local selection change', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    installBridge()
    const calls = stubFetch((call) => {
      if (call.url === `${plane}/api/agent-sessions/s1/turns`) return json({ mode: 'started' })
      return undefined
    })
    installStudioAgentIpc()
    const bridge = globalThis.api!.ipcApi
    const input = {
      topicId: 'agent-session:s1',
      trigger: 'submit-message',
      userMessageParts: [],
      orgId: 'captured-team',
      harnessComponents: ['system', 'tools', 'budget', 'infer']
    }
    try {
      globalThis.localStorage.setItem('a3s.org.center', 'personal')
      await bridge.request('ai.stream.open', input)
      globalThis.localStorage.setItem('a3s.org.center', '0b9d2a4e-1c3f-4a5b-8d6e-7f8091a2b3c4')
      await bridge.request('ai.stream.open', input)
      expect(calls.map((call) => call.body)).toEqual([input, input])
    } finally {
      globalThis.localStorage.removeItem('a3s.org.center')
    }
  })

  it('serves agent and session reads from the agent plane', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    installBridge()
    const calls = stubFetch((call) => {
      if (call.url.startsWith(`${plane}/api/agents?`)) return json({ items: [], total: 0, page: 1 })
      if (call.url.startsWith(`${plane}/api/pins?`)) return json([])
      return undefined
    })

    const listed = await answerStudioAgentRequest({
      id: 'r',
      method: 'GET',
      path: '/agents',
      params: { ids: ['a', 'b'], page: 1 }
    } as never)
    expect(listed?.data).toEqual({ items: [], total: 0, page: 1 })
    expect(calls[0]).toMatchObject({
      url: `${plane}/api/agents?ids=a%2Cb&page=1`,
      auth: `Bearer ${token}`,
      cache: 'no-store'
    })

    const missing = await answerStudioAgentRequest({ id: 'r', method: 'GET', path: '/agent-sessions/s1' } as never)
    expect(missing?.error).toMatchObject({ code: 'NOT_FOUND', status: 404 })

    await expect(
      answerStudioAgentRequest({ id: 'r', method: 'GET', path: '/pins', params: { entityType: 'session' } } as never)
    ).resolves.toMatchObject({ data: [] })
    expect(calls.at(-1)).toMatchObject({
      url: `${plane}/api/pins?entityType=session`,
      method: 'GET',
      cache: 'no-store'
    })
    expect(calls.map((call) => call.cache)).toEqual(['no-store', 'no-store', 'no-store'])
  })

  it('forwards task pin and unpin to the agent plane', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    installBridge()
    const pin = {
      id: 'p1',
      entityType: 'session',
      entityId: 's1',
      orderKey: 'a0',
      createdAt: '2026-10-04T00:00:00.000Z',
      updatedAt: '2026-10-04T00:00:00.000Z'
    }
    const calls = stubFetch((call) => {
      if (call.method === 'POST' && call.url === `${plane}/api/pins`) return json(pin)
      if (call.method === 'DELETE' && call.url === `${plane}/api/pins/p1`) return json(null)
      return undefined
    })

    const created = await answerStudioAgentRequest({
      id: 'r',
      method: 'POST',
      path: '/pins',
      body: { entityType: 'session', entityId: 's1' }
    } as never)
    expect(created?.data).toEqual(pin)

    const removed = await answerStudioAgentRequest({ id: 'r', method: 'DELETE', path: '/pins/p1' } as never)
    expect(removed?.status).toBe(200)
    expect(removed?.data).toBeNull()
    expect(calls.map((call) => `${call.method} ${call.url}`)).toEqual([
      `POST ${plane}/api/pins`,
      `DELETE ${plane}/api/pins/p1`
    ])
    expect(calls[0].body).toEqual({ entityType: 'session', entityId: 's1' })
  })

  it('resolves user workspaces before creating a session', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    installBridge()
    const calls = stubFetch((call) => (call.url === `${plane}/api/agent-sessions` ? json({ id: 's1' }) : undefined))

    const created = await answerStudioAgentRequest({
      id: 'r',
      method: 'POST',
      path: '/agent-sessions',
      body: { agentId: 'a1', name: '', workspace: { type: 'user', workspaceId: 'w1', place: 'project' } }
    } as never)
    expect(created?.data).toEqual({ id: 's1' })
    expect(calls[0].body).toEqual({
      agentId: 'a1',
      name: '',
      workspace: {
        type: 'user',
        place: 'project',
        workspace: { id: 'w1', name: 'proj', path: '/tmp/proj', type: 'user' }
      }
    })
  })

  it('forwards an inbox return so the plane can move the task folder', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    installBridge()
    const calls = stubFetch((call) =>
      call.url === `${plane}/api/agent-sessions/s1/workspace` ? json({ id: 's1' }) : undefined
    )

    const moved = await answerStudioAgentRequest({
      id: 'r',
      method: 'PUT',
      path: '/agent-sessions/s1/workspace',
      body: { type: 'system', place: 'inbox', path: '/tmp/sessions/2026-10-02/s1' }
    } as never)
    expect(moved?.data).toEqual({ id: 's1' })
    expect(calls[0].body).toEqual({ type: 'system', place: 'inbox', path: '/tmp/sessions/2026-10-02/s1' })
  })

  it('routes agent IPC to the plane and leaves other routes alone', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const { original } = installBridge()
    const calls = stubFetch((call) => {
      if (call.url === `${plane}/api/agents`) return json({ id: 'a1' }, 201)
      if (call.url === `${plane}/api/agent-sessions/s1/turns`) return json({ mode: 'started' })
      if (call.url === `${plane}/api/agent-approvals/ap1`) return json({ ok: true })
      if (call.url === `${plane}/api/agent-sessions/s1/fork`) return json({ sessionId: 's2' })
      if (call.url === `${plane}/api/agent-sessions/archive` || call.url === `${plane}/api/agent-sessions/delete`) {
        return json({ deletedIds: ['s1'] })
      }
      return undefined
    })
    installStudioAgentIpc()
    const bridge = globalThis.api!.ipcApi

    await expect(bridge.request('ai.agent.create', { type: 'claude-code', name: 'x' })).resolves.toEqual({
      ok: true,
      data: { id: 'a1' }
    })
    await expect(
      bridge.request('ai.stream.open', { topicId: 'agent-session:s1', trigger: 'submit-message', userMessageParts: [] })
    ).resolves.toEqual({ ok: true, data: { mode: 'started' } })
    await expect(bridge.request('ai.tool.respond_approval', { approvalId: 'ap1', approved: true })).resolves.toEqual({
      ok: true,
      data: { ok: true }
    })
    await expect(
      bridge.request('ai.agent.session.fork', {
        sourceSessionId: 's1',
        messageId: '22222222-2222-4222-8222-222222222222'
      })
    ).resolves.toEqual({ ok: true, data: { sessionId: 's2' } })
    await expect(bridge.request('ai.agent.session.delete', { sessionIds: ['s1'] })).resolves.toEqual({
      ok: true,
      data: { deletedIds: ['s1'] }
    })
    await expect(bridge.request('ai.agent.session.delete_permanently', { sessionIds: ['s1'] })).resolves.toEqual({
      ok: true,
      data: { deletedIds: ['s1'] }
    })
    expect(calls.map((call) => call.body)).toEqual([
      { type: 'claude-code', name: 'x' },
      { topicId: 'agent-session:s1', trigger: 'submit-message', userMessageParts: [] },
      { approved: true },
      { messageId: '22222222-2222-4222-8222-222222222222' },
      { sessionIds: ['s1'] },
      { sessionIds: ['s1'] }
    ])
    expect(calls.map((call) => call.url)).toEqual([
      `${plane}/api/agents`,
      `${plane}/api/agent-sessions/s1/turns`,
      `${plane}/api/agent-approvals/ap1`,
      `${plane}/api/agent-sessions/s1/fork`,
      `${plane}/api/agent-sessions/archive`,
      `${plane}/api/agent-sessions/delete`
    ])

    await expect(bridge.request('app.get_info')).resolves.toEqual({ ok: true, data: 'original' })
    expect(original.request).toHaveBeenCalledWith('app.get_info', undefined)

    const failed = await bridge.request('ai.stream.abort', { topicId: 'agent-session:gone' })
    expect(failed).toMatchObject({ ok: false, error: { code: 'INTERNAL' } })
  })

  it('restores archived sessions through the plane instead of the retired bridge', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const { original } = installBridge()
    const restored = { id: 's1', name: 'Restored report' }
    const calls = stubFetch((call) =>
      call.url === `${plane}/api/agent-sessions/s1/restore` ? json(restored) : undefined
    )
    installStudioAgentIpc()
    await expect(globalThis.api!.ipcApi.request('ai.agent.session.restore', { sessionId: 's1' })).resolves.toEqual({
      ok: true,
      data: restored
    })
    expect(calls.map(({ url, method }) => ({ url, method }))).toEqual([
      { url: `${plane}/api/agent-sessions/s1/restore`, method: 'POST' }
    ])
    expect(original.request).not.toHaveBeenCalled()
  })

  it('honors the permanent flag on the existing session delete contract', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    installBridge()
    const calls = stubFetch((call) =>
      call.url === `${plane}/api/agent-sessions/delete` ? json({ deletedIds: ['s1'] }) : undefined
    )
    installStudioAgentIpc()
    await expect(
      globalThis.api!.ipcApi.request('ai.agent.session.delete', { sessionIds: ['s1'], permanent: true })
    ).resolves.toEqual({ ok: true, data: { deletedIds: ['s1'] } })
    expect(calls[0]?.url).toBe(`${plane}/api/agent-sessions/delete`)
  })

  it('fans plane events out to stream listeners, the status cache, and data-change listeners', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    installBridge()
    stubFetch(() => undefined)
    installStudioAgentIpc()
    const api = globalThis.api!
    const chunks: unknown[] = []
    const effects: unknown[] = []
    api.ipcApi.on('ai.stream.chunk', (payload) => chunks.push(payload))
    api.dataApi.onDataChanged((batch) => effects.push(...batch))
    await vi.waitFor(() => expect(FakeEventSource.last).not.toBeNull())
    expect(FakeEventSource.last!.url).toBe(`${plane}/api/agent-events?token=${token}`)
    FakeEventSource.last!.open()

    FakeEventSource.last!.send({ channel: 'ai.stream.chunk', payload: { topicId: 'agent-session:s1', chunk: {} } })
    FakeEventSource.last!.send({
      channel: 'topic.status',
      payload: { topicId: 'agent-session:s1', status: { status: 'pending', activeExecutions: [] } }
    })
    FakeEventSource.last!.send({ channel: 'data.changed', payload: { sessionId: 's1' } })

    expect(chunks).toEqual([{ topicId: 'agent-session:s1', chunk: {} }])
    expect(cacheService.getShared('topic.stream.statuses.agent-session:s1')).toEqual({
      status: 'pending',
      activeExecutions: []
    })
    expect(effects).toContainEqual({
      endpoint: '/agent-sessions/:sessionId/messages',
      kind: 'membership',
      routeParams: { sessionId: 's1' }
    })
    FakeEventSource.last!.send({ channel: 'data.changed', payload: { taskId: 'task-1' } })
    expect(effects).toContainEqual({ endpoint: '/agent-tasks', kind: 'membership' })
  })

  // Catch late transport packets reopening a retired Run, or a new window inheriting an unqualified shared identity.
  it('withdraws observations on disconnect and requires a fresh snapshot before accepting deltas', async () => {
    const source = await connectEvents()
    const old = observed('old', 2, 1)
    source.send({
      channel: 'topic.status',
      payload: {
        topicId: 'agent-session:s1',
        status: { status: 'streaming', nativeExecution: old }
      }
    })
    expect(cacheService.get(executionCache)).toBeNull()
    source.send({
      channel: 'execution.snapshot',
      payload: {
        protocolVersion: 1,
        topics: [{ topicId: 'agent-session:s1', execution: old }]
      }
    })
    expect(cacheService.get(executionCache)?.['agent-session:s1']).toEqual(old)
    source.send({
      channel: 'topic.status',
      payload: {
        topicId: 'agent-session:s1',
        status: { status: 'streaming', nativeExecution: observed('old', 3) }
      }
    })
    expect(cacheService.get(executionCache)?.['agent-session:s1']?.modelTurn).toBeNull()
    expect(cacheService.getShared('topic.stream.statuses.agent-session:s1')).toEqual({ status: 'streaming' })

    source.disconnect()
    expect(cacheService.get(executionCache)).toBeNull()
    source.send({
      channel: 'execution.snapshot',
      payload: {
        protocolVersion: 1,
        topics: [{ topicId: 'agent-session:s1', execution: old }]
      }
    })
    expect(cacheService.get(executionCache)).toBeNull()
    source.open()
    source.send({
      channel: 'topic.status',
      payload: {
        topicId: 'agent-session:s1',
        status: { nativeExecution: old }
      }
    })
    expect(cacheService.get(executionCache)).toBeNull()
    source.send({ channel: 'execution.snapshot', payload: { protocolVersion: 1, topics: [] } })
    expect(cacheService.get(executionCache)).toEqual({})
    source.send({
      channel: 'topic.status',
      payload: {
        topicId: 'agent-session:s2',
        status: { nativeExecution: observed('new') }
      }
    })
    expect(cacheService.get(executionCache)?.['agent-session:s2']?.runId).toBe('new')
    source.send({ channel: 'topic.status', payload: { topicId: 'agent-session:s2', status: { status: 'done' } } })
    expect(cacheService.get(executionCache)).toEqual({})
  })

  it('restores a live topic status from the execution snapshot', async () => {
    const source = await connectEvents()
    source.send({
      channel: 'execution.snapshot',
      payload: {
        protocolVersion: 1,
        topics: [],
        liveStatuses: [
          {
            topicId: 'agent-session:s1',
            status: {
              status: 'streaming',
              activeExecutions: [{ executionId: 'm' }],
              nativeExecution: observed('held')
            }
          }
        ]
      }
    })
    expect(cacheService.getShared('topic.stream.statuses.agent-session:s1')).toEqual({
      status: 'streaming',
      activeExecutions: [{ executionId: 'm' }]
    })
    expect(cacheService.get(executionCache)).toEqual({})
  })

  // Catch malformed bootstrap selecting an arbitrary owner, and one malformed delta withdrawing a healthy sibling.
  it('refuses incompatible or ambiguous snapshots and contains malformed topic observations', async () => {
    const source = await connectEvents()
    const one = { topicId: 'agent-session:s1', execution: observed('one') }
    const two = { topicId: 'agent-session:s2', execution: observed('two') }
    for (const payload of [
      { protocolVersion: 2, topics: [one] },
      { protocolVersion: 1, topics: [one, one] },
      { protocolVersion: 1, topics: [{ ...one, execution: { ...one.execution, modelTurn: 0 } }] },
      { protocolVersion: 1, topics: [{ ...one, execution: { ...one.execution, principal: 'guest' } }] }
    ]) {
      source.send({ channel: 'execution.snapshot', payload })
      expect(cacheService.get(executionCache)).toBeNull()
    }
    source.send({ channel: 'execution.snapshot', payload: { protocolVersion: 1, topics: [one, two] } })
    source.send({
      channel: 'topic.status',
      payload: {
        topicId: one.topicId,
        status: { nativeExecution: { ...one.execution, sequence: Number.MAX_SAFE_INTEGER + 1 } }
      }
    })
    expect(cacheService.get(executionCache)).toEqual({ [two.topicId]: two.execution })
    source.send({ channel: 'execution.invalidated', payload: { protocolVersion: 1 } })
    expect(cacheService.get(executionCache)).toBeNull()
    source.send({
      channel: 'topic.status',
      payload: { topicId: two.topicId, status: { nativeExecution: two.execution } }
    })
    expect(cacheService.get(executionCache)).toBeNull()
    source.send({ channel: 'execution.snapshot', payload: { protocolVersion: 1, topics: [two] } })
    expect(cacheService.get(executionCache)).toEqual({ [two.topicId]: two.execution })
  })

  // Catch the actual native SSE projection losing identities or keeping settled Runs in renderer memory.
  it.skipIf(!process.env.A3S_CLOUD_TEST_AIX_EXECUTION_SSE_INPUT)(
    'consumes retained actual Core/ACP/native SSE observations through the production renderer ingress',
    async () => {
      const source = await connectEvents()
      const rows = readFileSync(process.env.A3S_CLOUD_TEST_AIX_EXECUTION_SSE_INPUT!, 'utf8').trim().split('\n')
      const runs = new Set<string>()
      let settled = 0
      expect(rows).toHaveLength(16)
      for (const row of rows) {
        const event = JSON.parse(row)
        source.send(event)
        if (event.channel !== 'topic.status') continue
        const { topicId, status } = event.payload
        const active = cacheService.get(executionCache)?.[topicId]
        if (status.nativeExecution) {
          expect(active).toEqual(status.nativeExecution)
          runs.add(active!.runId)
        } else {
          expect(active).toBeUndefined()
          settled++
        }
        expect(cacheService.getShared(`topic.stream.statuses.${topicId}`)).not.toHaveProperty('nativeExecution')
      }
      expect(runs.size).toBe(3)
      expect(settled).toBe(3)
      expect(cacheService.get(executionCache)).toEqual({})
    }
  )

  it('resolves a user workspace and sends scheduled-task commands to the plane', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    installBridge()
    const calls = stubFetch((call) => {
      if (call.method === 'POST' && call.url === `${plane}/api/agent-tasks`) return json({ id: 'task-1' }, 201)
      if (call.method === 'POST' && call.url === `${plane}/api/agent-tasks/task-1/run`) return json(null)
      return undefined
    })
    installStudioAgentIpc()
    const bridge = globalThis.api!.ipcApi

    await expect(
      bridge.request('ai.agent.task.create', {
        agentId: 'a1',
        name: 'Daily',
        prompt: 'Summarize',
        trigger: { kind: 'cron', expr: '0 9 * * *' },
        workspace: { type: 'user', workspaceId: 'w1' },
        timeoutMinutes: 15,
        reuseSession: false
      })
    ).resolves.toEqual({ ok: true, data: { id: 'task-1' } })
    expect(calls[0].body).toMatchObject({
      workspacePath: '/tmp/proj',
      workspaceName: 'proj'
    })

    await expect(bridge.request('ai.agent.task.run', { agentId: 'a1', taskId: 'task-1' })).resolves.toEqual({
      ok: true,
      data: undefined
    })
    expect(calls[1]).toMatchObject({ method: 'POST', url: `${plane}/api/agent-tasks/task-1/run` })

    const listed = await answerStudioAgentRequest({
      id: 'r',
      method: 'GET',
      path: '/agent-tasks',
      params: { page: 1, limit: 500 }
    } as never)
    expect(listed?.error).toMatchObject({ status: 404 })
  })
})
