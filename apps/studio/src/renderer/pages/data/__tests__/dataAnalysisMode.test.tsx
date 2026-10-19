import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { type ComponentProps } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CORE_SIDEBAR_SHORTCUT_PROVIDERS, SidebarShortcutRegistry } from '@renderer/components/app/sidebarShortcuts'
import { createAixWorkbenchPlugins } from '@renderer/components/plugins/AixWorkbenchRuntime'
import { PluginPage } from '@renderer/components/plugins/PluginPage'
import { PluginScopeProvider } from '@renderer/components/plugins/PluginScopeProvider'
import {
  createPluginSidebarProviders,
  PLUGIN_SIDEBAR_PROVIDER_ID
} from '@renderer/components/plugins/pluginSidebarProviders'
import i18n from '@renderer/i18n/resolver'
import {
  resetHomeComposerModeForTests,
  selectHomeComposerMode,
  submitHomeComposerDraft
} from '@renderer/pages/agents/homeComposerMode'
import { WorkbenchHome, type WorkbenchSession } from '@renderer/pages/agents/WorkbenchHome'
import { installDataSqliteInvoke } from '@renderer/pages/data/__tests__/dataSqliteInvoke'
import { resetDataCatalogForTests } from '@renderer/pages/data/dataCatalog'
import { DATA_PACKAGE_ID, dataWorkbenchCatalogue } from '@renderer/pages/data/dataPackageCatalogue'
import { DataPackageSurface } from '@renderer/pages/data/DataPackageSurface'
import { executeSqlite } from '@renderer/pages/data/sqliteQuery'
import { WorkbenchPluginHost } from '@renderer/services/plugins'
import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import type { WorkbenchPlugin } from '@renderer/types/workbenchPlugin'
import { createSidebarShortcutId, type SidebarShortcutTarget } from '@shared/data/preference/preferenceTypes'

vi.mock('@a3s-lab/office/react', () => ({
  PdfViewer: () => null,
  DocumentEditor: () => null
}))
vi.mock('@a3s-lab/office/styles.css', () => ({}))
vi.mock('@a3s-lab/office/core', () => ({}))

const AMOUNTS = [10, 32]
const SQL = 'SELECT sku, amount FROM sales ORDER BY sku'

function assertDesignRoles(root: ParentNode, scope: string) {
  const page = root.querySelector('[data-ui="data.browser"], [data-ui="data.analysis"]')
  expect(page, scope).toBeInstanceOf(HTMLElement)
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

async function mountDataPage(plugins: readonly WorkbenchPlugin[]) {
  const plugin = plugins.find((item) => item.id === 'aix.a3s-data')
  if (!plugin) throw new Error('data plugin was not admitted')
  const activity = plugin.contributions.find((item) => item.slot === 'activity')
  if (!activity || activity.slot !== 'activity' || !activity.pageId) throw new Error('data activity is missing')
  const host = new WorkbenchPluginHost('data-analysis')
  await host.setPlugins([...plugins])
  const scope = await host.openScope({ id: 'main', kind: 'window' })
  const target: SidebarShortcutTarget = {
    kind: 'resource',
    locator: { providerId: PLUGIN_SIDEBAR_PROVIDER_ID, resourceId: `${plugin.id}/${activity.id}` }
  }
  const registry = new SidebarShortcutRegistry(
    createPluginSidebarProviders(CORE_SIDEBAR_SHORTCUT_PROVIDERS, [], scope.getSnapshot, (key) => key)
  )
  const provider = registry.resolve(target)
  const resource = provider ? (await provider.resolveMany([target])).get(createSidebarShortcutId(target)) : undefined
  if (!resource || resource.label !== '数据分析') throw new Error('数据分析 activity did not resolve')
  const pageKey = `${plugin.id}/${activity.pageId}`
  const view = render(
    <PluginScopeProvider scope={scope}>
      <PluginPage page={pageKey} />
    </PluginScopeProvider>
  )
  return { host, view }
}

describe('数据分析 work mode', () => {
  const directories: string[] = []
  const hosts: WorkbenchPluginHost[] = []
  let restoreInvoke = () => {}

  beforeEach(() => {
    restoreInvoke = installDataSqliteInvoke()
  })

  afterEach(async () => {
    restoreInvoke()
    resetDataCatalogForTests()
    resetHomeComposerModeForTests()
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    for (const host of hosts.splice(0)) await host.close()
    for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true })
  })

  it('queries a configured source from the work page and shows the aggregate of the rows', async () => {
    await i18n.changeLanguage('zh-CN')
    const user = userEvent.setup()
    const directory = mkdtempSync(join(tmpdir(), 'studio-analysis-'))
    directories.push(directory)
    const database = join(directory, 'shop.sqlite')
    const target = { path: database }
    await executeSqlite(target, 'CREATE TABLE sales (sku TEXT, amount INTEGER)')
    await executeSqlite(
      target,
      `INSERT INTO sales (sku, amount) VALUES ('pen', ${AMOUNTS[0]}), ('book', ${AMOUNTS[1]})`
    )

    const plugins = createAixWorkbenchPlugins(dataWorkbenchCatalogue())
    const opened = await mountDataPage(plugins)
    hosts.push(opened.host)
    await screen.findByRole('textbox', { name: '连接名称' }, { timeout: 15000 })
    const pathField = screen.getByRole('textbox', { name: '数据库文件' })
    pathField.value = database
    fireEvent.input(pathField)
    await user.type(screen.getByRole('textbox', { name: '连接名称' }), 'Shop')
    await user.click(screen.getByRole('button', { name: '保存连接' }))
    expect(await screen.findByRole('button', { name: 'sales' })).toBeInTheDocument()
    await user.type(screen.getByRole('textbox', { name: '数据源名称' }), 'Sales')
    await user.click(screen.getByRole('button', { name: '保存数据源' }))
    fireEvent.input(screen.getByRole('textbox', { name: 'SQL' }), { target: { value: SQL } })
    await user.click(screen.getByRole('button', { name: '运行' }))
    expect(await screen.findByText('pen')).toBeInTheDocument()
    expect(screen.getByText(String(AMOUNTS[0]))).toBeInTheDocument()
    expect(screen.getByText('book')).toBeInTheDocument()
    expect(screen.getByText(String(AMOUNTS[1]))).toBeInTheDocument()
    assertDesignRoles(opened.view.container, 'data-page')
    opened.view.unmount()

    publishAixCatalogue(dataWorkbenchCatalogue())
    resetHomeComposerModeForTests()
    const props: ComponentProps<typeof WorkbenchHome> = {
      sessions: [] as WorkbenchSession[],
      activeSessionId: null,
      homeActive: true,
      onNewTask: vi.fn(),
      onOpenSession: vi.fn(),
      onTogglePin: vi.fn(),
      onDeleteSession: vi.fn(),
      onRenameSession: vi.fn()
    }
    const workPage = render(<WorkbenchHome {...props} />)
    const modes = document.querySelector('[data-ui="workbench.composer.modes"]')
    const fallback = document.querySelector('[data-ui="workbench.composer.fallback"]')
    expect(modes).toBeInstanceOf(HTMLElement)
    expect(fallback).toBeInstanceOf(HTMLElement)
    expect(fallback?.contains(modes)).toBe(false)
    expect(modes!.compareDocumentPosition(fallback!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    await user.click(screen.getByRole('radio', { name: '数据分析' }))
    expect(workPage.container.querySelector('[data-ui="data.analysis"]')).toBeNull()
    expect(workPage.container.querySelector('[data-ui="data.browser"]')).toBeNull()

    const analysis = render(<DataPackageSurface kind="analysis" />)
    await screen.findByRole('button', { name: 'Sales' }, { timeout: 15000 })
    await user.click(screen.getByRole('button', { name: 'Sales' }))
    fireEvent.input(screen.getByRole('textbox', { name: 'SQL' }), { target: { value: SQL } })
    await user.click(screen.getByRole('button', { name: '运行' }))

    expect(await screen.findByText('pen')).toBeInTheDocument()
    expect(screen.getByText('book')).toBeInTheDocument()
    expect(screen.getByText(String(AMOUNTS[0]))).toBeInTheDocument()
    expect(screen.getByText(String(AMOUNTS[1]))).toBeInTheDocument()
    const figureNode = analysis.container.querySelector('[data-ui="data.analysis.figure"]')
    expect(figureNode).toBeInstanceOf(HTMLElement)
    expect(Number(figureNode?.textContent)).toBe(AMOUNTS.reduce((sum, amount) => sum + amount, 0))
    assertDesignRoles(analysis.container, 'analysis-mode')
  }, 30000)

  it('opens the analysis workspace in the plugin window and hands the homepage draft to that window', async () => {
    await i18n.changeLanguage('zh-CN')
    const plugins = createAixWorkbenchPlugins(dataWorkbenchCatalogue())
    const plugin = plugins.find((item) => item.id === 'aix.a3s-data')
    const applet = plugin?.contributions.find((item) => item.slot === 'applet')
    if (!plugin || !applet || applet.slot !== 'applet' || !applet.pageId) throw new Error('data applet is missing')
    expect(applet.title).toBe('数据分析')
    const host = new WorkbenchPluginHost('data-window')
    hosts.push(host)
    await host.setPlugins([...plugins])
    const scope = await host.openScope({ id: 'main', kind: 'window' })
    const page = render(
      <PluginScopeProvider scope={scope}>
        <PluginPage page={`${plugin.id}/${applet.pageId}`} />
      </PluginScopeProvider>
    )
    expect(await screen.findByText('还没有数据源', {}, { timeout: 15000 })).toBeInTheDocument()
    expect(page.container.querySelector('[data-ui="data.analysis"]')).toBeInstanceOf(HTMLElement)
    expect(page.container.querySelector('[data-ui="data.browser"]')).toBeNull()
    page.unmount()

    publishAixCatalogue(dataWorkbenchCatalogue())
    resetHomeComposerModeForTests()
    selectHomeComposerMode(`${DATA_PACKAGE_ID}/analysis`)
    const invoke = vi.fn(async (command: string) => {
      if (command === 'open_aix_applet_window') return 4
      if (command === 'read_aix_applet_home_deck') {
        return { delivery: 4, holdsWork: false, applied: true, seed: { content: '查询销售额' } }
      }
      throw new Error(`Unexpected native command: ${command}`)
    })
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } })
    await expect(
      submitHomeComposerDraft({ surface: 'live', text: '查询销售额', permissionMode: 'default' })
    ).resolves.toEqual({ handled: true, cleared: true })
    expect(invoke).toHaveBeenCalledWith('open_aix_applet_window', {
      slug: 'a3s-data',
      view: 'analysis',
      title: '数据分析',
      seed: JSON.stringify({ content: '查询销售额' })
    })
  }, 30000)
})
