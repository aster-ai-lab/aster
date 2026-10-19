import type { Page, Route } from '@playwright/test'

import { installWorkbenchUi, UI_MEMBER_ID, workbenchUiBootstrap, type WorkbenchUiInstall } from './uiFixture'

const STAMP = '2026-10-07T00:00:00.000Z'
const OFFICE_REVISION = `sha256:${'ab'.repeat(32)}`

/** Home cards the writing harness reads. A missing pin leaves that shelf empty. */
const OFFICE_HOME_CATALOGUE = {
  protocolVersion: 1,
  revision: '',
  packages: [
    {
      slug: 'home-writing',
      packageId: 'use/a3s/home-writing',
      generation: 1,
      revision: OFFICE_REVISION,
      containers: [],
      views: [],
      home: [
        {
          id: 'notice',
          shelf: 'write',
          title: '通知',
          description: '发布需要周知或执行的事项。',
          order: 10,
          action: 'writing',
          writingType: 'notice',
          seed: '发布需要周知或执行的事项。',
          tag: '通知'
        },
        {
          id: 'report',
          shelf: 'write',
          title: '报告',
          description: '向上级汇报工作进展和需要说明的情况。',
          order: 20,
          action: 'writing',
          writingType: 'report',
          seed: '向上级汇报工作进展和需要说明的情况。',
          tag: '报告'
        },
        {
          id: 'request',
          shelf: 'write',
          title: '请示',
          description: '向上级请求指示或批准',
          order: 30,
          action: 'writing',
          writingType: 'request',
          seed: '就下列事项请求指示或批准。',
          tag: '请示'
        },
        {
          id: 'minutes',
          shelf: 'write',
          title: '纪要',
          description: '记载会议的主要情况和议定事项。',
          order: 40,
          action: 'writing',
          writingType: 'minutes',
          seed: '记载会议的主要情况和议定事项。',
          tag: '纪要'
        }
      ]
    },
    {
      slug: 'home-create',
      packageId: 'use/a3s/home-create',
      generation: 1,
      revision: OFFICE_REVISION,
      containers: [],
      views: [],
      home: [
        {
          id: 'ppt',
          shelf: 'create',
          title: '幻灯片',
          description: '',
          order: 10,
          action: 'unavailable',
          cover: 'ppt'
        },
        {
          id: 'site',
          shelf: 'create',
          title: '网站',
          description: '',
          order: 20,
          action: 'unavailable',
          cover: 'site'
        },
        {
          id: 'video',
          shelf: 'create',
          title: '视频',
          description: '',
          order: 30,
          action: 'unavailable',
          cover: 'video'
        },
        {
          id: 'image',
          shelf: 'create',
          title: '图片',
          description: '',
          order: 40,
          action: 'applet',
          applet: 'plan',
          cover: 'image'
        }
      ]
    },
    {
      slug: 'home-process',
      packageId: 'use/a3s/home-process',
      generation: 1,
      revision: OFFICE_REVISION,
      containers: [],
      views: [],
      home: [
        {
          id: 'pdf-to-word',
          shelf: 'process',
          title: 'PDF 转 Word',
          description: '把 PDF 转成可编辑的 Word 文档',
          order: 10,
          action: 'file',
          fileKind: 'pdf-to-word',
          format: 'pdf'
        },
        {
          id: 'word-to-pdf',
          shelf: 'process',
          title: 'Word 转 PDF',
          description: '把 Word 转成 PDF',
          order: 20,
          action: 'file',
          fileKind: 'word-to-pdf',
          format: 'word'
        }
      ]
    }
  ]
}

/** Member, catalogue, and pins the writing shelf waits for. The ui harness installs its own. */
async function installOfficeHomeShelf(page: Page) {
  const json = (route: Route, body: unknown) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) })
  const answer = async (route: Route, pattern: RegExp, body: unknown) => {
    const path = new URL(route.request().url()).pathname
    if (route.request().method().toUpperCase() !== 'GET' || !pattern.test(path)) return route.fallback()
    await json(route, body)
  }
  await page.route(/\/api\/member\/me(?:\?|$)/, (route) =>
    answer(route, /^\/api\/member\/me$/, { id: UI_MEMBER_ID, display_name: '成员', phone: '1' })
  )
  await page.route(/\/api\/member-session\/profile(?:\?|$)/, (route) =>
    answer(route, /^\/api\/member-session\/profile$/, { id: UI_MEMBER_ID, display_name: '成员', phone: '1' })
  )
  await page.route(/\/api\/member-orgs(?:\?|$)/, (route) => answer(route, /^\/api\/member-orgs$/, []))
  await page.route(/\/api\/member-center(?:\?|$)/, async (route) => {
    await route.fulfill({ status: 204, body: '' })
  })
  await page.route(/\/api\/aix\/views(?:\?|$)/, (route) => answer(route, /^\/api\/aix\/views$/, OFFICE_HOME_CATALOGUE))
}

/** Gateway unique id. The signed-in provider plane only resolves `a3s::<id>`. */
const GATEWAY_MODEL_ID = 'a3s::gpt-work'

export const workAgent = {
  id: 'agent-work',
  type: 'a3s-code',
  name: 'ToWork',
  model: GATEWAY_MODEL_ID,
  createdAt: STAMP,
  updatedAt: STAMP,
  orderKey: 'a0'
}

export const QUOTE = '甲方应在三日内付款。'

export interface OfficeSession {
  id: string
  name: string
  messages: OfficeMessage[]
}

export interface OfficeMessage {
  id: string
  role: 'user' | 'assistant'
  text: string
  parts?: readonly Record<string, unknown>[]
  status?: 'success' | 'pending'
}

export interface WorkbenchInstall {
  preferences?: Record<string, unknown>
  /** When set, session create can bind to this agent and leave the home page. */
  withAgent?: boolean
  sessions?: OfficeSession[]
  /** `record` resolves window commands. `reject-deliver` makes a Word handoff fail. */
  invoke?: 'record' | 'reject-deliver'
  /** Remember a user-information allow for the review applet. */
  grantReviewUser?: boolean
  /** Answer `/api/local-skills` with the review skill. Requires the studio runtime. */
  publishReview?: boolean
  /** Messages for the session created from the home page. */
  createdMessages?: OfficeMessage[]
  /**
   * Task conversation can send follow-ups. The stub answers the agent model
   * and records each turn instead of calling a live model.
   */
  collab?: boolean
  /**
   * Keep the topic stream live after open until abort. Emits tool+prose parts
   * and topic.status=streaming via the agent-events bridge.
   */
  heldStream?: boolean
  /** Stub the shell file API so a handoff can write a workspace docx. */
  workspaceFiles?: boolean
  /** Home shelves to pin. The writing card is the default. */
  homePins?: readonly string[]
  /**
   * Deterministic work-home reads: member, catalogue, sessions, projects, and
   * scheduled tasks. Does not abort unknown `/api` calls.
   */
  ui?: WorkbenchUiInstall
}

/** Fake EventSource so tests can push `topic.status` without a live plane. */
export async function installAgentEventsBridge(page: Page) {
  await page.addInitScript(() => {
    type Client = { onmessage: ((message: { data: string }) => void) | null; onopen: ((event: Event) => void) | null }
    const clients: Client[] = []
    class FakeEventSource {
      static CONNECTING = 0
      static OPEN = 1
      static CLOSED = 2
      readyState = 1
      onmessage: Client['onmessage'] = null
      onopen: Client['onopen'] = null
      onerror: ((event: Event) => void) | null = null
      constructor(_url: string) {
        clients.push(this)
        queueMicrotask(() => this.onopen?.({ type: 'open' } as Event))
      }
      close() {
        this.readyState = 2
      }
    }
    ;(globalThis as { EventSource: typeof FakeEventSource }).EventSource = FakeEventSource
    ;(globalThis as { __E2E_SSE_PUSH__?: (channel: string, payload: unknown) => void }).__E2E_SSE_PUSH__ = (
      channel,
      payload
    ) => {
      const data = JSON.stringify({ channel, payload })
      for (const client of clients) client.onmessage?.({ data })
    }
  })
}

function sessionRecord(session: OfficeSession) {
  const workspace = {
    id: 'workspace-alpha',
    name: 'Alpha',
    path: '/tmp/a3s-workbench-office',
    type: 'user',
    orderKey: 'a0',
    createdAt: STAMP,
    updatedAt: STAMP
  }
  return {
    id: session.id,
    agentId: workAgent.id,
    name: session.name,
    isNameManuallyEdited: true,
    workspaceId: workspace.id,
    workspace,
    orderKey: 'a0',
    lastActivityAt: STAMP,
    createdAt: STAMP,
    updatedAt: STAMP
  }
}

function messageRecord(sessionId: string, message: OfficeMessage) {
  return {
    id: message.id,
    sessionId,
    role: message.role,
    data: { parts: message.parts ?? [{ type: 'text', text: message.text }] },
    status: message.status ?? 'success',
    modelId: null,
    messageSnapshot: null,
    stats: null,
    searchableText: message.text,
    runtimeResumeToken: null,
    createdAt: STAMP,
    updatedAt: STAMP
  }
}

export const createdSession: OfficeSession = {
  id: 'session-writing',
  name: '公文任务',
  messages: []
}

/** Seed language, stub the data bridge, and optionally the review skill catalog. */
export async function installWorkbench(page: Page, options: WorkbenchInstall = {}) {
  const sessions = (options.sessions ?? []).map(sessionRecord)
  const messages = (options.sessions ?? []).flatMap((session) =>
    session.messages.map((message) => messageRecord(session.id, message))
  )
  const createdMessages = (options.createdMessages ?? []).map((message) => messageRecord(createdSession.id, message))
  const created = sessionRecord(createdSession)

  await page.addInitScript(
    (fixture) => {
      const defaults = {
        'app.onboarding.provider_setup.status': 'skipped',
        'app.privacy.policy_version': '20260820',
        'app.language': 'zh-CN',
        ...(fixture.workspaceFiles ? { 'chat.default_model_id': 'a3s::gpt-work' } : {}),
        'ui.sidebar_shortcut': [
          {
            type: 'shortcut',
            id: 'sidebar-shortcut:core.app:agents',
            target: { kind: 'resource', locator: { providerId: 'core.app', resourceId: 'agents' } }
          }
        ]
      }
      const host = globalThis as typeof globalThis & {
        __A3S_PREFERENCE_DEFAULTS__?: Record<string, unknown>
        __A3S_STUDIO_RUNTIME__?: { apiBaseUrl: string; token: string }
        __E2E_IPC__?: { route: string; input: unknown }[]
        __E2E_HTTP__?: { method: string; url: string; body: string }[]
        __E2E_HOLD_COMPOSER__?: boolean
        __E2E_HELD_COMPOSER__?: Node[]
        __E2E_INVOKE__?: string[]
        __E2E_SESSION_MESSAGES__?: unknown[]
        __E2E_DATA_LISTENERS__?: Array<(effects: unknown) => void>
        __E2E_STREAM_ONS__?: Record<string, Array<(payload: unknown) => void>>
        __TAURI_INTERNALS__?: {
          transformCallback?: (callback: () => void) => number
          unregisterCallback?: (id: number) => void
          invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
          metadata?: { currentWindow?: { label?: string } }
        }
      }
      host.__A3S_PREFERENCE_DEFAULTS__ = defaults
      host.__E2E_IPC__ = []
      host.__E2E_HTTP__ = []
      host.__E2E_INVOKE__ = []
      host.__E2E_SESSION_MESSAGES__ = fixture.createdMessages
      host.__E2E_DATA_LISTENERS__ = []
      host.__E2E_STREAM_ONS__ = {}
      if (fixture.grantReviewUser) {
        localStorage.setItem('a3s.applet.permission-grant.v1', JSON.stringify({ review: 'user' }))
      }
      if (fixture.publishReview || fixture.workspaceFiles || !fixture.ui) {
        host.__A3S_STUDIO_RUNTIME__ = host.__A3S_STUDIO_RUNTIME__ ?? {
          apiBaseUrl: 'http://127.0.0.1:4318',
          token: 'ab'.repeat(32)
        }
      }
      if (!fixture.ui) {
        try {
          localStorage.setItem(`workbench.home.plugin-pins:v1:${fixture.memberId}`, JSON.stringify(fixture.homePins))
        } catch {
          // The writing shelf assertions report a missing pin.
        }
      }
      if (fixture.ui) {
        host.__A3S_STUDIO_RUNTIME__ = fixture.ui.runtime
        const preferenceOverrides = {
          ...fixture.preferences,
          'app.onboarding.provider_setup.status': 'skipped',
          'app.privacy.policy_version': '20260820',
          'app.language': 'zh-CN',
          'ui.sidebar_shortcut': [
            {
              type: 'shortcut',
              id: 'sidebar-shortcut:core.app:agents',
              target: { kind: 'resource', locator: { providerId: 'core.app', resourceId: 'agents' } }
            }
          ]
        }
        let preferenceDefaults: Record<string, unknown> = { ...preferenceOverrides }
        Object.defineProperty(globalThis, '__A3S_PREFERENCE_DEFAULTS__', {
          configurable: true,
          enumerable: true,
          get: () => preferenceDefaults,
          set: (value: unknown) => {
            preferenceDefaults = {
              ...(value && typeof value === 'object' ? (value as Record<string, unknown>) : {}),
              ...preferenceOverrides
            }
          }
        })
        try {
          localStorage.setItem('a3s.settings.preferences', JSON.stringify({ 'app.language': 'zh-CN' }))
          localStorage.setItem('a3s.workbench.projects', JSON.stringify(fixture.ui.projects))
          localStorage.setItem(`workbench.home.plugin-pins:v1:${fixture.ui.memberId}`, JSON.stringify(['home-writing']))
        } catch {
          // The home assertions report a missing pin or project.
        }
        if (fixture.ui.holdComposer) {
          const held: Node[] = []
          host.__E2E_HELD_COMPOSER__ = held
          host.__E2E_HOLD_COMPOSER__ = true
          const append = Element.prototype.appendChild
          const insert = Element.prototype.insertBefore
          const remove = Element.prototype.removeChild
          const holdChild = (parent: Element, child: Node) => {
            if (!host.__E2E_HOLD_COMPOSER__ || parent.getAttribute('data-ui') !== 'workbench.composer') return false
            if (!held.includes(child)) held.push(child)
            return true
          }
          Element.prototype.appendChild = function <T extends Node>(this: Element, child: T): T {
            if (holdChild(this, child)) return child
            append.call(this, child)
            return child
          }
          Element.prototype.insertBefore = function <T extends Node>(this: Element, child: T, ref: Node | null): T {
            if (holdChild(this, child)) return child
            insert.call(this, child, ref)
            return child
          }
          Element.prototype.removeChild = function <T extends Node>(this: Element, child: T): T {
            if (host.__E2E_HOLD_COMPOSER__ && this.getAttribute('data-ui') === 'workbench.composer') {
              const index = held.indexOf(child)
              if (index >= 0) held.splice(index, 1)
              return child
            }
            remove.call(this, child)
            return child
          }
        }
      }
      if (fixture.invoke) {
        let preferenceRevision = 0
        const nativePreferences = { ...fixture.preferences }
        host.__TAURI_INTERNALS__ = {
          transformCallback: () => 1,
          unregisterCallback: () => {},
          metadata: { currentWindow: { label: 'applet-review-e2e' } },
          invoke: (command: string, args?: Record<string, unknown>) => {
            host.__E2E_INVOKE__?.push(command)
            if (command === 'preference_commit') {
              const input = args?.input as { updates?: Record<string, unknown> } | undefined
              Object.assign(nativePreferences, input?.updates)
              preferenceRevision += 1
            }
            if (command === 'preference_read' || command === 'preference_commit') {
              return Promise.resolve({ scope: 'e2e-member', revision: preferenceRevision, values: nativePreferences })
            }
            if (fixture.invoke === 'reject-deliver' && command === 'deliver_workbench_review') {
              return Promise.reject(new Error('unavailable'))
            }
            return Promise.resolve(null)
          }
        }
      }

      const answer = (request: { id?: string; method?: string; path?: string }) => {
        const method = String(request?.method || 'GET').toUpperCase()
        const path = String(request?.path || '').split('?')[0]
        if (method !== 'GET') return null
        const envelope = (data: unknown) => ({
          id: request?.id,
          status: 200,
          data,
          metadata: { timestamp: Date.now() }
        })
        if (path === '/agents') {
          return envelope(
            fixture.withAgent ? { items: [fixture.agent], total: 1, page: 1 } : { items: [], total: 0, page: 1 }
          )
        }
        if (fixture.collab && path === `/agents/${fixture.agent.id}`) return envelope(fixture.agent)
        if (fixture.collab && path.startsWith('/models/')) return envelope(fixture.model)
        if (path === '/agent-sessions') return envelope({ items: fixture.sessions })
        if (path === '/agent-sessions/latest') {
          return envelope({ session: fixture.sessions[0] ?? null })
        }
        if (path === `/agent-sessions/${fixture.created.id}`) return envelope(fixture.created)
        const listed = fixture.sessions.find((session) => path === `/agent-sessions/${session.id}`)
        if (listed) return envelope(listed)
        if (path === `/agent-sessions/${fixture.created.id}/messages`) {
          return envelope({ items: [...(host.__E2E_SESSION_MESSAGES__ ?? [])], nextCursor: null })
        }
        const owner = fixture.sessions.find((session) => path === `/agent-sessions/${session.id}/messages`)
        if (owner) return envelope({ items: fixture.messages.filter((item) => item.sessionId === owner.id) })
        if (path === '/pins') return envelope([])
        return null
      }

      const nativeFetch = globalThis.fetch.bind(globalThis)
      globalThis.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url
        const method = String(
          init?.method || (typeof input === 'object' && input && 'method' in input ? input.method : 'GET')
        ).toUpperCase()
        if (method === 'POST' && url.includes('/api/agent-sessions/reuse-or-create')) {
          host.__E2E_IPC__?.push({ route: 'ai.agent.session.reuse_or_create', input: null })
        } else if (method === 'POST' && /\/api\/agent-sessions\/[^/]+\/turns(?:\?|$)/.test(url)) {
          host.__E2E_IPC__?.push({ route: 'ai.stream.open', input: null })
        }
        if (fixture.ui) {
          host.__E2E_HTTP__?.push({
            method,
            url,
            body: typeof init?.body === 'string' ? init.body : ''
          })
        }
        return nativeFetch(input, init)
      }

      const wrap = (api: {
        dataApi?: {
          request?: (request: unknown) => Promise<unknown>
          onDataChanged?: (callback: (effects: unknown) => void) => () => void
        }
        ipcApi?: {
          request?: (route: string, input?: unknown) => Promise<unknown>
          on?: (event: string, callback: (payload: unknown) => void) => () => void
        }
        __e2eWrapped?: boolean
      }) => {
        if (!api || api.__e2eWrapped) return api
        if (api.dataApi?.request) {
          const request = api.dataApi.request.bind(api.dataApi)
          api.dataApi.request = (input: unknown) => {
            const hit = answer(input as { id?: string; method?: string; path?: string })
            return hit ? Promise.resolve(hit) : request(input)
          }
          const previousData = api.dataApi.onDataChanged?.bind(api.dataApi)
          api.dataApi.onDataChanged = (callback) => {
            host.__E2E_DATA_LISTENERS__?.push(callback)
            return previousData ? previousData(callback) : () => {}
          }
        }
        if (api.ipcApi?.request) {
          const request = api.ipcApi.request.bind(api.ipcApi)
          const previousOn = api.ipcApi.on?.bind(api.ipcApi)
          api.ipcApi.on = (event, callback) => {
            const ons = host.__E2E_STREAM_ONS__ ?? {}
            const list = ons[event] ?? []
            list.push(callback)
            ons[event] = list
            host.__E2E_STREAM_ONS__ = ons
            const detach = previousOn ? previousOn(event, callback) : () => {}
            return () => {
              const index = list.indexOf(callback)
              if (index >= 0) list.splice(index, 1)
              detach()
            }
          }
          api.ipcApi.request = (route: string, input?: unknown) => {
            host.__E2E_IPC__?.push({ route, input })
            if (route === 'ai.agent.session.reuse_or_create') {
              return Promise.resolve({
                ok: true,
                data: {
                  session: fixture.created,
                  created: true,
                  deletedDuplicateSessionIds: []
                }
              })
            }
            if ((fixture.collab || fixture.workspaceFiles) && route === 'file.get_metadata') {
              return Promise.resolve({
                ok: true,
                data: { kind: 'directory', size: 0, createdAt: 0, modifiedAt: 0 }
              })
            }
            if (fixture.collab && route === 'ai.stream.attach') {
              return Promise.resolve({ ok: true, data: { status: 'not-found' } })
            }
            if (fixture.collab && route === 'ai.stream.abort') {
              const held = (host as { __E2E_HELD__?: { topicId?: string; assistantId?: string } }).__E2E_HELD__
              if (held?.topicId) {
                const push = (globalThis as { __E2E_SSE_PUSH__?: (channel: string, payload: unknown) => void })
                  .__E2E_SSE_PUSH__
                push?.('topic.status', {
                  topicId: held.topicId,
                  status: { status: 'aborted', activeExecutions: [], lastCompletedAt: new Date().toISOString() }
                })
                const done = host.__E2E_STREAM_ONS__?.['ai.stream.done'] ?? []
                for (const callback of done) {
                  callback({ topicId: held.topicId, isTopicDone: true, status: 'paused' })
                }
                ;(host as { __E2E_HELD__?: unknown }).__E2E_HELD__ = undefined
              }
              return Promise.resolve({ ok: true, data: undefined })
            }
            if ((fixture.collab || fixture.workspaceFiles) && route === 'ai.stream.open') {
              const topicId =
                input && typeof input === 'object' && typeof (input as { topicId?: unknown }).topicId === 'string'
                  ? (input as { topicId: string }).topicId
                  : undefined
              if (fixture.heldStream && topicId) {
                const assistantId = `held-assistant-${Date.now()}`
                ;(host as { __E2E_HELD__?: { topicId: string; assistantId: string } }).__E2E_HELD__ = {
                  topicId,
                  assistantId
                }
                window.setTimeout(() => {
                  const push = (globalThis as { __E2E_SSE_PUSH__?: (channel: string, payload: unknown) => void })
                    .__E2E_SSE_PUSH__
                  push?.('topic.status', {
                    topicId,
                    status: {
                      status: 'streaming',
                      activeExecutions: [
                        { executionId: 'held-exec', attemptId: 1, anchorMessageId: assistantId }
                      ]
                    }
                  })
                  const rows = host.__E2E_SESSION_MESSAGES__ ?? []
                  const stamp = new Date().toISOString()
                  const sessionId = topicId.startsWith('agent-session:')
                    ? topicId.slice('agent-session:'.length)
                    : 'session-writing'
                  rows.unshift({
                    id: assistantId,
                    sessionId,
                    role: 'assistant',
                    data: {
                      parts: [
                        {
                          type: 'tool-bash',
                          toolCallId: 'held-done',
                          state: 'output-available',
                          input: { command: 'pwd' },
                          output: '/tmp/a3s-workbench-office'
                        },
                        { type: 'text', text: '环境就绪，开始构建工作簿。' }
                      ]
                    },
                    status: 'success',
                    modelId: null,
                    messageSnapshot: null,
                    stats: null,
                    searchableText: '环境就绪，开始构建工作簿。',
                    runtimeResumeToken: null,
                    createdAt: stamp,
                    updatedAt: stamp
                  })
                  host.__E2E_SESSION_MESSAGES__ = rows
                  for (const listener of host.__E2E_DATA_LISTENERS__ ?? []) {
                    listener([{ endpoint: '/agent-sessions/:sessionId/messages', kind: 'membership' }])
                  }
                }, 30)
                return Promise.resolve({
                  ok: true,
                  data: {
                    mode: 'started',
                    reservedMessages: [
                      {
                        id: assistantId,
                        role: 'assistant',
                        parts: [],
                        metadata: { status: 'pending', createdAt: new Date().toISOString() }
                      }
                    ]
                  }
                })
              }
              window.setTimeout(() => {
                const done = host.__E2E_STREAM_ONS__?.['ai.stream.done'] ?? []
                for (const callback of done) {
                  callback({ topicId, isTopicDone: true, status: 'success' })
                }
              }, 30)
              return Promise.resolve({ ok: true, data: { mode: 'started' } })
            }
            return request(route, input)
          }
        }
        api.__e2eWrapped = true
        return api
      }

      let current: unknown
      Object.defineProperty(globalThis, 'api', {
        configurable: true,
        enumerable: true,
        get() {
          return current
        },
        set(value) {
          current = wrap(value)
        }
      })
    },
    {
      agent: workAgent,
      preferences: options.preferences ?? {},
      model: {
        id: GATEWAY_MODEL_ID,
        providerId: 'a3s',
        modelId: 'gpt-work',
        name: '工作模型',
        isEnabled: true
      },
      sessions,
      messages,
      created,
      withAgent: options.withAgent === true,
      collab: options.collab === true,
      heldStream: options.heldStream === true,
      invoke: options.invoke ?? null,
      grantReviewUser: options.grantReviewUser === true,
      publishReview: options.publishReview === true,
      workspaceFiles: options.workspaceFiles === true,
      createdMessages,
      ui: options.ui ? workbenchUiBootstrap(options.ui) : null,
      memberId: UI_MEMBER_ID,
      homePins: options.homePins ?? ['home-writing']
    }
  )

  if (options.ui) await installWorkbenchUi(page, options.ui, workAgent)

  if (options.publishReview || options.workspaceFiles) {
    const liveMessages: unknown[] = [...createdMessages]
    let sessionCreated = false
    await page.exposeBinding('__e2eNoteSessionMessages', async (_source, rows: unknown) => {
      if (!Array.isArray(rows)) return
      liveMessages.splice(0, liveMessages.length, ...rows)
    })
    await page.route('**/api/**', async (route) => {
      const url = new URL(route.request().url())
      let path = url.pathname
      try {
        path = decodeURIComponent(path)
      } catch {
        // Keep the raw pathname when it is not valid percent-encoding.
      }
      const method = route.request().method().toUpperCase()
      const json = (body: unknown) =>
        route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) })
      const pageBody = (items: unknown[]) => json({ items, nextCursor: null, total: items.length })
      if (path === '/api/agent-plane' && method === 'GET') return json({ baseUrl: 'http://127.0.0.1:4318' })
      if (path === '/api/os-gateway/credential' && method === 'GET') {
        return json({ origin: 'http://127.0.0.1:4318', token: 'session-token' })
      }
      if (path === '/v1/api/org/models' && method === 'GET') {
        return json({
          success: true,
          data: {
            items: [
              {
                id: 'gpt-work',
                name: '工作模型',
                protocols: ['openai'],
                capabilities: ['tool_call'],
                available: true
              },
              {
                id: 'auto',
                name: '自动',
                protocols: ['openai'],
                capabilities: ['tool_call'],
                available: true
              }
            ],
            defaultModel: 'gpt-work',
            autoModel: 'gpt-work'
          }
        })
      }
      if (path === '/api/agent-events') return route.fulfill({ status: 204, body: '' })
      if (path === '/api/agent-sessions/reuse-or-create' && method === 'POST') {
        sessionCreated = true
        return json({ session: created, created: true, deletedDuplicateSessionIds: [] })
      }
      const turnPath = /^\/api\/agent-sessions\/([^/]+)\/turns$/.exec(path)
      if (turnPath && method === 'POST') return json({ mode: 'started' })
      if (/^\/api\/agent-sessions\/[^/]+\/turn$/u.test(path)) return json({ status: 'not-found' })
      if (path === '/api/local-skills' && options.publishReview) {
        return json({
          skills: [{ name: 'review', description: '', source: 'installed' }],
          roots: [{ path: '/tmp/a3s-review-skills', source: 'installed' }]
        })
      }
      if (path === '/api/agents') return pageBody(options.withAgent || options.publishReview ? [workAgent] : [])
      if (path === `/api/agents/${workAgent.id}`) return json(workAgent)
      const listedSessions = sessionCreated ? [created, ...sessions] : sessions
      if (path === '/api/agent-sessions') return pageBody(listedSessions)
      if (path === '/api/agent-sessions/latest') {
        return json({ session: listedSessions[0] ?? null, topic: null })
      }
      if (path === '/api/pins' || path.startsWith('/api/pins/')) return json([])
      if (path === '/api/agent-tasks' || /\/tasks(\/|$)/.test(path)) return pageBody([])
      const messagesPath = /^\/api\/agent-sessions\/([^/]+)\/messages$/.exec(path)
      if (messagesPath) {
        const sessionId = messagesPath[1]
        if (sessionId === createdSession.id) return pageBody(liveMessages)
        return pageBody(messages.filter((item) => item.sessionId === sessionId))
      }
      if (path === `/api/agent-sessions/${createdSession.id}`) return json(created)
      const session = sessions.find((item) => path === `/api/agent-sessions/${item.id}`)
      if (session) return json(session)
      // An empty object becomes an infinite-query page and throws while spreading `.items`.
      if (path.startsWith('/api/agent') || path.startsWith('/api/agents')) return pageBody([])
      return options.ui ? route.fallback() : route.abort()
    })
  }

  if (options.workspaceFiles) await installWorkspaceFiles(page)

  // Registered last so it wins over the `/api` handler. The catalog fetch is
  // `GET {apiBaseUrl}/v1/api/org/models`, which that glob can miss.
  if (options.publishReview || options.workspaceFiles) {
    await page.route('**/v1/api/org/models', async (route) => {
      if (route.request().method().toUpperCase() !== 'GET') {
        await route.fallback()
        return
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          success: true,
          data: {
            items: [
              {
                id: 'gpt-work',
                name: '工作模型',
                protocols: ['openai'],
                capabilities: ['tool_call'],
                available: true
              },
              {
                id: 'auto',
                name: '自动',
                protocols: ['openai'],
                capabilities: ['tool_call'],
                available: true
              }
            ],
            defaultModel: 'gpt-work',
            autoModel: 'gpt-work'
          }
        })
      })
    })
  }

  // After the broad `/api` stub, so the writing shelf is not aborted with the unknown paths.
  if (!options.ui) await installOfficeHomeShelf(page)
}

const WORKSPACE_FILE_ETAG = `"${'a'.repeat(64)}"`

/** Accept one workspace docx write and serve those bytes back to the editor. */
async function installWorkspaceFiles(page: Page) {
  const files = new Map<string, Buffer>()
  const taken = new Set<string>()
  await page.route('**/api/local-documents/entries**', async (route) => {
    if (route.request().method() !== 'POST') {
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ entries: [] }) })
      return
    }
    const body = route.request().postDataJSON() as { root?: string; name?: string }
    const root = String(body.root ?? '').replace(/\/+$/u, '')
    const name = String(body.name ?? '')
    const path = `${root}/${name}`
    if (taken.has(path)) {
      await route.fulfill({ status: 409, contentType: 'application/json', body: '{}' })
      return
    }
    taken.add(path)
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ name, path, kind: 'file', sizeBytes: 1, modifiedMs: 0 })
    })
  })
  await page.route('**/api/local-documents/changes**', async (route) => {
    await route.fulfill({
      status: 200,
      headers: { 'content-type': 'text/event-stream' },
      body: `data: ${JSON.stringify({ revision: 1, paths: [], fullScan: false })}\n\n`
    })
  })
  await page.route('**/api/workspace-file/**', async (route) => {
    const url = new URL(route.request().url())
    const path = url.searchParams.get('path') ?? ''
    if (route.request().method() === 'PUT') {
      files.set(path, route.request().postDataBuffer() ?? Buffer.alloc(0))
      await route.fulfill({ status: 204 })
      return
    }
    const file = files.get(path)
    if (!file) {
      await route.fulfill({ status: 404 })
      return
    }
    await route.fulfill({
      status: 200,
      headers: {
        etag: WORKSPACE_FILE_ETAG,
        'content-type': 'application/octet-stream',
        'content-length': String(file.byteLength)
      },
      body: file
    })
  })
}

/** The handoff opens this workspace editor tab. The live paper types beside the chat. */
export async function expectWorkspaceEditor(page: Page, name: string) {
  const { expect } = await import('@playwright/test')
  await expect(page.locator('[data-workspace-tab="editor"]').filter({ hasText: name })).toBeVisible({
    timeout: 15_000
  })
  await expect(page.locator('[data-workspace-file-editor]')).toBeVisible()
}

export async function openHome(page: Page) {
  await page.goto('/windows/main/index.html', { waitUntil: 'domcontentloaded' })
  await page.locator('[data-feature="write"]').scrollIntoViewIfNeeded()
}

export async function composerText(page: Page): Promise<string> {
  return page.evaluate(() => {
    const composer = document.querySelector('[data-ui="workbench.composer"] .composer-tiptap')
    if (composer) return (composer.textContent ?? '').trim()
    const areas = [...document.querySelectorAll('textarea')].map((node) => (node as HTMLTextAreaElement).value)
    const edits = [...document.querySelectorAll('[contenteditable="true"]')].map((node) => node.textContent ?? '')
    return areas.concat(edits).join('\n')
  })
}

export async function ipcCount(page: Page, route: string): Promise<number> {
  return page.evaluate((name) => {
    const host = globalThis as typeof globalThis & { __E2E_IPC__?: { route: string }[] }
    return (host.__E2E_IPC__ ?? []).filter((call) => call.route === name).length
  }, route)
}
