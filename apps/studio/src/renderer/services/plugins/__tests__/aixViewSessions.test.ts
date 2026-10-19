import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'

import { waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AIX_VIEW_BRIDGE_MESSAGES, AIX_VIEW_STATE_BYTES, aixViewEnvironmentSchema } from '@shared/types/aixViewBridge'
import { AIX_VIEW_CANCELLATION_TIMEOUT_MS, type AixWorkbenchPackage } from '@shared/types/aixWorkbench'

import { AixViewSessions } from '../aixWorkbench'
import { WorkbenchPluginHost } from '../workbenchPluginHost'

const item: AixWorkbenchPackage = {
  slug: 'proof',
  packageId: 'use/proof/views',
  generation: 1,
  revision: `sha256:${'a'.repeat(64)}`,
  containers: [],
  views: [
    { id: 'page', title: 'Page', description: '', point: 'studio.editor', icon: 'package', order: 1, container: null }
  ]
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}

function cancellation(input: globalThis.RequestInfo | globalThis.URL, init?: globalThis.RequestInit) {
  return new globalThis.Response(
    JSON.stringify({
      protocolVersion: 3,
      requestId: String(input).split('/').at(-1),
      activationId: JSON.parse(String(init?.body)).activationId,
      instanceId: JSON.parse(String(init?.body)).instanceId,
      expiresAt: JSON.parse(String(init?.body)).expiresAt,
      closed: true
    })
  )
}

function transport() {
  Object.assign(globalThis, { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:8123', token: 'b'.repeat(64) } })
  let sequence = 0
  const fetcher = vi.fn(async (input: globalThis.RequestInfo | globalThis.URL, init?: globalThis.RequestInit) => {
    const token = (++sequence).toString(16).padStart(64, '0')
    if (init?.method === 'DELETE') return cancellation(input, init)
    const body = JSON.parse(String(init?.body))
    return new globalThis.Response(
      JSON.stringify({
        protocolVersion: 3,
        requestId: body.requestId,
        activationId: body.activationId,
        instanceId: body.instanceId,
        expiresAt: body.expiresAt,
        token,
        path: `/aix-view/${token}/ui/page.html`
      })
    )
  })
  vi.stubGlobal('fetch', fetcher)
  return fetcher
}

async function owner(timeout = 2000, packageItem = item) {
  const host = new WorkbenchPluginHost('member', timeout)
  let sessions!: AixViewSessions
  await host.setPlugins([
    {
      id: 'proof',
      generation: packageItem.generation,
      scopes: ['window'],
      contributions: [],
      activate: (context) => {
        sessions = new AixViewSessions(context, packageItem)
      }
    }
  ])
  const scope = await host.openScope({ id: 'main', kind: 'window' })
  return { host, scope, sessions }
}

const peers: globalThis.MessagePort[] = []
async function connect(session: ReturnType<AixViewSessions['open']>) {
  vi.stubGlobal('globalThis.MessageChannel', NodeMessageChannel)
  const channel = new globalThis.MessageChannel()
  const peer = channel.port2
  peers.push(peer)
  const initial = messageFrom(peer)
  peer.start()
  const accepted = session.connect(session.documentId, channel.port1)
  const message = accepted ? await initial : undefined
  peer.start()
  return { peer, message }
}

function messageFrom(peer: globalThis.MessagePort) {
  return new Promise<unknown>((resolve) =>
    peer.addEventListener('message', ({ data }) => resolve(data), { once: true })
  )
}

function closed(peer: globalThis.MessagePort) {
  return new Promise<void>((resolve) => peer.addEventListener('close', () => resolve(), { once: true }))
}

async function store(peer: globalThis.MessagePort, stateJSON: string | null, id = 1) {
  const acknowledgment = messageFrom(peer)
  peer.postMessage({ protocolVersion: 1, type: 'aix.view.store', id, stateJSON })
  expect(await acknowledgment).toEqual({ protocolVersion: 1, type: 'aix.view.stored', id })
}

afterEach(() => {
  vi.useRealTimers()
  for (const peer of peers.splice(0)) peer.close()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('owned AIX asset sessions', () => {
  it('binds each document and native operation to its exact package activation', async () => {
    transport()
    const { host, sessions } = await owner()
    const first = sessions.open(item, item.views[0])
    const second = sessions.open(item, item.views[0])
    await Promise.all([first.ready, second.ready])

    expect(first.routeKey).not.toBe(second.routeKey)
    expect(first.identity.activation.scopeId).toBe('member/window/main')
    expect(first.identity.contribution).toEqual({
      package: expect.objectContaining({ packageId: item.packageId, generation: item.generation }),
      kind: 'view',
      localId: 'page'
    })
    expect(() => sessions.open({ ...item, generation: item.generation + 1 }, item.views[0])).toThrow(
      'AIX view package pin does not match its activation'
    )

    await host.close()
  })

  // Catch command routing replacing the state receiver or exposing a command in a sibling document.
  it('multiplexes state and declared operations on the same one-shot document port', async () => {
    transport()
    const { host, sessions } = await owner()
    const view = { ...item.views[0], commands: [{ id: 'refresh', title: 'Refresh' }] }
    const left = sessions.open(item, view)
    const right = sessions.open(item, view)
    await Promise.all([left.ready, right.ready])
    vi.stubGlobal('globalThis.MessageChannel', NodeMessageChannel)
    const channels = [new globalThis.MessageChannel(), new globalThis.MessageChannel()]
    for (const [index, session] of [left, right].entries()) {
      const peer = channels[index].port2
      peers.push(peer)
      const messages: unknown[] = []
      peer.onmessage = ({ data }) => {
        messages.push(data)
        if (data.type === 'aix.command.connect') peer.postMessage({ protocolVersion: 1, type: 'aix.command.ready' })
      }
      expect(session.connect(session.documentId, channels[index].port1)).toBe(true)
      await waitFor(() => expect(session.commands?.canExecute('refresh')).toBe(true))
      expect(messages).toEqual([
        { protocolVersion: 1, type: 'aix.view.connect', surface: 'page', stateJSON: null },
        { protocolVersion: 1, type: 'aix.command.connect', surface: 'page', commands: ['refresh'] }
      ])
    }
    await store(channels[0].port2, '{"query":"left"}')
    const request = messageFrom(channels[0].port2)
    const pending = left.commands!.execute('refresh', new globalThis.AbortController().signal)
    expect(await request).toEqual({ protocolVersion: 1, type: 'aix.command.execute', id: 1, command: 'refresh' })
    channels[0].port2.postMessage({ protocolVersion: 1, type: 'aix.command.completed', id: 1, command: 'refresh' })
    await expect(pending).resolves.toBeUndefined()
    await store(channels[1].port2, '{"query":"right"}')
    const disconnected = closed(channels[0].port2)
    await left.close()
    await disconnected
    expect(right.commands!.canExecute('refresh')).toBe(true)
    expect(left.commands!.signal.aborted).toBe(true)
    await host.close()
  })

  // Catch a malformed subprotocol retaining the other executable receiver on the shared port.
  it('withdraws both command and state operations when either document subprotocol is refused', async () => {
    transport()
    const { host, sessions } = await owner()
    vi.stubGlobal('globalThis.MessageChannel', NodeMessageChannel)
    for (const message of [
      { protocolVersion: 1, type: 'aix.command.completed', id: 1, command: 'refresh' },
      { protocolVersion: 1, type: 'aix.view.store', id: 1, stateJSON: '{}', resource: 'readFile' }
    ]) {
      const view = { ...item.views[0], commands: [{ id: 'refresh', title: 'Refresh' }] }
      const session = sessions.open(item, view)
      await session.ready
      const channel = new globalThis.MessageChannel()
      peers.push(channel.port2)
      const disconnected = closed(channel.port2)
      channel.port2.onmessage = ({ data }) => {
        if (data.type === 'aix.command.connect')
          channel.port2.postMessage({ protocolVersion: 1, type: 'aix.command.ready' })
      }
      session.connect(session.documentId, channel.port1)
      await waitFor(() => expect(session.commands!.canExecute('refresh')).toBe(true))
      channel.port2.postMessage(message)
      await disconnected
      expect(session.commands!.signal.aborted).toBe(true)
      await expect(session.commands!.execute('refresh', new globalThis.AbortController().signal)).rejects.toThrow(
        'unavailable'
      )
      await session.close()
    }
    await host.close()
  })

  // Catch a lost cancellation hanging an action forever or falsely reclaiming its native ticket.
  it('bounds cancellation, retains uncertainty and retries the same admission explicitly', async () => {
    const fetcher = transport()
    const { host, sessions } = await owner()
    const session = sessions.open(item, item.views[0])
    await session.ready
    const lostReply = deferred<globalThis.Response>()
    fetcher.mockImplementationOnce(() => lostReply.promise)
    vi.useFakeTimers()
    const failure = expect(session.close()).rejects.toThrow('outcome unknown')
    await vi.advanceTimersByTimeAsync(AIX_VIEW_CANCELLATION_TIMEOUT_MS)
    await failure
    expect(fetcher.mock.calls[1][1]?.signal?.aborted).toBe(true)
    const firstCancellation = String(fetcher.mock.calls[1][0])
    await session.close()
    expect(String(fetcher.mock.calls[2][0])).toBe(firstCancellation)
    lostReply.resolve(cancellation(fetcher.mock.calls[1][0], fetcher.mock.calls[1][1]))
    await expect(host.close()).resolves.toEqual({ failed: [], timedOut: [] })
  })
  // Catch appearance changes losing a frame's UI state or admitting non-UI host observations.
  it('updates bounded appearance and restores state through a fresh document admission', async () => {
    transport()
    const { host, sessions } = await owner()
    const session = sessions.open(item, item.views[0])
    await session.ready
    const environment = {
      theme: 'light',
      locale: 'en-US',
      direction: 'ltr',
      tokens: { '--background': '#ffffff' }
    } as const
    session.setEnvironment(environment)
    const initial = await connect(session)
    expect(initial.message).toEqual({
      protocolVersion: 1,
      type: 'aix.view.connect',
      surface: 'page',
      stateJSON: null,
      environment
    })
    await store(initial.peer, '{"query":"customers"}')
    const updated = {
      ...environment,
      theme: 'dark',
      locale: 'ar',
      direction: 'rtl',
      tokens: { '--background': '#171717' }
    } as const
    const notification = messageFrom(initial.peer)
    session.setEnvironment(updated)
    expect(await notification).toEqual({ protocolVersion: 1, type: 'aix.view.environment', environment: updated })
    session.setEnvironment(globalThis.structuredClone(updated))
    await store(initial.peer, '{"query":"customers"}', 2)
    for (const invalid of [
      { ...environment, theme: 'system' },
      { ...environment, locale: 'en-US\nAuthorization: credential' },
      { ...environment, tokens: { '--workspace-path': '/private/workspace' } },
      { ...environment, tokens: { '--foreground': 'x'.repeat(257) } },
      { ...environment, token: 'credential' }
    ])
      expect(aixViewEnvironmentSchema.safeParse(invalid).success).toBe(false)
    const disconnected = closed(initial.peer)
    await session.close()
    const nextSession = sessions.open(item, item.views[0])
    await nextSession.ready
    nextSession.setEnvironment(updated)
    const reopened = await connect(nextSession)
    expect(nextSession.documentId).not.toBe(session.documentId)
    await disconnected
    expect(reopened.message).toEqual({
      protocolVersion: 1,
      type: 'aix.view.connect',
      surface: 'page',
      stateJSON: '{"query":"customers"}',
      environment: updated
    })
    const revoked = closed(reopened.peer)
    await nextSession.close()
    await revoked
    session.setEnvironment(environment)
    await expect(host.close()).resolves.toEqual({ failed: [], timedOut: [] })
  })
  // Catch navigation losing UI state or a peer restoring state from a different surface, generation or owner.
  it('restores bounded state only for the same package generation and surface in the owning scope', async () => {
    transport()
    const first = await owner()
    const open = async (entry = item, view = entry.views[0]) => {
      const session = first.sessions.open(entry, view)
      await session.ready
      return { session, ...(await connect(session)) }
    }
    const page = await open()
    expect(page.message).toEqual({ protocolVersion: 1, type: 'aix.view.connect', surface: 'page', stateJSON: null })
    await store(page.peer, '{"query":"owned search","expanded":["node-a"]}')
    await page.session.close()
    const reopened = await open()
    expect(reopened.message).toEqual({
      protocolVersion: 1,
      type: 'aix.view.connect',
      surface: 'page',
      stateJSON: '{"query":"owned search","expanded":["node-a"]}'
    })
    const differentSurface = await open(item, { ...item.views[0], id: 'other' })
    expect(differentSurface.message).toEqual(expect.objectContaining({ stateJSON: null }))
    await differentSurface.session.close()

    const otherPackages: AixWorkbenchPackage[] = [
      { ...item, generation: 2, revision: `sha256:${'c'.repeat(64)}` },
      { ...item, packageId: 'use/proof/other' }
    ]
    const otherHosts: Awaited<ReturnType<typeof owner>>[] = []
    for (const entry of otherPackages) {
      const isolatedOwner = await owner(2000, entry)
      otherHosts.push(isolatedOwner)
      const session = isolatedOwner.sessions.open(entry, entry.views[0])
      await session.ready
      const isolated = await connect(session)
      expect(isolated.message).toEqual(expect.objectContaining({ stateJSON: null }))
      await session.close()
    }
    const second = await owner()
    const unrelated = second.sessions.open(item, item.views[0])
    await unrelated.ready
    expect((await connect(unrelated)).message).toEqual(expect.objectContaining({ stateJSON: null }))
    await store(reopened.peer, null)
    await reopened.session.close()
    const cleared = await open()
    expect(cleared.message).toEqual(expect.objectContaining({ stateJSON: null }))
    await Promise.all([first.host.close(), second.host.close(), ...otherHosts.map(({ host }) => host.close())])
  })

  it('withdrawing the presentation activation leaves that activation deck store empty', async () => {
    const presenton: AixWorkbenchPackage = {
      slug: 'a3s-presenton',
      packageId: 'use/a3s/presenton',
      generation: 4,
      revision: `sha256:${'b'.repeat(64)}`,
      containers: [],
      views: [
        {
          id: 'workbench',
          title: '演示文稿',
          description: '',
          point: 'studio.editor',
          icon: 'panels',
          order: 20,
          container: null
        }
      ]
    }
    transport()
    const { host, sessions } = await owner(2000, presenton)
    const states = (sessions as unknown as { states: Map<string, string> }).states
    const session = sessions.open(presenton, presenton.views[0])
    await session.ready
    const channel = await connect(session)
    const deck = JSON.stringify({ route: { pathname: '/outline', search: '?id=' }, deck: { content: '人写锚点' } })
    await store(channel.peer, deck)
    await session.close()
    const reopened = sessions.open(presenton, presenton.views[0])
    await reopened.ready
    const restored = await connect(reopened)
    expect(restored.message).toEqual(expect.objectContaining({ stateJSON: deck }))
    expect(states.size).toBe(1)
    await host.close()
    expect(states.size).toBe(0)
    expect([...states.values()]).toEqual([])
    expect(() => sessions.open(presenton, presenton.views[0])).toThrow('AIX view owner closed')
  })

  // Catch a deck snapshot or tool result revoking the document port. The outline
  // invoke is posted on that same port after the view reports the new deck.
  it('keeps the document port open after a deck snapshot and a deck result', async () => {
    transport()
    const { host, sessions } = await owner()
    const session = sessions.open(item, item.views[0])
    await session.ready
    const channel = await connect(session)
    const revoked = closed(channel.peer)
    channel.peer.postMessage({
      protocolVersion: 1,
      type: 'aix.view.home',
      delivery: 1,
      holdsWork: false,
      applied: true,
      modelRequests: 0
    })
    channel.peer.postMessage({ protocolVersion: 1, type: 'aix.view.deck-snapshot', text: '{"id":"deck"}' })
    channel.peer.postMessage({ protocolVersion: 1, type: 'aix.view.deck-result', result: { status: 'idle' } })
    const winner = await Promise.race([
      store(channel.peer, '{"saved":true}').then(() => 'stored' as const),
      revoked.then(() => 'closed' as const)
    ])
    expect(winner).toBe('stored')
    await session.close()
    await revoked
    await host.close()
  })

  // Catch a stale/replayed port changing state or requesting a host command/native credential.
  it.each(
    [
      { protocolVersion: 2, type: 'aix.view.store', id: 2, stateJSON: 'null' },
      { protocolVersion: 1, type: 'aix.view.execute', id: 2, stateJSON: 'null' },
      { protocolVersion: 1, type: 'aix.view.store', id: 1, stateJSON: 'null' },
      { protocolVersion: 1, type: 'aix.view.store', id: 2, stateJSON: '{}', command: 'readFile' },
      { protocolVersion: 1, type: 'aix.view.store', id: 2, stateJSON: 'invalid JSON' },
      {
        protocolVersion: 1,
        type: 'aix.view.store',
        id: 2,
        stateJSON: '"' + '界'.repeat(AIX_VIEW_STATE_BYTES / 2) + '"'
      },
      { protocolVersion: 1, type: 'aix.view.store', id: 2, stateJSON: '[[[[[[[[[[[[[[[[[[0]]]]]]]]]]]]]]]]]]' }
    ].map((value, index) => [index, value] as const)
  )('closes a refused state port while preserving its last accepted state (case %i)', async (_case, invalid) => {
    transport()
    const { host, sessions } = await owner()
    const session = sessions.open(item, item.views[0])
    await session.ready
    const initial = await connect(session)
    await store(initial.peer, '{"saved":true}')
    const revoked = closed(initial.peer)
    initial.peer.postMessage(invalid)
    await revoked
    expect((await connect(session)).message).toBeUndefined()
    await session.close()
    const nextSession = sessions.open(item, item.views[0])
    await nextSession.ready
    const replacement = await connect(nextSession)
    expect(replacement.message).toEqual(expect.objectContaining({ stateJSON: '{"saved":true}' }))
    await host.close()
  })

  // Catch queued traffic from a replaced frame or an aborted owner being accepted during delayed native cleanup.
  it('revokes replaced and withdrawn ports before waiting for native cancellation acknowledgment', async () => {
    const fetcher = transport()
    const { host, sessions } = await owner()
    const session = sessions.open(item, item.views[0])
    await session.ready
    const original = await connect(session)
    const originalClosed = closed(original.peer)
    await session.close()
    const nextSession = sessions.open(item, item.views[0])
    await nextSession.ready
    const replacement = await connect(nextSession)
    await originalClosed
    const ownerClosed = closed(replacement.peer)
    const ack = deferred<void>()
    fetcher.mockImplementation(async (input, init) => {
      await ack.promise
      return cancellation(input, init)
    })
    const closing = host.close()
    await ownerClosed
    replacement.peer.postMessage({ protocolVersion: 1, type: 'aix.view.store', id: 1, stateJSON: '"late"' })
    expect((await connect(nextSession)).message).toBeUndefined()
    ack.resolve()
    await expect(closing).resolves.toEqual({ failed: [], timedOut: [] })
  })

  // Catch a long-lived frame retaining an unbounded message history or monopolizing its host state port.
  it('bounds the message budget and closes a flooding port', async () => {
    transport()
    const { host, sessions } = await owner()
    const session = sessions.open(item, item.views[0])
    await session.ready
    const channel = await connect(session)
    const revoked = closed(channel.peer)
    for (let id = 1; id <= AIX_VIEW_BRIDGE_MESSAGES + 1; id++) {
      channel.peer.postMessage({ protocolVersion: 1, type: 'aix.view.store', id, stateJSON: String(id) })
    }
    await revoked
    expect((await connect(session)).message).toBeUndefined()
    await session.close()
    const nextSession = sessions.open(item, item.views[0])
    await nextSession.ready
    expect((await connect(nextSession)).message).toEqual(
      expect.objectContaining({ stateJSON: String(AIX_VIEW_BRIDGE_MESSAGES) })
    )
    await host.close()
  })

  // Catch scope success being reported before native session retirement is acknowledged.
  it('withdraws authority immediately but waits for native close acknowledgment', async () => {
    const fetcher = transport()
    const { host, scope, sessions } = await owner()
    await sessions.open(item, item.views[0]).ready
    const admittedClose = deferred<void>()
    const acknowledgment = deferred<void>()
    fetcher.mockImplementation(async (input, init) => {
      if (init?.method !== 'DELETE') throw new Error('Unexpected admission')
      admittedClose.resolve()
      await acknowledgment.promise
      return cancellation(input, init)
    })
    let completed = false
    const close = host.close().then((report) => {
      completed = true
      return report
    })
    await admittedClose.promise
    expect(scope.isOpen()).toBe(false)
    expect(scope.getSnapshot().contributions).toEqual([])
    expect(() => sessions.open(item, item.views[0])).toThrow('owner closed')
    expect(completed).toBe(false)
    acknowledgment.resolve()
    await expect(close).resolves.toEqual({ failed: [], timedOut: [] })
  })

  // Catch closure waiting forever for a lost POST reply or a late reply resurrecting its owner.
  it('cancels pending admission without waiting for its response and refuses the late ticket', async () => {
    const fetcher = transport()
    const reply = deferred<globalThis.Response>()
    fetcher.mockImplementation(async (input, init) =>
      init?.method === 'POST' ? reply.promise : cancellation(input, init)
    )
    const { host, sessions } = await owner()
    const ready = expect(sessions.open(item, item.views[0]).ready).rejects.toThrow('owner closed')
    await expect(host.close()).resolves.toEqual({ failed: [], timedOut: [] })
    const body = JSON.parse(String(fetcher.mock.calls[0][1]?.body))
    reply.resolve(
      new globalThis.Response(
        JSON.stringify({
          protocolVersion: 3,
          requestId: body.requestId,
          activationId: body.activationId,
          instanceId: body.instanceId,
          expiresAt: body.expiresAt,
          token: 'c'.repeat(64),
          path: `/aix-view/${'c'.repeat(64)}/ui/page.html`
        })
      )
    )
    await ready
    expect(
      fetcher.mock.calls.filter(
        ([input, init]) => init?.method === 'DELETE' && String(input).endsWith(`/requests/${body.requestId}`)
      )
    ).toHaveLength(1)
  })

  // Catch rejected or non-settling native cleanup being swallowed as successful scope closure.
  it.each(['refused', 'pending'] as const)('reports %s cleanup without admitting more views', async (mode) => {
    const fetcher = transport()
    const { host, sessions } = await owner(40)
    await sessions.open(item, item.views[0]).ready
    const reply = deferred<void>()
    fetcher.mockImplementation(async (input, init) =>
      mode === 'refused'
        ? new globalThis.Response('{}', { status: 503 })
        : reply.promise.then(() => cancellation(input, init))
    )
    const report = await host.close()
    if (mode === 'refused') expect(report.failed.join(' ')).toContain('AIX asset cleanup failed')
    else expect(report.timedOut).toHaveLength(1)
    expect(() => sessions.open(item, item.views[0])).toThrow('owner closed')
    reply.resolve()
    if (mode === 'refused') await expect(sessions.close()).rejects.toThrow('AIX asset cleanup failed')
    else await expect(sessions.close()).resolves.toBeUndefined()
  })

  // Catch lost admission responses preventing an independently acknowledged native cancellation.
  it('can settle a lost admission reply through its preallocated request identity', async () => {
    const fetcher = transport()
    fetcher.mockImplementation(async (input, init) => {
      if (init?.method === 'POST') throw new Error('Admission response lost')
      return cancellation(input, init)
    })
    const { host, sessions } = await owner()
    await expect(sessions.open(item, item.views[0]).ready).rejects.toThrow('Admission response lost')
    await expect(host.close()).resolves.toEqual({ failed: [], timedOut: [] })
    expect(() => sessions.open(item, item.views[0])).toThrow('owner closed')
    expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1)
  })

  // Catch uncorrelated or malformed ACKs falsely certifying native resource retirement.
  it.each([
    { protocolVersion: 1 },
    { requestId: 'd'.repeat(64) },
    { activationId: 'd'.repeat(64) },
    { instanceId: 'd'.repeat(64) },
    { expiresAt: 1 },
    { closed: false },
    { foreign: true }
  ])('refuses a cancellation receipt changed by %j', async (change) => {
    const fetcher = transport()
    const { host, sessions } = await owner()
    await sessions.open(item, item.views[0]).ready
    fetcher.mockImplementation(
      async (input, init) =>
        new globalThis.Response(JSON.stringify({ ...(await cancellation(input, init).json()), ...change }))
    )
    const report = await host.close()
    expect(report.failed.join(' ')).toContain('AIX asset cleanup failed')
  })

  // Catch a reply for another admission being executed as this owner's frame.
  it('refuses an unrelated admission reply but still cancels its own original request', async () => {
    const fetcher = transport()
    fetcher.mockImplementationOnce(async (_input, init) => {
      const body = JSON.parse(String(init?.body))
      return new globalThis.Response(
        JSON.stringify({
          protocolVersion: 3,
          requestId: 'd'.repeat(64),
          activationId: body.activationId,
          instanceId: body.instanceId,
          expiresAt: body.expiresAt,
          token: 'c'.repeat(64),
          path: `/aix-view/${'c'.repeat(64)}/ui/page.html`
        })
      )
    })
    const { host, sessions } = await owner()
    await expect(sessions.open(item, item.views[0]).ready).rejects.toThrow('Invalid native AIX asset session')
    await expect(host.close()).resolves.toEqual({ failed: [], timedOut: [] })
    const original = JSON.parse(String(fetcher.mock.calls[0][1]?.body))
    expect(
      fetcher.mock.calls.some(
        ([input, init]) => init?.method === 'DELETE' && String(input).endsWith(original.requestId)
      )
    ).toBe(true)
  })

  // Catch mount/unmount navigation accumulating deferred callbacks past the effect budget.
  it('reclaims acknowledged view slots across repeated navigation and closes each ticket once', async () => {
    const fetcher = transport()
    const { host, sessions } = await owner()
    for (let index = 0; index < 260; index++) {
      const session = sessions.open(item, item.views[0])
      await session.ready
      await Promise.all([session.close(), session.close()])
    }
    await expect(host.close()).resolves.toEqual({ failed: [], timedOut: [] })
    expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(260)
  })
})
