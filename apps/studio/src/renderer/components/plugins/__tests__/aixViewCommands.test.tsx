import { readFileSync } from 'node:fs'
import process from 'node:process'
import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'

import { Button } from '@a3s-lab/ui'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import i18n from 'i18next'
import { JSDOM } from 'jsdom'
import JSZip from 'jszip'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CommandProvider } from '@renderer/components/command/CommandProvider'
import { useCommandRuntime } from '@renderer/hooks/command'
import { extensionCommandId } from '@renderer/services/rendererCommandService'
import type { CommandRuntime } from '@renderer/types/command'
import type { WorkbenchPlugin } from '@renderer/types/workbenchPlugin'
import { aixWorkbenchCatalogueSchema } from '@shared/types/aixWorkbench'

import { createAixWorkbenchPlugins } from '../AixWorkbenchRuntime'
import { usePluginContributionContext } from '../PluginContributionContext'
import { PluginPage } from '../PluginPage'
import { usePluginContributions, usePluginScope, usePluginScopeSnapshot } from '../PluginScopeProvider'
import { createPluginSidebarProviders, PLUGIN_SIDEBAR_PROVIDER_ID } from '../pluginSidebarProviders'
import { PluginWindowRuntime } from '../PluginWindowRuntime'
import {
  PluginWorkbenchLayoutProvider,
  PluginWorkbenchSidebar,
  usePluginContainerSelection
} from '../PluginWorkbenchLayout'

vi.unmock('@a3s-lab/ui')

const cataloguePath = process.env.A3S_CLOUD_TEST_AIX_COMMANDS_CATALOGUE_INPUT
const archivePath = process.env.A3S_CLOUD_TEST_AIX_COMMANDS_PACKAGE
const archive = archivePath ? JSZip.loadAsync(readFileSync(archivePath)) : undefined
const catalogue = cataloguePath
  ? aixWorkbenchCatalogueSchema.parse(JSON.parse(readFileSync(cataloguePath, 'utf8')))
  : undefined
const documents: JSDOM[] = []
const peers: globalThis.MessagePort[] = []
const originalClass = globalThis.document.documentElement.className
const originalLocale = i18n.language

function installTransport() {
  Object.assign(globalThis, { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:8123', token: 'b'.repeat(64) } })
  let sequence = 0
  const fetcher = vi.fn(async (input: globalThis.RequestInfo | globalThis.URL, init?: globalThis.RequestInit) => {
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
        path: `/aix-view/${token}/ui/${body.surface}/index.html`
      })
    )
  })
  vi.stubGlobal('fetch', fetcher)
  return fetcher
}

async function startScript(frame: globalThis.HTMLIFrameElement) {
  let initialized!: () => void
  const ready = new Promise<void>((resolve) => {
    initialized = resolve
  })
  if (!archive) throw new Error('The native catalogue proof requires its actual AIX archive')
  const zip = await archive
  const surface = new globalThis.URL(frame.src).pathname.split('/ui/')[1]?.split('/')[0]
  const scriptFile = zip.file(`ui/${surface}/app.js`)
  const entryFile = zip.file(`ui/${surface}/index.html`)
  if (!scriptFile || !entryFile) throw new Error('The admitted document is absent from the AIX archive')
  const script = await scriptFile.async('string')
  const entry = await entryFile.async('string')
  const provider = new JSDOM(
    entry.replace('<script src="app.js"></script>', () => `<script>${script}</script>`),
    {
      url: frame.src,
      runScripts: 'dangerously',
      beforeParse(window) {
        Object.defineProperty(window, 'MessageChannel', {
          value: function () {
            const channel = new NodeMessageChannel()
            peers.push(channel.port1 as unknown as globalThis.MessagePort)
            channel.port1.addEventListener('message', (event) => {
              if ((event as globalThis.MessageEvent).data?.type === 'aix.command.connect') initialized()
            })
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
  documents.push(provider)
  fireEvent.load(frame)
  await ready
  return provider
}

function ActivityChrome() {
  const activities = usePluginContributions('activity')
  const snapshot = usePluginScopeSnapshot()
  const selection = usePluginContainerSelection()
  const visibility = usePluginContributionContext(usePluginScope())
  const provider = createPluginSidebarProviders(
    [],
    [],
    () => snapshot,
    (key) => key,
    selection,
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
              { openWorkspace: () => {} }
            )
          }>
          {item.contribution.title}
        </Button>
      ))}
      <PluginWorkbenchSidebar />
    </>
  )
}

function RuntimeObserver({ observe }: { observe: (runtime: CommandRuntime) => void }) {
  observe(useCommandRuntime())
  return null
}

afterEach(async () => {
  cleanup()
  peers.splice(0).forEach((peer) => peer.close())
  documents.splice(0).forEach((document) => document.window.close())
  globalThis.document.documentElement.className = originalClass
  await i18n.changeLanguage(originalLocale)
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('commands from the actual native AIX catalogue', () => {
  // Catch commands targeting sibling frames, tree routing loss, or appearance changes restarting a live document.
  it.skipIf(!catalogue)(
    'runs view-local title/context commands and tree refresh without replacing the executing frames',
    async () => {
      const fetcher = installTransport()
      await i18n.changeLanguage('en-US')
      globalThis.document.documentElement.classList.remove('dark')
      const installed = createAixWorkbenchPlugins(catalogue!)
      const user = userEvent.setup()
      const tree = (plugins: readonly WorkbenchPlugin[]) => (
        <CommandProvider>
          <PluginWindowRuntime namespace="view-commands" windowId="main" plugins={plugins}>
            <PluginWorkbenchLayoutProvider>
              <ActivityChrome />
            </PluginWorkbenchLayoutProvider>
          </PluginWindowRuntime>
        </CommandProvider>
      )
      const rendered = render(tree(installed))
      await user.click(await screen.findByRole('button', { name: 'Command Resources' }))
      const left = (await screen.findByTitle('Left commands')) as globalThis.HTMLIFrameElement
      const right = (await screen.findByTitle('Right commands')) as globalThis.HTMLIFrameElement
      const tasks = (await screen.findByTitle('Command tasks')) as globalThis.HTMLIFrameElement
      const leftDocument = await startScript(left)
      const rightDocument = await startScript(right)
      await startScript(tasks)
      const result = within(leftDocument.window.document.body).getByLabelText('Command result')
      const otherResult = within(rightDocument.window.document.body).getByLabelText('Command result')
      await waitFor(() => expect(screen.getByRole('button', { name: 'Reload left' })).toBeEnabled())
      await user.click(screen.getByRole('button', { name: 'Reload left' }))
      await waitFor(() => expect(result).toHaveTextContent('refresh:1'))
      expect(otherResult).toHaveTextContent('Ready')
      fireEvent.contextMenu(screen.getByRole('toolbar', { name: 'Left commands' }))
      await user.click(await screen.findByRole('menuitem', { name: 'Clear left' }))
      await waitFor(() => expect(result).toHaveTextContent('clear:0'))
      await screen.findByRole('treeitem', { name: 'Tasks 0' })
      await user.click(screen.getByRole('button', { name: 'Reload tasks' }))
      await screen.findByRole('treeitem', { name: 'Tasks 1' })
      const input = within(leftDocument.window.document.body).getByLabelText('Query')
      await userEvent.setup({ document: leftDocument.window.document }).type(input, 'private query')
      act(() => globalThis.document.documentElement.classList.add('dark'))
      await waitFor(() => expect(screen.getByRole('button', { name: 'Reload left' })).toBeDisabled())
      expect(screen.getByTitle('Left commands')).toBe(left)
      expect(input).toHaveValue('private query')
      act(() => globalThis.document.documentElement.classList.remove('dark'))
      await waitFor(() => expect(screen.getByRole('button', { name: 'Reload left' })).toBeEnabled())
      rendered.rerender(tree([]))
      await waitFor(() => expect(screen.queryByTitle('Left commands')).not.toBeInTheDocument())
      await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(3))
    }
  )

  it.skipIf(!catalogue || process.env.A3S_CLOUD_TEST_AIX_NEGATIVE_CASE !== 'unsupported-port-version')(
    'withdraws an installed guest that answers its command handshake with an unsupported port version',
    async () => {
      const fetcher = installTransport()
      const installed = createAixWorkbenchPlugins(catalogue!)
      const user = userEvent.setup()
      const tree = (
        <CommandProvider>
          <PluginWindowRuntime namespace="view-port-version" windowId="main" plugins={installed}>
            <PluginWorkbenchLayoutProvider>
              <ActivityChrome />
            </PluginWorkbenchLayoutProvider>
          </PluginWindowRuntime>
        </CommandProvider>
      )
      render(tree)
      await user.click(await screen.findByRole('button', { name: 'Command Resources' }))
      const frame = (await screen.findByTitle('Left commands')) as globalThis.HTMLIFrameElement
      const guest = await startScript(frame)
      expect(guest.window.document.querySelector('script')?.textContent).toContain(
        "protocolVersion: 2, type: 'aix.command.ready'"
      )
      await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(1))
      expect(screen.queryByRole('button', { name: 'Reload left' })).not.toBeInTheDocument()
    }
  )

  // Catch two instances sharing handlers, menu visibility restarting a page, or a saved operation targeting a reopened page.
  it.skipIf(!catalogue)(
    'isolates mini-app operations and revokes captured commands before the same page is reopened',
    async () => {
      const fetcher = installTransport()
      await i18n.changeLanguage('en-US')
      globalThis.document.documentElement.classList.remove('dark')
      const installed = createAixWorkbenchPlugins(catalogue!)
      const page = installed[0].contributions.find((item) => item.slot === 'page' && item.title === 'Command Lab')!
      const pageKey = `${installed[0].id}/${page.id}`
      let runtime!: CommandRuntime
      const tree = (first: boolean, plugins = installed) => (
        <CommandProvider>
          <RuntimeObserver
            observe={(value) => {
              runtime = value
            }}
          />
          <PluginWindowRuntime namespace="command-instances" windowId="main" plugins={plugins}>
            {first && (
              <section aria-label="First instance">
                <PluginPage page={pageKey} />
              </section>
            )}
            <section aria-label="Second instance">
              <PluginPage page={pageKey} />
            </section>
          </PluginWindowRuntime>
        </CommandProvider>
      )
      const rendered = render(tree(true))
      const first = within(screen.getByRole('region', { name: 'First instance' }))
      const second = within(screen.getByRole('region', { name: 'Second instance' }))
      const firstFrame = (await first.findByTitle('Command Lab')) as globalThis.HTMLIFrameElement
      const secondFrame = (await second.findByTitle('Command Lab')) as globalThis.HTMLIFrameElement
      const left = await startScript(firstFrame)
      const right = await startScript(secondFrame)
      const user = userEvent.setup()
      await waitFor(() => expect(first.getByRole('button', { name: 'Reload app' })).toBeEnabled())
      const documentId = new globalThis.URLSearchParams(new globalThis.URL(firstFrame.src).hash.slice(1)).get(
        'aix-document'
      )!
      const saved = runtime.resolveCommand(extensionCommandId('aix.proof-view-commands.app', documentId, 'refresh'))!
      await user.click(first.getByRole('button', { name: 'Reload app' }))
      await waitFor(() =>
        expect(within(left.window.document.body).getByLabelText('Command result')).toHaveTextContent('refresh:1')
      )
      expect(within(right.window.document.body).getByLabelText('Command result')).toHaveTextContent('Ready')
      await act(() => i18n.changeLanguage('zh-CN'))
      await waitFor(() => expect(first.queryByRole('button', { name: 'Reload app' })).not.toBeInTheDocument())
      expect(first.getByTitle('Command Lab')).toBe(firstFrame)
      fireEvent.contextMenu(first.getByRole('toolbar', { name: 'Command Lab' }))
      await user.click(await screen.findByRole('menuitem', { name: 'Reload app' }))
      await waitFor(() =>
        expect(within(left.window.document.body).getByLabelText('Command result')).toHaveTextContent('refresh:2')
      )
      rendered.rerender(tree(false))
      await waitFor(() => expect(screen.queryByRole('region', { name: 'First instance' })).not.toBeInTheDocument())
      await expect(saved.execute()).resolves.toBe(false)
      expect(second.getByTitle('Command Lab')).toBe(secondFrame)
      await act(() => i18n.changeLanguage('en-US'))
      rendered.rerender(tree(true))
      const reopened = (await within(screen.getByRole('region', { name: 'First instance' })).findByTitle(
        'Command Lab'
      )) as globalThis.HTMLIFrameElement
      const fresh = await startScript(reopened)
      await waitFor(() =>
        expect(
          within(screen.getByRole('region', { name: 'First instance' })).getByRole('button', { name: 'Reload app' })
        ).toBeEnabled()
      )
      await expect(saved.execute()).resolves.toBe(false)
      expect(within(fresh.window.document.body).getByLabelText('Command result')).toHaveTextContent('Ready')
      rendered.rerender(tree(true, []))
      await waitFor(() => expect(screen.queryAllByTitle('Command Lab')).toHaveLength(0))
      await waitFor(() => expect(fetcher.mock.calls.filter(([, init]) => init?.method === 'DELETE')).toHaveLength(3))
    }
  )
})
