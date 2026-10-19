import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { SWRConfig } from 'swr'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { MemberOrgError } from '@renderer/data/memberOrgs'

import type * as AdminApi from '../orgAdminApi'
import { runLogPage, runStoppable, type RunPage, type TeamRun } from '../orgAdminApi'
import { OrgRunsPanel } from '../OrgRunsPanel'

const admin = vi.hoisted(() => ({ listTeamRuns: vi.fn(), stopTeamRun: vi.fn(), teamRunLogs: vi.fn() }))
const toast = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }))

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) => (vars ? `${key} ${JSON.stringify(vars)}` : key),
    i18n: { language: 'zh-CN' }
  })
}))
vi.mock('@renderer/services/toast', () => ({ toast }))
vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))
vi.mock('../orgAdminApi', async (importOriginal) => ({
  ...(await importOriginal<typeof AdminApi>()),
  ...admin
}))

const EXEC_A = 'aaaa0001-0000-7000-8000-00000000000a'
const EXEC_B = 'bbbb0002-0000-7000-8000-00000000000b'
const EXEC_C = 'cccc0003-0000-7000-8000-00000000000c'

function run(execution_id: string, patch: Partial<TeamRun> = {}): TeamRun {
  return {
    execution_id,
    conversation_id: 'c0nv0001-0000-4000-8000-000000000001',
    agent_asset_id: 'asset-1',
    started_by: { member_id: 'm-owner', name: '林舟' },
    started_at: '2026-10-13T01:20:00Z',
    status: 'running',
    ...patch
  }
}

function renderPanel(canStop = true) {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      <OrgRunsPanel orgId="org-1" canStop={canStop} />
    </SWRConfig>
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  admin.listTeamRuns.mockReset()
  admin.stopTeamRun.mockReset()
  admin.teamRunLogs.mockReset()
})

describe('run helpers', () => {
  it('stops only a live run and reads either trajectory shape', () => {
    for (const status of ['pending', 'running', 'awaiting_approval']) expect(runStoppable(status)).toBe(true)
    for (const status of ['cancelling', 'succeeded', 'failed', 'cancelled', 'paused', null]) {
      expect(runStoppable(status)).toBe(false)
    }
    const page = {
      records: [
        { sequence: 3, kind: 'model_output', content: { text: '你好' } },
        { sequence: 4, kind: 'execution_failed', content: { reason: 'quota' } },
        { kind: 'tool_call', content: { name: 'bash' } }
      ],
      nextCursor: 'c-2'
    }
    expect(runLogPage(page)).toEqual({
      lines: [
        { key: '3', kind: 'model_output', text: '你好' },
        { key: '4', kind: 'execution_failed', text: 'quota' },
        { key: '2', kind: 'tool_call', text: '{"name":"bash"}' }
      ],
      next: 'c-2'
    })
    expect(runLogPage({ success: true, data: { records: [], nextCursor: '' } })).toEqual({ lines: [], next: null })
    expect(runLogPage('nonsense')).toEqual({ lines: [], next: null })
  })
})

describe('OrgRunsPanel', () => {
  it('lists runs with starters, former members and runs Cloud no longer has', async () => {
    admin.listTeamRuns.mockResolvedValue({
      runs: [
        run(EXEC_A),
        run(EXEC_B, { started_by: { member_id: 'm-gone', name: null }, status: 'succeeded' }),
        run(EXEC_C, { status: null })
      ],
      next: null
    } satisfies RunPage)
    renderPanel()

    const table = await screen.findByRole('table', { name: 'org.nav.runs' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows).toHaveLength(3)
    expect(rows[0]).toHaveTextContent('org.runs.status.running')
    expect(rows[0]).toHaveTextContent('林舟')
    expect(rows[0]).toHaveTextContent('c0nv0001')
    expect(within(rows[0]).getByRole('button', { name: /org.runs.stop/ })).toBeInTheDocument()
    expect(rows[1]).toHaveTextContent('org.runs.status.succeeded')
    expect(rows[1]).toHaveTextContent('org.runs.formerMember')
    expect(within(rows[1]).queryByRole('button', { name: /org.runs.stop/ })).not.toBeInTheDocument()
    expect(rows[2]).toHaveTextContent('org.runs.status.missing')
    expect(within(rows[2]).getByRole('button', { name: /org.runs.logs/ })).toBeDisabled()
    expect(screen.getByText('org.runs.end')).toBeInTheDocument()
    expect(admin.listTeamRuns).toHaveBeenCalledWith('org-1', null)
  })

  it('offers no stop without the stop grant', async () => {
    admin.listTeamRuns.mockResolvedValue({ runs: [run(EXEC_A)], next: null } satisfies RunPage)
    renderPanel(false)
    await screen.findByRole('table', { name: 'org.nav.runs' })
    expect(screen.queryByRole('button', { name: /org.runs.stop/ })).not.toBeInTheDocument()
  })

  it('loads earlier runs after the cursor', async () => {
    const cursor = { before_at: '2026-10-13T01:20:00Z', before_id: EXEC_A }
    admin.listTeamRuns.mockImplementation(async (_org, after) =>
      after ? { runs: [run(EXEC_B)], next: null } : { runs: [run(EXEC_A)], next: cursor }
    )
    renderPanel()
    fireEvent.click(await screen.findByRole('button', { name: 'org.runs.more' }))
    await screen.findByText('org.runs.end')
    expect(admin.listTeamRuns).toHaveBeenCalledWith('org-1', cursor)
    const rows = within(screen.getByRole('table', { name: 'org.nav.runs' }))
      .getAllByRole('row')
      .slice(1)
    expect(rows).toHaveLength(2)
  })

  it('shows stopping at once and keeps the owner answer after the stop', async () => {
    let status = 'running'
    admin.listTeamRuns.mockImplementation(async () => ({ runs: [run(EXEC_A, { status })], next: null }))
    let finish: (value: unknown) => void = () => {}
    admin.stopTeamRun.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve
        })
    )
    renderPanel()
    fireEvent.click(await screen.findByRole('button', { name: `org.runs.stop ${EXEC_A.slice(0, 8)}` }))
    const confirm = await screen.findAllByRole('button', { name: 'org.runs.stop' })
    fireEvent.click(confirm[confirm.length - 1])
    await screen.findByText('org.runs.status.cancelling')
    const [org, execution, requestId] = admin.stopTeamRun.mock.calls[0]
    expect([org, execution]).toEqual(['org-1', EXEC_A])
    expect(requestId).toMatch(/^[0-9a-f-]{36}$/)
    status = 'cancelled'
    finish({ execution_id: EXEC_A, status: 'cancelling', replayed: false })
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('org.runs.stopRequested'))
    await screen.findByText('org.runs.status.cancelled')
  })

  it('puts the run back and reuses the request id when a stop fails', async () => {
    admin.listTeamRuns.mockResolvedValue({ runs: [run(EXEC_A)], next: null } satisfies RunPage)
    admin.stopTeamRun.mockRejectedValueOnce(new Error('network down'))
    admin.stopTeamRun.mockRejectedValueOnce(new MemberOrgError('agent run not found', 404))
    renderPanel()
    const stopOnce = async () => {
      fireEvent.click(await screen.findByRole('button', { name: `org.runs.stop ${EXEC_A.slice(0, 8)}` }))
      const confirm = await screen.findAllByRole('button', { name: 'org.runs.stop' })
      fireEvent.click(confirm[confirm.length - 1])
    }
    await stopOnce()
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('org.error.generic'))
    expect(await screen.findByText('org.runs.status.running')).toBeInTheDocument()
    await stopOnce()
    await waitFor(() => expect(toast.error).toHaveBeenLastCalledWith('org.runs.stopGone'))
    const [first, second] = admin.stopTeamRun.mock.calls
    expect(second[2]).toBe(first[2])
  })

  it('reads logs page by page and says when a run is gone', async () => {
    admin.listTeamRuns.mockResolvedValue({ runs: [run(EXEC_A), run(EXEC_B)], next: null } satisfies RunPage)
    admin.teamRunLogs.mockImplementation(async (_org, execution, cursor) => {
      if (execution === EXEC_B) throw new MemberOrgError('agent run not found', 404)
      return cursor
        ? { records: [{ sequence: 2, kind: 'execution_completed', content: {} }], nextCursor: null }
        : { records: [{ sequence: 1, kind: 'model_output', content: { text: '第一段' } }], nextCursor: 'c-2' }
    })
    renderPanel()
    fireEvent.click(await screen.findByRole('button', { name: `org.runs.logs ${EXEC_A.slice(0, 8)}` }))
    expect(await screen.findByText('第一段')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'org.runs.logsMore' }))
    expect(await screen.findByText('execution_completed')).toBeInTheDocument()
    expect(admin.teamRunLogs).toHaveBeenLastCalledWith('org-1', EXEC_A, 'c-2')
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByText('第一段')).not.toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: `org.runs.logs ${EXEC_B.slice(0, 8)}` }))
    expect(await screen.findByText('org.runs.logsGone')).toBeInTheDocument()
  })

  it('explains a denied page without a retry', async () => {
    admin.listTeamRuns.mockRejectedValueOnce(new MemberOrgError('team runs are unavailable', 404))
    renderPanel()
    expect(await screen.findByText('org.runs.denied')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'common.retry' })).not.toBeInTheDocument()
  })
})
