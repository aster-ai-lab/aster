import { fireEvent, render, screen } from '@testing-library/react'
import i18n from 'i18next'
import { I18nextProvider, initReactI18next } from 'react-i18next'
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

import zhCN from '@renderer/i18n/locales/zh-cn.json'
import { ExecutionTraceSettings } from '@renderer/pages/settings/ExecutionTraceSettings/ExecutionTraceSettings'
import { RL_TRAJECTORY_SCHEMA } from '@renderer/pages/settings/ExecutionTraceSettings/extractExecutionTrace'
import { TRACE_PAGE_SIZE } from '@renderer/pages/settings/ExecutionTraceSettings/traceBrowse'
import { Route } from '@renderer/routes/settings/traces'

const loadMock = vi.hoisted(() => vi.fn())
const navigate = vi.hoisted(() => vi.fn())

vi.mock('@renderer/pages/settings/ExecutionTraceSettings/loadLocalSessionRecords', () => ({
  loadLocalSessionRecords: loadMock
}))

vi.mock('@tanstack/react-router', async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>()
  return { ...actual, useNavigate: () => navigate }
})

const catalog = zhCN as Record<string, string>
const i18nInstance = i18n.createInstance()

beforeAll(async () => {
  await i18nInstance.use(initReactI18next).init({
    lng: 'zh-CN',
    fallbackLng: 'zh-CN',
    resources: { 'zh-CN': { translation: zhCN } },
    keySeparator: false,
    interpolation: { escapeValue: false },
    react: { useSuspense: false },
    initImmediate: false
  })
})

beforeEach(() => {
  localStorage.clear()
  sessionStorage.clear()
  loadMock.mockReset()
  navigate.mockReset()
})

function endedSession(sessionId: string, name: string, success: boolean) {
  return {
    sessionId,
    name,
    updatedAt: '2026-10-07T00:00:00.000Z',
    events: [
      {
        messageId: `${sessionId}-end`,
        record: {
          schema: RL_TRAJECTORY_SCHEMA,
          sequence: 1,
          timestamp_ms: 1,
          event_type: 'execution_end' as const,
          session_id: sessionId,
          mode: 'on' as const,
          context: {},
          payload: success
            ? { success: true }
            : {
                success: false,
                error_message: { text: 'plugin selection requires a matching native launcher binding' }
              }
        }
      }
    ]
  }
}

function renderPage() {
  expect(Route.options.component).toBeTypeOf('function')
  return render(
    <I18nextProvider i18n={i18nInstance}>
      <ExecutionTraceSettings />
    </I18nextProvider>
  )
}

const records = [
  {
    id: 'session-1',
    name: '样本会话',
    updatedAt: '2026-10-07T00:00:00.000Z',
    messages: [
      {
        id: 'message-1',
        createdAt: '2026-10-07T00:00:00.000Z',
        updatedAt: '2026-10-07T00:00:01.000Z',
        parts: [
          {
            type: 'dynamic-tool',
            toolName: 'bash',
            toolCallId: 'call-1',
            state: 'output-available',
            input: { command: 'printf ok' }
          }
        ]
      }
    ]
  }
]

describe('ExecutionTraceSettings', () => {
  it('extracts a session trace and skips that record on the next scan', async () => {
    loadMock.mockResolvedValue(records)
    renderPage()

    expect(screen.getByRole('button', { name: '分析轨迹' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: catalog['settings.traces.export'] })).toBeDisabled()
    expect(screen.getByText(catalog['settings.traces.empty'])).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '分析轨迹' }))
    expect(await screen.findByText('样本会话')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /样本会话/ }))
    expect(screen.getAllByText('bash')).toHaveLength(2)
    expect(screen.getByText('printf ok')).toBeInTheDocument()
    expect(screen.getAllByText(catalog['settings.traces.states.done']).length).toBeGreaterThan(0)
    expect(document.querySelector('[data-event-type="tool_call"]')?.getAttribute('data-schema')).toBe(
      'a3s.rl_trajectory.v1'
    )
    expect(document.querySelector('[data-event-type="tool_call"]')?.getAttribute('data-decision')).toBe('invoked')
    expect(document.querySelector('[data-event-type="tool_result"]')?.getAttribute('data-decision')).toBe('succeeded')
    expect(document.querySelector('[data-event-type="llm_response"]')?.getAttribute('data-decision')).toBe('tool_use')
    expect(document.querySelector('[data-event-type="execution_end"]')?.getAttribute('data-decision')).toBe('finished')
    expect(screen.queryByText(catalog['settings.traces.decision.invoked'])).not.toBeInTheDocument()
    expect(screen.queryByText(catalog['settings.traces.decision.succeeded'])).not.toBeInTheDocument()
    expect(screen.queryByText(catalog['settings.traces.decision.tool_use'])).not.toBeInTheDocument()
    expect(screen.queryByText(catalog['settings.traces.decision.finished'])).not.toBeInTheDocument()
    expect(screen.queryByText(catalog['settings.traces.signal.decision'])).not.toBeInTheDocument()
    expect(document.querySelectorAll('[data-ui="settings.traces.event.icon"]')).toHaveLength(5)
    fireEvent.change(screen.getByRole('searchbox', { name: '搜索轨迹' }), {
      target: { value: catalog['settings.traces.decision.invoked'] }
    })
    expect(document.querySelectorAll('[data-ui="settings.traces.event"]').length).toBeGreaterThan(0)
    expect(screen.queryByText(catalog['settings.traces.decision.invoked'])).not.toBeInTheDocument()
    fireEvent.change(screen.getByRole('searchbox', { name: '搜索轨迹' }), { target: { value: '' } })
    fireEvent.click(screen.getAllByRole('button', { name: catalog['settings.traces.open'] })[0])
    expect(navigate).toHaveBeenCalledWith({ to: '/app/agents', search: { sessionId: 'session-1' } })
    expect(screen.getByText('1–1 / 1')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '下一页' })).toBeDisabled()
    expect(document.querySelector('[data-ui="settings.traces.status"]')?.textContent).toContain('新提取 5 条')

    fireEvent.change(screen.getByRole('searchbox', { name: '搜索轨迹' }), { target: { value: '没有这个' } })
    expect(screen.getByText(catalog['settings.traces.search.empty'])).toBeInTheDocument()
    expect(document.querySelectorAll('[data-ui="settings.traces.event"]')).toHaveLength(0)
    fireEvent.change(screen.getByRole('searchbox', { name: '搜索轨迹' }), { target: { value: '' } })
    expect(document.querySelectorAll('[data-ui="settings.traces.event"]')).toHaveLength(5)

    fireEvent.click(screen.getByRole('button', { name: '分析轨迹' }))
    expect(await screen.findByText(catalog['settings.traces.scan.none'])).toBeInTheDocument()
    expect(document.querySelectorAll('[data-ui="settings.traces.event"]')).toHaveLength(5)
    expect(loadMock).toHaveBeenCalledTimes(2)
  })

  it('ignores the older clipped step book', () => {
    localStorage.setItem(
      'a3s.studio.execution-trace.v1',
      JSON.stringify({
        version: 1,
        seen: {},
        sessions: [{ sessionId: 'old', name: '旧轨迹', updatedAt: '2026-10-07T00:00:00.000Z', steps: [] }]
      })
    )
    localStorage.setItem(
      'a3s.studio.rl-trajectory.v1',
      JSON.stringify({
        version: 1,
        schema: 'a3s.rl_trajectory.v1',
        seen: {},
        sessions: [{ sessionId: 'bad', name: '坏账本', updatedAt: '2026-10-07T00:00:00.000Z', events: [] }]
      })
    )
    renderPage()
    expect(screen.queryByText('旧轨迹')).not.toBeInTheDocument()
    expect(screen.queryByText('坏账本')).not.toBeInTheDocument()
    expect(screen.getByText(catalog['settings.traces.empty'])).toBeInTheDocument()
  })

  it('pages sessions and finds a note on another page', () => {
    const sessions = Array.from({ length: TRACE_PAGE_SIZE + 1 }, (_, index) => ({
      sessionId: `session-${index}`,
      name: `会话-${index}`,
      updatedAt: '2026-10-07T00:00:00.000Z',
      events: [
        {
          messageId: `message-${index}`,
          record: {
            schema: RL_TRAJECTORY_SCHEMA,
            sequence: 1,
            timestamp_ms: index,
            event_type: 'tool_call' as const,
            session_id: `session-${index}`,
            mode: 'on' as const,
            context: {},
            payload: {
              tool: `tool-${index}`,
              signals:
                index === TRACE_PAGE_SIZE
                  ? { decision: 'denied', duration_ms: 1500, feedback: '请改用只读接口' }
                  : { decision: 'invoked', duration_ms: 12 }
            }
          }
        }
      ]
    }))
    localStorage.setItem(
      'a3s.studio.rl-trajectory.v1',
      JSON.stringify({
        version: 2,
        schema: RL_TRAJECTORY_SCHEMA,
        signalRevision: 1,
        seen: {},
        sessions
      })
    )
    renderPage()
    expect(document.querySelectorAll('[data-ui="settings.traces.session"]')).toHaveLength(TRACE_PAGE_SIZE)
    expect(document.querySelectorAll('[data-ui="settings.traces.event"]')).toHaveLength(0)
    expect(screen.queryByText('请改用只读接口')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '上一页' })).toBeDisabled()

    fireEvent.click(screen.getByRole('button', { name: /会话-0/ }))
    expect(screen.getByText('12 毫秒')).toBeInTheDocument()
    expect(screen.queryByText(catalog['settings.traces.signal.decision'])).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '下一页' }))
    expect(screen.getByText('会话-20')).toBeInTheDocument()
    expect(screen.queryByText('请改用只读接口')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /会话-20/ }))
    expect(screen.getByText('请改用只读接口')).toBeInTheDocument()
    expect(screen.getByText('1.5 秒')).toBeInTheDocument()
    expect(screen.getByText(catalog['settings.traces.decision.denied'])).toBeInTheDocument()
    expect(screen.getByText(catalog['settings.traces.signal.decision'])).toBeInTheDocument()
    expect(document.querySelectorAll('[data-ui="settings.traces.session"]')).toHaveLength(1)
    expect(document.querySelector('[data-ui="settings.traces.pager"]')?.getAttribute('data-page')).toBe('2')

    fireEvent.change(screen.getByRole('searchbox', { name: '搜索轨迹' }), { target: { value: '只读接口' } })
    expect(document.querySelectorAll('[data-ui="settings.traces.event"]')).toHaveLength(1)
    expect(document.querySelector('[data-ui="settings.traces.pager"]')?.getAttribute('data-page')).toBe('1')
    expect(screen.getByText('请改用只读接口')).toBeInTheDocument()
  })

  it('filters to failed sessions and opens one without expanding the timeline', () => {
    localStorage.setItem(
      'a3s.studio.rl-trajectory.v1',
      JSON.stringify({
        version: 2,
        schema: RL_TRAJECTORY_SCHEMA,
        signalRevision: 1,
        seen: {},
        sessions: [endedSession('ok', '已完成', true), endedSession('bad', '已失败', false)]
      })
    )
    renderPage()
    expect(screen.getByText('已完成')).toBeInTheDocument()
    expect(screen.getByText('已失败')).toBeInTheDocument()
    expect(screen.getByText('plugin selection requires a matching native launcher binding')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /^失败/ }))
    expect(screen.queryByText('已完成')).not.toBeInTheDocument()
    expect(screen.getByText('已失败')).toBeInTheDocument()
    expect(document.querySelectorAll('[data-ui="settings.traces.event"]')).toHaveLength(0)

    fireEvent.click(screen.getByRole('button', { name: catalog['settings.traces.open'] }))
    expect(navigate).toHaveBeenCalledWith({ to: '/app/agents', search: { sessionId: 'bad' } })
  })

  it('exports the whole book and limits the page to one session', () => {
    sessionStorage.setItem('a3s.studio.traces.focus', 'keep')
    localStorage.setItem(
      'a3s.studio.rl-trajectory.v1',
      JSON.stringify({
        version: 2,
        schema: RL_TRAJECTORY_SCHEMA,
        signalRevision: 1,
        seen: {},
        sessions: [
          {
            sessionId: 'keep',
            name: '保留',
            updatedAt: '2026-10-07T00:00:02.000Z',
            messageWatermark: '2026-10-07T00:00:02.000Z',
            messageCount: 1,
            events: [
              {
                messageId: 'keep-1',
                record: {
                  schema: RL_TRAJECTORY_SCHEMA,
                  sequence: 1,
                  timestamp_ms: 1,
                  event_type: 'tool_call',
                  session_id: 'keep',
                  mode: 'on',
                  context: {},
                  payload: { tool: 'tool-keep', signals: { decision: 'invoked' } }
                }
              }
            ]
          },
          {
            sessionId: 'drop',
            name: '隐藏',
            updatedAt: '2026-10-07T00:00:01.000Z',
            messageWatermark: '2026-10-07T00:00:01.000Z',
            messageCount: 1,
            events: [
              {
                messageId: 'drop-1',
                record: {
                  schema: RL_TRAJECTORY_SCHEMA,
                  sequence: 1,
                  timestamp_ms: 1,
                  event_type: 'tool_call',
                  session_id: 'drop',
                  mode: 'on',
                  context: {},
                  payload: { tool: 'tool-drop', signals: { decision: 'approved', duration_ms: 12 } }
                }
              }
            ]
          }
        ]
      })
    )
    renderPage()
    expect(screen.getByText('保留')).toBeInTheDocument()
    expect(screen.queryByText('隐藏')).not.toBeInTheDocument()
    expect(screen.getByText(catalog['settings.traces.focus'])).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: catalog['settings.traces.export'] }))
    expect(document.querySelector('[data-ui="settings.traces.export.status"]')).toHaveAttribute('data-measured', '1')
    expect(screen.getByText('文件含 1 条实测耗时，可作为训练样本。')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: catalog['settings.traces.focus.clear'] }))
    expect(screen.getByText('隐藏')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /隐藏/ }))
    expect(screen.getByText(catalog['settings.traces.decision.approved'])).toBeInTheDocument()
    expect(sessionStorage.getItem('a3s.studio.traces.focus')).toBeNull()
    expect(screen.queryByText(catalog['settings.traces.focus'])).not.toBeInTheDocument()
  })

  it('shows two lines of a long record until the text is opened', () => {
    const raw = `<| DSML || calls>\nfor d in ${'~/Library/Application '.repeat(12)}`
    localStorage.setItem(
      'a3s.studio.rl-trajectory.v1',
      JSON.stringify({
        version: 2,
        schema: RL_TRAJECTORY_SCHEMA,
        signalRevision: 1,
        seen: {},
        sessions: [
          {
            sessionId: 'session-long',
            name: '长记录',
            updatedAt: '2026-10-07T00:00:00.000Z',
            events: [
              {
                messageId: 'message-long',
                record: {
                  schema: RL_TRAJECTORY_SCHEMA,
                  sequence: 1,
                  timestamp_ms: 1,
                  event_type: 'llm_response',
                  session_id: 'session-long',
                  mode: 'on',
                  context: {},
                  payload: { response_text: { text: raw } }
                }
              }
            ]
          }
        ]
      })
    )
    renderPage()
    fireEvent.click(screen.getByRole('button', { name: /长记录/ }))
    expect(screen.queryByText(/DSML/)).not.toBeInTheDocument()
    expect(screen.getByText(/for d in/)).toBeInTheDocument()
    expect(document.querySelector('[data-ui="settings.traces.detail"]')).toHaveAttribute('data-open', 'false')
    fireEvent.click(screen.getByRole('button', { name: '展开原文' }))
    expect(document.querySelector('[data-ui="settings.traces.detail"]')).toHaveAttribute('data-open', 'true')
    expect(screen.getByText(/DSML/)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '收起原文' }))
    expect(screen.queryByText(/DSML/)).not.toBeInTheDocument()
  })

  it('reports a failed local scan', async () => {
    loadMock.mockRejectedValue(new Error('offline'))
    renderPage()
    fireEvent.click(screen.getByRole('button', { name: '分析轨迹' }))
    expect(await screen.findByText(catalog['settings.traces.scan.failed'])).toBeInTheDocument()
  })
})
