import { mockPreferenceState } from '@test-mocks/renderer/PreferenceService'
import { MockUsePreferenceUtils } from '@test-mocks/renderer/usePreference'
import { cleanup, fireEvent, render, renderHook, screen, waitFor, within } from '@testing-library/react'
import { createInstance } from 'i18next'
import type { ReactNode } from 'react'
import { I18nextProvider } from 'react-i18next'
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

import { dataApiService } from '@data/DataApiService'
import { preferenceService } from '@data/PreferenceService'
import zhCN from '@renderer/i18n/locales/zh-cn.json'
import { agentHostContext } from '@renderer/utils/alignedSessionPreface'
import type { WorktreeRecord } from '@shared/git/worktreePorcelain'

import { ArchivedTasksPage } from '../aligned/ArchivedTasksPage'
import { getCodeReviewMode } from '../aligned/codeReviewMode'
import { CodeReviewSettings } from '../aligned/CodeReviewSettings'
import { CUSTOM_INSTRUCTION_MAX_CHARS, loadCustomInstructions } from '../aligned/customInstructions'
import { CustomInstructionsPanel } from '../aligned/CustomInstructionsPanel'
import { applyAlignedSessionPreface, prefaceComposerText, useAlignedSessionPreface } from '../aligned/sessionPreface'
import { WorktreeSettings } from '../aligned/WorktreeSettings'
import { SearchResultsPage } from '../settingsSearch/SearchResultsPage'
import { setLiveQuery } from '../settingsSearch/store'

const { routerSearch } = vi.hoisted(() => ({ routerSearch: { q: '' } }))

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
  useSearch: () => routerSearch
}))

type TrashSession = {
  id: string
  name: string
  workspace: { id: string; name: string; path: string; type: 'user' | 'system' }
}

let trash: TrashSession[] = []
let latestSession: { workspace: { id: string; name: string; path: string; type: 'user' } } | null = null
let worktrees: WorktreeRecord[] = []

const userSession = (id: string, title: string, projectId: string, projectName: string): TrashSession => ({
  id,
  name: title,
  workspace: { id: projectId, name: projectName, path: '/proj', type: 'user' }
})

const looseSession = (id: string, title: string): TrashSession => ({
  id,
  name: title,
  workspace: { id: 'sys', name: '系统', path: '/sys', type: 'system' }
})

const i18n = createInstance()

beforeAll(async () => {
  await i18n.init({
    lng: 'zh-CN',
    resources: { 'zh-CN': { translation: zhCN } },
    keySeparator: false,
    interpolation: { escapeValue: false }
  })
  const prototype = HTMLElement.prototype
  if (!prototype.hasPointerCapture) prototype.hasPointerCapture = () => false
  if (!prototype.releasePointerCapture) prototype.releasePointerCapture = () => undefined
  if (!prototype.setPointerCapture) prototype.setPointerCapture = () => undefined
})

async function chooseArchiveProject(name: string) {
  const trigger = screen.getByRole('combobox', { name: '所有项目' })
  fireEvent.pointerDown(trigger, { button: 0, ctrlKey: false, pointerType: 'mouse' })
  fireEvent.click(trigger)
  const listbox = await screen.findByRole('listbox')
  expect(listbox).toHaveClass('rounded-[8px]', 'bg-white')
  const option = within(listbox).getByRole('option', { name, exact: true })
  fireEvent.pointerDown(option, { button: 0, ctrlKey: false, pointerType: 'mouse' })
  fireEvent.pointerUp(option, { button: 0, ctrlKey: false, pointerType: 'mouse' })
  fireEvent.click(option)
  await waitFor(() => expect(trigger).toHaveTextContent(name))
}

const renderZh = (node: ReactNode) => render(<I18nextProvider i18n={i18n}>{node}</I18nextProvider>)

describe('aligned settings pages', () => {
  beforeEach(() => {
    trash = []
    latestSession = null
    worktrees = []
    MockUsePreferenceUtils.resetMocks()
    vi.mocked(preferenceService.get).mockImplementation((key: string) => {
      const value = mockPreferenceState.get(key)
      return Promise.resolve(value !== undefined ? value : null)
    })
    vi.mocked(preferenceService.set).mockImplementation((key: string, value: unknown) => {
      mockPreferenceState.set(key, value)
      return Promise.resolve()
    })
    vi.mocked(dataApiService.get).mockImplementation(async (path: string) => {
      if (path === '/agent-sessions') return { items: trash }
      if (path === '/agent-sessions/latest') return { session: latestSession }
      return { id: 'mock_id', data: 'mock_data', timestamp: new Date().toISOString() }
    })
    vi.mocked(window.api.ipcApi.request).mockImplementation(
      async (
        route: string,
        input?: {
          sessionId?: string
          sessionIds?: string[]
          action?: string
          path?: string
          sinceMs?: number
          branch?: string
        }
      ) => {
        if (route === 'ai.agent.session.restore' && input?.sessionId) {
          trash = trash.filter((item) => item.id !== input.sessionId)
          return { ok: true, data: { id: input.sessionId } }
        }
        if (route === 'ai.agent.session.delete' && input?.sessionIds) {
          const ids = new Set(input.sessionIds)
          trash = trash.filter((item) => !ids.has(item.id))
          return { ok: true, data: { deletedIds: input.sessionIds } }
        }
        if (route === 'app.git.worktrees') {
          if (input?.action === 'remove') {
            const item = worktrees.find((entry) => entry.path === input.path)
            if (!item) return { ok: true, data: { items: worktrees, refused: 'missing' } }
            if (item.dirty || item.primary) return { ok: true, data: { items: worktrees, refused: 'dirty' } }
            worktrees = worktrees.filter((entry) => entry.path !== input.path)
          }
          if (input?.action === 'clear') {
            const sinceMs = input.sinceMs ?? 0
            worktrees = worktrees.filter((entry) => entry.primary || entry.dirty || entry.mtimeMs < sinceMs)
          }
          if (input?.action === 'create') {
            worktrees = [
              ...worktrees,
              { id: '/repo-topic', path: '/repo-topic', dirty: false, primary: false, mtimeMs: Date.now() }
            ]
          }
          return { ok: true, data: { items: worktrees } }
        }
        return { ok: true, data: undefined }
      }
    )
    routerSearch.q = ''
    setLiveQuery(undefined)
  })

  it('keeps the custom-instruction draft when save fails and when retry reloads', async () => {
    MockUsePreferenceUtils.setPreferenceValue('app.custom_instructions', '已保存的指令')
    vi.mocked(preferenceService.set).mockRejectedValueOnce(new Error('save failed'))
    renderZh(<CustomInstructionsPanel />)
    const draft = screen.getByRole('textbox', { name: '自定义指令' })
    await waitFor(() => expect(draft).toHaveValue('已保存的指令'))
    expect(vi.mocked(preferenceService.get)).toHaveBeenCalledWith('app.custom_instructions')
    fireEvent.change(draft, { target: { value: '已保存的指令，还没写完' } })
    fireEvent.click(screen.getByRole('button', { name: '保存' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('自定义指令保存失败，请重试。')
    expect(draft).toHaveValue('已保存的指令，还没写完')
    expect(vi.mocked(preferenceService.set)).toHaveBeenCalledWith('app.custom_instructions', '已保存的指令，还没写完')
    const loaded = await loadCustomInstructions()
    expect(
      applyAlignedSessionPreface('你好', {
        customInstructions: loaded,
        reviewMode: 'inline'
      })
    ).toContain('已保存的指令')

    fireEvent.change(draft, { target: { value: 'x'.repeat(CUSTOM_INSTRUCTION_MAX_CHARS + 1) } })
    fireEvent.click(screen.getByRole('button', { name: '保存' }))
    expect(await screen.findByText('自定义指令内容过长，请缩短后再保存。')).toBeInTheDocument()
    expect(draft).toHaveValue('x'.repeat(CUSTOM_INSTRUCTION_MAX_CHARS + 1))
  })

  it('keeps a dirty custom-instruction draft when load fails and the user retries', async () => {
    vi.mocked(preferenceService.get).mockRejectedValue(new Error('load failed'))
    renderZh(<CustomInstructionsPanel />)
    const draft = screen.getByRole('textbox', { name: '自定义指令' })
    expect(await screen.findByRole('alert')).toHaveTextContent('自定义指令加载失败。')
    fireEvent.change(draft, { target: { value: '草稿还在' } })
    fireEvent.click(screen.getByRole('button', { name: '重试' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('自定义指令加载失败。')
    expect(draft).toHaveValue('草稿还在')
  })

  it('attaches saved custom instructions and the review mode on the chat path', async () => {
    MockUsePreferenceUtils.setPreferenceValue('app.custom_instructions', '每次都带上')
    MockUsePreferenceUtils.setPreferenceValue('app.code_review.mode', 'subagent')
    const { result } = renderHook(() => useAlignedSessionPreface(), {
      wrapper: ({ children }) => <I18nextProvider i18n={i18n}>{children}</I18nextProvider>
    })
    await waitFor(() => expect(result.current.customInstructions).toBe('每次都带上'))
    expect(result.current.reviewMode).toBe('subagent')
    const sent = prefaceComposerText('你好', result.current)
    expect(sent).toContain('每次都带上')
    expect(sent).toContain('子会话')
    expect(sent).not.toContain('电脑操控')
    expect(sent).not.toContain('Agent 控制')
  })

  it('keeps agent host instructions out of the user sentence', () => {
    const host = agentHostContext({ customInstructions: '每次都带上', reviewMode: 'subagent' }, 'zh-CN')
    expect(host).toEqual({ guidelines: '每次都带上', review: 'subagent', outputLanguage: 'zh-CN' })
    expect('你好').not.toContain(host.guidelines)
  })

  it('keeps a plain user message free of permission denials', () => {
    expect(
      prefaceComposerText('你好', {
        customInstructions: '',
        reviewMode: 'inline'
      })
    ).toBe('你好')
  })

  it('exposes the code-review mode choice', () => {
    renderZh(<CodeReviewSettings />)
    const mode = screen.getByRole('combobox', { name: '审查方式' })
    expect(mode).toHaveTextContent('当前会话')
    fireEvent.click(mode)
    fireEvent.click(screen.getByRole('option', { name: '子会话' }))
    expect(mode).toHaveTextContent('子会话')
    expect(getCodeReviewMode()).toBe('subagent')
    expect(MockUsePreferenceUtils.getPreferenceValue('app.code_review.mode')).toBe('subagent')
    expect(
      prefaceComposerText('请看这段', {
        customInstructions: '',
        reviewMode: getCodeReviewMode()
      })
    ).toContain('子会话')
  })

  it('shows worktree settings and refuses to remove a dirty tree', async () => {
    renderZh(<WorktreeSettings />)
    expect(await screen.findByText('打开本地项目会话后，即可在这里管理项目中的工作树')).toBeInTheDocument()
    expect(document.getElementById('setting-worktree-refresh')).toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: '时间范围' })).toBeInTheDocument()
    cleanup()

    const now = Date.now()
    latestSession = { workspace: { id: 'w', name: 'repo', path: '/repo', type: 'user' } }
    worktrees = [
      { id: '/repo', path: '/repo', dirty: false, primary: true, mtimeMs: now },
      { id: '/repo/feature', path: '/repo/feature', dirty: true, primary: false, mtimeMs: now },
      { id: '/repo/old', path: '/repo/old', dirty: false, primary: false, mtimeMs: now - 40 * 24 * 60 * 60 * 1000 }
    ]
    renderZh(<WorktreeSettings />)
    expect(await screen.findByText('/repo/feature')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('combobox', { name: '时间范围' }))
    fireEvent.click(screen.getByRole('option', { name: '近 7 天' }))
    expect(screen.queryByText('/repo/old')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '移除' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('暂时不能移除')
    expect(screen.getByText('/repo/feature')).toBeInTheDocument()

    fireEvent.change(screen.getByRole('textbox', { name: '分支名' }), { target: { value: 'topic' } })
    fireEvent.click(screen.getByRole('button', { name: '新建工作树' }))
    await waitFor(() => {
      const actions = vi
        .mocked(window.api.ipcApi.request)
        .mock.calls.filter((call) => call[0] === 'app.git.worktrees')
        .map((call) => (call[1] as { action?: string } | undefined)?.action)
      const createAt = actions.lastIndexOf('create')
      expect(createAt).toBeGreaterThan(0)
      expect(actions[createAt - 1]).toBe('list')
    })
  })

  it('searches archived tasks and deletes one only after confirmation', async () => {
    trash = [userSession('weekly', '周报', 'proj', '项目甲'), looseSession('notes', '纪要')]
    renderZh(<ArchivedTasksPage />)

    expect(await screen.findByText('周报')).toBeInTheDocument()
    expect(screen.getByText('周报').parentElement).toHaveClass('h-9')
    expect(document.querySelector('[data-ui="settings.archive"]')).toHaveClass('rounded-[14px]')
    expect(screen.getByText('周报').parentElement?.querySelector('svg')).toHaveClass('lucide-archive')
    expect(dataApiService.get).toHaveBeenCalledWith('/agent-sessions', { query: { inTrash: true, limit: 200 } })
    const search = screen.getByRole('searchbox', { name: '搜索已归档任务' })
    expect(search.closest('[data-slot="input-group"]')).toHaveClass('h-[30px]', 'bg-[#f5f5f5]', 'border-0')
    const projectFilter = screen.getByRole('combobox', { name: '所有项目' })
    expect(projectFilter.tagName).toBe('BUTTON')
    expect(projectFilter).toHaveClass('h-[30px]', 'bg-[#f5f5f5]')
    expect(projectFilter).not.toHaveClass('h-9')
    expect(document.querySelector('[data-ui="settings.archive"] select')).toBeNull()
    fireEvent.change(search, { target: { value: '没有这个任务' } })
    expect(screen.getByText('暂无内容')).toBeInTheDocument()

    fireEvent.change(search, { target: { value: '' } })
    fireEvent.click(screen.getAllByRole('button', { name: '恢复' })[1])
    expect(window.api.ipcApi.request).not.toHaveBeenCalledWith('ai.agent.session.restore', { sessionId: 'notes' })
    const restoreDialog = screen.getByRole('dialog', { name: '恢复这个任务？' })
    fireEvent.click(within(restoreDialog).getByRole('button', { name: '取消' }))
    expect(screen.getByText('纪要')).toBeInTheDocument()

    fireEvent.click(screen.getAllByRole('button', { name: '恢复' })[1])
    fireEvent.click(
      within(screen.getByRole('dialog', { name: '恢复这个任务？' })).getByRole('button', { name: '恢复' })
    )
    await waitFor(() => expect(screen.queryByText('纪要')).not.toBeInTheDocument())
    expect(window.api.ipcApi.request).toHaveBeenCalledWith('ai.agent.session.restore', { sessionId: 'notes' })
    expect(window.api.ipcApi.request).not.toHaveBeenCalledWith(
      'ai.agent.session.delete',
      expect.objectContaining({ sessionIds: ['notes'] })
    )

    await chooseArchiveProject('项目甲')
    expect(screen.getByText('周报')).toBeInTheDocument()
    expect(screen.queryByText('纪要')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '删除', exact: true }))
    const dialog = screen.getByRole('dialog', { name: '确认删除' })
    expect(dialog).toHaveAttribute('data-tone', 'danger')
    expect(dialog).toHaveTextContent('删除后不可恢复')
    fireEvent.click(within(dialog).getByRole('button', { name: '取消' }))
    expect(screen.getByText('周报')).toBeInTheDocument()
    expect(window.api.ipcApi.request).not.toHaveBeenCalledWith(
      'ai.agent.session.delete',
      expect.objectContaining({ sessionIds: ['weekly'] })
    )

    fireEvent.click(screen.getByRole('button', { name: '删除', exact: true }))
    fireEvent.click(within(screen.getByRole('dialog', { name: '确认删除' })).getByRole('button', { name: '确认删除' }))
    await waitFor(() => expect(screen.queryByText('周报')).not.toBeInTheDocument())
    expect(screen.getByText('暂无已归档任务')).toBeInTheDocument()
    expect(window.api.ipcApi.request).toHaveBeenCalledWith('ai.agent.session.delete', {
      sessionIds: ['weekly'],
      permanent: true
    })
  })

  it('starts from the empty archive and confirms deleting a project', async () => {
    renderZh(<ArchivedTasksPage />)
    expect(await screen.findByText('暂无已归档任务')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '删除全部' })).toBeDisabled()
    cleanup()

    trash = [userSession('a', '请示', 'proj', '项目甲'), userSession('b', '通知', 'other', '项目乙')]
    renderZh(<ArchivedTasksPage />)
    expect(await screen.findByText('请示')).toBeInTheDocument()
    await chooseArchiveProject('项目甲')
    fireEvent.click(screen.getByRole('button', { name: '删除项目中的全部任务' }))
    const projectDialog = screen.getByRole('dialog', { name: '确认删除' })
    expect(projectDialog).toHaveTextContent('删除后不可恢复')
    fireEvent.click(within(projectDialog).getByRole('button', { name: '确认删除' }))
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '确认删除' })).not.toBeInTheDocument())
    await chooseArchiveProject('所有项目')
    await waitFor(() => expect(screen.queryByText('请示')).not.toBeInTheDocument())
    expect(screen.getByText('通知')).toBeInTheDocument()
    expect(window.api.ipcApi.request).toHaveBeenCalledWith('ai.agent.session.delete', {
      sessionIds: ['a'],
      permanent: true
    })
  })

  it('shows the empty settings search state when nothing matches', () => {
    routerSearch.q = 'zzzz-no-such-setting-xyz'
    renderZh(<SearchResultsPage />)
    expect(screen.getByText('没有匹配的设置')).toBeInTheDocument()
  })
})
