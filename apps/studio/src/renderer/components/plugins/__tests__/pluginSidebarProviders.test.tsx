import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  CORE_SIDEBAR_SHORTCUT_PROVIDERS,
  resolveSidebarShortcuts,
  SidebarShortcutRegistry,
  type SidebarShortcutProvider
} from '@renderer/components/app/sidebarShortcuts'
import { WorkbenchPluginHost, type WorkbenchContributionContext } from '@renderer/services/plugins'
import type { WorkbenchPlugin } from '@renderer/types/workbenchPlugin'
import { parsePluginPageSearch, pluginPageUrl } from '@renderer/utils/pluginNavigation'
import {
  createSidebarShortcutId,
  type SidebarShortcutItem,
  type SidebarShortcutTarget
} from '@shared/data/preference/preferenceTypes'

import { PluginPage } from '../PluginPage'
import { PluginScopeProvider } from '../PluginScopeProvider'
import { createPluginSidebarProviders, PLUGIN_SIDEBAR_PROVIDER_ID } from '../pluginSidebarProviders'
import { usePluginActivityShortcuts } from '../PluginSidebarRegistryProvider'

afterEach(cleanup)

function Page() {
  return <h2>Contributed page</h2>
}
const builtin: WorkbenchPlugin = {
  id: 'studio.plugins',
  generation: 1,
  scopes: ['window'],
  contributions: [
    { id: 'activity', slot: 'activity', labelKey: 'plugins.title', applicationId: 'plugins', pageId: 'page' },
    { id: 'page', slot: 'page', labelKey: 'plugins.title', component: Page }
  ]
}
const extension: WorkbenchPlugin = {
  id: 'review',
  generation: 1,
  scopes: ['window'],
  contributions: [
    { id: 'entry', slot: 'activity', labelKey: 'common.add', pageId: 'view' },
    { id: 'view', slot: 'page', labelKey: 'common.add', component: Page }
  ]
}
function shortcut(providerId: string, resourceId: string): SidebarShortcutItem {
  const target: SidebarShortcutTarget = { kind: 'resource', locator: { providerId, resourceId } }
  return { type: 'shortcut', id: createSidebarShortcutId(target), target }
}
async function setup(plugins = [builtin, extension]) {
  const host = new WorkbenchPluginHost('test')
  await host.setPlugins(plugins)
  const scope = await host.openScope({ id: 'main', kind: 'window' })
  const providers = createPluginSidebarProviders(
    CORE_SIDEBAR_SHORTCUT_PROVIDERS,
    [builtin],
    scope.getSnapshot,
    (key) => key
  )
  return { host, scope, registry: new SidebarShortcutRegistry(providers) }
}

describe('plugin ActivityBar contracts', () => {
  // Catch retained shortcuts reopening a hidden view, including delayed built-in navigation.
  it('rechecks host conditions for saved targets and delayed navigation', async () => {
    const plugin = {
      ...builtin,
      contributions: builtin.contributions.map((item) => ({ ...item, when: { theme: 'dark' as const } }))
    }
    const { host, scope } = await setup([plugin])
    let visibility: WorkbenchContributionContext = {
      workspace: false,
      document: false,
      conversation: false,
      run: false,
      turn: false,
      harness: false,
      theme: 'dark',
      locale: 'en-US'
    }
    let finish!: () => void
    const app = CORE_SIDEBAR_SHORTCUT_PROVIDERS.find((item) => item.id === 'core.app')!
    const deferred: SidebarShortcutProvider = {
      ...app,
      activate: (_target, gateway) =>
        new Promise<void>((resolve) => {
          finish = () => {
            gateway.openWorkspace({ url: '/app/plugins', title: 'plugins.title' })
            resolve()
          }
        })
    }
    const registry = new SidebarShortcutRegistry(
      createPluginSidebarProviders(
        [deferred],
        [plugin],
        scope.getSnapshot,
        (key) => key,
        undefined,
        () => visibility
      )
    )
    const item = shortcut('core.app', 'plugins')
    const retained = registry.resolve(item.target)!
    expect((await resolveSidebarShortcuts([item], registry))[0].status).toBe('resolved')
    const opened: string[] = []
    const gateway = {
      openWorkspace: (destination: { url: string }) => {
        opened.push(destination.url)
      }
    }
    const activation = retained.activate(item.target, gateway)
    visibility = { ...visibility, theme: 'light' }
    finish()
    await activation
    expect(opened).toEqual([])
    expect(await resolveSidebarShortcuts([item], registry)).toEqual([{ status: 'missing', shortcut: item }])
    await retained.activate(item.target, gateway)
    expect(opened).toEqual([])
    await host.close()
  })

  it('keeps built-in route adapters out of the generic route', async () => {
    const { host, scope, registry } = await setup()
    const item = shortcut(PLUGIN_SIDEBAR_PROVIDER_ID, 'studio.plugins/activity')
    expect(await resolveSidebarShortcuts([item], registry)).toEqual([{ status: 'missing', shortcut: item }])
    const view = render(
      <PluginScopeProvider scope={scope}>
        <PluginPage page="studio.plugins/page" allowBuiltins={false} />
      </PluginScopeProvider>
    )
    expect(screen.queryByRole('heading')).toBeNull()
    expect(screen.getByRole('status')).toBeTruthy()
    view.rerender(
      <PluginScopeProvider scope={scope}>
        <PluginPage page="studio.plugins/page" />
      </PluginScopeProvider>
    )
    expect(screen.getByRole('heading', { name: 'Contributed page' })).toBeTruthy()
    view.unmount()
    await host.close()
  })
  it('preserves built-in locator, icon, label and URL while disabling its withdrawn contribution', async () => {
    const { host, registry } = await setup()
    const item = shortcut('core.app', 'plugins')
    const provider = registry.resolve(item.target)!
    const entries = await resolveSidebarShortcuts([item], registry)
    expect(entries[0]).toMatchObject({ status: 'resolved', resource: { label: 'plugins.title', supportsNewTab: true } })
    const opened: string[] = []
    await provider.activate(item.target, {
      openWorkspace: (destination) => {
        opened.push(destination.url)
      }
    })
    expect(opened).toEqual(['/app/plugins'])
    await host.setPlugins([extension])
    expect(await resolveSidebarShortcuts([item], registry)).toEqual([{ status: 'missing', shortcut: item }])
    await provider.activate(item.target, {
      openWorkspace: (destination) => {
        opened.push(destination.url)
      }
    })
    expect(opened).toEqual(['/app/plugins'])
    expect(provider.isActive?.(item.target, { url: '/app/plugins' })).toBe(false)
    await host.close()
  })

  it('opens an activity plugin across the content column and closes a docked rail', async () => {
    const chart: WorkbenchPlugin = {
      id: 'aix.a3s-finance',
      generation: 1,
      scopes: ['window'],
      contributions: [
        { id: 'page.market', slot: 'page', labelKey: 'plugins.title', title: '金融分析', component: Page },
        { id: 'sidebar.market', slot: 'sidebar', labelKey: 'plugins.title', title: '金融分析', component: Page },
        {
          id: 'activity.container.market',
          slot: 'activity',
          labelKey: 'plugins.title',
          title: '金融分析',
          pageId: 'page.market',
          sidebarId: 'sidebar.market',
          icon: () => null
        }
      ]
    }
    const { host, scope } = await setup([builtin, chart])
    const containers = {
      active: null as string | null,
      reveal(key: string) {
        this.active = this.active === key ? null : key
      }
    }
    const workspace = { url: '/app/agents' }
    const registry = new SidebarShortcutRegistry(
      createPluginSidebarProviders(
        CORE_SIDEBAR_SHORTCUT_PROVIDERS,
        [builtin],
        scope.getSnapshot,
        (key) => key,
        containers,
        undefined,
        () => workspace.url
      )
    )
    const target = shortcut(PLUGIN_SIDEBAR_PROVIDER_ID, 'aix.a3s-finance/activity.container.market').target
    const provider = registry.resolve(target)!
    const opened: string[] = []
    const gateway = {
      openWorkspace: (destination: { url: string }) => {
        opened.push(destination.url)
      }
    }
    containers.active = 'aix.a3s-finance/sidebar.market'
    await provider.activate(target, gateway)
    expect(opened).toEqual([pluginPageUrl('aix.a3s-finance/page.market')])
    expect(containers.active).toBeNull()
    const plugins = shortcut('core.app', 'plugins').target
    const apps = registry.resolve(plugins)!
    expect(provider.isActive?.(target, { url: opened[0] })).toBe(true)
    expect(apps.isActive?.(plugins, { url: opened[0] })).toBe(false)
    expect(apps.isActive?.(plugins, { url: '/app/plugins' })).toBe(true)
    await host.close()
  })

  it('opens additional plugins through namespaced pages and rejects unregistered activations', async () => {
    const { host, registry } = await setup()
    const target = shortcut(PLUGIN_SIDEBAR_PROVIDER_ID, 'review/entry').target
    const provider = registry.resolve(target)!
    const opened: string[] = []
    await provider.activate(target, {
      openWorkspace: (destination) => {
        opened.push(destination.url)
        expect(destination.matchesCurrent?.('/app/plugin?extra=1&page=review%2Fview')).toBe(true)
      }
    })
    expect(opened).toEqual([pluginPageUrl('review/view')])
    expect(provider.validate({ ...target, activationId: 'execute' })).toBe(false)
    expect(provider.validate(shortcut(PLUGIN_SIDEBAR_PROVIDER_ID, '../escape').target)).toBe(false)
    expect(parsePluginPageSearch({ page: 'review/view' })).toEqual({ page: 'review/view' })
    expect(parsePluginPageSearch({ page: ['review/view'] })).toEqual({})
    await host.close()
  })

  it('fences delegated asynchronous navigation after the owning generation is withdrawn', async () => {
    const { host, scope } = await setup()
    let finish!: () => void
    const app = CORE_SIDEBAR_SHORTCUT_PROVIDERS.find((item) => item.id === 'core.app')!
    const deferred: SidebarShortcutProvider = {
      ...app,
      activate: (_target, gateway) =>
        new Promise<void>((resolve) => {
          finish = () => {
            gateway.openWorkspace({ url: '/app/plugins', title: 'plugins.title' })
            resolve()
          }
        })
    }
    const registry = new SidebarShortcutRegistry(
      createPluginSidebarProviders([deferred], [builtin], scope.getSnapshot, (key) => key)
    )
    const item = shortcut('core.app', 'plugins')
    const opened: string[] = []
    const activation = registry.resolve(item.target)!.activate(item.target, {
      openWorkspace: (destination) => {
        opened.push(destination.url)
      }
    })
    await host.setPlugins([])
    finish()
    await activation
    expect(opened).toEqual([])
    await host.close()
  })

  it('adds unpinned contributions once and retains configured missing entries', async () => {
    const { host, scope } = await setup()
    const pinned = shortcut(PLUGIN_SIDEBAR_PROVIDER_ID, 'review/entry')
    const configured = [shortcut('core.app', 'plugins'), pinned]
    function Menu() {
      return (
        <ul>
          {usePluginActivityShortcuts(configured).map((item) => (
            <li key={item.id}>{item.target.locator.resourceId}</li>
          ))}
        </ul>
      )
    }
    render(
      <PluginScopeProvider scope={scope}>
        <Menu />
      </PluginScopeProvider>
    )
    expect(screen.getAllByRole('listitem')).toHaveLength(2)
    await act(async () => {
      await host.setPlugins([])
    })
    expect(screen.getAllByRole('listitem')).toHaveLength(2)
    await act(async () => {
      await host.close()
    })
  })

  it('removes an automatic ActivityBar item and the already-open page together', async () => {
    const { host, scope } = await setup([extension])
    function Menu() {
      const items = usePluginActivityShortcuts([])
      return (
        <>
          {items.map((item) => (
            <button key={item.id} type="button">
              {item.target.locator.resourceId}
            </button>
          ))}
          <PluginPage page="review/view" />
        </>
      )
    }
    render(
      <PluginScopeProvider scope={scope}>
        <Menu />
      </PluginScopeProvider>
    )
    fireEvent.click(screen.getByRole('button', { name: 'review/entry' }))
    expect(screen.getByRole('heading', { name: 'Contributed page' })).toBeTruthy()
    await act(async () => {
      await host.setPlugins([])
    })
    expect(screen.queryByRole('button', { name: 'review/entry' })).toBeNull()
    expect(screen.queryByRole('heading', { name: 'Contributed page' })).toBeNull()
    expect(screen.getByRole('status')).toBeTruthy()
    await act(async () => {
      await host.close()
    })
  })
})
