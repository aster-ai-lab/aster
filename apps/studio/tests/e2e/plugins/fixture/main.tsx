import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from '@tanstack/react-router'
import i18n from 'i18next'
import { useState, useSyncExternalStore } from 'react'
import { createRoot } from 'react-dom/client'
import { initReactI18next } from 'react-i18next'

import { useSidebarShortcutRegistry } from '@renderer/components/app/sidebarShortcuts'
import { PluginActivityItems } from '@renderer/components/plugins/PluginActivityItems'
import { PluginComposerActions } from '@renderer/components/plugins/PluginComposerActions'
import { PluginScopeProvider } from '@renderer/components/plugins/PluginScopeProvider'
import { PLUGIN_SIDEBAR_PROVIDER_ID } from '@renderer/components/plugins/pluginSidebarProviders'
import { PluginSidebarRegistryProvider } from '@renderer/components/plugins/PluginSidebarRegistryProvider'
import { PluginSlot } from '@renderer/components/plugins/PluginSlot'
import { Route as PluginRoute } from '@renderer/routes/app/plugin'
import { WorkbenchPluginHost, type WorkbenchPluginScope } from '@renderer/services/plugins'
import type { PluginRenderProps, WorkbenchPlugin } from '@renderer/types/workbenchPlugin'

const routerRoot = createRootRoute()
const routerApp = createRoute({ getParentRoute: () => routerRoot, path: 'app' })
const routerPlugin = PluginRoute.update({ getParentRoute: () => routerApp, path: '/plugin', id: '/plugin' })
const routeTree = routerRoot.addChildren([routerApp.addChildren([routerPlugin])])
const NO_BUILTINS: readonly WorkbenchPlugin[] = []

const bus = new EventTarget()
const events: string[] = []
const host = new WorkbenchPluginHost('fixture-tenant')

function Page({ context }: PluginRenderProps) {
  return <h2>Review page: {context.services.workspace?.id}</h2>
}
function Sidebar({ context }: PluginRenderProps) {
  return <p>Sidebar: {context.services.workspace?.id}</p>
}
function Workspace({ context }: PluginRenderProps) {
  return <p>Workspace: {context.services.workspace?.root}</p>
}

function review(generation: number): WorkbenchPlugin {
  return {
    id: 'review',
    generation,
    scopes: ['workspace'],
    services: ['workspace'],
    contributions: [
      { id: 'activity', slot: 'activity', labelKey: 'fixture.review', pageId: 'page' },
      { id: 'page', slot: 'page', labelKey: 'fixture.review', component: Page },
      { id: 'sidebar', slot: 'sidebar', labelKey: 'fixture.review', component: Sidebar },
      { id: 'workspace', slot: 'workspace', labelKey: 'fixture.review', component: Workspace },
      {
        id: 'append',
        slot: 'composer',
        labelKey: 'fixture.append',
        invoke: async (ctx, text) => `${text} [${ctx.services.workspace?.id}:${generation}]`
      }
    ],
    activate(context) {
      const workspace = context.services.workspace!.id
      context.listen(bus, 'change', () => {
        events.push(`${workspace}:${generation}`)
      })
      context.defer(() => {
        events.push(`closed:${workspace}:${generation}`)
      })
    }
  }
}

function SpaceContent({ scope }: { scope: WorkbenchPluginScope }) {
  const [page, setPage] = useState<string>()
  const [text, setText] = useState('draft')
  const [router] = useState(() =>
    createRouter({ routeTree, history: createMemoryHistory({ initialEntries: ['/app/plugin'] }) })
  )
  const route = useSyncExternalStore(
    (notify) => router.subscribe('onResolved', notify),
    () => router.state.location.href
  )
  const registry = useSidebarShortcutRegistry()
  const open = (key: string) => {
    const activity = scope
      .getSnapshot()
      .contributions.find(
        (item) => item.contribution.slot === 'activity' && `${item.pluginId}/${item.contribution.pageId}` === key
      )
    if (!activity) return
    const target = {
      kind: 'resource' as const,
      locator: { providerId: PLUGIN_SIDEBAR_PROVIDER_ID, resourceId: activity.key }
    }
    const provider = registry.resolve(target)
    if (!provider) return
    void provider.activate(target, {
      openWorkspace: (destination) => {
        setPage(key)
        void router.navigate({ href: destination.url })
      }
    })
  }
  return (
    <section aria-label={`Workspace ${scope.options.id}`}>
      <nav aria-label="Plugin activity">
        <PluginActivityItems onOpen={open} activePage={page} />
      </nav>
      {page ? <RouterProvider router={router} /> : null}
      <aside>
        <PluginSlot slot="sidebar" />
      </aside>
      <PluginSlot slot="workspace" />
      <label>
        Draft {scope.options.id}
        <textarea value={text} onChange={(event) => setText(event.target.value)} />
      </label>
      <PluginComposerActions
        text={text}
        applyEdit={(expected, replacement) => setText((current) => (current === expected ? replacement : current))}
      />
      <output aria-label={`Route ${scope.options.id}`}>{route}</output>
    </section>
  )
}

function Space({ scope }: { scope: WorkbenchPluginScope }) {
  return (
    <PluginScopeProvider scope={scope}>
      <PluginSidebarRegistryProvider providers={[]} builtins={NO_BUILTINS}>
        <SpaceContent scope={scope} />
      </PluginSidebarRegistryProvider>
    </PluginScopeProvider>
  )
}

function App({ alpha, beta }: { alpha: WorkbenchPluginScope; beta: WorkbenchPluginScope }) {
  const [evidence, setEvidence] = useState('ready')
  const [error, setError] = useState('')
  const run = (action: () => Promise<void> | void) => {
    void Promise.resolve()
      .then(action)
      .then(
        () => setEvidence(events.join(',') || 'empty'),
        (value) => setError(String(value))
      )
  }
  return (
    <>
      <h1>Studio plugin composition</h1>
      <button onClick={() => run(() => bus.dispatchEvent(new Event('change')) && undefined)}>Emit event</button>
      <button
        onClick={() =>
          run(async () => {
            await host.setPlugins([review(2)])
          })
        }>
        Upgrade plugin
      </button>
      <button
        onClick={() =>
          run(async () => {
            await alpha.close()
          })
        }>
        Close alpha
      </button>
      <button
        onClick={() =>
          run(async () => {
            await host.setPlugins([])
          })
        }>
        Remove plugin
      </button>
      <Space scope={alpha} />
      <Space scope={beta} />
      <output aria-label="Lifecycle evidence">{evidence}</output>
      <output aria-label="Fixture error">{error}</output>
    </>
  )
}

async function main() {
  await i18n.use(initReactI18next).init({
    lng: 'en',
    resources: {
      en: {
        translation: {
          'fixture.review': 'Review',
          'fixture.append': 'Append marker',
          'error.boundary.default.message': 'Plugin view failed'
        }
      }
    }
  })
  await host.setPlugins([review(1)])
  const root = await host.openScope({ id: 'main', kind: 'window' })
  const alpha = await host.openScope({
    id: 'alpha',
    kind: 'workspace',
    parent: root,
    services: { workspace: { id: 'alpha', root: '/work/alpha' } }
  })
  const beta = await host.openScope({
    id: 'beta',
    kind: 'workspace',
    parent: root,
    services: { workspace: { id: 'beta', root: '/work/beta' } }
  })
  createRoot(document.getElementById('root')!).render(<App alpha={alpha} beta={beta} />)
}

void main()
