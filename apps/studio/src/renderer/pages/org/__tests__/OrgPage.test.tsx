import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { SWRConfig } from 'swr'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { TabsProvider } from '@renderer/components/layout/TabsProvider'
import { PluginScopeProvider } from '@renderer/components/plugins/PluginScopeProvider'
import type * as MemberOrgApi from '@renderer/data/memberOrgs'
import { WorkbenchPluginHost } from '@renderer/services/plugins'
import { BUILTIN_WORKBENCH_PLUGINS } from '@renderer/windows/builtinWorkbenchPlugins'

import type * as OrgApi from '../orgApi'
import type { OrgDepartment, OrgMemberRow, PendingApplicant } from '../orgApi'
import OrgPage from '../OrgPage'

const api = vi.hoisted(() => ({
  listOrgMembers: vi.fn(),
  listOrgDepartments: vi.fn(),
  listJoinRequests: vi.fn(),
  listOrgRoles: vi.fn(),
  listOrgPositions: vi.fn(),
  approveJoinRequest: vi.fn(),
  rejectJoinRequest: vi.fn(),
  removeOrgMember: vi.fn(),
  setOrgMemberRole: vi.fn(),
  setOrgMemberPosition: vi.fn(),
  setOrgMemberDepartment: vi.fn(),
  createOrgDepartment: vi.fn(),
  renameOrgDepartment: vi.fn(),
  deleteOrgDepartment: vi.fn(),
  createOrgPosition: vi.fn(),
  renameOrgPosition: vi.fn(),
  deleteOrgPosition: vi.fn(),
  createOrgRole: vi.fn(),
  updateOrgRole: vi.fn(),
  deleteOrgRole: vi.fn(),
  fetchJoinInvite: vi.fn(),
  configureJoinInvite: vi.fn(),
  readOrgOrigin: vi.fn(),
  updateOrgProfile: vi.fn(),
  leaveOrg: vi.fn(),
  dissolveOrg: vi.fn(),
  listTransferCandidates: vi.fn(),
  transferOrg: vi.fn()
}))

const account = vi.hoisted(() => ({
  readCenterId: vi.fn(),
  subscribeCenter: vi.fn(),
  listMemberOrgs: vi.fn(),
  readMemberAccount: vi.fn(),
  writeCenterId: vi.fn()
}))

const translate = vi.hoisted(() => (key: string, vars?: { name?: unknown; page?: number }) => {
  if (key === 'collection.page') return `${key} ${vars?.page}`
  if (typeof vars?.name === 'string' && (key === 'org.members.editTitle' || key === 'org.members.approveConfirm')) {
    return `${key}:${vars.name}`
  }
  return key
})

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: translate,
    i18n: { language: 'zh-CN' }
  })
}))

vi.mock('@renderer/services/toast', () => ({
  toast: { success: vi.fn(), error: vi.fn() }
}))

vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))

vi.mock('@renderer/pages/settings/UsageSettings', () => ({
  UsageSettings: ({ scope }: { scope?: { departmentId?: string; userId?: string } }) => (
    <div data-ui="org.usage.charts">{scope?.departmentId ?? scope?.userId ?? 'team'}</div>
  )
}))

vi.mock('@renderer/data/memberOrgs', async (importOriginal) => ({
  ...(await importOriginal<typeof MemberOrgApi>()),
  readCenterId: account.readCenterId,
  subscribeCenter: account.subscribeCenter,
  listMemberOrgs: account.listMemberOrgs,
  readMemberAccount: account.readMemberAccount,
  writeCenterId: account.writeCenterId
}))

vi.mock('../orgApi', async (importOriginal) => ({
  ...(await importOriginal<typeof OrgApi>()),
  ...api
}))

function department(id: string, parent_id: string | null, name: string): OrgDepartment {
  return { id, parent_id, name, sort_order: 0 }
}

function member(id: string, patch: Partial<OrgMemberRow> = {}): OrgMemberRow {
  return {
    id,
    name: id,
    email: `${id}@example.com`,
    phone: null,
    status: 'active',
    admin: false,
    creator: false,
    department_id: 'dept-root',
    joined_at: '2026-01-02T00:00:00Z',
    position_id: null,
    position_name: null,
    role_code: 'member',
    role_name: '成员',
    role_built_in: true,
    can_remove: true,
    can_assign_position: true,
    can_assign_department: true,
    assignable_department_ids: ['dept-root', 'dept-rd', 'dept-all'],
    can_assign_role: true,
    assignable_role_ids: ['role-1', 'role-2', 'role-9', 'role-admin', 'role-member'],
    ...patch
  }
}

function renderPage() {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      <OrgPage initialSection="members" />
    </SWRConfig>
  )
}

describe('OrgPage', () => {
  beforeEach(() => {
    api.listOrgMembers.mockReset().mockResolvedValue([])
    api.listOrgDepartments.mockReset().mockResolvedValue([])
    api.listJoinRequests.mockReset().mockResolvedValue([])
    api.listOrgRoles.mockReset().mockResolvedValue([])
    api.listOrgPositions.mockReset().mockResolvedValue([])
    api.createOrgPosition.mockReset().mockResolvedValue({ id: 'pos-1', name: '产品经理', sort_order: 0 })
    api.renameOrgPosition.mockReset().mockResolvedValue({ id: 'pos-1', name: '产品经理', sort_order: 0 })
    api.deleteOrgPosition.mockReset().mockResolvedValue({ ok: true })
    api.createOrgRole.mockReset().mockResolvedValue({
      id: 'role-9',
      code: 'custom_9',
      name: '财务',
      description: '',
      built_in: false,
      sort_order: 100,
      member_count: 0,
      data_scope: { kind: 'all', department_ids: [] },
      rules: []
    })
    api.updateOrgRole.mockReset().mockResolvedValue({
      policy_revision: 'b'.repeat(64),
      id: 'role-9',
      code: 'custom_9',
      name: '财务',
      description: '',
      built_in: false,
      sort_order: 100,
      member_count: 0,
      data_scope: { kind: 'all', department_ids: [] },
      rules: []
    })
    api.deleteOrgRole.mockReset().mockResolvedValue({ ok: true })
    api.approveJoinRequest.mockReset().mockResolvedValue({ ok: true })
    account.readMemberAccount.mockReset().mockResolvedValue({ displayName: '张三', phone: '13800000000' })
    account.subscribeCenter.mockReset().mockReturnValue(() => undefined)
    account.listMemberOrgs.mockReset().mockResolvedValue([])
  })

  it('opens the built-in workbench with schedule in the team sidebar', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([{ id: 'org-1', name: '通友科技', logo: '', membership: 'member' }])
    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <OrgPage />
      </SWRConfig>
    )
    expect(await screen.findByRole('heading', { name: 'org.nav.workbench' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'org.nav.knowledge' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'org.nav.schedule' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.nav.mail' })).not.toBeInTheDocument()
    expect(screen.getByText('org.nav.group.work')).toBeInTheDocument()
    expect(screen.getByText('org.nav.group.organization')).toBeInTheDocument()
    expect(screen.getByText('org.nav.group.admin')).toBeInTheDocument()
    expect(screen.getAllByRole('button', { name: 'workbench.applet.approval' }).length).toBeGreaterThan(0)
    expect(screen.queryByRole('heading', { name: 'org.nav.members' })).not.toBeInTheDocument()
  })

  it('opens schedule inside the team sidebar and keeps mail off that menu', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([{ id: 'org-1', name: '通友科技', logo: '', membership: 'member' }])
    const host = new WorkbenchPluginHost('team-navigation')
    await host.setPlugins(BUILTIN_WORKBENCH_PLUGINS)
    const scope = await host.openScope({ id: 'main', kind: 'window' })
    const tree = render(
      <PluginScopeProvider scope={scope}>
        <TabsProvider>
          <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
            <OrgPage />
          </SWRConfig>
        </TabsProvider>
      </PluginScopeProvider>
    )
    try {
      await screen.findByRole('heading', { name: 'org.nav.workbench' })
      expect(screen.queryByRole('button', { name: 'org.nav.mail' })).not.toBeInTheDocument()
      fireEvent.click(screen.getByRole('button', { name: 'org.nav.schedule' }))
      expect(await screen.findByRole('heading', { name: 'schedule.title' })).toBeInTheDocument()
      expect(screen.queryByRole('heading', { name: 'mail.title' })).not.toBeInTheDocument()
      fireEvent.click(screen.getByRole('button', { name: 'org.nav.knowledge' }))
      expect(await screen.findByText('documents.subtitle.shared')).toBeInTheDocument()
      expect(screen.getByRole('heading', { name: 'documents.section.drive' })).toBeInTheDocument()
      expect(screen.queryByTestId('segmented-control')).not.toBeInTheDocument()
      expect(screen.queryByRole('heading', { name: 'schedule.title' })).not.toBeInTheDocument()
      const activities = scope.getSnapshot().contributions.filter((item) => item.contribution.slot === 'activity')
      expect(activities.some((item) => item.pluginId === 'studio.mail' || item.pluginId === 'studio.schedule')).toBe(
        false
      )
      expect(activities.some((item) => item.pluginId === 'studio.knowledge')).toBe(true)
    } finally {
      tree.unmount()
      await act(async () => host.close())
    }
  })

  it('shows the switch-team hint when the personal center is active', async () => {
    account.readCenterId.mockReturnValue('personal')
    renderPage()

    expect(await screen.findByText('org.empty.noOrgTitle')).toBeInTheDocument()
    expect(screen.getByText('org.empty.noOrg')).toBeInTheDocument()
    expect(api.listOrgMembers).not.toHaveBeenCalled()
  })

  it('explains a pending join request instead of opening the team', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([{ id: 'org-1', name: '通友科技', logo: '', membership: 'pending' }])
    renderPage()

    expect(await screen.findByText('org.empty.pendingTitle')).toBeInTheDocument()
    expect(api.listOrgMembers).not.toHaveBeenCalled()
  })

  it('offers sign-in again when the member session is denied', async () => {
    const { MemberOrgError } = await import('@renderer/data/memberOrgs')
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockRejectedValue(new MemberOrgError('invalid session', 401))
    renderPage()

    expect(await screen.findByText('sidebar.center.session_expired')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'sidebar.center.sign_in_again' })).toBeInTheDocument()
  })

  it('renders the tree, invite entry, and pending queue for an admin', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([
      department('dept-root', null, '总部'),
      department('dept-rd', 'dept-root', '研发中心')
    ])
    api.listOrgMembers.mockResolvedValue([member('user-1', { name: '李四' })])
    api.listJoinRequests.mockResolvedValue([{ member_id: 'member-9', name: '王五' }] as PendingApplicant[])
    api.fetchJoinInvite.mockResolvedValue({
      org_id: 'org-1',
      code: 'join-code',
      enabled: true,
      expires_at: null,
      rotated_at: ''
    })

    renderPage()

    expect(await screen.findByText('研发中心')).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'org.nav.members' })).toBeInTheDocument()
    expect(screen.getByRole('searchbox', { name: 'org.members.search' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'org.invite.open' })).toBeInTheDocument()
    await screen.findByText('李四')
    expect(screen.queryByText('王五')).not.toBeInTheDocument()
    fireEvent.click(await screen.findByRole('radio', { name: /org\.members\.filter\.pending/ }))
    expect(await screen.findByText('王五')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('radio', { name: /org\.members\.filter\.members/ }))
    await screen.findByText('李四')
    fireEvent.click(
      within(screen.getByText('李四').closest('tr') as HTMLElement).getByRole('button', { name: 'common.more' })
    )
    expect(screen.getByRole('button', { name: 'org.members.remove' })).toBeInTheDocument()
  })

  it('pages the whole member collection and resets when searching or changing departments', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([
      department('dept-root', null, '总部'),
      department('dept-rd', 'dept-root', '研发中心')
    ])
    const rows = Array.from({ length: 121 }, (_, index) =>
      member(`person-${index + 1}`, {
        department_id: index < 60 ? 'dept-rd' : 'dept-root'
      })
    )
    api.listOrgMembers.mockResolvedValue(rows)
    renderPage()
    await screen.findByText('person-1')
    const table = () => screen.getByRole('table', { name: 'org.nav.members' })
    expect(within(table()).getAllByRole('row')).toHaveLength(21)
    fireEvent.click(screen.getByRole('button', { name: 'collection.page 6' }))
    expect(within(table()).getByText('person-101')).toBeInTheDocument()
    expect(within(table()).queryByText('person-1')).not.toBeInTheDocument()

    fireEvent.change(screen.getByRole('searchbox', { name: 'org.members.search' }), { target: { value: 'person-121' } })
    expect(within(table()).getAllByRole('row')).toHaveLength(2)
    expect(screen.queryByRole('button', { name: 'collection.page 1' })).not.toBeInTheDocument()
    fireEvent.change(screen.getByRole('searchbox', { name: 'org.members.search' }), { target: { value: '' } })
    expect(within(table()).getByText('person-1')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'collection.rowsPerPage' }))
    fireEvent.click(screen.getByRole('option', { name: '10' }))
    expect(within(table()).getAllByRole('row')).toHaveLength(11)
    fireEvent.click(screen.getByRole('button', { name: 'collection.page 13' }))
    expect(within(table()).getByText('person-121')).toBeInTheDocument()
    fireEvent.click(screen.getByText('研发中心'))
    expect(screen.getByRole('button', { name: 'collection.rowsPerPage' })).toHaveTextContent('10')
    expect(within(table()).getAllByRole('row')).toHaveLength(11)
    expect(within(table()).getByText('person-1')).toBeInTheDocument()
    expect(within(table()).queryByText('person-121')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'collection.page 1' })).toHaveAttribute('aria-current', 'page')
  })

  it('returns to the last valid member page after deleting its final row', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    const rows = Array.from({ length: 101 }, (_, index) => member(`person-${index + 1}`))
    api.listOrgMembers.mockResolvedValue(rows)
    api.removeOrgMember.mockResolvedValue({ ok: true })
    renderPage()
    await screen.findByText('person-1')
    fireEvent.click(screen.getByRole('button', { name: 'collection.page 6' }))
    const row = screen.getByText('person-101').closest('tr') as HTMLElement
    fireEvent.click(within(row).getByRole('button', { name: 'common.more' }))
    fireEvent.click(screen.getByRole('button', { name: 'org.members.remove' }))
    const dialog = screen.getByRole('dialog', { name: 'org.members.remove' })
    api.listOrgMembers.mockResolvedValue(rows.slice(0, -1))
    fireEvent.click(within(dialog).getByRole('button', { name: 'org.members.remove' }))
    await waitFor(() => expect(api.removeOrgMember).toHaveBeenCalledWith('org-1', 'person-101'))
    await screen.findByText('person-81')
    expect(screen.getByRole('button', { name: 'collection.page 5' })).toHaveAttribute('aria-current', 'page')
    expect(screen.queryByText('person-101')).not.toBeInTheDocument()
  })

  it('names the member edit dialog from the list label and shows a close button', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([
      member('user-1', { name: ' ', phone: '18700001111', email: '' }),
      member('user-2', { name: 'RoyLin', phone: '19100002222' })
    ])
    api.listJoinRequests.mockResolvedValue([])
    renderPage()

    const unnamed = (await screen.findAllByText('18700001111'))[0]?.closest('tr') as HTMLElement
    fireEvent.click(within(unnamed).getByRole('button', { name: 'common.edit' }))
    expect(screen.getByRole('dialog', { name: 'org.members.editTitle:18700001111' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'common.close' }))
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'org.members.editTitle:18700001111' })).not.toBeInTheDocument()
    )

    const named = screen.getByText('RoyLin').closest('tr') as HTMLElement
    fireEvent.click(within(named).getByRole('button', { name: 'common.edit' }))
    expect(screen.getByRole('dialog', { name: 'org.members.editTitle:RoyLin' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'common.close' })).toBeInTheDocument()
  })

  it('opens a searchable position list in the member edit dialog', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([member('user-1', { name: '李四' })])
    api.listJoinRequests.mockResolvedValue([])
    api.listOrgRoles.mockResolvedValue([
      {
        id: 'role-admin',
        code: 'admin',
        name: '管理员',
        description: '',
        built_in: true,
        sort_order: 0,
        member_count: 0
      },
      {
        id: 'role-member',
        code: 'member',
        name: '成员',
        description: '',
        built_in: true,
        sort_order: 1,
        member_count: 1
      }
    ])
    api.listOrgPositions.mockResolvedValue([
      { id: 'pos-1', name: '财务负责人', sort_order: 0 },
      { id: 'pos-2', name: '行政专员', sort_order: 1 }
    ])
    renderPage()

    const row = (await screen.findByText('李四')).closest('tr') as HTMLElement
    fireEvent.click(within(row).getByRole('button', { name: 'common.edit' }))
    expect(await screen.findByRole('dialog', { name: 'org.members.editTitle:李四' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /org\.members\.col\.role/ }))
    expect(await screen.findByPlaceholderText('org.roles.search')).toBeInTheDocument()
    fireEvent.keyDown(document.body, { key: 'Escape' })

    fireEvent.click(screen.getByRole('button', { name: /org\.members\.col\.position/ }))
    expect(await screen.findByPlaceholderText('org.positions.search')).toBeInTheDocument()
    expect(screen.getAllByText('org.positions.unset').length).toBeGreaterThan(0)
    expect(screen.getByText('财务负责人')).toBeInTheDocument()
    expect(screen.getByText('行政专员')).toBeInTheDocument()
  })

  it('picks a member department from a searchable tree', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([
      department('dept-all', null, '全员'),
      department('dept-finance', 'dept-all', '财务部'),
      department('dept-rd', 'dept-all', '研发部')
    ])
    api.listOrgMembers.mockResolvedValue([
      member('user-1', { name: '温和的风筝', creator: true, department_id: 'dept-all' })
    ])
    api.listJoinRequests.mockResolvedValue([])
    api.setOrgMemberDepartment.mockReset().mockResolvedValue({ ok: true })
    renderPage()

    const row = (await screen.findByText('温和的风筝')).closest('tr') as HTMLElement
    fireEvent.click(within(row).getByRole('button', { name: 'common.edit' }))
    const dialog = await screen.findByRole('dialog', { name: 'org.members.editTitle:温和的风筝' })
    const departmentField = within(dialog).getByRole('combobox', { name: 'org.members.col.department' })
    expect(departmentField).toHaveTextContent('全员')
    expect(within(dialog).queryByText('全员 / 财务部')).not.toBeInTheDocument()

    fireEvent.click(departmentField)
    expect(await within(dialog).findByPlaceholderText('org.members.deptSearch')).toBeInTheDocument()
    expect(within(dialog).getByRole('treeitem', { name: '财务部' })).toBeInTheDocument()
    expect(within(dialog).getByRole('treeitem', { name: '研发部' })).toBeInTheDocument()

    fireEvent.change(within(dialog).getByPlaceholderText('org.members.deptSearch'), { target: { value: '研发' } })
    expect(within(dialog).queryByRole('treeitem', { name: '财务部' })).not.toBeInTheDocument()
    fireEvent.click(within(dialog).getByRole('treeitem', { name: '研发部' }))
    fireEvent.click(within(dialog).getByRole('button', { name: 'common.save' }))

    await waitFor(() => expect(api.setOrgMemberDepartment).toHaveBeenCalledWith('org-1', 'user-1', 'dept-rd'))
  })

  it('spins the member-list refresh icon while the directory reloads', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([member('user-1', { name: '李四' })])
    api.listJoinRequests.mockResolvedValue([])
    renderPage()
    expect(await screen.findByText('李四')).toBeInTheDocument()

    let release: (rows: OrgMemberRow[]) => void = () => undefined
    api.listOrgMembers.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve
        })
    )
    const callsBefore = api.listOrgMembers.mock.calls.length
    fireEvent.click(screen.getByRole('button', { name: 'common.refresh' }))

    const icon = () => screen.getByRole('button', { name: 'common.refresh' }).querySelector('svg')
    await waitFor(() => expect(icon()).toHaveClass('animate-spin'))
    expect(icon()).toHaveStyle({ animationDuration: '0.7s' })
    expect(screen.getByRole('button', { name: 'common.refresh' })).toHaveAttribute('aria-busy', 'true')
    release([member('user-1', { name: '李四' })])
    await waitFor(() => expect(api.listOrgMembers.mock.calls.length).toBeGreaterThan(callsBefore))
    await waitFor(() => expect(icon()).not.toHaveClass('animate-spin'))
  })

  it('hides management affordances for a plain member', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      { id: 'org-1', name: '通友科技', logo: '', membership: 'member', creator: false }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([member('user-1')])

    renderPage()

    expect(await screen.findByText('user-1')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.invite.open' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.members.remove' })).not.toBeInTheDocument()
  })

  it('opens the department dialog on the shared 12px panel', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([])

    renderPage()

    const root = await screen.findByRole('treeitem', { name: '总部' })
    expect(root.firstElementChild).toHaveClass('h-9', 'rounded-[8px]')
    const deptAdd = screen.getByRole('button', { name: 'org.members.deptAdd' })
    expect(deptAdd.parentElement).toHaveClass('visible', 'flex')
    expect(deptAdd.parentElement).not.toHaveClass('invisible')
    fireEvent.click(deptAdd)
    const title = await screen.findByRole('heading', { name: 'org.members.deptCreateTitle' })
    const panel = title.closest('[data-slot="dialog-content"]')
    expect(panel).toHaveClass('rounded-[12px]')
    expect(panel).not.toHaveClass('rounded-3xl')
    expect(screen.getByRole('textbox', { name: 'org.members.deptName' })).toHaveClass('rounded-[8px]')
  })

  it('approves a pending applicant from the row action', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([])
    api.listJoinRequests.mockResolvedValue([{ member_id: 'member-9', name: '王五' }])

    renderPage()
    fireEvent.click(await screen.findByRole('radio', { name: /org\.members\.filter\.pending/ }))

    // The row action and the ConfirmDialog confirm share the accessible name;
    // the dialog's button is the last one in the DOM (portal at body end).
    fireEvent.click(await screen.findByRole('button', { name: 'org.invite.approve' }))
    const buttons = await screen.findAllByRole('button', { name: 'org.invite.approve' })
    fireEvent.click(buttons[buttons.length - 1])

    await waitFor(() => expect(api.approveJoinRequest).toHaveBeenCalledWith('org-1', 'member-9'))
  })

  it('uses a fixed team sidebar and keeps the department tree on the members page', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([])

    renderPage()

    const nav = await screen.findByRole('navigation', { name: '通友科技' })
    const sidebarTitle = screen.getByRole('heading', { level: 1, name: '通友科技' })
    expect(sidebarTitle).toHaveClass('text-[14px]', 'font-medium')
    expect(sidebarTitle.closest('header')).toHaveClass('pt-4', 'pb-3')
    expect(sidebarTitle.closest('header')?.querySelector('img')).toHaveClass('size-8', 'rounded-[8px]')
    for (const section of ['members', 'positions', 'roles', 'usage', 'settings']) {
      expect(within(nav).getByRole('button', { name: `org.nav.${section}` })).toHaveClass('h-9', 'rounded-[8px]')
    }
    expect(screen.queryByText('org.nav.group')).not.toBeInTheDocument()
    expect(nav.closest('aside')).toHaveClass('w-[15.25rem]', 'border-r', 'border-[#efefef]')
    expect(screen.queryByRole('separator', { name: 'org.resize' })).not.toBeInTheDocument()
    expect(nav.querySelector('[role="treeitem"]')).toBeNull()
    expect(screen.getByRole('button', { name: 'org.nav.members' })).toHaveAttribute('aria-current', 'page')
    expect(await screen.findByRole('treeitem', { name: '总部' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'org.nav.positions' }))
    expect(await screen.findByRole('heading', { name: 'org.nav.positions' })).toHaveClass('text-[24px]', 'font-bold')
    expect(screen.queryByRole('treeitem', { name: '总部' })).not.toBeInTheDocument()
    expect(screen.getByRole('treeitem', { name: 'org.positions.all' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('button', { name: 'org.positions.add' })).toHaveClass('size-6', 'rounded-[6px]')

    fireEvent.click(screen.getByRole('button', { name: 'org.positions.add' }))
    const title = await screen.findByRole('heading', { name: 'org.positions.createTitle' })
    expect(title.closest('[data-slot="dialog-content"]')).toHaveClass('rounded-[12px]')
    fireEvent.change(screen.getByRole('textbox', { name: 'org.positions.name' }), { target: { value: '产品经理' } })
    fireEvent.click(screen.getByRole('button', { name: 'common.save' }))
    await waitFor(() => expect(api.createOrgPosition).toHaveBeenCalledWith('org-1', { name: '产品经理' }))
  })

  it('keeps a failed position deletion open for retry', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '团队',
        logo: '',
        creator: true,
        membership: 'admin',
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgPositions.mockResolvedValue([{ id: 'pos-1', name: '工程师', sort_order: 0 }])
    api.deleteOrgPosition.mockRejectedValueOnce(new Error('unavailable')).mockResolvedValue({ ok: true })
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.positions' }))
    fireEvent.click(await screen.findByRole('button', { name: 'org.positions.delete 工程师' }))
    const dialog = screen.getByRole('dialog', { name: 'org.positions.deleteTitle' })
    fireEvent.click(within(dialog).getByRole('button', { name: 'org.positions.delete' }))
    await waitFor(() => expect(api.deleteOrgPosition).toHaveBeenCalledOnce())
    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'org.positions.delete' })).toBeEnabled())
    expect(dialog).toBeInTheDocument()
    fireEvent.click(within(dialog).getByRole('button', { name: 'org.positions.delete' }))
    await waitFor(() => expect(api.deleteOrgPosition).toHaveBeenCalledTimes(2))
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'org.positions.deleteTitle' })).not.toBeInTheDocument()
    )
  })

  it('lets the creator open team usage and filter a department or a member', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([
      department('dept-root', null, '总部'),
      department('dept-rd', 'dept-root', '研发')
    ])
    api.listOrgMembers.mockResolvedValue([member('user-1', { name: '李四', department_id: 'dept-rd' })])

    renderPage()

    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.usage' }))
    expect(await screen.findByRole('heading', { name: 'org.nav.usage' })).toHaveClass('text-[24px]', 'font-bold')
    expect(screen.getByRole('radio', { name: 'org.usage.team' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByText('org.usage.description')).toBeInTheDocument()
    expect(document.querySelector('[data-ui="org.usage.charts"]')).toHaveTextContent('team')
    expect(screen.queryByRole('treeitem', { name: '总部' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('radio', { name: 'org.usage.department' }))
    expect(await screen.findByRole('combobox', { name: 'org.usage.pickDepartment' })).toHaveClass(
      'h-9',
      'rounded-[8px]',
      'bg-[#f5f5f5]'
    )
    expect(document.querySelector('[data-ui="org.usage.charts"]')).toHaveTextContent('dept-root')

    fireEvent.click(screen.getByRole('radio', { name: 'org.usage.member' }))
    expect(screen.getByRole('combobox', { name: 'org.usage.pickMember' })).toBeInTheDocument()
    expect(document.querySelector('[data-ui="org.usage.charts"]')).toHaveTextContent('user-1')
  })

  it('lets the creator manage custom roles and keeps built-in roles locked', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgRoles.mockResolvedValue([
      {
        id: 'role-owner',
        code: 'owner',
        name: '创建人',
        description: '',
        built_in: true,
        sort_order: 0,
        member_count: 1,
        data_scope: { kind: 'all', department_ids: [] },
        rules: [{ action: 'manage', subject: 'all' }]
      },
      {
        id: 'role-finance',
        code: 'custom_finance',
        name: '财务',
        description: '',
        built_in: false,
        sort_order: 100,
        member_count: 0,
        data_scope: { kind: 'all', department_ids: [] },
        policy_revision: 'a'.repeat(64),
        can_update: true,
        can_delete: true,
        rules: []
      }
    ])

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.roles' }))

    expect(await screen.findByRole('button', { name: '创建人' })).toBeInTheDocument()
    expect(screen.getAllByText('org.roles.builtInLocked').length).toBeGreaterThan(0)
    expect(screen.getByRole('checkbox', { name: 'org.perm.rule.inviteMember' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('checkbox', { name: 'org.perm.rule.inviteMember' })).toHaveAttribute(
      'aria-disabled',
      'true'
    )
    expect(screen.getByText('org.roles.builtIn')).toBeInTheDocument()
    expect(screen.getByText('org.roles.custom')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.roles.delete 创建人' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'org.roles.delete 财务' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'org.roles.create' }))
    fireEvent.change(await screen.findByRole('textbox', { name: 'org.roles.name' }), { target: { value: '人事' } })
    fireEvent.click(screen.getByRole('button', { name: 'common.save' }))

    await waitFor(() =>
      expect(api.createOrgRole).toHaveBeenCalledWith('org-1', {
        name: '人事',
        description: '',
        data_scope: { kind: 'self', department_ids: [] },
        rules: []
      })
    )

    fireEvent.click(screen.getByRole('button', { name: '财务' }))
    const inviteRule = screen.getByRole('checkbox', { name: 'org.perm.rule.inviteMember' })
    const permissionSave = screen.getByRole('button', { name: 'common.save' })
    expect(permissionSave).toBeDisabled()
    fireEvent.click(inviteRule)
    const checkedRule = screen.getByRole('checkbox', { name: 'org.perm.rule.inviteMember' })
    expect(checkedRule).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('button', { name: 'org.perm.rule.inviteMember' })).toBeEnabled()
    expect(api.updateOrgRole).not.toHaveBeenCalled()
    expect(permissionSave).toBeEnabled()
    fireEvent.click(permissionSave)
    await waitFor(() =>
      expect(api.updateOrgRole).toHaveBeenCalledWith('org-1', 'role-finance', {
        expected_revision: 'a'.repeat(64),
        name: '财务',
        description: '',
        data_scope: { kind: 'all', department_ids: [] },
        rules: [{ action: 'invite', subject: 'Member' }]
      })
    )
    fireEvent.click(screen.getByRole('checkbox', { name: 'org.perm.rule.inviteMember' }))
    expect(screen.getByRole('checkbox', { name: 'org.perm.rule.inviteMember' })).toHaveAttribute(
      'aria-checked',
      'false'
    )
    expect(api.updateOrgRole).toHaveBeenCalledTimes(1)
    const permissionSaveAgain = screen.getByRole('button', { name: 'common.save' })
    await waitFor(() => expect(permissionSaveAgain).toBeEnabled())
    fireEvent.click(permissionSaveAgain)
    await waitFor(() =>
      expect(api.updateOrgRole).toHaveBeenLastCalledWith('org-1', 'role-finance', {
        expected_revision: 'b'.repeat(64),
        name: '财务',
        description: '',
        data_scope: { kind: 'all', department_ids: [] },
        rules: []
      })
    )
  })

  it('lets the creator clear a checked permission on the built-in admin role', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] }
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    const grantable = [
      { action: 'update', subject: 'Member' },
      { action: 'delete', subject: 'Member' },
      { action: 'invite', subject: 'Member' },
      { action: 'assign', subject: 'Position' },
      { action: 'assign', subject: 'Role' }
    ]
    api.listOrgRoles.mockResolvedValue([
      {
        id: 'role-owner',
        code: 'owner',
        name: '超级管理员',
        description: '',
        built_in: true,
        sort_order: 0,
        member_count: 1,
        data_scope: { kind: 'all', department_ids: [] },
        rules: [{ action: 'manage', subject: 'all' }]
      },
      {
        id: 'role-admin',
        code: 'admin',
        name: '管理员',
        description: '',
        built_in: true,
        sort_order: 1,
        member_count: 0,
        data_scope: { kind: 'all', department_ids: [] },
        policy_revision: 'a'.repeat(64),
        can_update: true,
        can_delete: false,
        rules: grantable
      }
    ])

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.roles' }))
    expect(await screen.findByRole('checkbox', { name: 'org.perm.rule.inviteMember' })).toHaveAttribute(
      'aria-disabled',
      'true'
    )

    fireEvent.click(screen.getByRole('button', { name: '管理员' }))
    const inviteRule = screen.getByRole('checkbox', { name: 'org.perm.rule.inviteMember' })
    expect(inviteRule).toBeEnabled()
    expect(inviteRule).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('button', { name: 'common.save' })).toBeDisabled()
    fireEvent.click(inviteRule)
    expect(screen.getByRole('checkbox', { name: 'org.perm.rule.inviteMember' })).toHaveAttribute(
      'aria-checked',
      'false'
    )
    expect(api.updateOrgRole).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'common.save' }))
    await waitFor(() => {
      const rules = api.updateOrgRole.mock.calls.at(-1)?.[2]?.rules as { action: string; subject: string }[]
      expect(rules).toHaveLength(4)
      expect(rules).not.toContainEqual({ action: 'invite', subject: 'Member' })
    })
    expect(screen.queryByRole('button', { name: 'org.roles.delete 管理员' })).not.toBeInTheDocument()
  })

  it('supports whole-team position management independently of role management', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: false,
        dataScope: { kind: 'all', departmentIds: [] },
        rules: [
          { action: 'create', subject: 'Position' },
          { action: 'update', subject: 'Position' },
          { action: 'delete', subject: 'Position' },
          { action: 'assign', subject: 'Role' }
        ]
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.positions' }))
    expect(await screen.findByRole('button', { name: 'org.positions.add' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'org.nav.roles' }))
    expect(await screen.findByRole('heading', { name: 'org.nav.roles' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.roles.create' })).not.toBeInTheDocument()
  })

  it('edits the team profile and regenerates the text logo from the new name', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canUpdateOrg: true,
        canTransferOrg: true,
        canDeleteOrg: true,
        dataScope: { kind: 'all', departmentIds: [] },
        industry: 'internet',
        scale: '1-20',
        memberCount: 3,
        rules: []
      }
    ])
    api.updateOrgProfile.mockResolvedValue({ id: 'org-1', name: '通友', industry: 'internet', scale: '1-20', logo: '' })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.settings' }))

    expect(await screen.findByRole('heading', { name: 'org.nav.settings' })).toBeInTheDocument()
    const save = screen.getByRole('button', { name: 'common.save' })
    expect(save).toBeDisabled()
    fireEvent.change(screen.getByRole('textbox', { name: 'org.settings.name' }), { target: { value: '通友' } })
    fireEvent.click(save)

    await waitFor(() =>
      expect(api.updateOrgProfile).toHaveBeenCalledWith('org-1', {
        name: '通友',
        industry: 'internet',
        scale: '1-20',
        logo: expect.stringMatching(/^data:image\/svg\+xml/)
      })
    )
    expect(screen.getByText('org.settings.transfer')).toBeInTheDocument()
    expect(screen.queryByText('org.settings.leave')).not.toBeInTheDocument()
    expect(screen.getByText('org.settings.dissolve')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'org.settings.dissolveCta' })).toBeInTheDocument()
  })

  it('hides the position and role pages when the role only has directory read', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'member',
        creator: false,
        rules: [
          { action: 'read', subject: 'Position' },
          { action: 'read', subject: 'Role' }
        ]
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([member('user-1', { name: '李四' })])

    renderPage()
    expect(await screen.findByRole('button', { name: 'org.nav.members' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'org.nav.settings' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.nav.positions' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.nav.roles' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.nav.usage' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /org.nav.audit/ })).not.toBeInTheDocument()
  })

  it('opens the audit page only for a role that reads Audit', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'member',
        creator: false,
        rules: [{ action: 'read', subject: 'Audit' }]
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([member('user-1', { name: '李四' })])

    renderPage()
    expect(await screen.findByRole('button', { name: /org.nav.audit/ })).toBeInTheDocument()
  })

  it('opens the runs page only for a whole-team role that reads Run', async () => {
    const team = (dataScope: 'all' | 'own') => ({
      id: 'org-1',
      name: '通友科技',
      logo: '',
      membership: 'member',
      creator: false,
      rules: [{ action: 'read', subject: 'Run' }],
      dataScope: { kind: dataScope, departmentIds: [] }
    })
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([team('own')])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])
    api.listOrgMembers.mockResolvedValue([member('user-1', { name: '李四' })])

    const view = renderPage()
    expect(await screen.findByRole('button', { name: 'org.nav.members' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /org.nav.runs/ })).not.toBeInTheDocument()
    view.unmount()

    account.listMemberOrgs.mockResolvedValue([team('all')])
    renderPage()
    expect(await screen.findByRole('button', { name: /org.nav.runs/ })).toBeInTheDocument()
    expect(screen.getByText('org.nav.group.agents')).toBeInTheDocument()
  })

  it('opens only the position page when the role can edit positions', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'member',
        creator: false,
        rules: [{ action: 'update', subject: 'Position' }]
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.positions' }))
    expect(await screen.findByRole('heading', { name: 'org.nav.positions' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.nav.roles' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.positions.add' })).not.toBeInTheDocument()
  })

  it('opens only the role page when the role can assign roles', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'member',
        creator: false,
        rules: [{ action: 'assign', subject: 'Role' }]
      }
    ])
    api.listOrgDepartments.mockResolvedValue([department('dept-root', null, '总部')])

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.roles' }))
    expect(await screen.findByRole('heading', { name: 'org.nav.roles' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.nav.positions' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.roles.create' })).not.toBeInTheDocument()
  })

  it('lets a plain member read the profile and leave the team', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'member',
        creator: false,
        canLeaveOrg: true,
        industry: 'finance',
        rules: []
      }
    ])
    api.leaveOrg.mockResolvedValue({ ok: true })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.settings' }))

    expect(await screen.findByText('org.settings.industries.finance')).toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: 'org.settings.name' })).not.toBeInTheDocument()
    expect(screen.queryByText('org.settings.dissolve')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'org.settings.leaveCta' }))
    const confirms = await screen.findAllByRole('button', { name: 'org.settings.leaveCta' })
    fireEvent.click(confirms[confirms.length - 1])

    await waitFor(() => expect(api.leaveOrg).toHaveBeenCalledWith('org-1'))
    await waitFor(() => expect(account.writeCenterId).toHaveBeenCalledWith('personal'))
  })

  it('dissolves only after the team name is typed', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canTransferOrg: true,
        canDeleteOrg: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] },
        rules: []
      }
    ])
    api.dissolveOrg.mockResolvedValue({ ok: true })

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.settings' }))
    fireEvent.click(await screen.findByRole('button', { name: 'org.settings.dissolveCta' }))
    const confirms = await screen.findAllByRole('button', { name: 'org.settings.dissolveCta' })
    const confirm = confirms[confirms.length - 1]
    expect(confirm).not.toBeDisabled()
    await act(async () => {
      fireEvent.click(confirm)
    })
    expect(api.dissolveOrg).not.toHaveBeenCalled()
    const field = screen.getByRole('textbox', { name: 'org.settings.dissolveTypeName' })
    expect(field).toHaveAttribute('id', 'org-dissolve-name')
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
    setter?.call(field, '通友科技')
    const retry = screen.getAllByRole('button', { name: 'org.settings.dissolveCta' })
    await act(async () => {
      fireEvent.click(retry[retry.length - 1])
    })

    await waitFor(() => expect(api.dissolveOrg).toHaveBeenCalledWith('org-1'))
    await waitFor(() => expect(account.writeCenterId).toHaveBeenCalledWith('personal'))
  })

  it('lists transfer candidates for the creator', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canTransferOrg: true,
        canDeleteOrg: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] },
        rules: []
      }
    ])
    api.listTransferCandidates.mockResolvedValue([])

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.settings' }))
    fireEvent.click(await screen.findByRole('button', { name: 'org.settings.transferCta' }))

    expect(await screen.findByText('org.settings.transferEmpty')).toBeInTheDocument()
    expect(api.listTransferCandidates).toHaveBeenCalledWith('org-1')
  })

  it('shows each transfer candidate with an avatar in the member search', async () => {
    account.readCenterId.mockReturnValue('org-1')
    account.listMemberOrgs.mockResolvedValue([
      {
        id: 'org-1',
        name: '通友科技',
        logo: '',
        membership: 'admin',
        creator: true,
        canTransferOrg: true,
        canDeleteOrg: true,
        canReviewJoin: true,
        canInviteMembers: true,
        dataScope: { kind: 'all', departmentIds: [] },
        rules: []
      }
    ])
    api.listOrgMembers.mockResolvedValue([
      member('user-1', { name: '温和的风筝', phone: '19116506182', avatar: 'data:image/svg+xml;base64,QQ' }),
      member('user-2', { name: '林书豪', phone: '19165061825' })
    ])
    api.listTransferCandidates.mockResolvedValue([
      { id: 'user-1', name: '温和的风筝', phone: '19116506182' },
      { id: 'user-2', name: '林书豪', phone: '19165061825', avatar: 'data:image/webp;base64,AA' },
      { id: 'user-3', name: 'Leon', phone: '17370859985' }
    ])

    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: 'org.nav.settings' }))
    fireEvent.click(await screen.findByRole('button', { name: 'org.settings.transferCta' }))
    const dialog = await screen.findByRole('dialog', { name: 'org.settings.transfer' })
    fireEvent.click(within(dialog).getByRole('button', { name: /org\.settings\.transferPick/ }))

    const kite = await screen.findByRole('img', { name: '温和的风筝' })
    expect(kite.querySelector('img')).toHaveAttribute('src', 'data:image/svg+xml;base64,QQ')
    expect(screen.getByRole('img', { name: '林书豪' }).querySelector('img')).toHaveAttribute(
      'src',
      'data:image/webp;base64,AA'
    )
    const leon = screen.getByRole('img', { name: 'Leon' })
    expect(leon.querySelector('img')).toBeNull()
    expect(leon).toHaveTextContent('L')

    const row = screen.getByText('19116506182').closest('[data-slot="command-item"]')
    expect(row).toHaveClass('h-9')
    expect(row).toHaveTextContent('温和的风筝')
    expect(
      screen.getByPlaceholderText('org.members.search').closest('[data-slot="command-input-wrapper"]')
    ).toHaveClass('h-[30px]', 'rounded-[8px]', 'bg-[#f5f5f5]')

    fireEvent.change(screen.getByPlaceholderText('org.members.search'), { target: { value: '17370859985' } })
    expect(screen.getByRole('img', { name: 'Leon' })).toBeInTheDocument()
    expect(screen.queryByRole('img', { name: '林书豪' })).not.toBeInTheDocument()

    fireEvent.change(screen.getByPlaceholderText('org.members.search'), { target: { value: '' } })
    fireEvent.click(screen.getByText('林书豪'))
    expect(
      within(dialog)
        .getByRole('button', { name: /林书豪/ })
        .querySelector('img')
    ).toHaveAttribute('src', 'data:image/webp;base64,AA')
  })
})
