import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createAixWorkbenchPlugins, publishedAixCatalogue } from '@renderer/components/plugins/AixWorkbenchRuntime'
import { resetAppletHomeForTests } from '@renderer/services/plugins/aixAppletHome'
import { findAixWindow } from '@renderer/services/plugins/aixApplets'
import { publishAixCatalogue, readAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import {
  HomeComposerModeBar,
  readHomeComposerSendEffect,
  requestHomeComposerSend,
  resetHomeComposerModeForTests,
  selectHomeComposerMode,
  setHomeComposerControl,
  submitHomeComposerDraft
} from '../homeComposerMode'
import { PRESENTON_PACKAGE_ID, PRESENTON_SLUG, PRESENTON_TITLE, withPresentonHomepage } from '../presentonHomepage'

const REVISION = `sha256:${'ab'.repeat(32)}`

function installedApplet(): AixWorkbenchCatalogue {
  return {
    protocolVersion: 1,
    revision: '',
    packages: [
      {
        slug: 'a3s-presenton',
        packageId: PRESENTON_PACKAGE_ID,
        generation: 4,
        revision: REVISION,
        containers: [],
        views: [
          {
            id: 'workbench',
            title: 'Presenton 演示文稿',
            description: '',
            icon: 'panels',
            order: 20,
            container: null,
            point: 'studio.applets',
            presentation: 'webview'
          }
        ],
        composerModes: [
          {
            id: 'slides',
            contractVersion: 1,
            title: '幻灯片',
            description: '用主题生成演示文稿大纲',
            icon: 'panels',
            agent: 'presenton',
            editor: 'workbench',
            scope: 'workspace',
            context: ['workspace'],
            order: 20,
            prompt: '输入演示文稿的主题',
            controls: [{ id: 'n_slides', kind: 'number', label: '页数', min: 1, max: 20 }]
          }
        ]
      }
    ]
  }
}

describe('presenton homepage mode', () => {
  beforeEach(() => {
    resetHomeComposerModeForTests()
    resetAppletHomeForTests()
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
  })

  it('hides the work mode until the package is installed', async () => {
    const catalogue = withPresentonHomepage({ protocolVersion: 1, revision: '', packages: [] })
    expect(catalogue.packages).toEqual([])
    expect(publishedAixCatalogue(catalogue).packages).toEqual([])
    publishAixCatalogue(catalogue)
    render(<HomeComposerModeBar />)
    expect(screen.queryByRole('radio', { name: PRESENTON_TITLE })).toBeNull()
  })

  it('shows the work mode from the shipped ACL view after install', async () => {
    const installed = installedApplet()
    const current = installed.packages[0]
    if (!current) throw new Error('missing package')
    const catalogue = withPresentonHomepage({ ...installed, packages: [{ ...current, composerModes: [] }] })
    const pack = catalogue.packages.find((item) => item.packageId === PRESENTON_PACKAGE_ID)
    expect(pack?.composerModes?.[0]).toMatchObject({
      id: 'slides',
      title: PRESENTON_TITLE,
      editor: 'workbench',
      keptNotice: 'draft',
      task: true,
      taskPrompt:
        '调用 mcp__presenton__generate_deck，把上面的正文作为 content。要读当前文稿调用 mcp__presenton__read_deck。要改一节大纲调用 mcp__presenton__edit_outline。要改一页正文和备注调用 mcp__presenton__edit_slide。不要写 pptx，不要调用别的文稿技能。'
    })
    const plugins = createAixWorkbenchPlugins(catalogue)
    expect(pack?.views.find((view) => view.id === 'workbench')?.title).toBe(PRESENTON_TITLE)
    expect(plugins[0]?.scopes).toContain('workspace')
    expect(plugins[0]?.contributions.find((item) => item.id.startsWith('task-editor.'))?.title).toBe(PRESENTON_TITLE)
    expect(
      plugins[0]?.contributions.some((item) => item.slot === 'workspace' && item.id.startsWith('task-editor.'))
    ).toBe(true)
    expect(plugins[0]?.contributions.some((item) => item.slot === 'applet' || item.slot === 'activity')).toBe(false)
    expect(findAixWindow(catalogue, PRESENTON_SLUG, 'workbench')?.view.id).toBe('workbench')
    publishAixCatalogue(catalogue)
    render(<HomeComposerModeBar />)
    expect(screen.getByRole('radio', { name: PRESENTON_TITLE })).toHaveAttribute('aria-checked', 'false')
    fireEvent.click(screen.getByRole('radio', { name: PRESENTON_TITLE }))
    expect(document.querySelector('[data-ui="workbench.composer.controls"]')).toBeNull()
    let pending!: Promise<{ handled: boolean; cleared: boolean }>
    act(() => {
      pending = requestHomeComposerSend({ surface: 'live', text: '季度业务回顾', permissionMode: 'default' })
    })
    const dialog = screen.getByRole('dialog', { name: PRESENTON_TITLE })
    expect(within(dialog).getByRole('textbox', { name: '页数' })).toHaveValue('')
    expect(within(dialog).queryByRole('listbox')).toBeNull()
    expect(within(dialog).queryByRole('option')).toBeNull()
    const language = within(dialog).getByRole('combobox', { name: '语言' })
    expect(language).toHaveAttribute('aria-expanded', 'false')
    expect(language).toHaveTextContent('简体中文')
    expect(language).not.toHaveTextContent('萨摩亚语')
    fireEvent.click(language)
    expect(screen.getByRole('option', { name: '简体中文' })).toHaveAttribute('aria-selected', 'true')
    expect(within(dialog).queryByRole('option')).toBeNull()
    fireEvent.click(within(dialog).getByRole('button', { name: '取消' }))
    await expect(pending).resolves.toEqual({ handled: true, cleared: false })
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('submits through the catalogue the workbench runtime publishes', async () => {
    const published = publishedAixCatalogue(installedApplet())
    publishAixCatalogue(published)
    const catalogue = readAixCatalogue()
    const pack = catalogue.packages.find((item) => item.packageId === PRESENTON_PACKAGE_ID)
    expect(findAixWindow(catalogue, PRESENTON_SLUG, 'workbench')?.view.point).toBe('studio.applets')
    expect(pack?.composerModes?.[0]).toMatchObject({
      id: 'slides',
      title: PRESENTON_TITLE,
      editor: 'workbench',
      keptNotice: 'draft',
      task: true,
      taskPrompt:
        '调用 mcp__presenton__generate_deck，把上面的正文作为 content。要读当前文稿调用 mcp__presenton__read_deck。要改一节大纲调用 mcp__presenton__edit_outline。要改一页正文和备注调用 mcp__presenton__edit_slide。不要写 pptx，不要调用别的文稿技能。'
    })
    let fact = {
      delivery: 0,
      holdsWork: false,
      applied: true as boolean | null,
      seed: null as unknown,
      notice: null as string | null,
      modelRequests: 0
    }
    const invoke = vi.fn(async (command: string, args: Record<string, unknown> = {}) => {
      if (command === 'open_aix_applet_window') {
        fact = {
          ...fact,
          delivery: fact.delivery + 1,
          holdsWork: false,
          applied: true,
          seed: JSON.parse(String(args.seed))
        }
        return fact.delivery
      }
      if (command === 'read_aix_applet_home_deck') return fact.delivery > 0 ? fact : null
      return null
    })
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } })
    render(<HomeComposerModeBar />)
    fireEvent.click(screen.getByRole('radio', { name: PRESENTON_TITLE }))
    const empty = await act(() => submitHomeComposerDraft({ surface: 'live', text: '   ', permissionMode: 'default' }))
    expect(empty).toEqual({ handled: true, cleared: false })
    expect(invoke).not.toHaveBeenCalled()
    const sent = await act(() =>
      submitHomeComposerDraft({ surface: 'live', text: '季度业务回顾', permissionMode: 'default' })
    )
    expect(sent.handled).toBe(false)
    expect(sent.cleared).toBe(false)
    expect(sent.text).toContain('季度业务回顾')
    expect(sent.text).toContain('页数：自动')
    expect(sent.text).not.toContain('{"step":"outline"')
    expect(sent.deck).toMatchObject({ title: PRESENTON_TITLE, topic: '季度业务回顾' })
    expect(readHomeComposerSendEffect()).toMatchObject({
      kind: 'chat',
      modeId: 'slides',
      opened: null,
      seed: { content: '季度业务回顾', n_slides: null }
    })
    expect(invoke).not.toHaveBeenCalled()
    fact = { ...fact, holdsWork: true, applied: true }
    const again = await act(() =>
      submitHomeComposerDraft({ surface: 'live', text: '另一份草稿', permissionMode: 'default' })
    )
    expect(again.handled).toBe(false)
    expect(again.text).toContain('另一份草稿')
    expect(screen.queryByRole('status')).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
    act(() => {
      selectHomeComposerMode(`${PRESENTON_PACKAGE_ID}/slides`)
      setHomeComposerControl(`${PRESENTON_PACKAGE_ID}/slides`, 'n_slides', '99')
    })
    const refused = await act(() =>
      submitHomeComposerDraft({ surface: 'live', text: '季度业务回顾', permissionMode: 'default' })
    )
    expect(refused).toEqual({ handled: true, cleared: false })
    expect(invoke).not.toHaveBeenCalled()
  })

  it('keeps settings stable during delivery and reports a failed start without losing the topic', async () => {
    publishAixCatalogue(installedApplet())
    let refuse!: (error: Error) => void
    const invoke = vi.fn(
      () =>
        new Promise((_, reject) => {
          refuse = reject
        })
    )
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } })
    render(<HomeComposerModeBar />)
    fireEvent.click(screen.getByRole('radio', { name: '幻灯片' }))
    let pending!: Promise<{ handled: boolean; cleared: boolean }>
    act(() => {
      pending = requestHomeComposerSend({ surface: 'live', text: '季度业务回顾', permissionMode: 'default' })
    })
    const dialog = screen.getByRole('dialog', { name: '幻灯片' })
    expect(dialog.querySelector('[data-presentation-topic]')).toHaveTextContent('季度业务回顾')
    fireEvent.keyDown(within(dialog).getByRole('textbox', { name: '页数' }), { key: 'Enter', isComposing: true })
    expect(invoke).not.toHaveBeenCalled()
    fireEvent.click(within(dialog).getByRole('button', { name: '生成' }))
    expect(dialog.querySelector('fieldset')).toBeDisabled()
    expect(within(dialog).getByRole('button', { name: '生成' })).toBeDisabled()
    expect(invoke).toHaveBeenCalledTimes(1)
    await act(async () => {
      refuse(new Error('offline'))
    })
    expect(within(dialog).getByRole('alert')).toHaveTextContent('未能开始生成')
    expect(within(dialog).getByRole('textbox', { name: '页数' })).toHaveValue('')
    expect(within(dialog).getByRole('button', { name: '生成' })).toBeEnabled()
    fireEvent.click(within(dialog).getByRole('button', { name: '取消' }))
    await expect(pending).resolves.toEqual({ handled: true, cleared: false })
    expect(screen.queryByRole('alert')).toBeNull()
  })
})
