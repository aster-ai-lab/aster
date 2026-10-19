import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { SWRConfig } from 'swr'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { MemberOrgError } from '@renderer/data/memberOrgs'

import type * as AdminApi from '../orgAdminApi'
import { AUDIT_CSV_COLUMNS, auditCsv, type AuditEntry, type AuditPage } from '../orgAdminApi'
import { OrgAuditPanel } from '../OrgAuditPanel'

const admin = vi.hoisted(() => ({ listTeamAudit: vi.fn() }))
const toast = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }))
const files = vi.hoisted(() => ({ save: vi.fn() }))

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) => {
      const shown = Object.fromEntries(Object.entries(vars ?? {}).filter(([name]) => name !== 'defaultValue'))
      return Object.keys(shown).length ? `${key} ${JSON.stringify(shown)}` : key
    },
    i18n: { language: 'zh-CN' }
  })
}))
vi.mock('@renderer/services/toast', () => ({ toast }))
vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))
vi.mock('../orgAdminApi', async (importOriginal) => ({
  ...(await importOriginal<typeof AdminApi>()),
  ...admin
}))

function entry(id: string, patch: Partial<AuditEntry> = {}): AuditEntry {
  return {
    id,
    occurred_at: '2026-10-13T01:20:00Z',
    action: 'member.status.changed',
    kind: 'member',
    actor: { kind: 'member', member_id: 'm-owner', name: '林舟' },
    target: { kind: 'member', id: 'u-1', name: '周岚', phone: null, repository: null, change_request: null },
    from: { kind: 'status', code: 'active', seats: null, id: null, label: null },
    to: { kind: 'status', code: 'disabled', seats: null, id: null, label: null },
    ...patch
  }
}

const members = [
  { member_id: 'm-owner', name: '林舟' },
  { member_id: null, name: '未绑定' }
]

function renderPanel() {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      <OrgAuditPanel orgId="org-1" members={members} />
    </SWRConfig>
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  admin.listTeamAudit.mockReset()
  vi.stubGlobal('api', { ...window.api, file: { ...window.api.file, save: files.save } })
})

describe('audit CSV', () => {
  it('keeps fixed columns, quotes every field and defuses formulas', () => {
    const csv = auditCsv([
      entry('a', {
        actor: { kind: 'member', member_id: 'm-1', name: '=HYPERLINK("x")' },
        target: { kind: 'repository', id: null, name: null, phone: null, repository: 'team/web', change_request: 11 },
        from: null,
        to: null,
        action: 'change_request.merged',
        kind: 'repository'
      }),
      entry('b', {
        actor: { kind: 'operator', member_id: null, name: null },
        action: 'org.subscription.changed',
        kind: 'organization',
        target: { kind: 'organization', id: null, name: null, phone: null, repository: null, change_request: null },
        from: { kind: 'plan', code: 'builder', seats: 3, id: null, label: null },
        to: { kind: 'plan', code: 'team', seats: 10, id: null, label: null }
      })
    ])
    const lines = csv.trimEnd().split('\r\n')
    expect(lines[0]).toBe(AUDIT_CSV_COLUMNS.join(','))
    expect(lines).toHaveLength(3)
    expect(lines[1]).toContain(`"'=HYPERLINK(""x"")"`)
    expect(lines[1]).toContain('"team/web","11"')
    expect(lines[2]).toContain('"operator","",""')
    expect(lines[2]).toMatch(/"builder:3","team:10"$/)
  })

  it('writes a team capability change as mode and count', () => {
    const csv = auditCsv([
      entry('a', {
        action: 'org.capabilities.changed',
        kind: 'organization',
        target: { kind: 'organization', id: null, name: null, phone: null, repository: null, change_request: null },
        from: null,
        to: { kind: 'capabilities', code: 'restricted', seats: null, count: 2, id: null, label: null }
      })
    ])
    expect(csv.trimEnd().split('\r\n')[1]).toMatch(/"","restricted:2"$/)
  })
})

describe('OrgAuditPanel', () => {
  it('lists entries newest first with names, the platform and former members', async () => {
    admin.listTeamAudit.mockResolvedValue({
      entries: [
        entry('a'),
        entry('b', {
          actor: { kind: 'operator', member_id: null, name: null },
          action: 'org.subscription.changed',
          kind: 'organization',
          target: { kind: 'organization', id: null, name: null, phone: null, repository: null, change_request: null },
          from: { kind: 'plan', code: 'builder', seats: 3, id: null, label: null },
          to: { kind: 'plan', code: 'team', seats: 10, id: null, label: null }
        }),
        entry('d', {
          action: 'org.capabilities.changed',
          kind: 'organization',
          target: { kind: 'organization', id: null, name: null, phone: null, repository: null, change_request: null },
          from: { kind: 'capabilities', code: 'open', seats: null, count: 0, id: null, label: null },
          to: { kind: 'capabilities', code: 'restricted', seats: null, count: 2, id: null, label: null }
        }),
        entry('c', {
          actor: { kind: 'member', member_id: 'm-gone', name: null },
          action: 'invite.created',
          kind: 'invitation',
          target: {
            kind: 'invitation',
            id: null,
            name: null,
            phone: '138****9402',
            repository: null,
            change_request: null
          },
          from: null,
          to: null
        })
      ],
      next: null
    } satisfies AuditPage)
    renderPanel()

    const table = await screen.findByRole('table', { name: 'org.nav.audit' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows).toHaveLength(4)
    expect(rows[0]).toHaveTextContent('林舟')
    expect(rows[0]).toHaveTextContent('org.audit.action.member_status_changed')
    expect(rows[0]).toHaveTextContent('周岚')
    expect(rows[0]).toHaveTextContent('org.status.active → org.status.disabled')
    expect(rows[1]).toHaveTextContent('org.audit.operator')
    expect(rows[1]).toHaveTextContent('org.audit.team')
    expect(rows[1]).toHaveTextContent('org.audit.plan {"plan":"org.plan.name.builder","count":3} → org.audit.plan')
    expect(rows[2]).toHaveTextContent('org.audit.action.org_capabilities_changed')
    expect(rows[2]).toHaveTextContent('org.audit.capabilitiesOpen → org.audit.capabilitiesRestricted {"count":2}')
    expect(rows[3]).toHaveTextContent('org.audit.formerMember')
    expect(rows[3]).toHaveTextContent('138****9402')
    expect(screen.getByText('org.audit.end')).toBeInTheDocument()
    expect(admin.listTeamAudit).toHaveBeenCalledWith('org-1', { kind: null, actor: null, days: 30 }, null)

    files.save.mockResolvedValueOnce('/tmp/out/team-audit.csv')
    fireEvent.click(screen.getByRole('button', { name: /org.audit.export/ }))
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('org.audit.exported {"count":4}'))
    const [name, body, options] = files.save.mock.calls[0]
    expect(name).toMatch(/^team-audit-org-1-\d{4}-\d{2}-\d{2}\.csv$/)
    expect(body.startsWith(`\uFEFF${AUDIT_CSV_COLUMNS.join(',')}\r\n`)).toBe(true)
    expect(body.trimEnd().split('\r\n')).toHaveLength(5)
    expect(options).toEqual({ filters: [{ name: 'CSV', extensions: ['csv'] }] })

    files.save.mockResolvedValueOnce(null)
    fireEvent.click(screen.getByRole('button', { name: /org.audit.export/ }))
    await waitFor(() => expect(files.save).toHaveBeenCalledTimes(2))
    files.save.mockRejectedValueOnce(new Error('workspace folder 500'))
    fireEvent.click(screen.getByRole('button', { name: /org.audit.export/ }))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('org.audit.exportFailed'))
    expect(toast.success).toHaveBeenCalledTimes(1)
  })

  it('reads the owner again on a return visit and on refresh', async () => {
    const cache = new Map()
    const visit = () =>
      render(
        <SWRConfig value={{ provider: () => cache, dedupingInterval: 0 }}>
          <OrgAuditPanel orgId="org-1" members={members} />
        </SWRConfig>
      )
    admin.listTeamAudit.mockResolvedValueOnce({ entries: [entry('a')], next: null } satisfies AuditPage)
    const first = visit()
    expect(await screen.findByText('org.audit.loaded {"count":1}')).toBeInTheDocument()
    first.unmount()

    admin.listTeamAudit.mockResolvedValueOnce({ entries: [entry('b'), entry('a')], next: null } satisfies AuditPage)
    visit()
    expect(await screen.findByText('org.audit.loaded {"count":2}')).toBeInTheDocument()

    admin.listTeamAudit.mockResolvedValueOnce({
      entries: [entry('c'), entry('b'), entry('a')],
      next: null
    } satisfies AuditPage)
    fireEvent.click(screen.getByRole('button', { name: 'common.refresh' }))
    expect(await screen.findByText('org.audit.loaded {"count":3}')).toBeInTheDocument()
  })

  it('reads the owner again when a filter comes back to one it already read', async () => {
    const repository = { kind: 'repository' as const, action: 'repository.created' }
    const rows = [entry('r1', repository)]
    admin.listTeamAudit.mockImplementation(async (_org, filter: { kind: string | null }) => ({
      entries: filter.kind === 'repository' ? [...rows] : [],
      next: null
    }))
    const kind = async (option: string) => {
      fireEvent.click(screen.getByRole('button', { name: 'org.audit.filter.kind' }))
      fireEvent.click(await screen.findByRole('option', { name: option }))
    }
    renderPanel()
    expect(await screen.findByText('org.audit.empty')).toBeInTheDocument()

    await kind('org.audit.kind.repository')
    expect(await screen.findByText('org.audit.loaded {"count":1}')).toBeInTheDocument()
    await kind('org.audit.kind.invitation')
    expect(await screen.findByText('org.audit.emptyFiltered')).toBeInTheDocument()

    rows.unshift(entry('r2', repository))
    await kind('org.audit.kind.repository')
    expect(await screen.findByText('org.audit.loaded {"count":2}')).toBeInTheDocument()
  })

  it('loads earlier pages after the cursor', async () => {
    const cursor = { before_at: '2026-10-12T00:00:00.000000Z', before_id: 'a' }
    admin.listTeamAudit.mockImplementation(async (_org, _filter, after) =>
      after
        ? ({ entries: [entry('b')], next: null } satisfies AuditPage)
        : ({ entries: [entry('a')], next: cursor } satisfies AuditPage)
    )
    renderPanel()

    fireEvent.click(await screen.findByRole('button', { name: 'org.audit.more' }))
    await waitFor(() => expect(screen.getByText('org.audit.loaded {"count":2}')).toBeInTheDocument())
    expect(admin.listTeamAudit).toHaveBeenCalledWith('org-1', { kind: null, actor: null, days: 30 }, cursor)
    expect(screen.getByText('org.audit.end')).toBeInTheDocument()
  })

  it('asks the owner again when a filter changes and says when nothing matches', async () => {
    admin.listTeamAudit.mockResolvedValue({ entries: [], next: null } satisfies AuditPage)
    renderPanel()
    expect(await screen.findByText('org.audit.empty')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /org.audit.export/ })).toBeDisabled()

    fireEvent.click(screen.getByRole('button', { name: 'org.audit.filter.kind' }))
    fireEvent.click(await screen.findByRole('option', { name: 'org.audit.kind.repository' }))
    expect(await screen.findByText('org.audit.emptyFiltered')).toBeInTheDocument()
    expect(admin.listTeamAudit).toHaveBeenLastCalledWith('org-1', { kind: 'repository', actor: null, days: 30 }, null)

    fireEvent.click(screen.getByRole('button', { name: 'org.audit.filter.actor' }))
    const options = await screen.findAllByRole('option')
    expect(options.map((option) => option.textContent)).toEqual(['林舟'])
    fireEvent.click(options[0])
    await waitFor(() =>
      expect(admin.listTeamAudit).toHaveBeenLastCalledWith(
        'org-1',
        { kind: 'repository', actor: 'm-owner', days: 30 },
        null
      )
    )
    fireEvent.click(screen.getByRole('button', { name: 'org.audit.filter.days' }))
    fireEvent.click(await screen.findByRole('option', { name: 'org.audit.window {"count":7}' }))
    await waitFor(() =>
      expect(admin.listTeamAudit).toHaveBeenLastCalledWith(
        'org-1',
        { kind: 'repository', actor: 'm-owner', days: 7 },
        null
      )
    )

    fireEvent.click(screen.getAllByRole('button', { name: 'org.audit.clearFilters' })[0])
    expect(await screen.findByText('org.audit.empty')).toBeInTheDocument()
    expect(admin.listTeamAudit).toHaveBeenLastCalledWith('org-1', { kind: null, actor: null, days: 7 }, null)
    expect(screen.queryByRole('button', { name: 'org.audit.clearFilters' })).not.toBeInTheDocument()
    expect(screen.queryByText('org.audit.emptyFiltered')).not.toBeInTheDocument()
  })

  it('tells a narrow scope it cannot read the feed, and offers retry only for failures', async () => {
    admin.listTeamAudit.mockRejectedValueOnce(new MemberOrgError('team audit is unavailable', 404))
    const first = renderPanel()
    expect(await screen.findByText('org.audit.denied')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'common.retry' })).not.toBeInTheDocument()
    first.unmount()

    admin.listTeamAudit
      .mockRejectedValueOnce(new MemberOrgError('boom', 500))
      .mockResolvedValueOnce({ entries: [entry('a')], next: null } satisfies AuditPage)
    renderPanel()
    fireEvent.click(await screen.findByRole('button', { name: 'common.retry' }))
    expect(await screen.findByRole('table', { name: 'org.nav.audit' })).toBeInTheDocument()
  })
})
