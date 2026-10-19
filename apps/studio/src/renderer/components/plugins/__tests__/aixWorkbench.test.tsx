import { readFileSync } from 'node:fs'
import { basename, dirname, resolve } from 'node:path'
import process from 'node:process'
import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'

import { Button } from '@a3s-lab/ui'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import i18n from 'i18next'
import { JSDOM } from 'jsdom'
import { useState } from 'react'

vi.unmock('@a3s-lab/ui')
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TabsProvider } from '@renderer/components/layout/TabsProvider'
import { useTabs } from '@renderer/hooks/tab'
import type { WorkbenchPluginScope } from '@renderer/services/plugins'
import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import type { WorkbenchPlugin } from '@renderer/types/workbenchPlugin'
import { AIX_DOCUMENT_ATTEMPT_LIMIT } from '@shared/types/aixViewBridge'
import {
  aixWorkbenchCatalogueSchema,
  aixViewCancellationSchema,
  aixViewSessionSchema,
  type AixWorkbenchCatalogue
} from '@shared/types/aixWorkbench'

import { publishMemberProfile } from '../../account/memberProfile'
import { createAixWorkbenchPlugins, AixWorkbenchRuntime } from '../AixWorkbenchRuntime'
import { usePluginContributionContext } from '../PluginContributionContext'
import { PluginMiniAppItems, usePluginMiniApps } from '../PluginMiniAppItems'
import { PluginPage } from '../PluginPage'
import { usePluginContributions, usePluginScope, usePluginScopeSnapshot } from '../PluginScopeProvider'
import { createPluginSidebarProviders, PLUGIN_SIDEBAR_PROVIDER_ID } from '../pluginSidebarProviders'
import { PluginWindowRuntime } from '../PluginWindowRuntime'
import {
  PluginWorkbenchLayoutProvider,
  PluginWorkbenchPanels,
  PluginWorkbenchSidebar,
  usePluginContainerSelection
} from '../PluginWorkbenchLayout'

const revision = `sha256:${'a'.repeat(64)}`
const contextCataloguePath = process.env.A3S_CLOUD_TEST_AIX_CONTEXT_CATALOGUE_INPUT
const contextCatalogue = contextCataloguePath
  ? aixWorkbenchCatalogueSchema.parse(JSON.parse(readFileSync(contextCataloguePath, 'utf8')))
  : undefined
const catalogue: AixWorkbenchCatalogue = {
  protocolVersion: 1,
  revision,
  packages: [
    {
      slug: 'proof-views',
      packageId: 'use/proof/views',
      generation: 1,
      revision,
      containers: [
        { id: 'resources', title: 'Resources', icon: 'database', order: 1, location: 'activitybar' },
        { id: 'output', title: 'Output', icon: 'terminal', order: 2, location: 'panel' }
      ],
      views: [
        {
          id: 'connections',
          title: 'Connections',
          icon: 'package',
          order: 1,
          container: 'resources',
          point: 'studio.sidebar',
          description: ''
        },
        {
          id: 'preview',
          title: 'Preview',
          icon: 'package',
          order: 2,
          container: 'resources',
          point: 'studio.sidebar',
          description: ''
        },
        {
          id: 'output',
          title: 'Output',
          icon: 'package',
          order: 1,
          container: 'output',
          point: 'studio.panel',
          description: ''
        }
      ]
    }
  ]
}

function Chrome({ openWorkspace }: { openWorkspace: () => void }) {
  const activities = usePluginContributions('activity')
  const snapshot = usePluginScopeSnapshot()
  const containers = usePluginContainerSelection()
  const visibility = usePluginContributionContext(usePluginScope())
  const provider = createPluginSidebarProviders(
    [],
    [],
    () => snapshot,
    (key) => key,
    containers,
    () => visibility
  ).find((item) => item.id === PLUGIN_SIDEBAR_PROVIDER_ID)!
  return (
    <>
      {activities.map((item) => (
        <Button
          key={item.key}
          onClick={() =>
            provider.activate(
              { kind: 'resource', locator: { providerId: PLUGIN_SIDEBAR_PROVIDER_ID, resourceId: item.key } },
              { openWorkspace }
            )
          }>
          {item.contribution.title}
        </Button>
      ))}
      <PluginWorkbenchSidebar />
      <main aria-label="Workspace">Editor</main>
      <PluginWorkbenchPanels />
    </>
  )
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

function installTransport() {
  Object.assign(globalThis, { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:8123', token: 'b'.repeat(64) } })
  let catalogueResponse: unknown = catalogue
  let sequence = 0
  const fetcher = vi.fn(async (input: globalThis.RequestInfo | globalThis.URL, init?: globalThis.RequestInit) => {
    const url = String(input)
    if (url.endsWith('/api/aix/views')) return new globalThis.Response(JSON.stringify(catalogueResponse))
    if (init?.method === 'POST') {
      const token = (++sequence).toString(16).padStart(64, '0')
      const body = JSON.parse(String(init.body))
      return new globalThis.Response(
        JSON.stringify({
          protocolVersion: 3,
          requestId: body.requestId,
          activationId: body.activationId,
          instanceId: body.instanceId,
          expiresAt: body.expiresAt,
          token,
          path: `/aix-view/${token}/ui/${body.surface}/page.html`
        })
      )
    }
    return cancellation(input, init)
  })
  vi.stubGlobal('fetch', fetcher)
  return {
    fetcher,
    setCatalogue: (next: unknown) => {
      catalogueResponse = next
    },
    remove: () => {
      catalogueResponse = { protocolVersion: 1, revision: `sha256:${'c'.repeat(64)}`, packages: [] }
    }
  }
}

function MiniAppChrome({ search }: { search: string }) {
  const apps = usePluginMiniApps(search)
  const { activeTab } = useTabs()
  const page = activeTab && new globalThis.URL(activeTab.url, 'https://studio.invalid').searchParams.get('page')
  return (
    <>
      <PluginMiniAppItems items={apps} />
      <PluginPage page={page || undefined} />
    </>
  )
}

function ScopeObserver({ observe }: { observe: (scope: WorkbenchPluginScope | null) => void }) {
  observe(usePluginScope())
  return null
}

const statePeers: globalThis.MessagePort[] = []
const providerDocuments: JSDOM[] = []

async function startPackagedScript(frame: globalThis.HTMLIFrameElement, fixture: string) {
  let initialized!: () => void
  const ready = new Promise<void>((resolve) => {
    initialized = resolve
  })
  const script = readFileSync(resolve('tests/fixtures/aix', fixture), 'utf8')
  const entry = readFileSync(resolve('tests/fixtures/aix', dirname(fixture), 'index.html'), 'utf8')
  const provider = new JSDOM(
    entry.replace(`<script src="${basename(fixture)}"></script>`, () => `<script>${script}</script>`),
    {
      url: frame.src,
      runScripts: 'dangerously',
      beforeParse(window) {
        Object.defineProperty(window, 'MessageChannel', {
          value: function () {
            const channel = new NodeMessageChannel()
            statePeers.push(channel.port1 as unknown as globalThis.MessagePort)
            channel.port1.addEventListener(
              'message',
              (event) => {
                const { data } = event as globalThis.MessageEvent
                if (data?.type === 'aix.view.connect' || data?.type === 'aix.tree.connect') initialized()
              },
              { once: true }
            )
            return channel
          }
        })
        Object.defineProperty(window, 'parent', {
          value: {
            postMessage(data: unknown, _origin: string, ports: globalThis.MessagePort[]) {
              globalThis.window.dispatchEvent(
                new globalThis.MessageEvent('message', {
                  data,
                  source: frame.contentWindow,
                  origin: 'null',
                  ports
                })
              )
            }
          }
        })
      }
    }
  )
  providerDocuments.push(provider)
  fireEvent.load(frame)
  await ready
  return provider
}

async function settled<T>(promise: Promise<T>, stage: string): Promise<T> {
  let timer: ReturnType<typeof globalThis.setTimeout> | undefined
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_resolve, reject) => {
        timer = globalThis.setTimeout(() => reject(new Error(`AIX document deadline: ${stage}`)), 5000)
      })
    ])
  } finally {
    if (timer) globalThis.clearTimeout(timer)
  }
}

afterEach(() => {
  for (const peer of statePeers.splice(0)) peer.close()
  vi.restoreAllMocks()
  cleanup()
  for (const document of providerDocuments.splice(0)) document.window.close()
  publishMemberProfile(null)
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('native AIX workbench', () => {
  // Catch hidden mini-app entries retaining executing documents or appearance changes restarting their sibling.
  it('withdraws a conditional mini app and its live port while retaining an unconditional editor', async () => {
    const { fetcher } = installTransport()
    const root = globalThis.document.documentElement
    const originalClass = root.className
    root.classList.remove('dark')
    const native = contextCatalogue ?? globalThis.structuredClone(catalogue)
    if (!contextCatalogue) {
      native.packages[0].containers = []
      const template = { ...catalogue.packages[0].views[0], container: null }
      native.packages[0].views = [
        {
          ...template,
          id: 'conditional',
          title: 'Conditional App',
          description: 'Inspect conditional data',
          point: 'studio.applets',
          when: { theme: 'light' }
        },
        { ...template, id: 'stable', title: 'Stable Editor', point: 'studio.editor' }
      ]
    }
    const plugins = createAixWorkbenchPlugins(aixWorkbenchCatalogueSchema.parse(native))
    const page = plugins[0].contributions.find((item) => item.slot === 'page' && item.title === 'Stable Editor')!
    const rendered = render(
      <PluginWindowRuntime namespace="conditional" windowId="main" plugins={plugins}>
        <PluginPage page={`${plugins[0].id}/${page.id}`} />
        <TabsProvider>
          <MiniAppChrome search="" />
        </TabsProvider>
      </PluginWindowRuntime>
    )
    try {
      const stable = await screen.findByTitle('Stable Editor')
      fireEvent.click(await screen.findByRole('button', { name: 'Conditional App' }))
      const frame = (await screen.findByTitle('Conditional App')) as globalThis.HTMLIFrameElement
      const provider = await settled(
        startPackagedScript(frame, 'miniapp-environment/app.js'),
        'initial conditional document'
      )
      const input = provider.window.document.querySelector('input')!
      fireEvent.input(input, { target: { value: 'private query' } })
      await waitFor(() => expect(provider.window.document.querySelector('output')?.textContent).toBe('Saved'))
      const closed = new Promise<void>((resolve) =>
        statePeers.at(-1)!.addEventListener('close', () => resolve(), { once: true })
      )
      await act(async () => {
        root.classList.add('dark')
      })
      await waitFor(() => expect(screen.queryByRole('button', { name: 'Conditional App' })).not.toBeInTheDocument())
      expect(screen.queryByTitle('Conditional App')).not.toBeInTheDocument()
      await settled(closed, 'conditional port closure')
      expect(screen.getByTitle('Stable Editor')).toBe(stable)
      await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1))
      fireEvent.input(input, { target: { value: 'late private query' } })
      await act(async () => {
        root.classList.remove('dark')
      })
      await screen.findByRole('button', { name: 'Conditional App' })
      const next = (await screen.findByTitle('Conditional App')) as globalThis.HTMLIFrameElement
      expect(next).not.toBe(frame)
      expect(next.src).not.toBe(frame.src)
      const reopened = await settled(
        startPackagedScript(next, 'miniapp-environment/app.js'),
        'reopened conditional document'
      )
      expect(reopened.window.document.querySelector('input')?.value).toBe('')
      expect(screen.getByTitle('Stable Editor')).toBe(stable)
    } finally {
      rendered.unmount()
      root.className = originalClass
    }
  })

  // Catch container conditions dropping only the button while leaving sidebar/panel execution or hidden nested views alive.
  it('applies container and per-view conditions to ActivityBar, sidebar and panel placement', async () => {
    installTransport()
    const root = globalThis.document.documentElement
    const originalClass = root.className
    const language = i18n.language
    root.classList.remove('dark')
    const native = contextCatalogue ?? globalThis.structuredClone(catalogue)
    if (!contextCatalogue) {
      native.packages[0].containers[0].when = { theme: 'dark' }
      native.packages[0].containers[1].when = { theme: 'light' }
      native.packages[0].views[0].when = { locale: 'en-US' }
    }
    const rendered = render(
      <PluginWindowRuntime
        namespace="conditional-containers"
        windowId="main"
        plugins={createAixWorkbenchPlugins(aixWorkbenchCatalogueSchema.parse(native))}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => {}} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    try {
      await act(async () => {
        await i18n.changeLanguage('en-US')
      })
      await screen.findByRole('button', { name: 'Output' })
      expect(screen.queryByRole('button', { name: 'Resources' })).not.toBeInTheDocument()
      fireEvent.click(screen.getByRole('button', { name: 'Output' }))
      await screen.findByTitle('Output')
      await act(async () => {
        root.classList.add('dark')
      })
      await screen.findByRole('button', { name: 'Resources' })
      expect(screen.queryByRole('button', { name: 'Output' })).not.toBeInTheDocument()
      expect(screen.queryByTitle('Output')).not.toBeInTheDocument()
      fireEvent.click(screen.getByRole('button', { name: 'Resources' }))
      await screen.findByTitle('Connections')
      const preview = await screen.findByTitle('Preview')
      await act(async () => {
        await i18n.changeLanguage('zh-CN')
      })
      await waitFor(() => expect(screen.queryByTitle('Connections')).not.toBeInTheDocument())
      expect(screen.getByTitle('Preview')).toBe(preview)
      expect(screen.getByRole('button', { name: 'Resources' })).toBeInTheDocument()
      await act(async () => {
        root.classList.remove('dark')
      })
      await waitFor(() => expect(screen.queryByRole('button', { name: 'Resources' })).not.toBeInTheDocument())
      expect(screen.queryByTitle('Preview')).not.toBeInTheDocument()
    } finally {
      rendered.unmount()
      root.className = originalClass
      await act(async () => {
        await i18n.changeLanguage(language)
      })
    }
  })

  // Catch a navigated WindowProxy receiving UI state, or a replay stealing an admitted document's port.
  it('admits only a document-owned channel and revokes it on navigation', async () => {
    const { fetcher } = installTransport()
    vi.stubGlobal('MessageChannel', NodeMessageChannel)
    const plugins = createAixWorkbenchPlugins(catalogue)
    render(
      <PluginWindowRuntime namespace="document-member" windowId="main" plugins={plugins}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    const frame = (await screen.findByTitle('Connections')) as globalThis.HTMLIFrameElement
    const post = vi.spyOn(frame.contentWindow!, 'postMessage').mockImplementation((...args: unknown[]) => {
      statePeers.push(...(args[2] as globalThis.MessagePort[]))
    })
    fireEvent.load(frame)
    expect(post).not.toHaveBeenCalled()
    const documentId = new globalThis.URLSearchParams(new globalThis.URL(frame.src).hash.slice(1)).get('aix-document')!
    expect(documentId).toMatch(/^[a-f0-9]{64}$/)
    const ready = (
      id: string,
      origin = 'null',
      extra = {},
      source: globalThis.MessageEventSource | null = frame.contentWindow
    ) => {
      const channel = new globalThis.MessageChannel()
      const peer = channel.port2
      statePeers.push(peer)
      const first = new Promise<unknown>((resolve) =>
        peer.addEventListener('message', ({ data }) => resolve(data), { once: true })
      )
      const closed = new Promise<void>((resolve) => peer.addEventListener('close', () => resolve(), { once: true }))
      peer.start()
      globalThis.window.dispatchEvent(
        new globalThis.MessageEvent('message', {
          source,
          origin,
          data: { protocolVersion: 1, type: 'aix.document.ready', documentId: id, ...extra },
          ports: [channel.port1]
        })
      )
      return { peer, first, closed }
    }
    const foreign = ready(documentId, 'null', {}, globalThis.window)
    for (const invalid of [
      ready('0'.repeat(64)),
      ready(documentId, 'https://external.invalid'),
      ready(documentId, 'null', { protocolVersion: 2 }),
      ready(documentId, 'null', { command: 'readFile' })
    ])
      await invalid.closed
    const admitted = ready(documentId)
    expect(await admitted.first).toEqual(
      expect.objectContaining({ type: 'aix.view.connect', surface: 'connections', stateJSON: null })
    )
    foreign.peer.close()
    const save = async (id: number) => {
      const receipt = new Promise<unknown>((resolve) =>
        admitted.peer.addEventListener('message', ({ data }) => resolve(data), { once: true })
      )
      admitted.peer.postMessage({ protocolVersion: 1, type: 'aix.view.store', id, stateJSON: '{"draft":"private"}' })
      expect(await receipt).toEqual({ protocolVersion: 1, type: 'aix.view.stored', id })
    }
    await save(1)
    const replay = ready(documentId)
    await replay.closed
    await save(2)
    fireEvent.load(frame)
    await admitted.closed
    await waitFor(() => expect(screen.queryByTitle('Connections')).not.toBeInTheDocument())
    expect(screen.getByTitle('Preview')).toBeInTheDocument()
    await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1))
    expect(post).not.toHaveBeenCalled()
  })

  // Catch a blank or top-left view while it loads, or a loader that outlives its document.
  it('covers a loading view with the shared centered loader until its document loads', async () => {
    installTransport()
    vi.stubGlobal('MessageChannel', NodeMessageChannel)
    render(
      <PluginWindowRuntime namespace="loader-member" windowId="main" plugins={createAixWorkbenchPlugins(catalogue)}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    const frame = (await screen.findByTitle('Connections')) as globalThis.HTMLIFrameElement
    const view = frame.parentElement!
    expect(view).toHaveClass('relative')
    expect(view.querySelector('.page-loader.page-loader--contain')).toHaveAttribute('aria-busy', 'true')
    fireEvent.load(frame)
    await waitFor(() => expect(view.querySelector('.page-loader')).toBeNull())
    expect(screen.getByTitle('Connections')).toBe(frame)
  })

  // Catch a declared provider not reaching native tree UI, keyboard navigation or its own actions.
  it('executes the packaged provider script through an owned tree port and withdraws it on uninstall', async () => {
    const transport = installTransport()
    vi.stubGlobal('MessageChannel', NodeMessageChannel)
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].views[0].presentation = 'tree'
    const plugins = createAixWorkbenchPlugins(aixWorkbenchCatalogueSchema.parse(native))
    const tree = (installed: typeof plugins) => (
      <PluginWindowRuntime namespace="tree-member" windowId="main" plugins={installed}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    const rendered = render(tree(plugins))
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    const frame = (await screen.findByTitle('Connections')) as globalThis.HTMLIFrameElement
    expect(frame).not.toBeVisible()
    const provider = await startPackagedScript(frame, 'tree-provider/provider.js')
    const root = await screen.findByRole('treeitem', { name: 'Tasks' })
    expect(root).toHaveAttribute('aria-expanded', 'false')
    expect(screen.getByRole('tree', { name: 'Connections' })).toHaveTextContent('Refresh 1')
    root.focus()
    fireEvent.keyDown(root, { key: 'ArrowRight' })
    const leaf = await screen.findByRole('treeitem', { name: 'First task' })
    expect(root).toHaveAttribute('aria-expanded', 'true')
    fireEvent.keyDown(root, { key: 'ArrowDown' })
    expect(leaf).toHaveFocus()
    fireEvent.click(screen.getByRole('button', { name: 'First task' }))
    await waitFor(() => expect(provider.window.document.body.dataset.selected).toBe('task-1'))
    await waitFor(() => expect(screen.getByRole('button', { name: 'open-task' })).toBeEnabled())
    fireEvent.keyDown(leaf, { key: 'Enter' })
    await waitFor(() => expect(provider.window.document.body.dataset.opened).toBe('task-1'))
    expect(await screen.findByText('Opened')).toBeInTheDocument()
    await waitFor(() => expect(screen.getAllByRole('button', { name: i18n.t('common.refresh') })[0]).toBeEnabled())
    fireEvent.click(screen.getAllByRole('button', { name: i18n.t('common.refresh') })[0])
    expect(await screen.findByText('Refresh 2')).toBeInTheDocument()
    const disconnected = new Promise<void>((resolve) =>
      statePeers[0].addEventListener('close', () => resolve(), { once: true })
    )
    rendered.rerender(tree([]))
    await waitFor(() => expect(screen.queryByRole('tree', { name: 'Connections' })).not.toBeInTheDocument())
    await disconnected
    expect(screen.queryByTitle('Connections')).not.toBeInTheDocument()
    await waitFor(() =>
      expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(2)
    )
  })

  // Catch a refused provider leaving its executing document and native session alive.
  it('withdraws a protocol-failed provider while its native cancellation receipt is still pending', async () => {
    const { fetcher } = installTransport()
    vi.stubGlobal('MessageChannel', NodeMessageChannel)
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].views[0].presentation = 'tree'
    const rendered = render(
      <PluginWindowRuntime
        namespace="failed-tree"
        windowId="main"
        plugins={createAixWorkbenchPlugins(aixWorkbenchCatalogueSchema.parse(native))}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    await startPackagedScript(
      (await screen.findByTitle('Connections')) as globalThis.HTMLIFrameElement,
      'tree-provider/provider.js'
    )
    await screen.findByRole('treeitem', { name: 'Tasks' })
    let acknowledge!: () => void
    const pending = new Promise<void>((resolve) => {
      acknowledge = resolve
    })
    fetcher.mockImplementation(async (input, init) => {
      if (init?.method === 'DELETE') await pending
      return cancellation(input, init)
    })
    const disconnected = new Promise<void>((resolve) =>
      statePeers[0].addEventListener('close', () => resolve(), { once: true })
    )
    statePeers[0].postMessage({ protocolVersion: 1, type: 'aix.tree.data', id: 9999, nodes: [] })
    await waitFor(() => expect(screen.queryByTitle('Connections')).not.toBeInTheDocument())
    await disconnected
    expect(screen.queryByRole('treeitem', { name: 'Tasks' })).not.toBeInTheDocument()
    await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1))
    await act(async () => {
      acknowledge()
    })
    rendered.unmount()
  })

  // Catch unbounded handshake retries or one failed frame retiring its healthy sibling.
  it('bounds document admission attempts and retires only the flooding view', async () => {
    const { fetcher } = installTransport()
    vi.stubGlobal('MessageChannel', NodeMessageChannel)
    render(
      <PluginWindowRuntime namespace="document-budget" windowId="main" plugins={createAixWorkbenchPlugins(catalogue)}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    const frame = (await screen.findByTitle('Connections')) as globalThis.HTMLIFrameElement
    for (let attempt = 0; attempt <= AIX_DOCUMENT_ATTEMPT_LIMIT; attempt++) {
      const channel = new globalThis.MessageChannel()
      statePeers.push(channel.port2)
      const closed = new Promise<void>((resolve) =>
        channel.port2.addEventListener('close', () => resolve(), { once: true })
      )
      channel.port2.start()
      globalThis.window.dispatchEvent(
        new globalThis.MessageEvent('message', {
          source: frame.contentWindow,
          origin: 'null',
          data: { protocolVersion: 1, type: 'aix.document.ready', documentId: '0'.repeat(64) },
          ports: [channel.port1]
        })
      )
      await closed
      if (attempt < AIX_DOCUMENT_ATTEMPT_LIMIT) expect(screen.getByTitle('Connections')).toBe(frame)
    }
    await waitFor(() => expect(screen.queryByTitle('Connections')).not.toBeInTheDocument())
    expect(screen.getByTitle('Preview')).toBeInTheDocument()
    await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1))
  })

  // Catch a tree without a document handshake retaining an executing frame and native ticket indefinitely.
  it('retires a loaded tree that never provides its document channel', async () => {
    const { fetcher } = installTransport()
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].views[0].presentation = 'tree'
    render(
      <PluginWindowRuntime namespace="silent-document" windowId="main" plugins={createAixWorkbenchPlugins(native)}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    const frame = await screen.findByTitle('Connections')
    const sibling = screen.getByTitle('Preview')
    vi.useFakeTimers()
    try {
      fireEvent.load(frame)
      await act(async () => {
        await vi.advanceTimersByTimeAsync(4999)
      })
      expect(screen.getByTitle('Connections')).toBe(frame)
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1)
      })
      expect(screen.queryByTitle('Connections')).not.toBeInTheDocument()
      expect(screen.queryByRole('tree', { name: 'Connections' })).not.toBeInTheDocument()
      expect(screen.getByTitle('Preview')).toBe(sibling)
      expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1)
    } finally {
      vi.useRealTimers()
    }
  })

  // Catch a refused native URL leaking its admission request until the entire view owner closes.
  it('cancels a refused native asset receipt without withdrawing its sibling', async () => {
    const { fetcher } = installTransport()
    const original = fetcher.getMockImplementation()!
    fetcher.mockImplementation(async (input, init) => {
      const response = await original(input, init)
      if (init?.method === 'POST' && JSON.parse(String(init.body)).surface === 'connections') {
        const value = await response.json()
        value.path = `/aix-view/${value.token}/ui/pa\nge.html`
        return new globalThis.Response(JSON.stringify(value))
      }
      return response
    })
    render(
      <PluginWindowRuntime namespace="refused-document" windowId="main" plugins={createAixWorkbenchPlugins(catalogue)}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    expect(await screen.findByTitle('Preview')).toBeInTheDocument()
    expect(screen.queryByTitle('Connections')).not.toBeInTheDocument()
    await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1))
    const admission = fetcher.mock.calls.find(
      ([, init]) => init?.method === 'POST' && JSON.parse(String(init.body)).surface === 'connections'
    )!
    const retired = fetcher.mock.calls.find(([, init]) => init?.method === 'DELETE')!
    expect(String(retired[0]).split('/').at(-1)).toBe(JSON.parse(String(admission[1]?.body)).requestId)
  })

  // Catch isolated views withdrawing while the native factory falsely reports resource cleanup success.
  it('keeps actual mounted view sessions in the owning host close report until acknowledgment', async () => {
    const { fetcher } = installTransport()
    let scope: WorkbenchPluginScope | null = null
    render(
      <PluginWindowRuntime namespace="settlement-member" windowId="main" plugins={createAixWorkbenchPlugins(catalogue)}>
        <PluginWorkbenchLayoutProvider>
          <ScopeObserver
            observe={(value) => {
              scope = value
            }}
          />
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    expect(await screen.findByTitle('Connections')).toBeInTheDocument()
    expect(await screen.findByTitle('Preview')).toBeInTheDocument()
    let acknowledge!: () => void
    const reply = new Promise<void>((resolve) => {
      acknowledge = resolve
    })
    fetcher.mockImplementation(async (input, init) => {
      if (init?.method !== 'DELETE') throw new Error('Unexpected native admission')
      await reply
      return cancellation(input, init)
    })
    let completed = false
    let close!: ReturnType<WorkbenchPluginScope['host']['close']>
    act(() => {
      close = scope!.host.close().then((report) => {
        completed = true
        return report
      })
    })
    await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(2))
    expect(screen.queryByTitle('Connections')).not.toBeInTheDocument()
    expect(screen.queryByTitle('Preview')).not.toBeInTheDocument()
    expect(completed).toBe(false)
    await act(async () => {
      acknowledge()
      await expect(close).resolves.toEqual({ failed: [], timedOut: [] })
    })
  })
  it('keeps a container-less editor off the activity bar', () => {
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].containers = []
    native.packages[0].views = [
      {
        id: 'picker',
        title: '审查',
        description: 'Choose one Word document.',
        icon: 'file-search',
        order: 10,
        container: null,
        point: 'studio.editor'
      }
    ]
    const plugins = createAixWorkbenchPlugins(aixWorkbenchCatalogueSchema.parse(native))
    expect(plugins[0].contributions.some((item) => item.slot === 'activity' || item.slot === 'applet')).toBe(false)
    expect(plugins[0].contributions.some((item) => item.slot === 'page' && item.title === '审查')).toBe(true)
  })

  // Catch mini apps leaking into ActivityBar, losing their owned launch page or surviving uninstall.
  it('searches and launches a native mini app in a real workspace tab and revokes its page on uninstall', async () => {
    const { fetcher } = installTransport()
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].containers = []
    native.packages[0].views = [
      {
        id: 'app',
        title: 'Data Lens',
        description: 'Inspect datasets',
        icon: 'database',
        order: 1,
        container: null,
        point: 'studio.applets'
      },
      {
        id: 'workspace',
        title: 'Workspace Lens',
        description: '',
        icon: 'database',
        order: 2,
        container: null,
        point: 'studio.workspace'
      }
    ]
    const plugins = createAixWorkbenchPlugins(aixWorkbenchCatalogueSchema.parse(native))
    expect(plugins[0].contributions.some((item) => item.slot === 'activity')).toBe(false)
    const tree = (installed: typeof plugins, search: string) => (
      <PluginWindowRuntime namespace="miniapp-member" windowId="main" plugins={installed}>
        <TabsProvider>
          <MiniAppChrome search={search} />
        </TabsProvider>
      </PluginWindowRuntime>
    )
    const rendered = render(tree(plugins, ''))
    expect(await screen.findByRole('button', { name: 'Data Lens' })).toBeInTheDocument()
    expect(screen.queryByTitle('Workspace Lens')).not.toBeInTheDocument()
    rendered.rerender(tree(plugins, 'missing'))
    expect(screen.queryByRole('button', { name: 'Data Lens' })).not.toBeInTheDocument()
    rendered.rerender(tree(plugins, ' DATASETS '))
    fireEvent.click(screen.getByRole('button', { name: 'Data Lens' }))
    const frame = await screen.findByTitle('Data Lens')
    expect(frame).toHaveAttribute('sandbox', 'allow-scripts')
    expect(frame.getAttribute('src')).not.toContain('b'.repeat(64))
    expect(screen.queryByRole('heading', { name: 'Data Lens' })).not.toBeInTheDocument()
    vi.stubGlobal('MessageChannel', NodeMessageChannel)
    const channel = new globalThis.MessageChannel()
    const statePeer = channel.port2
    statePeers.push(statePeer)
    const greeting = new Promise<unknown>((resolve) =>
      statePeer.addEventListener('message', ({ data }) => resolve(data), { once: true })
    )
    statePeer.start()
    fireEvent.load(frame)
    globalThis.window.dispatchEvent(
      new globalThis.MessageEvent('message', {
        source: (frame as globalThis.HTMLIFrameElement).contentWindow,
        origin: 'null',
        data: {
          protocolVersion: 1,
          type: 'aix.document.ready',
          documentId: new globalThis.URLSearchParams(
            new globalThis.URL((frame as globalThis.HTMLIFrameElement).src).hash.slice(1)
          ).get('aix-document')
        },
        ports: [channel.port1]
      })
    )
    expect(await greeting).toEqual({
      protocolVersion: 1,
      type: 'aix.view.connect',
      surface: 'app',
      stateJSON: null,
      environment: expect.objectContaining({
        theme: expect.stringMatching(/^(light|dark)$/),
        locale: i18n.resolvedLanguage || i18n.language
      })
    })
    const stored = new Promise<unknown>((resolve) => {
      statePeer.addEventListener('message', ({ data }) => resolve(data), { once: true })
    })
    statePeer.postMessage({ protocolVersion: 1, type: 'aix.view.store', id: 1, stateJSON: '{"query":"customers"}' })
    await expect(stored).resolves.toEqual({ protocolVersion: 1, type: 'aix.view.stored', id: 1 })
    const disconnected = new Promise<void>((resolve) => {
      statePeer.addEventListener('close', () => resolve(), { once: true })
    })
    rendered.rerender(tree([], ''))
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Data Lens' })).not.toBeInTheDocument())
    expect(screen.queryByTitle('Data Lens')).not.toBeInTheDocument()
    await disconnected
    await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1))
  })
  // Catch a shell mini app opening as a workspace tab instead of its own system window.
  it('opens an AIX mini app in its own system window inside the shell', async () => {
    installTransport()
    const invoke = vi.fn(async () => undefined)
    vi.stubGlobal('__TAURI_INTERNALS__', { invoke })
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].containers = []
    native.packages[0].views = [
      {
        id: 'window-app',
        title: 'Data Window',
        description: 'Inspect datasets in a window',
        icon: 'database',
        order: 1,
        container: null,
        point: 'studio.applets'
      }
    ]
    const parsed = aixWorkbenchCatalogueSchema.parse(native)
    publishAixCatalogue(parsed)
    const plugins = createAixWorkbenchPlugins(parsed)
    render(
      <PluginWindowRuntime namespace="miniapp-window" windowId="main" plugins={plugins}>
        <TabsProvider>
          <MiniAppChrome search="" />
        </TabsProvider>
      </PluginWindowRuntime>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Data Window' }))
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('open_aix_applet_window', {
        slug: 'proof-views',
        view: 'window-app',
        title: 'Data Window'
      })
    )
    expect(screen.queryByTitle('Data Window')).not.toBeInTheDocument()
    publishAixCatalogue({ ...parsed, packages: [] })
  })
  // Catch one mini-app instance inheriting another's private draft or closing its still-executing sibling.
  it('isolates actual mini-app page instances and retires only the closed instance', async () => {
    const transport = installTransport()
    vi.stubGlobal('MessageChannel', NodeMessageChannel)
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].containers = []
    native.packages[0].views = [
      {
        id: 'app',
        title: 'Instance App',
        description: '',
        icon: 'package',
        order: 1,
        container: null,
        point: 'studio.applets'
      }
    ]
    const installed = createAixWorkbenchPlugins(native)
    const page = installed[0].contributions.find((item) => item.slot === 'page')!
    const pageKey = `${installed[0].id}/${page.id}`
    const tree = (first: boolean, second: boolean, plugins = installed) => (
      <PluginWindowRuntime namespace="instance-member" windowId="main" plugins={plugins}>
        {first && (
          <section aria-label="First instance">
            <PluginPage page={pageKey} />
          </section>
        )}
        {second && (
          <section aria-label="Second instance">
            <PluginPage page={pageKey} />
          </section>
        )}
      </PluginWindowRuntime>
    )
    const rendered = render(tree(true, false))
    const first = (await within(screen.getByRole('region', { name: 'First instance' })).findByTitle(
      'Instance App'
    )) as globalThis.HTMLIFrameElement
    const left = await startPackagedScript(first, 'miniapp-environment/app.js')
    const leftInput = left.window.document.querySelector('input')!
    fireEvent.input(leftInput, { target: { value: 'private first draft' } })
    await waitFor(() => expect(left.window.document.querySelector('output')?.textContent).toBe('Saved'))
    rendered.rerender(tree(true, true))
    const second = (await within(screen.getByRole('region', { name: 'Second instance' })).findByTitle(
      'Instance App'
    )) as globalThis.HTMLIFrameElement
    const right = await startPackagedScript(second, 'miniapp-environment/app.js')
    const rightInput = right.window.document.querySelector('input')!
    expect(rightInput.value).toBe('')
    fireEvent.input(rightInput, { target: { value: 'private second draft' } })
    await waitFor(() => expect(right.window.document.querySelector('output')?.textContent).toBe('Saved'))
    const disconnected = new Promise<void>((resolve) =>
      statePeers[0].addEventListener('close', () => resolve(), { once: true })
    )
    rendered.rerender(tree(false, true))
    await disconnected
    await waitFor(() =>
      expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1)
    )
    expect(screen.getByTitle('Instance App')).toBe(second)
    expect(rightInput.value).toBe('private second draft')
    const savedAgain = new Promise<unknown>((resolve) =>
      statePeers[1].addEventListener('message', ({ data }) => resolve(data), { once: true })
    )
    fireEvent.input(rightInput, { target: { value: 'second remains live' } })
    await expect(savedAgain).resolves.toEqual({ protocolVersion: 1, type: 'aix.view.stored', id: 2 })
    rendered.rerender(tree(true, true))
    const reopened = (await within(screen.getByRole('region', { name: 'First instance' })).findByTitle(
      'Instance App'
    )) as globalThis.HTMLIFrameElement
    const fresh = await startPackagedScript(reopened, 'miniapp-environment/app.js')
    expect(fresh.window.document.querySelector('input')!.value).toBe('')
    expect(within(screen.getByRole('region', { name: 'Second instance' })).getByTitle('Instance App')).toBe(second)
    expect(rightInput.value).toBe('second remains live')
    rendered.rerender(tree(true, true, []))
    await waitFor(() => expect(screen.queryAllByTitle('Instance App')).toHaveLength(0))
    await waitFor(() =>
      expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(3)
    )
  })

  // Catch a real mini-app losing editable state after unrelated installs or appearance updates.
  it('runs the packaged mini app with live bounded UI environment on the same isolated frame', async () => {
    installTransport()
    vi.stubGlobal('MessageChannel', NodeMessageChannel)
    const root = globalThis.document.documentElement
    const originalClass = root.className
    const originalStyle = root.getAttribute('style')
    const originalLanguage = i18n.language
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].containers = []
    native.packages[0].views = [
      {
        id: 'app',
        title: 'Environment App',
        description: 'Follow Studio appearance',
        icon: 'package',
        order: 1,
        container: null,
        point: 'studio.applets'
      }
    ]
    const plugins = createAixWorkbenchPlugins(aixWorkbenchCatalogueSchema.parse(native))
    const tree = (installed: typeof plugins) => (
      <PluginWindowRuntime namespace="environment" windowId="main" plugins={installed}>
        <TabsProvider>
          <MiniAppChrome search="" />
        </TabsProvider>
      </PluginWindowRuntime>
    )
    const rendered = render(tree(plugins))
    try {
      await act(async () => {
        await i18n.changeLanguage('en-US')
      })
      root.classList.remove('dark')
      root.style.setProperty('--background', '#ffffff')
      root.style.setProperty('--private-workspace', '/private/workspace')
      fireEvent.click(await screen.findByRole('button', { name: 'Environment App' }))
      const frame = (await screen.findByTitle('Environment App')) as globalThis.HTMLIFrameElement
      const provider = await startPackagedScript(frame, 'miniapp-environment/app.js')
      const page = provider.window.document
      expect(page.documentElement.lang).toBe('en-US')
      expect(page.documentElement.dir).toBe('ltr')
      expect(page.documentElement.dataset.theme).toBe('light')
      expect(page.documentElement.style.getPropertyValue('--background')).toBe('#ffffff')
      expect(page.documentElement.style.getPropertyValue('--private-workspace')).toBe('')
      const input = page.querySelector('input')!
      fireEvent.input(input, { target: { value: 'customers' } })
      await waitFor(() => expect(page.querySelector('output')?.textContent).toBe('Saved'))
      const other = globalThis.structuredClone(native)
      other.packages[0].slug = 'other-app'
      other.packages[0].packageId = 'use/proof/other'
      other.packages[0].views[0].title = 'Other App'
      rendered.rerender(tree([...plugins, ...createAixWorkbenchPlugins(other)]))
      await screen.findByRole('button', { name: 'Other App' })
      expect(screen.getByTitle('Environment App')).toBe(frame)
      expect(input.value).toBe('customers')
      rendered.rerender(tree(plugins))
      await waitFor(() => expect(screen.queryByRole('button', { name: 'Other App' })).not.toBeInTheDocument())
      expect(screen.getByTitle('Environment App')).toBe(frame)
      const savedAgain = new Promise<unknown>((resolve) => {
        statePeers[0].addEventListener('message', ({ data }) => resolve(data), { once: true })
      })
      fireEvent.input(input, { target: { value: 'orders' } })
      await expect(savedAgain).resolves.toEqual({ protocolVersion: 1, type: 'aix.view.stored', id: 2 })
      await act(async () => {
        root.classList.add('dark')
        root.style.setProperty('--background', '#171717')
        await i18n.changeLanguage('zh-CN')
      })
      await waitFor(() => expect(page.documentElement.lang).toBe('zh-CN'))
      expect(page.documentElement.dir).toBe('ltr')
      expect(page.documentElement.dataset.theme).toBe('dark')
      expect(page.documentElement.style.getPropertyValue('--background')).toBe('#171717')
      expect(input.value).toBe('orders')
      expect(screen.getByTitle('Environment App')).toBe(frame)
      root.style.removeProperty('--background')
      await waitFor(() => expect(page.documentElement.style.getPropertyValue('--background')).toBe(''))
      expect(input.value).toBe('orders')
      const disconnected = new Promise<void>((resolve) =>
        statePeers[0].addEventListener('close', () => resolve(), { once: true })
      )
      rendered.rerender(tree([]))
      await waitFor(() => expect(screen.queryByTitle('Environment App')).not.toBeInTheDocument())
      await disconnected
    } finally {
      rendered.unmount()
      root.className = originalClass
      if (originalStyle === null) root.removeAttribute('style')
      else root.setAttribute('style', originalStyle)
      await act(async () => {
        await i18n.changeLanguage(originalLanguage)
      })
    }
  })
  // Catch remote metadata or cross-package container references reaching trusted factories.
  it('rejects invalid view ownership and duplicate package identities at the native boundary', () => {
    expect(aixWorkbenchCatalogueSchema.parse(catalogue).packages[0].views).toHaveLength(3)
    expect(aixWorkbenchCatalogueSchema.safeParse({ ...catalogue, protocolVersion: 2 }).success).toBe(false)
    for (const change of [{ point: 'studio.applets' }, { order: 2147483648 }]) {
      const invalid = globalThis.structuredClone(catalogue)
      Object.assign(invalid.packages[0].views[0], change)
      expect(aixWorkbenchCatalogueSchema.safeParse(invalid).success).toBe(false)
    }
    const unicode = globalThis.structuredClone(catalogue)
    unicode.packages[0].views[0].title = '🔎'.repeat(64)
    expect(aixWorkbenchCatalogueSchema.safeParse(unicode).success).toBe(true)
    const invalid = globalThis.structuredClone(catalogue)
    invalid.packages[0].views[0].container = 'foreign'
    expect(aixWorkbenchCatalogueSchema.safeParse(invalid).success).toBe(false)
    expect(
      aixWorkbenchCatalogueSchema.safeParse({ ...catalogue, packages: [catalogue.packages[0], catalogue.packages[0]] })
        .success
    ).toBe(false)
    for (const diagnostic of [
      { slug: 'damaged', reason: 'grant-file-access' },
      { slug: '../foreign', reason: 'package-unavailable' },
      { slug: 'damaged', reason: 'package-content-refused', grants: ['workspace'] }
    ]) {
      expect(aixWorkbenchCatalogueSchema.safeParse({ ...catalogue, diagnostics: [diagnostic] }).success).toBe(false)
    }
  })

  it('requires matching structured activation and instance identities in native admission receipts', () => {
    const admission = {
      protocolVersion: 3,
      requestId: 'a'.repeat(64),
      activationId: 'b'.repeat(64),
      instanceId: 'c'.repeat(64),
      expiresAt: Date.now() + 1000
    }
    const receipt = {
      ...admission,
      token: 'd'.repeat(64),
      path: `/aix-view/${'d'.repeat(64)}/ui/connections/page.html`
    }
    expect(aixViewSessionSchema.safeParse(receipt).success).toBe(true)
    expect(aixViewCancellationSchema.safeParse({ ...admission, closed: true }).success).toBe(true)
    for (const invalid of [
      { ...admission, activationId: undefined },
      { ...admission, instanceId: 'malformed' },
      { ...admission, foreign: true }
    ]) {
      expect(aixViewSessionSchema.safeParse({ ...invalid, token: 'd'.repeat(64), path: receipt.path }).success).toBe(
        false
      )
      expect(aixViewCancellationSchema.safeParse({ ...invalid, closed: true }).success).toBe(false)
    }
  })

  // Catch activity containers opening editor tabs, missing sibling views, unsafe frames and leaked sessions.
  it('opens multiple owned views beside the editor, places panels below it and withdraws installed UI', async () => {
    const { fetcher } = installTransport()
    const openWorkspace = vi.fn()
    const plugins = createAixWorkbenchPlugins(catalogue)
    const tree = (installed: typeof plugins) => (
      <PluginWindowRuntime namespace="member" windowId="main" plugins={installed}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={openWorkspace} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    const view = render(tree(plugins))
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    await waitFor(() => expect(screen.getByTitle('Connections')).toBeInTheDocument())
    expect(screen.getByRole('complementary', { name: 'Resources' })).toContainElement(screen.getByTitle('Preview'))
    expect(screen.getByRole('main', { name: 'Workspace' })).toHaveTextContent('Editor')
    expect(openWorkspace).not.toHaveBeenCalled()
    const frame = screen.getByTitle('Connections')
    expect(frame).toHaveAttribute('sandbox', 'allow-scripts')
    expect(frame).toHaveAttribute('referrerpolicy', 'no-referrer')
    expect(frame.getAttribute('src')).not.toContain('b'.repeat(64))
    expect(screen.queryByTitle('Output')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Output' }))
    expect(await screen.findByTitle('Output')).toBeInTheDocument()
    const requests = fetcher.mock.calls.filter(([, init]) => init?.method === 'POST')
    expect(requests.map(([, init]) => JSON.parse(String(init?.body)))).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ slug: 'proof-views', surface: 'connections', revision, protocolVersion: 3 }),
        expect.objectContaining({ slug: 'proof-views', surface: 'preview', revision, protocolVersion: 3 }),
        expect.objectContaining({ slug: 'proof-views', surface: 'output', revision, protocolVersion: 3 })
      ])
    )
    view.rerender(tree([]))
    await waitFor(() => expect(screen.queryByRole('complementary', { name: 'Resources' })).not.toBeInTheDocument())
    expect(screen.queryByTitle('Connections')).not.toBeInTheDocument()
    expect(screen.queryByTitle('Output')).not.toBeInTheDocument()
    await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(3))
    view.rerender(tree(plugins))
    expect(await screen.findByRole('button', { name: 'Resources' })).toBeInTheDocument()
    expect(screen.queryByTitle('Connections')).not.toBeInTheDocument()
    expect(screen.queryByTitle('Output')).not.toBeInTheDocument()
  })

  // Catch accumulated obsolete factories refusing a valid upgrade or restarting its unchanged sibling.
  it('reclaims obsolete factories while preserving executing siblings and admitting the next upgrade', async () => {
    const transport = installTransport()
    const cache = new Map<string, WorkbenchPlugin>()
    const item = (slug: string, title: string) => ({
      ...catalogue.packages[0],
      slug,
      packageId: `use/proof/${slug}`,
      containers: [],
      views: [{ ...catalogue.packages[0].views[0], title, container: null, point: 'studio.editor' as const }]
    })
    const native = aixWorkbenchCatalogueSchema.parse({
      ...catalogue,
      packages: [item('healthy', 'Healthy view'), item('upgrade', 'Upgraded view')]
    })
    const admitted = createAixWorkbenchPlugins(native, cache)
    const page = (plugin: WorkbenchPlugin) =>
      `${plugin.id}/${plugin.contributions.find((value) => value.slot === 'page')?.id}`
    function Core() {
      const [count, setCount] = useState(0)
      return <Button onClick={() => setCount(count + 1)}>Core count {count}</Button>
    }
    const tree = (plugins: readonly WorkbenchPlugin[]) => (
      <PluginWindowRuntime namespace="capacity-owner" windowId="main" plugins={plugins}>
        <Core />
        {plugins.map((plugin) => (
          <PluginPage key={page(plugin)} page={page(plugin)} />
        ))}
      </PluginWindowRuntime>
    )
    const rendered = render(tree(admitted))
    const healthy = await screen.findByTitle('Healthy view')
    const oldUpgrade = await screen.findByTitle('Upgraded view')
    const oldRequest = transport.fetcher.mock.calls.find(
      ([, init]) => init?.method === 'POST' && JSON.parse(String(init.body)).slug === 'upgrade'
    )
    expect(oldRequest).toBeDefined()
    const oldRequestId = JSON.parse(String(oldRequest?.[1]?.body)).requestId

    for (let generation = 1; generation <= 1022; generation++) {
      createAixWorkbenchPlugins(
        aixWorkbenchCatalogueSchema.parse({
          ...catalogue,
          packages: [...native.packages, { ...item('history', 'Historical view'), generation }]
        }),
        cache
      )
    }
    const next = globalThis.structuredClone(native)
    next.packages[1].generation = 2
    next.packages[1].revision = `sha256:${'f'.repeat(64)}`
    next.packages.reverse()
    const upgraded = createAixWorkbenchPlugins(next, cache)
    expect(screen.getByTitle('Upgraded view')).toBe(oldUpgrade)
    rendered.rerender(tree(upgraded))
    await waitFor(() => expect(screen.getByTitle('Upgraded view')).not.toBe(oldUpgrade))
    expect(screen.getByTitle('Healthy view')).toBe(healthy)
    fireEvent.click(screen.getByRole('button', { name: 'Core count 0' }))
    expect(screen.getByRole('button', { name: 'Core count 1' })).toBeInTheDocument()
    await waitFor(() =>
      expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1)
    )
    expect(String(transport.fetcher.mock.calls.find(([, init]) => init?.method === 'DELETE')?.[0])).toContain(
      oldRequestId
    )
    await waitFor(() =>
      expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'POST')).toHaveLength(3)
    )

    const latest = globalThis.structuredClone(native)
    latest.packages[1].generation = 3
    latest.packages[1].revision = `sha256:${'e'.repeat(64)}`
    latest.packages[1].views[0].title = 'Latest view'
    latest.packages.reverse()
    rendered.rerender(tree(createAixWorkbenchPlugins(latest, cache)))
    await screen.findByTitle('Latest view')
    expect(screen.getByTitle('Healthy view')).toBe(healthy)
    expect(screen.getByRole('button', { name: 'Core count 1' })).toBeInTheDocument()
  })

  // Catch an unrelated package change rebuilding an executing view, its provider state or its native session.
  it('keeps the actual mounted provider and asset sessions through unrelated package changes', async () => {
    const transport = installTransport()
    vi.stubGlobal('MessageChannel', NodeMessageChannel)
    const native = globalThis.structuredClone(catalogue)
    native.packages[0].views[0].presentation = 'tree'
    const cache = new Map()
    const tree = (next: AixWorkbenchCatalogue) => (
      <PluginWindowRuntime namespace="factory-owner" windowId="main" plugins={createAixWorkbenchPlugins(next, cache)}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    const rendered = render(tree(native))
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    const frame = (await screen.findByTitle('Connections')) as globalThis.HTMLIFrameElement
    const provider = await startPackagedScript(frame, 'tree-provider/provider.js')
    fireEvent.keyDown(await screen.findByRole('treeitem', { name: 'Tasks' }), { key: 'ArrowRight' })
    fireEvent.click(await screen.findByRole('button', { name: 'First task' }))
    await waitFor(() => expect(provider.window.document.body.dataset.selected).toBe('task-1'))
    const next = globalThis.structuredClone(native)
    next.revision = `sha256:${'d'.repeat(64)}`
    const other = globalThis.structuredClone(next.packages[0])
    other.slug = 'other-views'
    other.packageId = 'use/proof/other'
    other.containers[0].title = 'Other Resources'
    other.containers[1].title = 'Other Output'
    next.packages.push(other)
    rendered.rerender(tree(next))
    await screen.findByRole('button', { name: 'Other Resources' })
    expect(screen.getByTitle('Connections')).toBe(frame)
    expect(screen.getByRole('treeitem', { name: 'First task' })).toHaveAttribute('aria-selected', 'true')
    const upgraded = globalThis.structuredClone(next)
    upgraded.revision = `sha256:${'e'.repeat(64)}`
    upgraded.packages[1].generation = 2
    upgraded.packages[1].revision = `sha256:${'f'.repeat(64)}`
    upgraded.packages[1].containers[0].title = 'Other Resources v2'
    rendered.rerender(tree(upgraded))
    await screen.findByRole('button', { name: 'Other Resources v2' })
    expect(screen.getByTitle('Connections')).toBe(frame)
    expect(screen.getByRole('treeitem', { name: 'First task' })).toHaveAttribute('aria-selected', 'true')
    rendered.rerender(tree(native))
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Other Resources v2' })).not.toBeInTheDocument())
    expect(screen.getByTitle('Connections')).toBe(frame)
    fireEvent.keyDown(screen.getByRole('treeitem', { name: 'First task' }), { key: 'Enter' })
    await waitFor(() => expect(provider.window.document.body.dataset.opened).toBe('task-1'))
    expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'POST')).toHaveLength(2)
    expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(0)
    const disconnected = new Promise<void>((resolve) =>
      statePeers[0].addEventListener('close', () => resolve(), { once: true })
    )
    rendered.rerender(tree({ ...native, packages: [] }))
    await waitFor(() => expect(screen.queryByTitle('Connections')).not.toBeInTheDocument())
    await disconnected
    await waitFor(() =>
      expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(2)
    )
  })

  // Catch a late session response resurrecting a withdrawn view or leaking a known asset ticket.
  it('closes a late session response after its owning view is withdrawn', async () => {
    const transport = installTransport()
    let complete!: (response: globalThis.Response) => void
    const pending = new Promise<globalThis.Response>((resolve) => {
      complete = resolve
    })
    transport.fetcher.mockImplementation(async (input, init) =>
      init?.method === 'POST'
        ? pending.then((response) => {
            const body = JSON.parse(String(init?.body))
            return response
              .clone()
              .json()
              .then(
                (value) =>
                  new globalThis.Response(
                    JSON.stringify({
                      ...value,
                      protocolVersion: 3,
                      requestId: body.requestId,
                      activationId: body.activationId,
                      instanceId: body.instanceId,
                      expiresAt: body.expiresAt
                    })
                  )
              )
          })
        : cancellation(input, init)
    )
    const plugins = createAixWorkbenchPlugins(catalogue)
    const tree = (installed: typeof plugins) => (
      <PluginWindowRuntime namespace="late-member" windowId="main" plugins={installed}>
        <PluginWorkbenchLayoutProvider>
          <Chrome openWorkspace={() => undefined} />
        </PluginWorkbenchLayoutProvider>
      </PluginWindowRuntime>
    )
    const rendered = render(tree(plugins))
    fireEvent.click(await screen.findByRole('button', { name: 'Resources' }))
    await waitFor(() =>
      expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'POST')).toHaveLength(2)
    )
    rendered.rerender(tree([]))
    await waitFor(() => expect(screen.queryByRole('complementary', { name: 'Resources' })).not.toBeInTheDocument())
    const token = 'e'.repeat(64)
    await act(async () => {
      complete(new globalThis.Response(JSON.stringify({ token, path: `/aix-view/${token}/ui/index.html` })))
    })
    await waitFor(() =>
      expect(
        transport.fetcher.mock.calls.filter(
          ([input, init]) => init?.method === 'DELETE' && String(input).includes('/api/aix/views/requests/')
        )
      ).toHaveLength(2)
    )
    expect(screen.queryByTitle('Connections')).not.toBeInTheDocument()
  })

  // Catch a native package refusal withdrawing healthy executing documents or leaving the refused owner alive.
  it('applies package quarantine while preserving the healthy document and its state channel', async () => {
    const transport = installTransport()
    const item = (slug: string, title: string) => ({
      ...catalogue.packages[0],
      slug,
      packageId: `use/proof/${slug}`,
      containers: [],
      views: [{ ...catalogue.packages[0].views[0], title, container: null, point: 'studio.editor' as const }]
    })
    const healthy = item('healthy', 'Healthy document')
    const damaged = item('damaged', 'Damaged document')
    transport.setCatalogue({ ...catalogue, packages: [damaged, healthy], diagnostics: [] })
    publishMemberProfile({ id: '11111111-1111-1111-1111-111111111111', displayName: 'Member', phone: '' })
    function Pages() {
      return usePluginContributions('page').map((page) => <PluginPage key={page.key} page={page.key} />)
    }
    render(
      <AixWorkbenchRuntime windowId="main" builtins={[]}>
        <Pages />
      </AixWorkbenchRuntime>
    )
    const frame = (await screen.findByTitle('Healthy document')) as globalThis.HTMLIFrameElement
    await screen.findByTitle('Damaged document')
    const provider = await startPackagedScript(frame, 'miniapp-environment/app.js')
    const input = provider.window.document.querySelector('input')!
    input.value = 'retained draft'
    await act(async () => {
      input.dispatchEvent(new provider.window.Event('input'))
    })
    await waitFor(() => expect(provider.window.document.querySelector('output')?.textContent).toBe('Saved'))
    const badRequest = transport.fetcher.mock.calls.find(
      ([, init]) => init?.method === 'POST' && JSON.parse(String(init.body)).slug === 'damaged'
    )!
    const badRequestId = JSON.parse(String(badRequest[1]?.body)).requestId
    transport.setCatalogue({
      ...catalogue,
      revision: `sha256:${'c'.repeat(64)}`,
      packages: [healthy],
      diagnostics: [{ slug: 'damaged', reason: 'package-content-refused' }]
    })
    act(() => {
      globalThis.window.dispatchEvent(new globalThis.Event('a3s:plugin-install'))
    })
    await waitFor(() => expect(screen.queryByTitle('Damaged document')).not.toBeInTheDocument())
    expect(screen.getByTitle('Healthy document')).toBe(frame)
    expect(input.value).toBe('retained draft')
    await waitFor(() =>
      expect(transport.fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1)
    )
    expect(String(transport.fetcher.mock.calls.find(([, init]) => init?.method === 'DELETE')?.[0])).toContain(
      badRequestId
    )
    expect(
      transport.fetcher.mock.calls.filter(
        ([, init]) => init?.method === 'POST' && JSON.parse(String(init.body)).slug === 'healthy'
      )
    ).toHaveLength(1)
    provider.window.document.querySelector('output')!.textContent = ''
    input.value = 'channel still usable'
    await act(async () => {
      input.dispatchEvent(new provider.window.Event('input'))
    })
    await waitFor(() => expect(provider.window.document.querySelector('output')?.textContent).toBe('Saved'))
  })

  // Catch the main window ignoring install events or carrying installed views into a signed-out owner.
  it('loads the native catalogue, refreshes after uninstall and drops the member namespace immediately', async () => {
    const transport = installTransport()
    publishMemberProfile({ id: '11111111-1111-1111-1111-111111111111', displayName: 'Member', phone: '' })
    const view = render(
      <AixWorkbenchRuntime windowId="main" builtins={[]}>
        <Chrome openWorkspace={() => undefined} />
      </AixWorkbenchRuntime>
    )
    expect(await screen.findByRole('button', { name: 'Resources' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Resources' }))
    expect(await screen.findByTitle('Connections')).toBeInTheDocument()
    transport.remove()
    act(() => {
      globalThis.window.dispatchEvent(new globalThis.Event('a3s:plugin-install'))
    })
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Resources' })).not.toBeInTheDocument())
    act(() => {
      publishMemberProfile(null)
    })
    expect(screen.queryByTitle('Connections')).not.toBeInTheDocument()
    view.unmount()
  })
})
