import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ComposerSerializedToken } from '@renderer/components/composer/tokens'
import { useStudioGoal } from '@renderer/hooks/useStudioGoal'
import { dataWorkbenchCatalogue } from '@renderer/pages/data/dataPackageCatalogue'
import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import {
  CodeWorkspacePlacement,
  HomeComposerModeBar,
  HOST_CODE_MODE_ID,
  noteHomeComposerTopic,
  requestHomeComposerSend,
  submitHomeComposerDraft,
  resetHomeComposerModeForTests,
  selectHomeComposerMode
} from '../homeComposerMode'
import { HomeComposerTaskBar } from '../HomeComposerTaskBar'
import { writeHomePluginPins } from '../homePins'
import { useWorkbenchHomeComposer } from '../WorkbenchHomeComposerExtras'
import { readTaskPermission, writeTaskPermission } from '../workbenchPermissionPostures'

vi.mock('@renderer/hooks/useStudioGoal', () => ({ useStudioGoal: vi.fn() }))

const REVISION = `sha256:${'ab'.repeat(32)}`

function catalogue(): AixWorkbenchCatalogue {
  const data = dataWorkbenchCatalogue()
  return {
    ...data,
    packages: [
      ...data.packages,
      {
        slug: 'home-writing',
        packageId: 'use/a3s/home-writing',
        generation: 1,
        revision: REVISION,
        containers: [],
        views: [],
        home: [
          {
            id: 'notice',
            shelf: 'write',
            title: '通知',
            description: '事项',
            order: 10,
            action: 'writing',
            writingType: 'notice',
            tag: '通知'
          },
          {
            id: 'report',
            shelf: 'write',
            title: '报告',
            description: '汇报',
            order: 20,
            action: 'writing',
            writingType: 'report',
            tag: '报告'
          }
        ]
      },
      {
        slug: 'home-slides',
        packageId: 'use/a3s/home-slides',
        generation: 1,
        revision: REVISION,
        containers: [],
        views: [
          {
            id: 'workbench',
            title: '幻灯片',
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
            description: '',
            icon: 'panels',
            agent: 'presenton',
            editor: 'workbench',
            scope: 'workspace',
            context: ['workspace'],
            order: 20,
            controls: [
              { id: 'n_slides', kind: 'number', label: '页数', min: 1, max: 20 },
              {
                id: 'tone',
                kind: 'select',
                label: '语气',
                initial: 'default',
                options: ['default', 'professional'],
                optionLabels: ['默认', '专业']
              },
              { id: 'include_title_slide', kind: 'toggle', label: '标题页', initial: true },
              { id: 'instructions', kind: 'text', label: '补充说明', max: 4000 }
            ]
          }
        ]
      }
    ]
  }
}

function paint() {
  publishAixCatalogue(catalogue())
  writeHomePluginPins(null, ['home-writing'])
  resetHomeComposerModeForTests()
  render(
    <>
      <HomeComposerModeBar />
      <HomeComposerTaskBar />
    </>
  )
}

/** The live home action row: permission picker, then workspace placement. */
function ShippedHomeToolsRow({ tokens = [] }: { tokens?: ComposerSerializedToken[] } = {}) {
  const home = useWorkbenchHomeComposer({
    enabled: true,
    fileCount: 0,
    text: '',
    onTextChange: () => undefined,
    permissionMode: 'default',
    tokens
  })
  return home.renderLayout({})
}

describe('homepage composer chrome', () => {
  afterEach(() => {
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    writeHomePluginPins(null, [])
    resetHomeComposerModeForTests()
  })

  it.each([
    ['request', requestHomeComposerSend],
    ['submit', submitHomeComposerDraft]
  ] as const)(
    'refuses an explicitly changed action in the %s planning preflight without clearing the draft',
    async (_name, send) => {
      publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
      resetHomeComposerModeForTests()
      selectHomeComposerMode(HOST_CODE_MODE_ID)
      const refused = await send({ surface: 'live', text: 'Keep this draft', permissionMode: 'auto', planFirst: true })
      expect(refused).toEqual({ handled: true, cleared: false })
      const supported = await send({
        surface: 'live',
        text: 'Keep this draft',
        permissionMode: 'auto',
        planFirst: false
      })
      expect(supported.code).toMatchObject({ placement: 'checkout' })
    }
  )

  it('hides a known local goal when the draft selects an explicit non-local lane', () => {
    const scope = document.createElement('div')
    document.body.append(scope)
    vi.mocked(useStudioGoal).mockReturnValue({
      status: 'known',
      sessionId: 's1',
      acpSessionId: 'acp',
      executionId: 'run',
      goal: {
        goalId: 'g',
        goalRevision: 1,
        goalSequence: 1,
        planRevision: null,
        state: 'active',
        objective: 'Local goal',
        tick: 'advance'
      }
    })
    function TaskGoalChrome({ tokens = [] }: { tokens?: ComposerSerializedToken[] }) {
      const home = useWorkbenchHomeComposer({
        enabled: true,
        fileCount: 0,
        text: '',
        tokens,
        onTextChange: () => undefined,
        projectLocked: true,
        sessionId: 's1',
        scopeElement: scope
      })
      return home.chrome
    }
    const view = render(<TaskGoalChrome />)
    expect(within(scope).getByText('Local goal')).toBeVisible()
    view.rerender(
      <TaskGoalChrome
        tokens={[
          {
            id: 'agent',
            index: 0,
            textOffset: 0,
            kind: 'reference',
            label: 'Cloud agent',
            payload: { agentRuntime: { runtime: 'cloud' } }
          }
        ]}
      />
    )
    expect(within(scope).queryByText('Local goal')).toBeNull()
    view.unmount()
    scope.remove()
    vi.mocked(useStudioGoal).mockReset()
  })

  it('disables planning for a declared non-local lane while allowing an already enabled mode to be turned off', () => {
    localStorage.clear()
    const tokens = [
      {
        id: 'agent',
        index: 0,
        textOffset: 0,
        kind: 'reference' as const,
        label: 'Cloud agent',
        payload: { agentRuntime: { runtime: 'cloud' } }
      }
    ]
    const view = render(<ShippedHomeToolsRow tokens={tokens} />)
    const toggle = screen.getByRole('switch', { name: '规划模式' })
    expect(toggle).toBeDisabled()
    expect(screen.getByText('当前不可用')).toBeVisible()
    act(() => writeTaskPermission('draft', { mode: 'default', planFirst: true }))
    expect(toggle).not.toBeDisabled()
    expect(toggle).toHaveAttribute('aria-checked', 'true')
    fireEvent.click(toggle)
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    expect(readTaskPermission('draft')?.planFirst).toBe(false)
    view.unmount()
  })

  it('shows an independent compact plan-first switch without a description row', () => {
    localStorage.clear()
    render(<ShippedHomeToolsRow />)
    const toggle = screen.getByRole('switch', { name: '规划模式' })
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    const approvalBefore = readTaskPermission('draft')?.mode ?? 'default'
    fireEvent.click(toggle)
    expect(toggle).toHaveAttribute('aria-checked', 'true')
    expect(readTaskPermission('draft')).toMatchObject({ mode: approvalBefore, planFirst: true })
    expect(screen.queryByText('下一条将先生成计划，等待审阅确认。正在运行的回合保留原模式。')).not.toBeInTheDocument()
  })

  it('left-aligns the work-mode track and lays quick tasks in one chip row', async () => {
    paint()

    const group = screen.getByRole('radiogroup', { name: '工作模式' })
    expect(group.className).not.toContain('mx-auto')
    expect(group.className).toContain('flex-nowrap')
    expect(group.className).not.toContain('flex-wrap')
    expect(group.className).toContain('rounded-full')
    expect(group.className).toContain('bg-[#f5f5f5]')

    const chat = screen.getByRole('radio', { name: '日常办公' })
    const analysis = screen.getByRole('radio', { name: '数据分析' })
    expect(chat).toHaveAttribute('aria-checked', 'true')
    expect(chat.className).toContain('bg-[#171717]')
    expect(analysis.className).not.toContain('bg-[#171717]')
    expect(analysis.className).not.toContain('bg-[#f5f5f5]')
    for (const radio of [chat, analysis]) {
      const icon = radio.querySelector('svg')
      expect(icon).toHaveAttribute('width', '16')
      expect(icon).toHaveAttribute('height', '16')
      expect(icon).toHaveAttribute('stroke-width', '1.7')
    }

    const bar = screen.getByRole('group', { name: '快捷任务' })
    expect(bar).toHaveAttribute('data-ui', 'workbench.composer.tasks')
    const taskRow = bar.querySelector('[data-ui="workbench.composer.tasks.row"]') as HTMLElement
    expect(taskRow.className).toContain('flex-nowrap')
    expect(taskRow.className).toContain('overflow-x-auto')
    const notice = within(bar).getByRole('button', { name: '通知' })
    expect(notice.className).toContain('rounded-full')
    expect(notice.className).toContain('border-[#ececec]')
    expect(notice.querySelector('svg')).toHaveAttribute('width', '16')
    expect(notice.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
    expect(within(bar).getByRole('button', { name: '报告' })).toBeInTheDocument()
    expect(within(bar).queryByRole('searchbox')).toBeNull()
    expect(bar.querySelector('[data-ui="workbench.composer.tasks.more"]')).toBeNull()

    fireEvent.click(analysis)
    expect(analysis).toHaveAttribute('aria-checked', 'true')
    expect(analysis.className).toContain('bg-[#171717]')
    expect(chat.className).not.toContain('bg-[#171717]')
    const modeTasks = document.querySelector('[data-ui="workbench.composer.tasks"]') as HTMLElement
    expect(within(modeTasks).queryByRole('button', { name: '通知' })).toBeNull()
    expect(within(modeTasks).getByRole('button', { name: '查询数据源' })).toBeInTheDocument()
    expect(within(modeTasks).getByRole('button', { name: '统计合计' })).toBeInTheDocument()
    expect(within(modeTasks).getByRole('button', { name: '列出数据表' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('radio', { name: '幻灯片' }))
    expect(document.querySelector('[data-ui="workbench.composer.controls"]')).toBeNull()
    expect(screen.queryByRole('dialog')).toBeNull()

    let pending!: Promise<{ handled: boolean; cleared: boolean }>
    act(() => {
      pending = requestHomeComposerSend({ surface: 'live', text: '季度汇报', permissionMode: 'default' })
    })
    const dialog = screen.getByRole('dialog', { name: '幻灯片' })
    expect(dialog.querySelector('[data-ui="workbench.composer.controls"]')).toBeTruthy()
    expect(dialog).toHaveTextContent('1–20')
    const pages = within(dialog).getByRole('textbox', { name: '页数' })
    expect(pages).toHaveValue('')
    fireEvent.click(within(dialog).getByRole('button', { name: '增加页数' }))
    expect(pages).toHaveValue('1')
    fireEvent.change(pages, { target: { value: '8' } })
    expect(pages).toHaveValue('8')
    const tone = dialog.querySelector('[data-control-id="tone"] [data-testid="segmented-control"]') as HTMLElement
    expect(tone).toHaveAttribute('aria-label', '语气')
    expect(tone).toHaveAttribute('data-value', 'default')
    fireEvent.click(within(tone).getByRole('button', { name: '专业' }))
    expect(dialog.querySelector('[data-control-id="tone"]')).toHaveAttribute('data-value', 'professional')
    const notes = within(dialog).getByRole('textbox', { name: '补充说明' })
    expect(notes.tagName).toBe('TEXTAREA')
    fireEvent.change(notes, { target: { value: '三条要点' } })
    fireEvent.click(within(dialog).getByRole('switch', { name: '标题页，开' }))
    expect(within(dialog).getByRole('switch', { name: '标题页，关' })).toHaveAttribute('aria-checked', 'false')
    fireEvent.click(within(dialog).getByRole('button', { name: '取消' }))
    await expect(pending).resolves.toEqual({ handled: true, cleared: false })
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.composer.controls"]')).toBeNull()
  })

  it('keeps a mode without controls on the direct send path', async () => {
    paint()
    const chat = await act(() =>
      requestHomeComposerSend({ surface: 'live', text: '整理今天的待办', permissionMode: 'default' })
    )
    expect(chat).toEqual({ handled: false, cleared: false })
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('offers the scroll arrow only while quick tasks overflow the row', () => {
    const widths = { scroll: 900, client: 400 }
    const scrollBy = vi.fn()
    const restore = [
      vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockImplementation(function (this: HTMLElement) {
        return this.dataset.ui === 'workbench.composer.tasks.row' ? widths.scroll : 0
      }),
      vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(function (this: HTMLElement) {
        return this.dataset.ui === 'workbench.composer.tasks.row' ? widths.client : 0
      })
    ]
    Object.defineProperty(HTMLElement.prototype, 'scrollBy', { configurable: true, value: scrollBy })
    try {
      paint()
      const bar = screen.getByRole('group', { name: '快捷任务' })
      const taskRow = bar.querySelector('[data-ui="workbench.composer.tasks.row"]') as HTMLElement
      fireEvent.scroll(taskRow)
      fireEvent.click(within(bar).getByRole('button', { name: '查看更多快捷任务' }))
      expect(scrollBy).toHaveBeenCalledWith({ left: 240, behavior: 'smooth' })

      taskRow.scrollLeft = 500
      fireEvent.scroll(taskRow)
      fireEvent.click(within(bar).getByRole('button', { name: '回到第一个快捷任务' }))
      expect(scrollBy).toHaveBeenLastCalledWith({ left: -500, behavior: 'smooth' })

      widths.scroll = 400
      taskRow.scrollLeft = 0
      fireEvent.scroll(taskRow)
      expect(bar.querySelector('[data-ui="workbench.composer.tasks.more"]')).toBeNull()
    } finally {
      for (const spy of restore) spy.mockRestore()
      delete (HTMLElement.prototype as { scrollBy?: unknown }).scrollBy
    }
  })

  it('keeps 日常办公 checked and uses 代码开发 only after it is chosen', async () => {
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    resetHomeComposerModeForTests()
    render(
      <>
        <HomeComposerModeBar />
        <CodeWorkspacePlacement />
      </>
    )

    const chat = screen.getByRole('radio', { name: '日常办公' })
    const code = screen.getByRole('radio', { name: '代码开发' })
    expect(chat).toHaveAttribute('aria-checked', 'true')
    expect(code).toHaveAttribute('aria-checked', 'false')
    expect(screen.queryByRole('radiogroup', { name: '工作区' })).toBeNull()

    fireEvent.click(code)
    expect(code).toHaveAttribute('aria-checked', 'true')
    expect(chat).toHaveAttribute('aria-checked', 'false')
    expect(screen.queryByRole('group', { name: '快捷任务' })).toBeNull()
    const modes = document.querySelector('[data-ui="workbench.composer.modes"]')
    expect(modes?.querySelector('[aria-label="工作区"]')).toBeNull()
    const placement = screen.getByRole('radiogroup', { name: '工作区' })
    const checkout = within(placement).getByRole('radio', { name: '当前目录' })
    const worktree = within(placement).getByRole('radio', { name: /^新工作树/ })
    expect(checkout).toHaveAttribute('aria-checked', 'true')
    expect(checkout.closest('[data-testid="tooltip"]')).toHaveAttribute('data-title', '当前目录')
    expect(checkout.className).toContain('h-8')
    expect(checkout.className).toContain('w-8')
    expect(checkout.className).toContain('bg-[#f2f3f5]')
    expect(checkout.querySelector('svg')).toBeTruthy()
    expect(checkout.textContent?.trim()).toBe('')
    expect(worktree.querySelector('svg')).toBeTruthy()
    expect(worktree.closest('[data-testid="tooltip"]')).toHaveAttribute('data-title', '新工作树')
    fireEvent.click(worktree)
    expect(worktree).toHaveAttribute('aria-checked', 'true')
    act(() => noteHomeComposerTopic('Fix login'))
    const preview = placement.querySelector('[data-code-branch]')
    expect(preview?.closest('[data-testid="tooltip"]')).toHaveAttribute('data-title', '分支')
    const generated = preview?.getAttribute('data-code-branch') ?? ''
    expect(generated).toMatch(/^code\/fix-login-[a-z0-9]{4}$/)
    expect(within(placement).queryByRole('textbox', { name: '新分支' })).toBeNull()
    expect(within(placement).getByRole('combobox', { name: '分支' })).toHaveTextContent(generated)
    expect(screen.queryByRole('dialog')).toBeNull()

    const outcome = await requestHomeComposerSend({
      surface: 'live',
      text: 'Fix login',
      permissionMode: 'default'
    })
    expect(outcome.handled).toBe(false)
    expect(outcome.code).toMatchObject({ placement: 'worktree', branch: generated, base: null, existing: false })
  })

  it('keeps the code plan and the chat text when the fallback card sends', async () => {
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    resetHomeComposerModeForTests()
    selectHomeComposerMode(HOST_CODE_MODE_ID)
    const code = await requestHomeComposerSend({
      surface: 'fallback',
      text: 'Fix login',
      permissionMode: 'default'
    })
    expect(code).toMatchObject({
      handled: true,
      cleared: false,
      code: { placement: 'checkout', branch: null, existing: false }
    })

    resetHomeComposerModeForTests()
    const chat = await requestHomeComposerSend({
      surface: 'fallback',
      text: 'hello',
      permissionMode: 'default'
    })
    expect(chat).toEqual({ handled: true, cleared: false, text: 'hello' })
  })

  it('picks an existing branch or creates one from a dialog', async () => {
    const invoke = vi.fn().mockResolvedValue({ head: 'main', branches: ['main', 'release'] })
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    resetHomeComposerModeForTests()
    render(
      <>
        <HomeComposerModeBar />
        <CodeWorkspacePlacement projectPath="/tmp/demo" />
      </>
    )
    fireEvent.click(screen.getByRole('radio', { name: '代码开发' }))
    const placement = screen.getByRole('radiogroup', { name: '工作区' })
    fireEvent.click(within(placement).getByRole('radio', { name: '新工作树' }))
    fireEvent.click(await within(placement).findByRole('combobox', { name: '分支' }))
    fireEvent.click(await screen.findByRole('option', { name: 'release' }))

    const picked = await requestHomeComposerSend({
      surface: 'live',
      text: 'Fix login',
      permissionMode: 'default'
    })
    expect(picked.code).toMatchObject({ placement: 'worktree', branch: 'release', base: null, existing: true })
    expect(invoke).toHaveBeenCalledWith('code_branches', { cwd: '/tmp/demo' })

    fireEvent.click(within(placement).getByRole('combobox', { name: '分支' }))
    fireEvent.click(await screen.findByRole('option', { name: '新建分支' }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).queryByRole('textbox', { name: '新分支' })).toBeInTheDocument()
    fireEvent.change(within(dialog).getByRole('textbox', { name: '新分支' }), {
      target: { value: 'feature/billing' }
    })
    fireEvent.click(within(dialog).getByRole('combobox', { name: '起点' }))
    fireEvent.click(await screen.findByRole('option', { name: 'release' }))
    fireEvent.click(within(dialog).getByRole('button', { name: '创建' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())

    const named = await requestHomeComposerSend({
      surface: 'live',
      text: 'Fix login',
      permissionMode: 'default'
    })
    expect(named.handled).toBe(false)
    expect(named.code).toMatchObject({
      placement: 'worktree',
      branch: 'feature/billing',
      base: 'release',
      existing: false
    })

    fireEvent.click(within(placement).getByRole('combobox', { name: '分支' }))
    fireEvent.click(await screen.findByRole('option', { name: '新建分支' }))
    const again = await screen.findByRole('dialog')
    fireEvent.change(within(again).getByRole('textbox', { name: '新分支' }), { target: { value: 'bad..name' } })
    fireEvent.click(within(again).getByRole('button', { name: '创建' }))
    expect(within(again).getByRole('alert')).toHaveTextContent('分支名')
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    const kept = await requestHomeComposerSend({
      surface: 'live',
      text: 'Fix login',
      permissionMode: 'default'
    })
    expect(kept.code).toMatchObject({ branch: 'feature/billing', existing: false })
  })

  it('shows 工作区 on the shipped tools row with the permission picker only while 代码开发 is selected', () => {
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    resetHomeComposerModeForTests()
    render(
      <>
        <HomeComposerModeBar />
        <ShippedHomeToolsRow />
      </>
    )

    const row = document.querySelector('[data-workbench-home-tools="true"]')
    expect(row).toBeInstanceOf(HTMLElement)
    const tools = row as HTMLElement
    expect(tools.querySelector('[data-workbench-chip="authorize"]')).not.toBeNull()
    expect(screen.getByRole('radio', { name: '日常办公' })).toHaveAttribute('aria-checked', 'true')
    expect(within(tools).queryByRole('radiogroup', { name: '工作区' })).toBeNull()

    fireEvent.click(screen.getByRole('radio', { name: '代码开发' }))

    expect(within(tools).getByRole('radiogroup', { name: '工作区' })).toBeInTheDocument()
    expect(tools.querySelector('[data-workbench-chip="authorize"]')).not.toBeNull()
  })

  it('keeps the dialog open and shows 这个分支已经存在 when 新建分支 confirms a branch name that is already listed', async () => {
    const invoke = vi.fn().mockResolvedValue({ head: 'main', branches: ['main', 'release'] })
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    resetHomeComposerModeForTests()
    render(
      <>
        <HomeComposerModeBar />
        <CodeWorkspacePlacement projectPath="/tmp/demo" />
      </>
    )
    fireEvent.click(screen.getByRole('radio', { name: '代码开发' }))
    const placement = screen.getByRole('radiogroup', { name: '工作区' })
    fireEvent.click(within(placement).getByRole('radio', { name: '新工作树' }))
    fireEvent.click(await within(placement).findByRole('combobox', { name: '分支' }))
    expect(await screen.findByRole('option', { name: 'release' })).toBeInTheDocument()
    const before = await requestHomeComposerSend({
      surface: 'live',
      text: 'Fix login',
      permissionMode: 'default'
    })

    fireEvent.click(screen.getByRole('option', { name: '新建分支' }))
    const dialog = await screen.findByRole('dialog')
    fireEvent.change(within(dialog).getByRole('textbox', { name: '新分支' }), { target: { value: 'release' } })
    fireEvent.click(within(dialog).getByRole('button', { name: '创建' }))

    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(within(dialog).getByRole('alert')).toHaveTextContent('这个分支已经存在')
    const after = await requestHomeComposerSend({
      surface: 'live',
      text: 'Fix login',
      permissionMode: 'default'
    })
    expect(after.code).toEqual(before.code)
  })

  it('keeps the generated branch when 新建分支 confirms a blank name', async () => {
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    resetHomeComposerModeForTests()
    render(
      <>
        <HomeComposerModeBar />
        <CodeWorkspacePlacement />
      </>
    )
    fireEvent.click(screen.getByRole('radio', { name: '代码开发' }))
    const placement = screen.getByRole('radiogroup', { name: '工作区' })
    fireEvent.click(within(placement).getByRole('radio', { name: '新工作树' }))
    act(() => noteHomeComposerTopic('Fix login'))
    const generated = placement.querySelector('[data-code-branch]')?.getAttribute('data-code-branch') ?? ''
    expect(generated).toMatch(/^code\/fix-login-[a-z0-9]{4}$/)

    fireEvent.click(within(placement).getByRole('combobox', { name: '分支' }))
    fireEvent.click(await screen.findByRole('option', { name: '新建分支' }))
    const dialog = await screen.findByRole('dialog')
    fireEvent.change(within(dialog).getByRole('textbox', { name: '新分支' }), { target: { value: '   ' } })
    fireEvent.click(within(dialog).getByRole('button', { name: '创建' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())

    const outcome = await requestHomeComposerSend({
      surface: 'live',
      text: 'Fix login',
      permissionMode: 'default'
    })
    expect(outcome.handled).toBe(false)
    expect(outcome.code).toMatchObject({
      placement: 'worktree',
      branch: generated,
      base: null,
      existing: false
    })
  })
})
