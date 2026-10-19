import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import process from 'node:process'
import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { JSDOM } from 'jsdom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.unmock('@a3s-lab/ui')
vi.unmock('@data/hooks/useCache')

import { cacheService } from '@renderer/data/CacheService'
import { installStudioAgentIpc, resetStudioAgentPlaneForTests } from '@renderer/data/studioAgentPlane'
import { WorkbenchPluginHost } from '@renderer/services/plugins'
import {
  AIX_VIEW_CANCELLATION_TIMEOUT_MS,
  aixWorkbenchCatalogueSchema,
  type AixWorkbenchCatalogue
} from '@shared/types/aixWorkbench'

import { createAixWorkbenchPlugins } from '../AixWorkbenchRuntime'
import { PluginComposerActions } from '../PluginComposerActions'
import { PluginScopeProvider } from '../PluginScopeProvider'
import { PluginSessionRuntime } from '../PluginSessionRuntime'

const catalogue: AixWorkbenchCatalogue = {
  protocolVersion: 1,
  revision: `sha256:${'a'.repeat(64)}`,
  packages: [
    {
      slug: 'composer',
      packageId: 'use/proof/composer',
      generation: 1,
      revision: `sha256:${'b'.repeat(64)}`,
      containers: [],
      views: [
        {
          id: 'normalize',
          title: 'Normalize',
          description: '',
          icon: 'package',
          order: 1,
          container: null,
          point: 'studio.composer.actions'
        }
      ]
    }
  ]
}
const hosts: WorkbenchPluginHost[] = []
const documents: JSDOM[] = []
const peers: globalThis.MessagePort[] = []
const rendererApi = globalThis.api

beforeEach(() => vi.stubGlobal('api', rendererApi))

async function setup(native = catalogue) {
  Object.assign(globalThis, { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:8123', token: 'c'.repeat(64) } })
  let sequence = 0
  const fetcher = vi.fn(async (input: globalThis.RequestInfo | globalThis.URL, init?: globalThis.RequestInit) => {
    if (String(input).endsWith('/api/agent-plane'))
      return new globalThis.Response(JSON.stringify({ baseUrl: 'http://127.0.0.1:8123' }))
    const body = JSON.parse(String(init?.body))
    if (init?.method === 'DELETE')
      return new globalThis.Response(
        JSON.stringify({
          protocolVersion: 3,
          requestId: String(input).split('/').at(-1),
          activationId: body.activationId,
          instanceId: body.instanceId,
          expiresAt: body.expiresAt,
          closed: true
        })
      )
    const token = (++sequence).toString(16).padStart(64, '0')
    return new globalThis.Response(
      JSON.stringify({
        protocolVersion: 3,
        requestId: body.requestId,
        activationId: body.activationId,
        instanceId: body.instanceId,
        expiresAt: body.expiresAt,
        token,
        path: `/aix-view/${token}/index.html`
      })
    )
  })
  vi.stubGlobal('fetch', fetcher)
  const host = new WorkbenchPluginHost('composer-member')
  hosts.push(host)
  await host.setPlugins(createAixWorkbenchPlugins(native))
  const scope = await host.openScope({ id: 'main', kind: 'window' })
  const edits: string[] = []
  const content = (disabled = false) => (
    <PluginScopeProvider scope={scope}>
      <PluginComposerActions
        disabled={disabled}
        readDraft={() => ({ id: 'private/session/message', revision: 9, text: '  Hello\n world  ' })}
        applyEdit={(_expected, replacement) => edits.push(replacement)}
      />
    </PluginScopeProvider>
  )
  const surface = render(content())
  return { host, scope, fetcher, edits, surface, disable: () => surface.rerender(content(true)) }
}

async function startPackage(frame: globalThis.HTMLIFrameElement) {
  const messages: unknown[] = []
  const script = readFileSync(resolve('tests/fixtures/aix/composer-normalize/provider.js'), 'utf8')
  const entry = readFileSync(resolve('tests/fixtures/aix/composer-normalize/index.html'), 'utf8')
  const provider = new JSDOM(
    entry.replace('<script src="provider.js"></script>', () => `<script>${script}</script>`),
    {
      url: frame.src,
      runScripts: 'dangerously',
      beforeParse(window) {
        Object.defineProperty(window, 'MessageChannel', {
          value: function () {
            const channel = new NodeMessageChannel()
            peers.push(channel.port1 as unknown as globalThis.MessagePort)
            channel.port1.addEventListener('message', (event) => messages.push((event as globalThis.MessageEvent).data))
            return channel
          }
        })
        Object.defineProperty(window, 'parent', {
          value: {
            postMessage(data: unknown, _origin: string, ports: globalThis.MessagePort[]) {
              globalThis.window.dispatchEvent(
                new globalThis.MessageEvent('message', { data, source: frame.contentWindow, origin: 'null', ports })
              )
            }
          }
        })
      }
    }
  )
  documents.push(provider)
  fireEvent.load(frame)
  return messages
}

async function connect(frame: globalThis.HTMLIFrameElement, surface = 'normalize') {
  const channel = new NodeMessageChannel()
  const peer = channel.port2 as unknown as globalThis.MessagePort
  peers.push(peer)
  const message = new Promise<unknown>((resolve) =>
    peer.addEventListener('message', ({ data }) => resolve(data), { once: true })
  )
  peer.start()
  globalThis.window.dispatchEvent(
    new globalThis.MessageEvent('message', {
      data: {
        protocolVersion: 1,
        type: 'aix.document.ready',
        documentId: new globalThis.URLSearchParams(new globalThis.URL(frame.src).hash.slice(1)).get('aix-document')
      },
      source: frame.contentWindow,
      origin: 'null',
      ports: [channel.port1 as unknown as globalThis.MessagePort]
    })
  )
  expect(await message).toEqual({ protocolVersion: 1, type: 'aix.composer.connect', surface })
  const request = new Promise<unknown>((resolve) =>
    peer.addEventListener('message', ({ data }) => resolve(data), { once: true })
  )
  peer.postMessage({ protocolVersion: 1, type: 'aix.composer.ready' })
  await request
  return peer
}

afterEach(async () => {
  cleanup()
  cacheService.set('studio.execution.observations', null)
  vi.useRealTimers()
  await Promise.all(hosts.splice(0).map((host) => host.close()))
  for (const document of documents.splice(0)) document.window.close()
  for (const peer of peers.splice(0)) peer.close()
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('external AIX composer actions', () => {
  it('blocks AIX workspace contributions when the required host service is unavailable', async () => {
    const path = process.env.A3S_STUDIO_TEST_AIX_MISSING_SERVICE_CATALOGUE
    if (!path) throw new Error('A3S_STUDIO_TEST_AIX_MISSING_SERVICE_CATALOGUE is required for this proof')
    const native = aixWorkbenchCatalogueSchema.parse(JSON.parse(readFileSync(path, 'utf8')))
    const host = new WorkbenchPluginHost('missing-workspace-service')
    hosts.push(host)
    await host.setPlugins(createAixWorkbenchPlugins(native))
    const window = await host.openScope({ id: 'main', kind: 'window' })
    const workspace = await host.openScope({ id: 'project', kind: 'workspace', parent: window })
    const snapshot = workspace.getSnapshot()
    expect(snapshot.diagnostics).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ point: 'studio.workspace', reason: 'service-unavailable', missing: ['workspace'] })
      ])
    )
    expect(snapshot.contributions.some((contribution) => contribution.point === 'studio.workspace')).toBe(false)
  })

  // Catch a native rejection accidentally becoming a renderer-visible plugin contribution.
  it('does not admit contributions from the native catalogue after an invalid archive is rejected', () => {
    const path = process.env.A3S_STUDIO_TEST_AIX_NEGATIVE_CATALOGUE
    if (!path) throw new Error('A3S_STUDIO_TEST_AIX_NEGATIVE_CATALOGUE is required for this proof')
    const native = aixWorkbenchCatalogueSchema.parse(JSON.parse(readFileSync(path, 'utf8')))
    expect(createAixWorkbenchPlugins(native)).toEqual([])
  })

  // Catch an ancestor-owned invocation surviving a now-false condition and applying output to a current draft.
  it('evaluates inherited conditions at the consumer and cancels hidden actions without closing their ancestor', async () => {
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].views[0].when = { all: ['conversation'], none: ['run'] }
    const p = await setup(aixWorkbenchCatalogueSchema.parse(native))
    p.surface.rerender(
      <PluginScopeProvider scope={p.scope}>
        <PluginSessionRuntime sessionId="conditional-session">
          <PluginComposerActions
            readDraft={() => ({ id: 'draft', revision: 9, text: '  Hello world  ' })}
            applyEdit={(_expected, replacement) => p.edits.push(replacement)}
          />
        </PluginSessionRuntime>
      </PluginScopeProvider>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Normalize' }))
    const frame = (await screen.findByTitle('Normalize')) as globalThis.HTMLIFrameElement
    const peer = await connect(frame)
    await act(async () => {
      cacheService.set('studio.execution.observations', {
        'agent-session:conditional-session': {
          protocolVersion: 1,
          coreSessionId: 'core',
          runId: 'run',
          promptId: 'prompt',
          sequence: 1,
          modelTurn: null
        }
      })
    })
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Normalize' })).not.toBeInTheDocument())
    await waitFor(() => expect(screen.queryByTitle('Normalize')).not.toBeInTheDocument())
    expect(p.scope.isOpen()).toBe(true)
    peer.postMessage({ protocolVersion: 1, type: 'aix.composer.edit', id: 1, revision: 9, text: 'stale' })
    await act(async () => {
      cacheService.set('studio.execution.observations', null)
    })
    fireEvent.click(await screen.findByRole('button', { name: 'Normalize' }))
    const next = (await screen.findByTitle('Normalize')) as globalThis.HTMLIFrameElement
    await act(async () => {
      await startPackage(next)
    })
    await waitFor(() => expect(p.edits).toEqual(['Hello world']))
  })

  // Catch a retired Run's delayed edit crossing into the next Run of the same conversation.
  it('cancels the old packaged Run invocation before admitting its replacement and rejects late output', async () => {
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].views[0] = {
      ...native.packages[0].views[0],
      id: 'run-normalize',
      title: 'Run Normalize',
      scope: 'run'
    }
    const p = await setup(aixWorkbenchCatalogueSchema.parse(native))
    p.surface.rerender(
      <PluginScopeProvider scope={p.scope}>
        <PluginSessionRuntime sessionId="session-alpha">
          <PluginComposerActions
            readDraft={() => ({ id: 'draft', revision: 9, text: '  Hello\n world  ' })}
            applyEdit={(_expected, replacement) => p.edits.push(replacement)}
          />
        </PluginSessionRuntime>
      </PluginScopeProvider>
    )
    const observation = {
      protocolVersion: 1 as const,
      promptId: 'first',
      coreSessionId: 'core',
      runId: 'old',
      sequence: 1,
      modelTurn: null
    }
    await act(async () =>
      cacheService.set('studio.execution.observations', { 'agent-session:session-alpha': observation })
    )
    await userEvent.setup().click(await screen.findByRole('button', { name: 'Run Normalize' }))
    const frame = (await screen.findByTitle('Run Normalize')) as globalThis.HTMLIFrameElement
    const peer = await connect(frame, 'run-normalize')
    await act(async () =>
      cacheService.set('studio.execution.observations', {
        'agent-session:session-alpha': { ...observation, promptId: 'next', runId: 'new' }
      })
    )
    await waitFor(() => expect(screen.queryByTitle('Run Normalize')).not.toBeInTheDocument())
    peer.postMessage({ protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 9, text: 'old Run' })
    await act(async () => {
      await new Promise((resolve) => globalThis.setTimeout(resolve, 10))
    })
    expect(p.edits).toEqual([])
    await userEvent.setup().click(await screen.findByRole('button', { name: 'Run Normalize' }))
    const current = (await screen.findByTitle('Run Normalize')) as globalThis.HTMLIFrameElement
    await act(async () => {
      await startPackage(current)
    })
    await waitFor(() => expect(p.edits).toEqual(['Hello world']))
    expect(p.scope.isOpen()).toBe(true)
  })

  // Catch the real native catalogue or execution wire failing to control actual UI contributions.
  it.skipIf(
    !process.env.A3S_CLOUD_TEST_AIX_EXECUTION_SSE_INPUT ||
      !process.env.A3S_CLOUD_TEST_AIX_EXECUTION_COMPOSER_CATALOGUE_INPUT
  )(
    'uses the admitted native package catalogue and actual Core SSE to activate and withdraw Run/Turn providers',
    async () => {
      class Source {
        static current: Source
        onopen?: () => void
        onerror?: () => void
        onmessage?: (event: { data: string }) => void
        constructor() {
          Source.current = this
        }
        close() {}
        send(event: unknown) {
          this.onmessage?.({ data: JSON.stringify(event) })
        }
      }
      vi.stubGlobal('EventSource', Source)
      const native = aixWorkbenchCatalogueSchema.parse(
        JSON.parse(readFileSync(process.env.A3S_CLOUD_TEST_AIX_EXECUTION_COMPOSER_CATALOGUE_INPUT!, 'utf8'))
      )
      const rows = readFileSync(process.env.A3S_CLOUD_TEST_AIX_EXECUTION_SSE_INPUT!, 'utf8')
        .trim()
        .split('\n')
        .map((line) => JSON.parse(line))
      const topicId = rows.find((event) => event.channel === 'topic.status').payload.topicId as string
      const p = await setup(native)
      p.surface.rerender(
        <PluginScopeProvider scope={p.scope}>
          <PluginSessionRuntime sessionId={topicId.slice('agent-session:'.length)}>
            <input aria-label="Draft" defaultValue="Local draft" />
            <PluginComposerActions
              readDraft={() => ({ id: 'private', revision: 9, text: '  Hello\n world  ' })}
              applyEdit={(_expected, replacement) => p.edits.push(replacement)}
            />
          </PluginSessionRuntime>
        </PluginScopeProvider>
      )
      await screen.findByRole('button', { name: 'Session Normalize' })
      const draft = screen.getByRole('textbox', { name: 'Draft' })
      const bridge = globalThis.api!.ipcApi
      const original = { on: bridge.on, request: bridge.request }
      try {
        installStudioAgentIpc()
        bridge.on('ai.stream.chunk', () => {})
        await waitFor(() => expect(Source.current).toBeDefined())
        Source.current.onopen?.()
        let invoked = false
        for (const event of rows) {
          await act(async () => Source.current.send(event))
          if (event.channel !== 'topic.status') continue
          const observation = event.payload.topicId === topicId ? event.payload.status.nativeExecution : undefined
          if (observation) {
            await screen.findByRole('button', { name: 'Run Normalize' })
            if (observation.modelTurn != null) await screen.findByRole('button', { name: 'Turn Normalize' })
            else
              await waitFor(() =>
                expect(screen.queryByRole('button', { name: 'Turn Normalize' })).not.toBeInTheDocument()
              )
            if (!invoked) {
              await userEvent.setup().click(screen.getByRole('button', { name: 'Run Normalize' }))
              const frame = (await screen.findByTitle('Run Normalize')) as globalThis.HTMLIFrameElement
              let messages!: unknown[]
              await act(async () => {
                messages = await startPackage(frame)
              })
              await waitFor(() => expect(p.edits).toEqual(['Hello world']))
              expect(messages).toEqual([
                { protocolVersion: 1, type: 'aix.composer.connect', surface: 'run-normalize' },
                { protocolVersion: 1, type: 'aix.composer.request', id: 1, revision: 9, text: '  Hello\n world  ' }
              ])
              invoked = true
            }
          } else {
            await waitFor(() => expect(screen.queryByRole('button', { name: 'Run Normalize' })).not.toBeInTheDocument())
            expect(screen.queryByRole('button', { name: 'Turn Normalize' })).not.toBeInTheDocument()
          }
          expect(screen.getByRole('textbox', { name: 'Draft' })).toBe(draft)
        }
        expect(invoked).toBe(true)
        expect(screen.getByRole('button', { name: 'Session Normalize' })).toBeInTheDocument()
      } finally {
        resetStudioAgentPlaneForTests()
        bridge.on = original.on
        bridge.request = original.request
      }
    }
  )
  // Catch model-turn changes restarting a Run, replacing the draft DOM, or leaving temporal actions after withdrawal.
  it('binds packaged Run and Turn actions to native observations while preserving session actions and draft state', async () => {
    const native = globalThis.structuredClone(catalogue)
    for (const scope of ['session', 'run', 'turn'] as const)
      native.packages[0].views.push({
        ...native.packages[0].views[0],
        id: `${scope}-normalize`,
        title: `${scope[0].toUpperCase()}${scope.slice(1)} Normalize`,
        scope
      })
    const p = await setup(aixWorkbenchCatalogueSchema.parse(native))
    const content = (
      <PluginScopeProvider scope={p.scope}>
        <PluginSessionRuntime sessionId="session-alpha" workspace={{ id: 'workspace-alpha', root: '/alpha' }}>
          <input aria-label="Draft" defaultValue="Local draft" />
          <PluginComposerActions
            readDraft={() => ({ id: 'private/draft', revision: 9, text: '  Hello\n world  ' })}
            applyEdit={(_expected, replacement) => p.edits.push(replacement)}
          />
        </PluginSessionRuntime>
      </PluginScopeProvider>
    )
    p.surface.rerender(content)
    await screen.findByRole('button', { name: 'Session Normalize' })
    const draft = screen.getByRole('textbox', { name: 'Draft' })
    await userEvent.setup().type(draft, ' edited')
    expect(screen.queryByRole('button', { name: 'Run Normalize' })).not.toBeInTheDocument()
    const observation = {
      protocolVersion: 1 as const,
      promptId: 'prompt',
      coreSessionId: 'core',
      runId: 'run-one',
      sequence: 1,
      modelTurn: null as number | null
    }
    const publish = async (execution: typeof observation | null) => {
      await act(async () =>
        cacheService.set(
          'studio.execution.observations',
          execution ? { 'agent-session:session-alpha': execution } : null
        )
      )
    }
    await publish(observation)
    await screen.findByRole('button', { name: 'Run Normalize' })
    expect(screen.queryByRole('button', { name: 'Turn Normalize' })).not.toBeInTheDocument()
    await userEvent.setup().click(screen.getByRole('button', { name: 'Run Normalize' }))
    const runFrame = (await screen.findByTitle('Run Normalize')) as globalThis.HTMLIFrameElement
    await publish({ ...observation, sequence: 2, modelTurn: 1 })
    await screen.findByRole('button', { name: 'Turn Normalize' })
    expect(screen.getByTitle('Run Normalize')).toBe(runFrame)
    await act(async () => {
      await startPackage(runFrame)
    })
    await waitFor(() => expect(p.edits).toEqual(['Hello world']))
    await userEvent.setup().click(screen.getByRole('button', { name: 'Turn Normalize' }))
    const turnFrame = (await screen.findByTitle('Turn Normalize')) as globalThis.HTMLIFrameElement
    const peer = await connect(turnFrame, 'turn-normalize')
    await publish({ ...observation, sequence: 3, modelTurn: null })
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Turn Normalize' })).not.toBeInTheDocument())
    expect(screen.getByRole('button', { name: 'Run Normalize' })).toBeInTheDocument()
    await publish({ ...observation, sequence: 4, modelTurn: 2 })
    await screen.findByRole('button', { name: 'Turn Normalize' })
    peer.postMessage({ protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 9, text: 'retired turn' })
    await act(async () => {
      await new Promise((resolve) => globalThis.setTimeout(resolve, 10))
    })
    expect(p.edits).toEqual(['Hello world'])
    await userEvent.setup().click(screen.getByRole('button', { name: 'Turn Normalize' }))
    const currentTurn = (await screen.findByTitle('Turn Normalize')) as globalThis.HTMLIFrameElement
    await act(async () => {
      await startPackage(currentTurn)
    })
    await waitFor(() => expect(p.edits).toEqual(['Hello world', 'Hello world']))
    await publish(null)
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Run Normalize' })).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: 'Turn Normalize' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Session Normalize' })).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'Draft' })).toBe(draft)
    expect(draft).toHaveValue('Local draft edited')
    expect(p.scope.isOpen()).toBe(true)
  })
  // Catch a session declaration being exposed globally, hiding inherited actions or leaking host identity to the guest.
  it('composes a packaged session action with its nearest inherited action and runs the actual provider', async () => {
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].views.push({
      ...native.packages[0].views[0],
      id: 'session-normalize',
      title: 'Session Normalize',
      scope: 'session'
    })
    const p = await setup(aixWorkbenchCatalogueSchema.parse(native))
    expect(screen.queryByRole('button', { name: 'Session Normalize' })).not.toBeInTheDocument()
    p.surface.rerender(
      <PluginScopeProvider scope={p.scope}>
        <PluginSessionRuntime sessionId="session-alpha" workspace={{ id: 'workspace-alpha', root: '/alpha' }}>
          <PluginComposerActions
            readDraft={() => ({ id: 'private/session-alpha/message', revision: 9, text: '  Hello\n world  ' })}
            applyEdit={(_expected, replacement) => p.edits.push(replacement)}
          />
        </PluginSessionRuntime>
      </PluginScopeProvider>
    )
    const action = await screen.findByRole('button', { name: 'Session Normalize' })
    expect(screen.getAllByRole('button', { name: 'Normalize' })).toHaveLength(1)
    await userEvent.setup().click(action)
    const frame = (await screen.findByTitle('Session Normalize')) as globalThis.HTMLIFrameElement
    let messages!: unknown[]
    await act(async () => {
      messages = await startPackage(frame)
    })
    await waitFor(() => expect(p.edits).toEqual(['Hello world']))
    expect(messages).toEqual([
      { protocolVersion: 1, type: 'aix.composer.connect', surface: 'session-normalize' },
      { protocolVersion: 1, type: 'aix.composer.request', id: 1, revision: 9, text: '  Hello\n world  ' }
    ])
    expect(p.scope.isOpen()).toBe(true)
  })

  // Catch delayed output from either a local or inherited action crossing a session/workspace ownership change.
  it.each([
    ['session', 'session-normalize', 'Session Normalize'],
    ['session', 'normalize', 'Normalize'],
    ['workspace', 'session-normalize', 'Session Normalize'],
    ['workspace', 'normalize', 'Normalize']
  ])('retires %s-owned consumption of %s before delayed output can reach the next draft', async (change, id, title) => {
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].views[0].scope = 'window'
    native.packages[0].views.push({
      ...native.packages[0].views[0],
      id: 'session-normalize',
      title: 'Session Normalize',
      scope: 'session'
    })
    const p = await setup(aixWorkbenchCatalogueSchema.parse(native))
    const content = (sessionId: string, workspaceId: string) => (
      <PluginScopeProvider scope={p.scope}>
        <PluginSessionRuntime sessionId={sessionId} workspace={{ id: workspaceId, root: `/${workspaceId}` }}>
          <PluginComposerActions
            readDraft={() => ({ id: sessionId, revision: 9, text: '  Hello\n world  ' })}
            applyEdit={(_expected, replacement) => p.edits.push(replacement)}
          />
        </PluginSessionRuntime>
      </PluginScopeProvider>
    )
    p.surface.rerender(content('session-alpha', 'workspace-alpha'))
    await screen.findByRole('button', { name: 'Session Normalize' })
    await userEvent.setup().click(screen.getByRole('button', { name: title }))
    const frame = (await screen.findByTitle(title)) as globalThis.HTMLIFrameElement
    const peer = await connect(frame, id)
    const closed = new Promise<void>((resolve) => peer.addEventListener('close', () => resolve(), { once: true }))
    p.surface.rerender(
      content(
        change === 'session' ? 'session-beta' : 'session-alpha',
        change === 'workspace' ? 'workspace-beta' : 'workspace-alpha'
      )
    )
    await closed
    expect(frame.isConnected).toBe(false)
    peer.postMessage({ protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 9, text: 'old session' })
    await userEvent.setup().click(await screen.findByRole('button', { name: title }))
    await act(async () => {
      await startPackage((await screen.findByTitle(title)) as globalThis.HTMLIFrameElement)
    })
    await waitFor(() => expect(p.edits).toEqual(['Hello world']))
    expect(p.fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(2)
    expect(p.scope.isOpen()).toBe(true)
  })

  it('refuses an unsupported owner and a scoped declaration on another contribution point', () => {
    for (const [point, scope] of [
      ['studio.composer.actions', 'subtask'],
      ['studio.composer.actions', ''],
      ['studio.editor', 'session'],
      ['studio.workspace', 'workspace']
    ]) {
      const value = globalThis.structuredClone(catalogue)
      Object.assign(value.packages[0].views[0], { point, scope })
      expect(aixWorkbenchCatalogueSchema.safeParse(value).success).toBe(false)
    }
  })

  // Catch eager draft disclosure, incorrect channel selection, or a fake editor page replacing the action.
  it('opens only on user action and runs the packaged provider through a private channel', async () => {
    const p = await setup()
    expect(p.fetcher.mock.calls).toHaveLength(0)
    expect(screen.queryByTitle('Normalize')).not.toBeInTheDocument()
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Normalize' }))
    const frame = (await screen.findByTitle('Normalize')) as globalThis.HTMLIFrameElement
    let messages!: unknown[]
    await act(async () => {
      messages = await startPackage(frame)
    })
    await waitFor(() => expect(p.edits).toEqual(['Hello world']))
    expect(messages).toEqual([
      { protocolVersion: 1, type: 'aix.composer.connect', surface: 'normalize' },
      { protocolVersion: 1, type: 'aix.composer.request', id: 1, revision: 9, text: '  Hello\n world  ' }
    ])
    expect(screen.queryByTitle('Normalize')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Normalize' })).toBeEnabled()
    expect(p.fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1)
  })

  // Catch a removed composer or upgraded package retaining its document and applying delayed output.
  it.each(['unmounted', 'upgraded', 'disabled'] as const)(
    'withdraws a %s action before native acknowledgment',
    async (mode) => {
      const p = await setup()
      await userEvent.setup().click(screen.getByRole('button', { name: 'Normalize' }))
      const frame = (await screen.findByTitle('Normalize')) as globalThis.HTMLIFrameElement
      const peer = await connect(frame)
      const closed = new Promise<void>((resolve) => peer.addEventListener('close', () => resolve(), { once: true }))
      let acknowledge!: () => void
      const pending = new Promise<void>((resolve) => {
        acknowledge = resolve
      })
      p.fetcher.mockImplementationOnce(async (input, init) => {
        await pending
        return new globalThis.Response(
          JSON.stringify({
            protocolVersion: 2,
            requestId: String(input).split('/').at(-1),
            expiresAt: JSON.parse(String(init?.body)).expiresAt,
            closed: true
          })
        )
      })
      let upgrading: Promise<void> | undefined
      await act(async () => {
        if (mode === 'unmounted') p.surface.unmount()
        else if (mode === 'disabled') p.disable()
        else
          upgrading = p.host.setPlugins(
            createAixWorkbenchPlugins({
              ...catalogue,
              packages: [{ ...catalogue.packages[0], generation: 2, revision: `sha256:${'d'.repeat(64)}` }]
            })
          )
      })
      await closed
      expect(frame.isConnected).toBe(false)
      peer.postMessage({ protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 9, text: 'late' })
      await act(async () => {
        acknowledge()
        await upgrading
      })
      expect(p.edits).toEqual([])
      await expect(p.host.close()).resolves.toEqual({ failed: [], timedOut: [] })
    }
  )

  // Catch a successful guest reply hanging the action forever on lost native cleanup.
  it('bounds uncertain retirement and refuses the edit rather than claiming cleanup success', async () => {
    const p = await setup()
    await userEvent.setup().click(screen.getByRole('button', { name: 'Normalize' }))
    const frame = (await screen.findByTitle('Normalize')) as globalThis.HTMLIFrameElement
    const peer = await connect(frame)
    p.fetcher.mockImplementationOnce(() => new Promise<globalThis.Response>(() => {}))
    vi.useFakeTimers()
    await act(async () => {
      peer.postMessage({ protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 9, text: 'Hello world' })
    })
    // MessageChannel delivery is native, so wait for the actual DELETE before advancing its deadline.
    await vi.waitFor(() => expect(p.fetcher.mock.calls.some(([, init]) => init?.method === 'DELETE')).toBe(true))
    await act(async () => {
      await vi.advanceTimersByTimeAsync(AIX_VIEW_CANCELLATION_TIMEOUT_MS)
    })
    expect(p.edits).toEqual([])
    expect(screen.queryByTitle('Normalize')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Normalize' })).toBeEnabled()
    vi.useRealTimers()
    await expect(p.host.close()).resolves.toEqual({ failed: [], timedOut: [] })
  })
})
