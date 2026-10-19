import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { CORE_SIDEBAR_SHORTCUT_PROVIDERS, SidebarShortcutRegistry } from '@renderer/components/app/sidebarShortcuts'
import type { SidebarActivationGateway } from '@renderer/components/app/sidebarShortcuts/types'
import { createAixWorkbenchPlugins } from '@renderer/components/plugins/AixWorkbenchRuntime'
import { PluginPage } from '@renderer/components/plugins/PluginPage'
import { PluginScopeProvider } from '@renderer/components/plugins/PluginScopeProvider'
import {
  createPluginSidebarProviders,
  PLUGIN_SIDEBAR_PROVIDER_ID
} from '@renderer/components/plugins/pluginSidebarProviders'
import i18n from '@renderer/i18n/resolver'
import { installDataSqliteInvoke } from '@renderer/pages/data/__tests__/dataSqliteInvoke'
import { resetDataCatalogForTests } from '@renderer/pages/data/dataCatalog'
import { dataWorkbenchCatalogue } from '@renderer/pages/data/dataPackageCatalogue'
import { executeSqlite, sqlSelectAll } from '@renderer/pages/data/sqliteQuery'
import { WorkbenchPluginHost } from '@renderer/services/plugins'
import { pluginPageUrl } from '@renderer/utils/pluginNavigation'
import {
  createSidebarShortcutId,
  type SidebarShortcutItem,
  type SidebarShortcutTarget
} from '@shared/data/preference/preferenceTypes'

import { SIDEBAR_FULL_THRESHOLD } from '../constants'
import { Sidebar } from '../Sidebar'

function assertDesignRoles(root: ParentNode, scope: string) {
  const page = root.querySelector('[data-ui="data.browser"], [data-ui="data.analysis"]')
  expect(page, 'page').toBeInstanceOf(HTMLElement)
  const surface = page as HTMLElement
  const node = (name: string) => {
    const element = name === 'page' ? surface : surface.querySelector(`[data-design="${name}"]`)
    expect(element, name).toBeInstanceOf(HTMLElement)
    return element as HTMLElement
  }
  const field = node('field')
  const selected = node('selected')
  const ink = node('ink')
  const muted = node('muted')
  expect(surface.className).toContain('#ffffff')
  expect(field.className).toContain('#f5f5f5')
  expect(selected.className).toContain('#f2f3f5')
  expect(ink.className).toContain('#171717')
  expect(muted.className).toContain('#8a8a8a')
  expect(getComputedStyle(surface).backgroundColor).toBe('rgb(255, 255, 255)')
  expect(getComputedStyle(field).backgroundColor).toBe('rgb(245, 245, 245)')
  expect(getComputedStyle(selected).backgroundColor).toBe('rgb(242, 243, 245)')
  expect(getComputedStyle(ink).color).toBe('rgb(23, 23, 23)')
  expect(getComputedStyle(muted).color).toBe('rgb(138, 138, 138)')
  const icon = surface.querySelector('[data-design="icon"]')
  expect(icon).toHaveAttribute('width', '16')
  expect(icon).toHaveAttribute('height', '16')
  expect(icon).toHaveAttribute('stroke-width', '1.7')
  expect(icon).toHaveAttribute('fill', 'none')
  expect(document.querySelector('[data-v-app]')).toBeNull()
  expect(document.querySelector('link[href*="dbx"]')).toBeNull()
  expect(document.body.innerHTML.toLowerCase()).not.toContain('dbx')
  console.log(
    `DESIGN ${JSON.stringify({
      scope,
      page: getComputedStyle(surface).backgroundColor,
      field: getComputedStyle(field).backgroundColor,
      selected: getComputedStyle(selected).backgroundColor,
      ink: getComputedStyle(ink).color,
      muted: getComputedStyle(muted).color
    })}`
  )
}

async function openDataActivity() {
  const plugins = createAixWorkbenchPlugins(dataWorkbenchCatalogue())
  const plugin = plugins.find((item) => item.id === 'aix.a3s-data')
  if (!plugin) throw new Error('data plugin was not admitted')
  const activity = plugin.contributions.find((item) => item.slot === 'activity')
  if (!activity || activity.slot !== 'activity' || !activity.pageId) throw new Error('data activity is missing')
  const host = new WorkbenchPluginHost('data-activity')
  await host.setPlugins(plugins)
  const scope = await host.openScope({ id: 'main', kind: 'window' })
  const registry = new SidebarShortcutRegistry(
    createPluginSidebarProviders(CORE_SIDEBAR_SHORTCUT_PROVIDERS, [], scope.getSnapshot, (key) => key)
  )
  const target: SidebarShortcutTarget = {
    kind: 'resource',
    locator: { providerId: PLUGIN_SIDEBAR_PROVIDER_ID, resourceId: `${plugin.id}/${activity.id}` }
  }
  const item: SidebarShortcutItem = { type: 'shortcut', id: createSidebarShortcutId(target), target }
  const provider = registry.resolve(target)
  if (!provider) throw new Error('data activity provider is missing')
  const resource = (await provider.resolveMany([target])).get(item.id)
  if (!resource) throw new Error('data activity did not resolve')
  return {
    host,
    scope,
    item,
    resource,
    provider,
    pageKey: `${plugin.id}/${activity.pageId}`
  }
}

describe('数据 ActivityBar', () => {
  const directories: string[] = []
  const hosts: WorkbenchPluginHost[] = []
  let restoreInvoke = () => {}

  beforeEach(() => {
    restoreInvoke = installDataSqliteInvoke()
  })

  afterEach(async () => {
    restoreInvoke()
    resetDataCatalogForTests()
    for (const host of hosts.splice(0)) await host.close()
    for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true })
  })

  it('opens the data page from the zh-CN 数据 item', async () => {
    await i18n.changeLanguage('zh-CN')
    const user = userEvent.setup()
    const directory = mkdtempSync(join(tmpdir(), 'studio-data-page-'))
    directories.push(directory)
    const database = join(directory, 'shop.sqlite')
    await executeSqlite({ path: database }, 'CREATE TABLE sales (sku TEXT, amount INTEGER)')
    await executeSqlite({ path: database }, "INSERT INTO sales (sku, amount) VALUES ('pen', 10), ('book', 32)")

    const opened = await openDataActivity()
    hosts.push(opened.host)
    const gateway: SidebarActivationGateway = {
      openWorkspace: (destination) => {
        openedUrls.push(destination.url)
      }
    }
    const openedUrls: string[] = []
    render(
      <Sidebar
        width={SIDEBAR_FULL_THRESHOLD}
        setWidth={() => {}}
        entries={[
          {
            key: opened.item.id,
            label: opened.resource.label,
            renderIcon: opened.resource.renderIcon,
            isActive: false,
            onOpen: () => {
              void opened.provider.activate(opened.item.target, gateway)
            }
          }
        ]}
      />
    )
    await user.click(screen.getByRole('button', { name: '数据分析' }))

    expect(opened.resource.label).toBe('数据分析')
    expect(openedUrls).toEqual([pluginPageUrl(opened.pageKey)])
    expect(openedUrls).not.toContain('/app/agents')
    console.log(JSON.stringify({ label: '数据分析', opened: openedUrls }))

    const page = render(
      <PluginScopeProvider scope={opened.scope}>
        <PluginPage page={opened.pageKey} />
      </PluginScopeProvider>
    )
    await screen.findByRole('textbox', { name: '连接名称' }, { timeout: 15000 })
    expect(screen.getByRole('button', { name: '新建连接' })).toBeInTheDocument()
    expect(screen.getByRole('searchbox', { name: '搜索' })).toBeInTheDocument()
    expect(screen.getByRole('dialog', { name: '新建连接' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '保存连接' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('连接需要名称和数据库目标')
    await user.selectOptions(screen.getByRole('combobox', { name: '数据库引擎' }), 'postgres')
    expect(await screen.findByRole('textbox', { name: '连接地址' })).toBeInTheDocument()
    await user.selectOptions(screen.getByRole('combobox', { name: '数据库引擎' }), 'sqlite')
    const pathField = screen.getByRole('textbox', { name: '数据库文件' })
    pathField.value = database
    fireEvent.input(pathField)
    await user.type(screen.getByRole('textbox', { name: '连接名称' }), 'Shop')
    await user.click(screen.getByRole('button', { name: '保存连接' }))
    expect(await screen.findByRole('button', { name: 'Shop' })).toHaveAttribute('aria-current', 'page')
    await user.click(await screen.findByRole('button', { name: 'sales' }))
    expect(await screen.findByText('book')).toBeInTheDocument()
    expect(screen.getByText('32')).toBeInTheDocument()
    expect(screen.getByText('pen')).toBeInTheDocument()
    expect(screen.getByText('10')).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'SQL' })).toHaveValue(sqlSelectAll('sales'))
    expect(screen.getByRole('button', { name: 'sales' })).toHaveAttribute('aria-current', 'page')
    assertDesignRoles(page.container, 'activity-bar-data-page')
  }, 30000)
})
