import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import i18next from 'i18next'
import { useState } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { preferenceService } from '@data/PreferenceService'
import { createAixWorkbenchPlugins } from '@renderer/components/plugins/AixWorkbenchRuntime'
import { usePluginActivityShortcuts } from '@renderer/components/plugins/PluginSidebarRegistryProvider'
import { PluginWindowRuntime } from '@renderer/components/plugins/PluginWindowRuntime'
import { dataWorkbenchCatalogue } from '@renderer/pages/data/dataPackageCatalogue'
import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import { createSidebarShortcutTarget, SIDEBAR_SHORTCUT_PROVIDER_IDS } from '@renderer/utils/sidebar'
import {
  createSidebarShortcutId,
  type SidebarShortcutItem,
  type SidebarShortcutTarget
} from '@shared/data/preference/preferenceTypes'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import {
  admittedHomeComposerModes,
  HomeComposerModeBar,
  readHomeComposerSendEffect,
  resetHomeComposerModeForTests,
  submitHomeComposerDraft
} from '../../agents/homeComposerMode'
import type { CapabilityEntry } from '../../capabilities/capabilitiesData'
import { PluginConfigDialog } from '../PluginConfigDialog'

vi.unmock('@a3s-lab/ui')
vi.unmock('@data/hooks/usePreference')

const revision = `sha256:${'a'.repeat(64)}`
const catalogue: AixWorkbenchCatalogue = {
  protocolVersion: 1,
  revision,
  packages: [
    {
      slug: 'a3s-digital-twin',
      packageId: 'use/a3s/digital-twin',
      generation: 1,
      revision,
      containers: [{ id: 'twin', title: '数字孪生', icon: 'panels', order: 70, location: 'activitybar' }],
      views: [
        {
          id: 'workbench',
          title: '数字孪生',
          description: '',
          icon: 'panels',
          order: 1,
          point: 'studio.sidebar',
          container: 'twin'
        },
        {
          id: 'editor',
          title: '数字孪生',
          description: '',
          icon: 'panels',
          order: 1,
          point: 'studio.editor',
          container: null
        }
      ],
      composerModes: [
        {
          id: 'build',
          contractVersion: 1,
          title: '数字孪生',
          description: '',
          icon: 'panels',
          agent: 'builder',
          editor: 'editor',
          scope: 'workspace',
          context: ['workspace'],
          order: 70
        }
      ]
    }
  ]
}
const entry: CapabilityEntry = {
  slug: 'a3s-digital-twin',
  name: '数字孪生引擎',
  kind: 'plugin',
  category: 'creative',
  description: '',
  color: '#475569',
  featured: false,
  installed: true,
  config: [],
  surfaces: ['ui', 'agent', 'skill']
}
const plugins = createAixWorkbenchPlugins(catalogue)

function appShortcut(resourceId: string): SidebarShortcutItem {
  const target: SidebarShortcutTarget = createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, resourceId)
  return { type: 'shortcut', id: createSidebarShortcutId(target), target }
}

function Rail({ configured = [] }: { configured?: readonly SidebarShortcutItem[] }) {
  const items = usePluginActivityShortcuts(configured)
  return (
    <nav aria-label="Plugin menus">
      {items.map((item) => (
        <span key={item.id}>{item.fallbackLabel}</span>
      ))}
    </nav>
  )
}

function Settings() {
  const [open, setOpen] = useState(false)
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Plugin settings
      </button>
      <PluginConfigDialog entry={open ? entry : null} hasActivityBar onOpenChange={setOpen} />
    </>
  )
}

describe('installed plugin experience', () => {
  beforeEach(async () => {
    await i18next.changeLanguage('zh-CN')
    await preferenceService.set('plugins.activity_bar', {})
    await preferenceService.set('ui.sidebar_shortcut', [])
    resetHomeComposerModeForTests()
    publishAixCatalogue(catalogue)
  })
  afterEach(() => {
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    Object.assign(globalThis, { __TAURI_INTERNALS__: undefined })
  })

  it('keeps 数据分析 on the homepage and adds 数据 to the ActivityBar only when the member asks', async () => {
    const plugins = createAixWorkbenchPlugins(dataWorkbenchCatalogue())
    publishAixCatalogue(dataWorkbenchCatalogue())
    function Order() {
      const items = usePluginActivityShortcuts([appShortcut('knowledge'), appShortcut('files')])
      return (
        <>
          <ol>
            {items.map((item) => (
              <li key={item.id}>{item.fallbackLabel ?? item.target.locator.resourceId}</li>
            ))}
          </ol>
          <HomeComposerModeBar />
        </>
      )
    }
    render(
      <PluginWindowRuntime namespace="data-rail" windowId="main" plugins={plugins}>
        <Order />
      </PluginWindowRuntime>
    )
    await waitFor(() => {
      expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual(['knowledge', 'files'])
    })
    expect(screen.getByRole('radio', { name: '数据分析' })).toBeVisible()
    await act(() => preferenceService.set('plugins.activity_bar', { 'aix.a3s-data': true }))
    await waitFor(() => {
      expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual([
        'knowledge',
        '数据分析',
        'files'
      ])
    })
    expect(screen.getByRole('radio', { name: '数据分析' })).toBeVisible()
    await act(() => preferenceService.set('plugins.activity_bar', { 'aix.a3s-data': false }))
    await waitFor(() => {
      expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual(['knowledge', 'files'])
    })
    expect(screen.getByRole('radio', { name: '数据分析' })).toBeVisible()
  })

  // A menu opt-out must not withdraw the editor or homepage mode, and must converge across consumers.
  it('adds and removes the menu through plugin settings while keeping its homepage mode', async () => {
    const user = userEvent.setup()
    render(
      <PluginWindowRuntime namespace="plugin-choice" windowId="main" plugins={plugins}>
        <Rail />
        <HomeComposerModeBar />
        <Settings />
      </PluginWindowRuntime>
    )
    expect(await screen.findByRole('radio', { name: '数字孪生' })).toBeVisible()
    expect(screen.getByRole('navigation', { name: 'Plugin menus' })).toBeEmptyDOMElement()
    await user.click(screen.getByRole('button', { name: 'Plugin settings' }))
    const toggle = screen.getByRole('switch', { name: '在活动栏中显示' })
    expect(toggle).not.toBeChecked()
    await user.click(toggle)
    await user.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() =>
      expect(preferenceService.getCachedValue('plugins.activity_bar')).toEqual({ 'aix.a3s-digital-twin': true })
    )
    await waitFor(() => expect(screen.getByRole('navigation', { name: 'Plugin menus' })).toHaveTextContent('数字孪生'))
    await act(() => preferenceService.set('plugins.activity_bar', { 'aix.a3s-digital-twin': false }))
    await waitFor(() => expect(screen.getByRole('navigation', { name: 'Plugin menus' })).toBeEmptyDOMElement())
    expect(screen.getByRole('radio', { name: '数字孪生' })).toBeVisible()
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  // Real profiles can contain the forward-compatible ActivityBar order marker beside shortcuts.
  it('opens settings with stored navigation markers and keeps an unconfirmed choice editable', async () => {
    await preferenceService.set('ui.sidebar_shortcut', [
      { type: 'activity-order', id: 'custom' } as unknown as SidebarShortcutItem
    ])
    const user = userEvent.setup()
    render(<Settings />)
    await user.click(screen.getByRole('button', { name: 'Plugin settings' }))
    await user.click(screen.getByRole('switch', { name: '在活动栏中显示' }))
    vi.mocked(preferenceService.set).mockRejectedValueOnce(new Error('Preference write refused'))
    await user.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => expect(screen.getByRole('button', { name: '保存' })).toBeEnabled())
    expect(screen.getByRole('switch', { name: '在活动栏中显示' })).toBeChecked()
    expect(preferenceService.getCachedValue('plugins.activity_bar')).toEqual({})
    await user.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(preferenceService.getCachedValue('plugins.activity_bar')).toEqual({ 'aix.a3s-digital-twin': true })
  })

  // Generic editor modes previously disappeared or failed Presenton's mandatory slide controls.
  it('automatically admits the installed editor mode and hands the draft to its system window', async () => {
    const user = userEvent.setup()
    const invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'open_aix_applet_window') return 7
      if (command === 'read_aix_applet_home_deck')
        return { delivery: 7, holdsWork: false, applied: true, seed: { content: '创建仓库场景' } }
      throw new Error(`Unexpected native command: ${command} ${String(args)}`)
    })
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } })
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    render(<HomeComposerModeBar />)
    expect(screen.queryByRole('radio', { name: '数字孪生' })).toBeNull()
    act(() => publishAixCatalogue(catalogue))
    await user.click(screen.getByRole('radio', { name: '数字孪生' }))
    await act(async () => {
      expect(
        await submitHomeComposerDraft({ surface: 'live', text: '创建仓库场景', permissionMode: 'default' })
      ).toEqual({ handled: true, cleared: true })
    })
    expect(invoke).toHaveBeenCalledWith('open_aix_applet_window', {
      slug: entry.slug,
      view: 'editor',
      title: '数字孪生',
      seed: JSON.stringify({ content: '创建仓库场景' })
    })
    expect(readHomeComposerSendEffect()).toMatchObject({
      kind: 'seed',
      seed: { content: '创建仓库场景' },
      research: false,
      autoReview: false
    })
    invoke.mockImplementationOnce(async () => ({
      delivery: 7,
      holdsWork: true,
      applied: true,
      seed: { content: '创建仓库场景' }
    }))
    await act(async () => {
      expect(
        await submitHomeComposerDraft({ surface: 'live', text: '保留这份需求', permissionMode: 'default' })
      ).toEqual({ handled: true, cleared: false })
    })
    expect(screen.getByRole('status')).toHaveTextContent('编辑器中已有内容，首页草稿未写入')
    expect(invoke.mock.calls.filter(([command]) => command === 'open_aix_applet_window')).toHaveLength(1)
    act(() => publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] }))
    expect(screen.queryByRole('radio', { name: '数字孪生' })).toBeNull()
    await act(async () => {
      expect(
        await submitHomeComposerDraft({ surface: 'live', text: '尚未发送的草稿', permissionMode: 'default' })
      ).toEqual({ handled: false, cleared: false })
    })
    expect(readHomeComposerSendEffect()).toMatchObject({ kind: 'chat', modeId: null, seed: null })
  })

  it('keeps modes with the same local name bound to their own package', () => {
    const other = { ...catalogue.packages[0], slug: 'other-twin', packageId: 'use/proof/other-twin' }
    const modes = admittedHomeComposerModes({ ...catalogue, packages: [catalogue.packages[0], other] })
    expect(new Set(modes.map((mode) => mode.key)).size).toBe(2)
  })
})
