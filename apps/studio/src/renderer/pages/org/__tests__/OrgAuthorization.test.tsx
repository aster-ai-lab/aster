import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import i18n from 'i18next'
import { SWRConfig, useSWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { readCenterId, writeCenterId } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'

import type {
  OrgAccessRule,
  OrgDepartment,
  OrgJoinInvite,
  OrgMemberRow,
  OrgRole,
  OrgRoleAssignmentAudit,
  OrgRoleAudit
} from '../orgApi'
import OrgPage from '../OrgPage'

vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))

const orgId = '11111111-1111-4111-8111-111111111111'
let grants: OrgAccessRule[]
let roles: OrgRole[]
let roleHistory: OrgRoleAudit[]
let assignmentHistory: OrgRoleAssignmentAudit[]
let restoreFetch: () => void
let admin: boolean
let reviewJoin: boolean
let inviteCapability: boolean
let creator: boolean
let official: boolean
let settingsCaps: { update: boolean; transfer: boolean; delete: boolean; leave: boolean }
let delayedSettings: { suffix: string; method: string; response: Promise<Response> } | null
let transferCandidates: { id: string; name: string }[]
let scopeKind: string
let orgIndexStatus: number
let catalogStatuses: Record<string, number>
let roleUpdateConflict: boolean
let refreshCatalog: (catalog: string) => Promise<unknown>
let joinInvite: OrgJoinInvite
let delayedConfigure: Promise<Response> | null
let revalidatePolicy: () => Promise<unknown>
let refreshPolicyOnly: () => Promise<unknown>
let requests: { path: string; method: string; body: unknown }[]
let directoryRows: OrgMemberRow[]
let departmentRows: OrgDepartment[]
let revalidateDirectory: () => Promise<unknown>
const colleague: OrgMemberRow = {
  id: 'user-2',
  name: '李四',
  email: '',
  phone: null,
  status: 'active',
  admin: false,
  creator: false,
  department_id: 'dept-leaf',
  joined_at: '2026-10-08',
  position_id: null,
  position_name: null,
  role_code: 'member',
  role_name: '成员',
  role_built_in: true
}
const t = (key: string) => String(i18n.t(key))

beforeEach(() => {
  grants = []
  roles = []
  roleHistory = [
    {
      id: 'audit-1',
      actor_member_id: 'recorded-actor',
      action: 'role_created',
      occurred_at: '2026-10-10T00:00:00Z',
      before: null,
      after: {
        id: 'bounded',
        name: '历史角色',
        description: '记录中的说明',
        rules: [{ action: 'invite', subject: 'Member' }],
        data_scope_kind: 'self',
        scope_department_ids: []
      }
    }
  ]
  assignmentHistory = [
    {
      id: 'assignment-1',
      actor_member_id: 'assignment-actor',
      actor_user_id: 'actor-user',
      target_member_id: 'member-2',
      target_user_id: colleague.id,
      authority: 'assign_role',
      admin_change: 'revoke',
      occurred_at: '2026-10-10T01:02:03Z',
      before: {
        department_id: 'dept-leaf',
        administrator: true,
        role: {
          id: 'previous-role',
          name: '原分配角色',
          description: '',
          rules: [],
          data_scope_kind: 'self',
          scope_department_ids: []
        }
      },
      after: {
        department_id: 'dept-leaf',
        administrator: true,
        role: {
          id: 'next-role',
          name: '新分配角色',
          description: '自定义授权保留管理员绑定',
          rules: [{ action: 'invite', subject: 'Member' }],
          data_scope_kind: 'custom_tree',
          scope_department_ids: ['dept-leaf']
        }
      }
    }
  ]
  admin = false
  reviewJoin = false
  inviteCapability = false
  creator = false
  official = false
  settingsCaps = { update: false, transfer: false, delete: false, leave: false }
  delayedSettings = null
  transferCandidates = []
  scopeKind = 'all'
  orgIndexStatus = 200
  catalogStatuses = {}
  roleUpdateConflict = false
  joinInvite = {
    org_id: orgId,
    revision: 'a'.repeat(64),
    code: 'current-code',
    enabled: true,
    expires_at: null,
    rotated_at: ''
  }
  delayedConfigure = null
  requests = []
  directoryRows = [
    {
      ...colleague,
      can_remove: true,
      can_assign_position: true,
      can_assign_department: true,
      assignable_department_ids: ['dept-root', 'dept-leaf'],
      can_assign_role: true,
      assignable_role_ids: ['safe-role']
    }
  ]
  departmentRows = [{ id: 'dept-leaf', parent_id: 'hidden-root', name: '研发部', sort_order: 0 }]
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32) }
  })
  const fetchSpy = vi.spyOn(globalThis, 'fetch').mockImplementation(async (url, init) => {
    const path = new URL(String(url)).pathname
    const method = init?.method ?? 'GET'
    const body = init?.body ? JSON.parse(String(init.body)) : null
    requests.push({ path, method, body })
    if (roleUpdateConflict && method === 'PUT' && path.includes('/roles/')) {
      return new Response(
        JSON.stringify({ statusCode: 409, message: 'role policy changed; reload before saving', error: 'Conflict' }),
        { status: 409 }
      )
    }
    if (path === '/api/member-orgs' && orgIndexStatus !== 200)
      return new Response(JSON.stringify({ message: 'policy unavailable' }), { status: orgIndexStatus })
    const catalogStatus = catalogStatuses[path.split('/').at(-1) ?? '']
    if (method === 'GET' && catalogStatus && catalogStatus !== 200)
      return new Response(JSON.stringify({ message: 'catalog unavailable' }), { status: catalogStatus })
    if (delayedSettings && path.endsWith(delayedSettings.suffix) && method === delayedSettings.method)
      return delayedSettings.response
    let data: unknown = []
    if (path === '/api/member-orgs')
      data = [
        {
          id: orgId,
          name: '权限测试团队',
          logo: '',
          membership: admin ? 'admin' : 'member',
          creator,
          is_official: official,
          can_review_join: reviewJoin,
          can_invite_members: inviteCapability,
          can_update_org: settingsCaps.update,
          can_transfer_org: settingsCaps.transfer,
          can_delete_org: settingsCaps.delete,
          can_leave_org: settingsCaps.leave,
          data_scope: {
            kind: scopeKind,
            department_ids: scopeKind === 'custom' || scopeKind === 'custom_tree' ? ['dept-leaf'] : []
          },
          rules: grants
        }
      ]
    else if (path === '/api/member/me') data = { display_name: '本人', phone: '13800000000' }
    else if (path.endsWith('/members')) data = directoryRows
    else if (path.endsWith('/departments')) data = departmentRows
    else if (path.endsWith('/positions')) data = [{ id: 'position-1', name: '工程师', sort_order: 0 }]
    else if (path.endsWith('/roles')) data = roles
    else if (path.endsWith('/audit')) data = roleHistory
    else if (path.endsWith('/role-audit')) data = assignmentHistory
    else if (path.endsWith('/transfer-candidates')) data = transferCandidates
    else if (path.endsWith('/join-requests')) data = [{ member_id: 'applicant-1', name: '申请人' }]
    else if (path.endsWith('/verification'))
      data = { status: 'unverified', enterprise_name: '', credit_code: '', legal_person: '', reject_reason: '' }
    if (path.endsWith('/join-invite')) {
      if (method === 'PUT' && delayedConfigure) return delayedConfigure
      data = joinInvite
    } else if (method !== 'GET') data = { ok: true }
    return new Response(JSON.stringify({ code: 0, data }))
  })
  restoreFetch = () => fetchSpy.mockRestore()
  writeCenterId(orgId)
})

afterEach(() => {
  restoreFetch()
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  localStorage.removeItem('a3s.org.center')
})

function PolicyPage({ section }: { section: 'members' | 'settings' }) {
  const { mutate } = useSWRConfig()
  refreshPolicyOnly = () => mutate('studio/member-orgs')
  refreshCatalog = (catalog) =>
    mutate((key) => typeof key === 'string' && key.startsWith(`studio/org-${catalog}:${orgId}:`))
  revalidatePolicy = () =>
    Promise.all([
      mutate('studio/member-orgs'),
      mutate((key) => typeof key === 'string' && key.startsWith(`studio/org-roles:${orgId}:`))
    ])
  revalidateDirectory = () =>
    mutate((key) => typeof key === 'string' && key.startsWith(`studio/org-directory:${orgId}:`))
  return <OrgPage initialSection={section} />
}

async function showPending() {
  fireEvent.click(await screen.findByRole('radio', { name: new RegExp(t('org.members.filter.pending')) }))
}

async function showMembers() {
  fireEvent.click(await screen.findByRole('radio', { name: new RegExp(t('org.members.filter.members')) }))
}

function openPage(section: 'members' | 'settings' = 'members') {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      <PolicyPage section={section} />
    </SWRConfig>
  )
}

describe('team button authority through the real member client', () => {
  const editableRole: OrgRole = {
    id: 'bounded',
    code: 'custom_bounded',
    name: '目录角色',
    description: '',
    built_in: false,
    sort_order: 1,
    member_count: 0,
    rules: [],
    data_scope: { kind: 'self', department_ids: [] },
    policy_revision: 'a'.repeat(64),
    can_update: true,
    can_delete: true
  }

  it.each([false, true])('offers supplier grants only within the official team: %s', async (isOfficial) => {
    official = isOfficial
    grants = [
      { action: 'update', subject: 'Role' },
      { action: 'create', subject: 'PlatformSupplier' }
    ]
    roles = [editableRole]
    openPage()
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await screen.findByRole('button', { name: editableRole.name })
    const create = screen.queryByRole('checkbox', { name: t('org.perm.rule.createPlatformSupplier') })
    if (isOfficial) {
      expect(screen.getByRole('button', { name: t('org.perm.rule.createPlatformSupplier') })).toBeEnabled()
      await user.click(screen.getByRole('checkbox', { name: t('org.perm.rule.createPlatformSupplier') }))
      expect(screen.getByRole('checkbox', { name: t('org.perm.rule.createPlatformSupplier') })).toHaveAttribute(
        'aria-checked',
        'true'
      )
      expect(screen.getByRole('checkbox', { name: t('org.perm.rule.deletePlatformSupplier') })).toHaveAttribute(
        'aria-disabled',
        'true'
      )
      expect(screen.getByRole('button', { name: t('common.save') })).toBeEnabled()
    } else expect(create).not.toBeInTheDocument()
    expect(requests.filter((request) => request.method !== 'GET' && request.path.includes('/roles'))).toEqual([])
  })

  it('sends the original role revision and recovers from a server conflict without retrying the stale grant', async () => {
    const user = userEvent.setup()
    grants = [
      { action: 'update', subject: 'Role' },
      { action: 'invite', subject: 'Member' }
    ]
    roles = [{ ...editableRole, policy_revision: 'a'.repeat(64) }]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await user.click(await screen.findByRole('checkbox', { name: t('org.perm.rule.inviteMember') }))
    roles = [{ ...editableRole, policy_revision: 'b'.repeat(64) }]
    roleUpdateConflict = true
    await user.click(screen.getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(requests.filter((request) => request.method === 'PUT')).toHaveLength(1))
    expect(requests.find((request) => request.method === 'PUT')?.body).toMatchObject({
      expected_revision: 'a'.repeat(64),
      rules: [{ action: 'invite', subject: 'Member' }]
    })
    await waitFor(() =>
      expect(screen.getByRole('checkbox', { name: t('org.perm.rule.inviteMember') })).toHaveAttribute(
        'aria-checked',
        'false'
      )
    )
    expect(screen.getByRole('button', { name: t('common.save') })).toBeDisabled()
    expect(requests.filter((request) => request.method === 'PUT')).toHaveLength(1)
  })

  it('discards role dialog and permission drafts after a new catalog revision without losing the edit grant', async () => {
    const user = userEvent.setup()
    grants = [
      { action: 'update', subject: 'Role' },
      { action: 'invite', subject: 'Member' }
    ]
    roles = [{ ...editableRole, policy_revision: 'a'.repeat(64) }]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await user.click(await screen.findByRole('checkbox', { name: t('org.perm.rule.inviteMember') }))
    await user.click(screen.getByRole('button', { name: `${t('common.edit')} ${editableRole.name}` }))
    await user.type(screen.getByRole('textbox', { name: t('org.roles.name') }), '旧草稿')
    roles = [{ ...editableRole, policy_revision: 'b'.repeat(64), description: 'other administrator change' }]
    await act(async () => {
      await refreshCatalog('roles')
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.getByRole('checkbox', { name: t('org.perm.rule.inviteMember') })).toHaveAttribute(
      'aria-checked',
      'false'
    )
    expect(requests.filter((request) => request.method === 'PUT')).toHaveLength(0)
    await user.click(screen.getByRole('button', { name: `${t('common.edit')} ${editableRole.name}` }))
    expect(screen.getByRole('textbox', { name: t('org.roles.name') })).toHaveValue(editableRole.name)
    expect(screen.getByRole('textbox', { name: t('org.roles.description') })).toHaveValue(roles[0].description)
  })

  it.each([undefined, '', 'invalid', 'g'.repeat(64)])(
    'withdraws role mutation buttons for an invalid revision %s',
    async (policy_revision) => {
      const user = userEvent.setup()
      grants = [
        { action: 'update', subject: 'Role' },
        { action: 'delete', subject: 'Role' },
        { action: 'invite', subject: 'Member' }
      ]
      roles = [{ ...editableRole, policy_revision }]
      openPage()
      await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
      expect(screen.queryByRole('button', { name: `${t('common.edit')} ${editableRole.name}` })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: t('common.save') })).not.toBeInTheDocument()
      expect(
        screen.queryByRole('button', { name: `${t('org.roles.delete')} ${editableRole.name}` })
      ).not.toBeInTheDocument()
      const checkbox = screen.getByRole('checkbox', { name: t('org.perm.rule.inviteMember') })
      expect(checkbox).toHaveAttribute('aria-disabled', 'true')
      await user.click(checkbox)
      expect(checkbox).toHaveAttribute('aria-checked', 'false')
      expect(requests.filter((request) => request.method === 'PUT')).toHaveLength(0)
      expect(requests.filter((request) => request.method === 'DELETE')).toHaveLength(0)
    }
  )

  it.each([
    ['roles', 404],
    ['roles', 503],
    ['positions', 404],
    ['positions', 503]
  ] as const)(
    'withdraws cached %s controls after a %s catalog response and recovers without the old draft',
    async (catalog, status) => {
      const user = userEvent.setup()
      const role = catalog === 'roles'
      grants = ['create', 'update', 'delete'].map((action) => ({ action, subject: role ? 'Role' : 'Position' }))
      roles = [editableRole]
      const rowName = role ? editableRole.name : '工程师'
      const editLabel = `${t(role ? 'common.edit' : 'org.positions.rename')} ${rowName}`
      const fieldLabel = t(role ? 'org.roles.name' : 'org.positions.name')
      openPage()
      await user.click(await screen.findByRole('button', { name: t(`org.nav.${catalog}`) }))
      await user.click(await screen.findByRole('button', { name: editLabel }))
      await user.type(screen.getByRole('textbox', { name: fieldLabel }), '旧草稿')
      catalogStatuses[catalog] = status
      await act(async () => {
        await refreshCatalog(catalog).catch(() => undefined)
      })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: rowName })).not.toBeInTheDocument()
      expect(
        screen.queryByRole('button', { name: t(role ? 'org.roles.create' : 'org.positions.add') })
      ).not.toBeInTheDocument()
      expect(screen.getByRole('button', { name: t('common.retry') })).toBeInTheDocument()
      catalogStatuses[catalog] = 200
      await user.click(screen.getByRole('button', { name: t('common.retry') }))
      await user.click(await screen.findByRole('button', { name: editLabel }))
      expect(screen.getByRole('textbox', { name: fieldLabel })).toHaveValue(rowName)
      expect(
        requests.filter((request) => request.method !== 'GET' && request.path.includes(`/orgs/${orgId}/`))
      ).toEqual([])
    }
  )

  it.each(['roles', 'positions'] as const)(
    'ignores a %s rename completed after catalog failure and recovery',
    async (catalog) => {
      const user = userEvent.setup()
      const role = catalog === 'roles'
      grants = [{ action: 'update', subject: role ? 'Role' : 'Position' }]
      roles = [editableRole]
      const rowName = role ? editableRole.name : '工程师'
      const editLabel = `${t(role ? 'common.edit' : 'org.positions.rename')} ${rowName}`
      const fieldLabel = t(role ? 'org.roles.name' : 'org.positions.name')
      let finish!: (response: Response) => void
      delayedSettings = {
        suffix: `/${catalog}/${role ? editableRole.id : 'position-1'}`,
        method: 'PUT',
        response: new Promise<Response>((resolve) => {
          finish = resolve
        })
      }
      openPage()
      await user.click(await screen.findByRole('button', { name: t(`org.nav.${catalog}`) }))
      await user.click(await screen.findByRole('button', { name: editLabel }))
      await user.clear(screen.getByRole('textbox', { name: fieldLabel }))
      await user.type(screen.getByRole('textbox', { name: fieldLabel }), '旧名称')
      await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: t('common.save') }))
      await waitFor(() => expect(requests.some((request) => request.method === 'PUT')).toBe(true))
      catalogStatuses[catalog] = 404
      await act(async () => {
        await refreshCatalog(catalog).catch(() => undefined)
      })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      catalogStatuses[catalog] = 200
      await user.click(screen.getByRole('button', { name: t('common.retry') }))
      await user.click(await screen.findByRole('button', { name: editLabel }))
      await user.clear(screen.getByRole('textbox', { name: fieldLabel }))
      await user.type(screen.getByRole('textbox', { name: fieldLabel }), '恢复后的草稿')
      const reads = requests.filter(
        (request) => request.method === 'GET' && request.path.endsWith(`/${catalog}`)
      ).length
      await act(async () => {
        finish(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
      })
      expect(screen.getByRole('textbox', { name: fieldLabel })).toHaveValue('恢复后的草稿')
      expect(
        requests.filter((request) => request.method === 'GET' && request.path.endsWith(`/${catalog}`))
      ).toHaveLength(reads)
    }
  )

  it('discards role deletion confirmation when the reviewed policy changes', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'delete', subject: 'Role' }]
    roles = [editableRole]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    const label = t('org.roles.delete')
    await user.click(await screen.findByRole('button', { name: `${label} ${editableRole.name}` }))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    roles = [{ ...editableRole, policy_revision: 'b'.repeat(64) }]
    await act(async () => {
      await refreshCatalog('roles')
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(requests.filter((request) => request.method === 'DELETE')).toEqual([])
    await user.click(screen.getByRole('button', { name: `${label} ${editableRole.name}` }))
    const memberReads = requests.filter(
      (request) => request.method === 'GET' && request.path.endsWith('/members')
    ).length
    roles = []
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: label }))
    await waitFor(() => expect(requests.filter((request) => request.method === 'DELETE')).toHaveLength(1))
    expect(requests.find((request) => request.method === 'DELETE')?.body).toEqual({ expected_revision: 'b'.repeat(64) })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: `${label} ${editableRole.name}` })).not.toBeInTheDocument()
    expect(toast.success).toHaveBeenLastCalledWith(t('org.roles.deleted'))
    await waitFor(() =>
      expect(
        requests.filter((request) => request.method === 'GET' && request.path.endsWith('/members')).length
      ).toBeGreaterThan(memberReads)
    )
  })

  it('requires audit permission for member history and shows recorded state without granting edits', async () => {
    const user = userEvent.setup()
    admin = true
    grants = [{ action: 'assign', subject: 'Role' }]
    openPage()
    await screen.findByRole('row', { name: /李四/ })
    expect(screen.queryByRole('button', { name: t('org.members.history.title') })).not.toBeInTheDocument()
    expect(requests.filter((request) => request.path.endsWith('/role-audit'))).toHaveLength(0)
    grants = [{ action: 'audit', subject: 'Role' }]
    await act(async () => {
      await refreshPolicyOnly()
    })
    await user.click(await screen.findByRole('button', { name: t('org.members.history.title') }))
    const dialog = screen.getByRole('dialog')
    expect(await within(dialog).findByText('新分配角色')).toBeInTheDocument()
    expect(dialog).toHaveAccessibleName(`${t('org.members.history.title')} · 李四`)
    const before = within(dialog).getByRole('region', { name: t('org.roles.history.before') })
    const after = within(dialog).getByRole('region', { name: t('org.roles.history.after') })
    expect(before).toHaveTextContent('原分配角色')
    expect(before).toHaveTextContent(t('org.members.history.administrator'))
    expect(after).toHaveTextContent(t('org.members.history.administrator'))
    expect(after).toHaveTextContent('研发部')
    expect(after).toHaveTextContent(t('org.scope.customTree'))
    expect(after).toHaveTextContent(t('org.perm.rule.inviteMember'))
    expect(dialog).toHaveTextContent(t('org.members.history.revoke'))
    expect(dialog).toHaveTextContent(String(i18n.t('org.roles.history.actor', { id: 'assignment-actor' })))
    expect(
      within(dialog).getByText(new Date(assignmentHistory[0].occurred_at).toLocaleString(i18n.language))
    ).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('common.edit') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('common.save') })).not.toBeInTheDocument()
    expect(requests.filter((request) => request.path.endsWith('/role-audit'))).toEqual([
      { path: `/api/member/orgs/${orgId}/members/${colleague.id}/role-audit`, method: 'GET', body: null }
    ])
    expect(
      requests.filter((request) => request.method !== 'GET' && request.path.startsWith('/api/member/orgs/'))
    ).toEqual([])
  })

  it.each([404, 503])('withdraws member history after a refused read %s and retries current access', async (status) => {
    const user = userEvent.setup()
    grants = [{ action: 'audit', subject: 'Role' }]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.members.history.title') }))
    expect(await screen.findByText('自定义授权保留管理员绑定')).toBeInTheDocument()
    catalogStatuses['role-audit'] = status
    await act(async () => {
      await refreshCatalog('member-role-audit').catch(() => undefined)
    })
    expect(screen.queryByText('自定义授权保留管理员绑定')).not.toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent(t('org.roles.history.unavailable'))
    catalogStatuses['role-audit'] = 200
    assignmentHistory = []
    await user.click(screen.getByRole('button', { name: t('common.retry') }))
    expect(await screen.findByText(t('org.roles.history.empty'))).toBeInTheDocument()
    expect(screen.queryByText('新分配角色')).not.toBeInTheDocument()
    expect(
      requests.filter((request) => request.method !== 'GET' && request.path.startsWith('/api/member/orgs/'))
    ).toEqual([])
  })

  it.each(['grant', 'scope', 'team'] as const)(
    'isolates late member history after an authority change and recovery: %s',
    async (change) => {
      const user = userEvent.setup()
      grants = [{ action: 'audit', subject: 'Role' }]
      let finish!: (response: Response) => void
      const oldHistory = assignmentHistory
      delayedSettings = {
        suffix: '/role-audit',
        method: 'GET',
        response: new Promise<Response>((resolve) => {
          finish = resolve
        })
      }
      openPage()
      await user.click(await screen.findByRole('button', { name: t('org.members.history.title') }))
      expect(within(screen.getByRole('dialog')).getByRole('status')).toBeInTheDocument()
      await waitFor(() => expect(requests.some((request) => request.path.endsWith('/role-audit'))).toBe(true))
      await act(async () => {
        if (change === 'team') writeCenterId('personal')
        else {
          if (change === 'grant') grants = []
          else scopeKind = 'custom'
          await refreshPolicyOnly()
        }
      })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      delayedSettings = null
      assignmentHistory = [
        {
          ...oldHistory[0],
          after: { ...oldHistory[0].after, role: { ...oldHistory[0].after.role, description: '当前授权下的新记录' } }
        }
      ]
      await act(async () => {
        grants = [{ action: 'audit', subject: 'Role' }]
        scopeKind = 'all'
        if (change === 'team') writeCenterId(orgId)
        else await refreshPolicyOnly()
      })
      await user.click(await screen.findByRole('button', { name: t('org.members.history.title') }))
      expect(await screen.findByText('当前授权下的新记录')).toBeInTheDocument()
      await act(async () => {
        finish(new Response(JSON.stringify({ code: 0, data: oldHistory })))
      })
      expect(screen.getByText('当前授权下的新记录')).toBeInTheDocument()
      expect(screen.queryByText('自定义授权保留管理员绑定')).not.toBeInTheDocument()
    }
  )

  it.each(['department', 'role', 'removed'] as const)(
    'disposes member history when its target changes: %s',
    async (change) => {
      const user = userEvent.setup()
      grants = [{ action: 'audit', subject: 'Role' }]
      openPage()
      await user.click(await screen.findByRole('button', { name: t('org.members.history.title') }))
      expect(await screen.findByText('自定义授权保留管理员绑定')).toBeInTheDocument()
      directoryRows =
        change === 'removed'
          ? []
          : [
              {
                ...directoryRows[0],
                ...(change === 'department' ? { department_id: 'dept-moved' } : { role_code: 'changed-role' })
              }
            ]
      await act(async () => {
        await revalidateDirectory()
      })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      expect(screen.queryByText('自定义授权保留管理员绑定')).not.toBeInTheDocument()
      expect(
        requests.filter((request) => request.method !== 'GET' && request.path.startsWith('/api/member/orgs/'))
      ).toEqual([])
    }
  )

  it('does not offer assignment history for a pending join applicant', async () => {
    grants = [
      { action: 'audit', subject: 'Role' },
      { action: 'approve', subject: 'JoinRequest' }
    ]
    reviewJoin = true
    openPage()
    await showPending()
    const applicant = await screen.findByRole('row', { name: /申请人/ })
    expect(within(applicant).queryByRole('button', { name: t('org.members.history.title') })).not.toBeInTheDocument()
    await showMembers()
    expect(
      within(await screen.findByRole('row', { name: /李四/ })).getByRole('button', {
        name: t('org.members.history.title')
      })
    ).toBeInTheDocument()
    expect(requests.filter((request) => request.path.endsWith('/role-audit'))).toHaveLength(0)
  })

  it('requires an independent audit grant and opens a read-only role history', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'create', subject: 'Role' }]
    roles = [editableRole]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    expect(screen.queryByRole('button', { name: t('org.roles.history.title') })).not.toBeInTheDocument()
    expect(requests.filter((request) => request.path.endsWith('/audit'))).toHaveLength(0)
    grants = [{ action: 'audit', subject: 'Role' }]
    await act(async () => {
      await refreshPolicyOnly()
    })
    await user.click(await screen.findByRole('button', { name: t('org.roles.history.title') }))
    const dialog = await screen.findByRole('dialog')
    expect(await within(dialog).findByText('历史角色')).toBeInTheDocument()
    expect(within(dialog).getByText('记录中的说明')).toBeInTheDocument()
    expect(within(dialog).getByText(t('org.roles.history.absent'))).toBeInTheDocument()
    expect(within(dialog).getByText(t('org.perm.rule.inviteMember'))).toBeInTheDocument()
    expect(
      within(dialog).getByText(String(i18n.t('org.roles.history.actor', { id: 'recorded-actor' })))
    ).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.roles.create') })).not.toBeInTheDocument()
    expect(requests.filter((request) => request.method !== 'GET' && request.path.includes('/roles'))).toHaveLength(0)
  })

  it('withdraws cached audit records on a refused read and retries current authority', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'audit', subject: 'Role' }]
    roles = [editableRole]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await user.click(await screen.findByRole('button', { name: t('org.roles.history.title') }))
    expect(await screen.findByText('记录中的说明')).toBeInTheDocument()
    catalogStatuses.audit = 403
    await act(async () => {
      await refreshCatalog('role-audit').catch(() => undefined)
    })
    expect(screen.queryByText('记录中的说明')).not.toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent(t('org.roles.history.unavailable'))
    catalogStatuses.audit = 200
    roleHistory = [{ ...roleHistory[0], after: { ...roleHistory[0].after!, description: '重新授权的记录' } }]
    await user.click(screen.getByRole('button', { name: t('common.retry') }))
    expect(await screen.findByText('重新授权的记录')).toBeInTheDocument()
    expect(screen.queryByText('记录中的说明')).not.toBeInTheDocument()
  })

  it('keeps a new audit view isolated from an old request after permission revocation and regrant', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'audit', subject: 'Role' }]
    roles = [editableRole]
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: `/roles/${editableRole.id}/audit`,
      method: 'GET',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await user.click(await screen.findByRole('button', { name: t('org.roles.history.title') }))
    expect(within(screen.getByRole('dialog')).getByRole('status')).toBeInTheDocument()
    grants = []
    await act(async () => {
      await refreshPolicyOnly()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    grants = [{ action: 'audit', subject: 'Role' }]
    delayedSettings = null
    const oldHistory = roleHistory
    roleHistory = [{ ...roleHistory[0], after: { ...roleHistory[0].after!, description: '当前记录' } }]
    await act(async () => {
      await refreshPolicyOnly()
    })
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await user.click(await screen.findByRole('button', { name: t('org.roles.history.title') }))
    expect(await screen.findByText('当前记录')).toBeInTheDocument()
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: oldHistory })))
    })
    expect(screen.getByText('当前记录')).toBeInTheDocument()
    expect(screen.queryByText('记录中的说明')).not.toBeInTheDocument()
  })

  it.each([200, 409])('ignores an old deletion response %s after a new role revision is reviewed', async (status) => {
    const user = userEvent.setup()
    grants = [{ action: 'delete', subject: 'Role' }]
    roles = [editableRole]
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: `/roles/${editableRole.id}`,
      method: 'DELETE',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    const label = t('org.roles.delete')
    const rowLabel = `${label} ${editableRole.name}`
    await user.click(await screen.findByRole('button', { name: rowLabel }))
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: label }))
    await waitFor(() => expect(requests.filter((request) => request.method === 'DELETE')).toHaveLength(1))
    expect(requests.find((request) => request.method === 'DELETE')?.body).toEqual({
      expected_revision: editableRole.policy_revision
    })
    roles = [{ ...editableRole, policy_revision: 'b'.repeat(64) }]
    await act(async () => {
      await refreshCatalog('roles')
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: rowLabel }))
    expect(within(screen.getByRole('dialog')).getByRole('button', { name: label })).toBeEnabled()
    const reads = requests.filter((request) => request.method === 'GET').length
    const errors = vi.mocked(toast.error).mock.calls.length
    const successes = vi.mocked(toast.success).mock.calls.length
    await act(async () => {
      finish(
        new Response(
          JSON.stringify(
            status === 200
              ? { code: 0, data: { ok: true } }
              : {
                  statusCode: 409,
                  message: 'role policy changed; reload before deleting',
                  error: 'Conflict'
                }
          ),
          { status }
        )
      )
    })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(within(screen.getByRole('dialog')).getByRole('button', { name: label })).toBeEnabled()
    expect(requests.filter((request) => request.method === 'GET')).toHaveLength(reads)
    expect(vi.mocked(toast.error).mock.calls).toHaveLength(errors)
    expect(vi.mocked(toast.success).mock.calls).toHaveLength(successes)
  })

  it('reloads a conflicted deletion and requires confirmation of the new revision', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'delete', subject: 'Role' }]
    roles = [editableRole]
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: `/roles/${editableRole.id}`,
      method: 'DELETE',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    const label = t('org.roles.delete')
    const rowLabel = `${label} ${editableRole.name}`
    await user.click(await screen.findByRole('button', { name: rowLabel }))
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: label }))
    const reads = requests.filter((request) => request.method === 'GET' && request.path.endsWith('/roles')).length
    roles = [{ ...editableRole, policy_revision: 'b'.repeat(64) }]
    await act(async () => {
      finish(
        new Response(
          JSON.stringify({
            statusCode: 409,
            message: 'role policy changed; reload before deleting',
            error: 'Conflict'
          }),
          { status: 409 }
        )
      )
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await waitFor(() =>
      expect(
        requests.filter((request) => request.method === 'GET' && request.path.endsWith('/roles')).length
      ).toBeGreaterThan(reads)
    )
    expect(toast.error).toHaveBeenLastCalledWith(t('org.roles.deleteChanged'))
    expect(requests.filter((request) => request.method === 'DELETE')).toHaveLength(1)
    delayedSettings = null
    await user.click(screen.getByRole('button', { name: rowLabel }))
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: label }))
    await waitFor(() => expect(requests.filter((request) => request.method === 'DELETE')).toHaveLength(2))
    expect(requests.filter((request) => request.method === 'DELETE')[1].body).toEqual({
      expected_revision: 'b'.repeat(64)
    })
  })

  it.each(['roles', 'positions'] as const)(
    'keeps the recovered %s confirmation when an old deletion completes',
    async (catalog) => {
      const user = userEvent.setup()
      const role = catalog === 'roles'
      grants = [{ action: 'delete', subject: role ? 'Role' : 'Position' }]
      roles = [editableRole]
      const label = t(role ? 'org.roles.delete' : 'org.positions.delete')
      const rowName = role ? editableRole.name : '工程师'
      let finish!: (response: Response) => void
      delayedSettings = {
        suffix: `/${catalog}/${role ? editableRole.id : 'position-1'}`,
        method: 'DELETE',
        response: new Promise<Response>((resolve) => {
          finish = resolve
        })
      }
      openPage()
      await user.click(await screen.findByRole('button', { name: t(`org.nav.${catalog}`) }))
      await user.click(await screen.findByRole('button', { name: `${label} ${rowName}` }))
      await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: label }))
      await waitFor(() => expect(requests.some((request) => request.method === 'DELETE')).toBe(true))
      catalogStatuses[catalog] = 503
      await act(async () => {
        await refreshCatalog(catalog).catch(() => undefined)
      })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      catalogStatuses[catalog] = 200
      await user.click(screen.getByRole('button', { name: t('common.retry') }))
      await user.click(await screen.findByRole('button', { name: `${label} ${rowName}` }))
      await act(async () => {
        finish(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
      })
      expect(within(screen.getByRole('dialog')).getByRole('button', { name: label })).toBeEnabled()
      expect(requests.filter((request) => request.method === 'DELETE')).toHaveLength(1)
    }
  )

  it('withdraws role mutations when the department catalog fails and discards its draft before recovery', async () => {
    const user = userEvent.setup()
    grants = ['create', 'update', 'delete'].map((action) => ({ action, subject: 'Role' }))
    roles = [{ ...editableRole, data_scope: { kind: 'custom', department_ids: ['dept-leaf'] } }]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await user.click(await screen.findByRole('button', { name: `${t('common.edit')} ${editableRole.name}` }))
    await user.type(screen.getByRole('textbox', { name: t('org.roles.name') }), '不可信草稿')
    catalogStatuses.departments = 404
    await act(async () => {
      await refreshCatalog('role-departments').catch(() => undefined)
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.roles.create') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: `${t('common.edit')} ${editableRole.name}` })).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: `${t('org.roles.delete')} ${editableRole.name}` })
    ).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: t('org.perm.rule.createRole') })).toBeDisabled()
    catalogStatuses.departments = 200
    await user.click(screen.getByRole('button', { name: t('common.retry') }))
    await user.click(await screen.findByRole('button', { name: `${t('common.edit')} ${editableRole.name}` }))
    expect(screen.getByRole('textbox', { name: t('org.roles.name') })).toHaveValue(editableRole.name)
    expect(requests.filter((request) => request.method !== 'GET' && request.path.includes(`/orgs/${orgId}/`))).toEqual(
      []
    )
  })

  it.each([
    ['members', 401],
    ['members', 404],
    ['departments', 503],
    ['join-requests', 503]
  ] as const)(
    'withdraws directory rows and drafts after %s returns %s, then recovers with fresh rows',
    async (catalog, status) => {
      const user = userEvent.setup()
      grants = [
        { action: 'assign', subject: 'Position' },
        { action: 'update', subject: 'Department' },
        { action: 'approve', subject: 'JoinRequest' }
      ]
      reviewJoin = true
      openPage()
      const row = await screen.findByRole('row', { name: new RegExp(colleague.name) })
      await user.click(within(row).getByRole('button', { name: t('common.edit') }))
      const dialog = screen.getByRole('dialog')
      await user.click(within(dialog).getByRole('button', { name: new RegExp(t('org.members.col.position')) }))
      await user.click(await screen.findByRole('option', { name: '工程师' }))
      catalogStatuses[catalog] = status
      await act(async () => {
        await revalidateDirectory().catch(() => undefined)
      })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      expect(screen.queryByText(colleague.name)).not.toBeInTheDocument()
      expect(screen.queryByText('申请人')).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: `${t('org.members.deptRename')} 研发部` })).not.toBeInTheDocument()
      expect(screen.getByRole('button', { name: t('common.retry') })).toBeInTheDocument()
      catalogStatuses[catalog] = 200
      directoryRows = [{ ...directoryRows[0], name: '刷新后的成员' }]
      await user.click(screen.getByRole('button', { name: t('common.retry') }))
      const fresh = await screen.findByRole('row', { name: /刷新后的成员/ })
      expect(screen.queryByText(colleague.name)).not.toBeInTheDocument()
      await user.click(within(fresh).getByRole('button', { name: t('common.edit') }))
      expect(
        within(screen.getByRole('dialog')).getByRole('button', { name: new RegExp(t('org.members.col.position')) })
      ).toHaveTextContent(t('org.positions.unset'))
      expect(
        requests.filter((request) => request.method !== 'GET' && request.path.includes(`/orgs/${orgId}/`))
      ).toEqual([])
    }
  )

  it('keeps a fresh department draft when a rename finishes after directory read failure', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'update', subject: 'Department' }]
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/departments/dept-leaf',
      method: 'PUT',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    const label = `${t('org.members.deptRename')} 研发部`
    await user.click(await screen.findByRole('button', { name: label }))
    await user.clear(screen.getByRole('textbox', { name: t('org.members.deptName') }))
    await user.type(screen.getByRole('textbox', { name: t('org.members.deptName') }), '旧部门名称')
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(requests.some((request) => request.method === 'PUT')).toBe(true))
    catalogStatuses.members = 404
    await act(async () => {
      await revalidateDirectory().catch(() => undefined)
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    catalogStatuses.members = 200
    await user.click(screen.getByRole('button', { name: t('common.retry') }))
    await user.click(await screen.findByRole('button', { name: label }))
    await user.clear(screen.getByRole('textbox', { name: t('org.members.deptName') }))
    await user.type(screen.getByRole('textbox', { name: t('org.members.deptName') }), '恢复后的部门草稿')
    const reads = requests.filter((request) => request.method === 'GET' && request.path.endsWith('/members')).length
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
    })
    expect(screen.getByRole('textbox', { name: t('org.members.deptName') })).toHaveValue('恢复后的部门草稿')
    expect(requests.filter((request) => request.method === 'GET' && request.path.endsWith('/members'))).toHaveLength(
      reads
    )
  })

  it('keeps a new department confirmation when a deletion finishes after directory read failure', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'delete', subject: 'Department' }]
    const label = t('org.members.deptDelete')
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/departments/dept-leaf',
      method: 'DELETE',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: `${label} 研发部` }))
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: label }))
    await waitFor(() => expect(requests.some((request) => request.method === 'DELETE')).toBe(true))
    catalogStatuses.departments = 503
    await act(async () => {
      await revalidateDirectory().catch(() => undefined)
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    catalogStatuses.departments = 200
    await user.click(screen.getByRole('button', { name: t('common.retry') }))
    await user.click(await screen.findByRole('button', { name: `${label} 研发部` }))
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
    })
    expect(within(screen.getByRole('dialog')).getByRole('button', { name: label })).toBeEnabled()
    expect(requests.filter((request) => request.method === 'DELETE')).toHaveLength(1)
  })

  it.each(['members', 'positions'] as const)(
    'keeps the recovered %s member editor after an old assignment finishes',
    async (section) => {
      const user = userEvent.setup()
      grants = [{ action: 'assign', subject: 'Position' }]
      let finish!: (response: Response) => void
      delayedSettings = {
        suffix: '/position',
        method: 'PUT',
        response: new Promise<Response>((resolve) => {
          finish = resolve
        })
      }
      openPage()
      if (section === 'positions') await user.click(await screen.findByRole('button', { name: t('org.nav.positions') }))
      const row = await screen.findByRole('row', { name: new RegExp(colleague.name) })
      await user.click(within(row).getByRole('button', { name: t('common.edit') }))
      await user.click(
        within(screen.getByRole('dialog')).getByRole('button', { name: new RegExp(t('org.members.col.position')) })
      )
      await user.click(await screen.findByRole('option', { name: '工程师' }))
      await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: t('common.save') }))
      await waitFor(() => expect(requests.some((request) => request.method === 'PUT')).toBe(true))
      catalogStatuses.members = 404
      await act(async () => {
        await revalidateDirectory().catch(() => undefined)
      })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      expect(screen.queryByText(colleague.name)).not.toBeInTheDocument()
      catalogStatuses.members = 200
      await user.click(screen.getByRole('button', { name: t('common.retry') }))
      await user.click(
        within(await screen.findByRole('row', { name: new RegExp(colleague.name) })).getByRole('button', {
          name: t('common.edit')
        })
      )
      const reads = requests.filter((request) => request.method === 'GET' && request.path.endsWith('/members')).length
      await act(async () => {
        finish(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
      })
      expect(screen.getByRole('dialog')).toBeInTheDocument()
      expect(requests.filter((request) => request.method === 'GET' && request.path.endsWith('/members'))).toHaveLength(
        reads
      )
    }
  )

  it('offers a retry instead of an indefinite usage loader after a directory failure', async () => {
    const user = userEvent.setup()
    creator = true
    openPage()
    await screen.findByRole('row', { name: new RegExp(colleague.name) })
    catalogStatuses.members = 503
    await act(async () => {
      await revalidateDirectory().catch(() => undefined)
    })
    await user.click(screen.getByRole('button', { name: t('org.nav.usage') }))
    expect(screen.getByRole('button', { name: t('common.retry') })).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: t('org.nav.usage') })).not.toBeInTheDocument()
    catalogStatuses.members = 200
    await user.click(screen.getByRole('button', { name: t('common.retry') }))
    expect(await screen.findByRole('heading', { name: t('org.nav.usage') })).toBeInTheDocument()
  })

  it('discards an invitation dialog after directory failure without restoring its old link', async () => {
    const user = userEvent.setup()
    grants = [
      { action: 'invite', subject: 'Member' },
      { action: 'configure', subject: 'JoinInvite' }
    ]
    inviteCapability = true
    let finish!: (response: Response) => void
    delayedConfigure = new Promise<Response>((resolve) => {
      finish = resolve
    })
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
    await screen.findByRole('textbox', { name: t('org.invite.link') })
    await user.click(screen.getByRole('button', { name: t('org.invite.disable') }))
    await waitFor(() =>
      expect(requests.some((request) => request.path.endsWith('/join-invite') && request.method === 'PUT')).toBe(true)
    )
    catalogStatuses.members = 404
    await act(async () => {
      await revalidateDirectory().catch(() => undefined)
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.invite.open') })).not.toBeInTheDocument()
    catalogStatuses.members = 200
    joinInvite = { ...joinInvite, code: 'recovered-code' }
    await user.click(screen.getByRole('button', { name: t('common.retry') }))
    await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
    expect(await screen.findByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/recovered-code')
    const reads = requests.filter((request) => request.method === 'GET' && request.path.endsWith('/members')).length
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: { ...joinInvite, code: 'old-link' } })))
    })
    expect(screen.getByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/recovered-code')
    expect(requests.filter((request) => request.method === 'GET' && request.path.endsWith('/members'))).toHaveLength(
      reads
    )
  })

  it('withdraws cached members when an automatic focus refresh fails', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'assign', subject: 'Position' }]
    openPage()
    await user.click(
      within(await screen.findByRole('row', { name: new RegExp(colleague.name) })).getByRole('button', {
        name: t('common.edit')
      })
    )
    catalogStatuses.members = 503
    fireEvent.focus(window)
    expect(await screen.findByRole('button', { name: t('common.retry') })).toBeInTheDocument()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.queryByText(colleague.name)).not.toBeInTheDocument()
  })

  it.each([401, 404, 503])(
    'withdraws cached role authority after a %s policy-index response and recovers with a fresh editor',
    async (status) => {
      const user = userEvent.setup()
      grants = [{ action: 'update', subject: 'Role' }]
      roles = [
        {
          id: 'bounded',
          code: 'custom_bounded',
          name: '可编辑角色',
          description: '',
          built_in: false,
          sort_order: 1,
          member_count: 0,
          rules: [],
          data_scope: { kind: 'self', department_ids: [] },
          policy_revision: 'a'.repeat(64),
          can_update: true,
          can_delete: false
        }
      ]
      openPage()
      await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
      await user.click(await screen.findByRole('button', { name: `${t('common.edit')} ${roles[0].name}` }))
      await user.type(screen.getByRole('textbox', { name: t('org.roles.name') }), '未提交')
      orgIndexStatus = status
      await act(async () => {
        await refreshPolicyOnly().catch(() => undefined)
      })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: t('org.nav.roles') })).not.toBeInTheDocument()
      expect(screen.queryByText(colleague.name)).not.toBeInTheDocument()
      expect(
        screen.getByRole('button', { name: t(status === 401 ? 'sidebar.center.sign_in_again' : 'common.retry') })
      ).toBeInTheDocument()
      orgIndexStatus = 200
      await act(async () => {
        await refreshPolicyOnly()
      })
      await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
      await user.click(await screen.findByRole('button', { name: `${t('common.edit')} ${roles[0].name}` }))
      expect(screen.getByRole('textbox', { name: t('org.roles.name') })).toHaveValue(roles[0].name)
    }
  )

  it('keeps a recovered role catalog independent of a read started before the policy-index failure', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'update', subject: 'Role' }]
    const row: OrgRole = {
      id: 'fresh',
      code: 'custom_fresh',
      name: '恢复后的角色',
      description: '',
      built_in: false,
      sort_order: 1,
      member_count: 0,
      rules: [],
      data_scope: { kind: 'self', department_ids: [] },
      policy_revision: 'a'.repeat(64),
      can_update: true,
      can_delete: false
    }
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/roles',
      method: 'GET',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await waitFor(() => expect(requests.some((request) => request.path.endsWith('/roles'))).toBe(true))
    orgIndexStatus = 404
    await act(async () => {
      await refreshPolicyOnly().catch(() => undefined)
    })
    delayedSettings = null
    roles = [row]
    orgIndexStatus = 200
    await act(async () => {
      await refreshPolicyOnly()
    })
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    expect(await screen.findByRole('button', { name: row.name })).toBeInTheDocument()
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: [{ ...row, id: 'old', name: '失效的旧角色' }] })))
    })
    expect(screen.queryByRole('button', { name: '失效的旧角色' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: row.name })).toBeInTheDocument()
  })

  it('disposes a department write after policy-index failure without closing a recovered editor', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'update', subject: 'Department' }]
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/departments/dept-leaf',
      method: 'PUT',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: `${t('org.members.deptRename')} 研发部` }))
    await user.clear(screen.getByRole('textbox', { name: t('org.members.deptName') }))
    await user.type(screen.getByRole('textbox', { name: t('org.members.deptName') }), '旧名称')
    await user.click(screen.getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(requests.some((request) => request.method === 'PUT')).toBe(true))
    orgIndexStatus = 404
    await act(async () => {
      await refreshPolicyOnly().catch(() => undefined)
    })
    orgIndexStatus = 200
    await act(async () => {
      await refreshPolicyOnly()
    })
    await user.click(await screen.findByRole('button', { name: `${t('org.members.deptRename')} 研发部` }))
    await user.clear(screen.getByRole('textbox', { name: t('org.members.deptName') }))
    await user.type(screen.getByRole('textbox', { name: t('org.members.deptName') }), '新名称')
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: { id: 'dept-leaf', name: '旧名称' } })))
    })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: t('org.members.deptName') })).toHaveValue('新名称')
  })

  it.each([
    ['self', ['self']],
    ['department', ['self', 'department', 'custom']],
    ['custom', ['self', 'department', 'custom']],
    ['department_tree', ['self', 'department', 'department_tree', 'custom', 'custom_tree']],
    ['custom_tree', ['self', 'department', 'department_tree', 'custom', 'custom_tree']],
    ['all', ['self', 'department', 'department_tree', 'custom', 'custom_tree', 'all']]
  ] as const)('limits role policy and creation choices to the %s authority ceiling', async (kind, allowed) => {
    const user = userEvent.setup()
    scopeKind = kind
    grants = [
      { action: 'create', subject: 'Role' },
      { action: 'update', subject: 'Role' }
    ]
    roles = [
      {
        id: 'bounded',
        code: 'custom_bounded',
        name: '范围内角色',
        description: '',
        built_in: false,
        sort_order: 1,
        member_count: 0,
        rules: [],
        data_scope: { kind: 'self', department_ids: [] },
        policy_revision: 'a'.repeat(64),
        can_update: true,
        can_delete: false
      }
    ]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    const choices = [
      ['self', 'org.scope.self'],
      ['department', 'org.scope.department'],
      ['department_tree', 'org.scope.departmentTree'],
      ['custom', 'org.scope.custom'],
      ['custom_tree', 'org.scope.customTree'],
      ['all', 'org.scope.all']
    ] as const
    ;(await screen.findByRole('combobox', { name: t('org.scope.title') })).focus()
    await user.keyboard('[Space]')
    for (const [choice, label] of choices) {
      const option = await screen.findByRole('option', { name: t(label) })
      if (allowed.some((item) => item === choice)) expect(option).not.toHaveAttribute('aria-disabled', 'true')
      else expect(option).toHaveAttribute('aria-disabled', 'true')
    }
    await user.keyboard('[Escape]')
    await user.click(screen.getByRole('button', { name: t('org.roles.create') }))
    const dialog = screen.getByRole('dialog')
    within(dialog)
      .getByRole('combobox', { name: t('org.scope.title') })
      .focus()
    await user.keyboard('[Space]')
    for (const [choice, label] of choices) {
      const option = await screen.findByRole('option', { name: t(label) })
      if (allowed.some((item) => item === choice)) expect(option).not.toHaveAttribute('aria-disabled', 'true')
      else expect(option).toHaveAttribute('aria-disabled', 'true')
    }
    await user.keyboard('[Escape]')
    await user.type(within(dialog).getByRole('textbox', { name: t('org.roles.name') }), '限定角色')
    await user.click(within(dialog).getByRole('button', { name: t('common.save') }))
    await waitFor(() =>
      expect(
        requests.some(
          (request) =>
            request.method === 'POST' &&
            request.path.endsWith('/roles') &&
            JSON.stringify(request.body).includes('"kind":"self"')
        )
      ).toBe(true)
    )
  })

  it('refuses role controls when actor scope is missing even with creator and row capability flags', async () => {
    const user = userEvent.setup()
    scopeKind = 'invalid'
    creator = true
    roles = [
      {
        id: 'bounded',
        code: 'custom_bounded',
        name: '不可操作角色',
        description: '',
        built_in: false,
        sort_order: 1,
        member_count: 0,
        rules: [],
        data_scope: { kind: 'self', department_ids: [] },
        policy_revision: 'a'.repeat(64),
        can_update: true,
        can_delete: true
      }
    ]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    expect(await screen.findByRole('button', { name: roles[0].name })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.roles.create') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: `${t('common.edit')} ${roles[0].name}` })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: `${t('org.roles.delete')} ${roles[0].name}` })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('common.save') })).not.toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: t('org.scope.title') })).toBeDisabled()
    expect(requests.filter((request) => request.method !== 'GET' && request.path.includes('/roles'))).toEqual([])
  })

  it.each(['roles', 'positions'] as const)(
    'keeps a new %s deletion confirmation when an old request finishes after regrant',
    async (catalog) => {
      const user = userEvent.setup()
      const role = catalog === 'roles'
      grants = [{ action: 'delete', subject: role ? 'Role' : 'Position' }]
      roles = [
        {
          id: 'bounded',
          code: 'custom_bounded',
          name: '可删除角色',
          description: '',
          built_in: false,
          sort_order: 1,
          member_count: 0,
          rules: [],
          data_scope: { kind: 'self', department_ids: [] },
          policy_revision: 'a'.repeat(64),
          can_update: false,
          can_delete: true
        }
      ]
      const label = t(role ? 'org.roles.delete' : 'org.positions.delete')
      const rowName = role ? roles[0].name : '工程师'
      let finish!: (response: Response) => void
      delayedSettings = {
        suffix: `/${catalog}/${role ? 'bounded' : 'position-1'}`,
        method: 'DELETE',
        response: new Promise<Response>((resolve) => {
          finish = resolve
        })
      }
      openPage()
      await user.click(await screen.findByRole('button', { name: t(`org.nav.${catalog}`) }))
      await user.click(await screen.findByRole('button', { name: `${label} ${rowName}` }))
      await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: label }))
      await waitFor(() => expect(requests.some((request) => request.method === 'DELETE')).toBe(true))
      scopeKind = 'self'
      await act(async () => {
        await refreshPolicyOnly()
      })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      scopeKind = 'all'
      await act(async () => {
        await refreshPolicyOnly()
      })
      await user.click(await screen.findByRole('button', { name: `${label} ${rowName}` }))
      expect(screen.getByRole('dialog')).toBeInTheDocument()
      await act(async () => {
        finish(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
      })
      expect(screen.getByRole('dialog')).toBeInTheDocument()
    }
  )

  it('refetches scoped roles and refuses a late broader catalog after scope changes', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'update', subject: 'Role' }]
    const ownRole: OrgRole = {
      id: 'bounded',
      code: 'custom_bounded',
      name: '当前范围角色',
      description: '',
      built_in: false,
      sort_order: 1,
      member_count: 0,
      rules: [],
      data_scope: { kind: 'self', department_ids: [] },
      policy_revision: 'a'.repeat(64),
      can_update: true,
      can_delete: false
    }
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/roles',
      method: 'GET',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await waitFor(() => expect(requests.some((request) => request.path.endsWith('/roles'))).toBe(true))
    delayedSettings = null
    roles = [ownRole]
    scopeKind = 'self'
    await act(async () => {
      await refreshPolicyOnly()
    })
    expect(await screen.findByRole('button', { name: ownRole.name })).toBeInTheDocument()
    await act(async () => {
      finish(
        new Response(
          JSON.stringify({
            code: 0,
            data: [
              {
                ...ownRole,
                id: 'secret',
                name: '旧范围部门策略',
                data_scope: { kind: 'custom', department_ids: ['hidden-dept'] }
              }
            ]
          })
        )
      )
    })
    expect(screen.queryByRole('button', { name: '旧范围部门策略' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: ownRole.name })).toBeInTheDocument()
  })

  it('clears a role editor on scope change and refuses a late rename after regrant', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'update', subject: 'Role' }]
    roles = [
      {
        id: 'bounded',
        code: 'custom_bounded',
        name: '范围内角色',
        description: '',
        built_in: false,
        sort_order: 1,
        member_count: 0,
        rules: [],
        data_scope: { kind: 'self', department_ids: [] },
        policy_revision: 'a'.repeat(64),
        can_update: true,
        can_delete: false
      }
    ]
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/roles/bounded',
      method: 'PUT',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await user.click(await screen.findByRole('button', { name: `${t('common.edit')} ${roles[0].name}` }))
    await user.clear(screen.getByRole('textbox', { name: t('org.roles.name') }))
    await user.type(screen.getByRole('textbox', { name: t('org.roles.name') }), '旧草稿')
    await user.click(screen.getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(requests.some((request) => request.method === 'PUT')).toBe(true))
    scopeKind = 'self'
    await act(async () => {
      await refreshPolicyOnly()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    scopeKind = 'all'
    await act(async () => {
      await refreshPolicyOnly()
    })
    await user.click(await screen.findByRole('button', { name: `${t('common.edit')} ${roles[0].name}` }))
    await user.clear(screen.getByRole('textbox', { name: t('org.roles.name') }))
    await user.type(screen.getByRole('textbox', { name: t('org.roles.name') }), '新草稿')
    const reads = requests.filter((request) => request.method === 'GET' && request.path.endsWith('/roles')).length
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: { ...roles[0], name: '旧草稿' } })))
    })
    expect(screen.getByRole('textbox', { name: t('org.roles.name') })).toHaveValue('新草稿')
    expect(requests.filter((request) => request.method === 'GET' && request.path.endsWith('/roles'))).toHaveLength(
      reads
    )
  })

  it('preserves a new permission draft when a revoked save completes after row-capability regrant', async () => {
    const user = userEvent.setup()
    grants = [
      { action: 'update', subject: 'Role' },
      { action: 'assign', subject: 'Position' },
      { action: 'assign', subject: 'Department' }
    ]
    roles = [
      {
        id: 'bounded',
        code: 'custom_bounded',
        name: '权限维护角色',
        description: '',
        built_in: false,
        sort_order: 1,
        member_count: 0,
        rules: [],
        data_scope: { kind: 'self', department_ids: [] },
        policy_revision: 'a'.repeat(64),
        can_update: true,
        can_delete: false
      }
    ]
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/roles/bounded',
      method: 'PUT',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await user.click(await screen.findByRole('button', { name: t('org.perm.rule.assignPosition') }))
    await user.click(screen.getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(requests.some((request) => request.method === 'PUT')).toBe(true))
    roles = roles.map((role) => ({ ...role, can_update: false }))
    await act(async () => {
      await revalidatePolicy()
    })
    roles = roles.map((role) => ({ ...role, can_update: true }))
    await act(async () => {
      await revalidatePolicy()
    })
    await user.click(screen.getByRole('button', { name: t('org.perm.rule.assignDepartment') }))
    expect(screen.getByRole('button', { name: t('common.save') })).toBeEnabled()
    const departmentCheckbox = () =>
      within(screen.getByRole('button', { name: t('org.perm.rule.assignDepartment') })).getByRole('checkbox')
    const positionCheckbox = () =>
      within(screen.getByRole('button', { name: t('org.perm.rule.assignPosition') })).getByRole('checkbox')
    expect(departmentCheckbox()).toHaveAttribute('aria-checked', 'true')
    const reads = requests.filter((request) => request.method === 'GET' && request.path.endsWith('/roles')).length
    await act(async () => {
      finish(
        new Response(
          JSON.stringify({ code: 0, data: { ...roles[0], rules: [{ action: 'assign', subject: 'Position' }] } })
        )
      )
    })
    expect(departmentCheckbox()).toHaveAttribute('aria-checked', 'true')
    expect(positionCheckbox()).toHaveAttribute('aria-checked', 'false')
    expect(requests.filter((request) => request.method === 'GET' && request.path.endsWith('/roles'))).toHaveLength(
      reads
    )
  })

  it('requires whole-team scope and independent grants for shared position changes', async () => {
    admin = true
    grants = ['create', 'update', 'delete', 'assign'].map((action) => ({ action, subject: 'Position' }))
    scopeKind = 'department_tree'
    openPage()
    fireEvent.click(await screen.findByRole('button', { name: t('org.nav.positions') }))
    await screen.findByRole('button', { name: '工程师' })
    for (const kind of ['self', 'department', 'department_tree', 'custom', 'custom_tree']) {
      scopeKind = kind
      await act(async () => {
        await revalidatePolicy()
      })
      expect(screen.queryByRole('button', { name: t('org.positions.add') })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: `${t('org.positions.rename')} 工程师` })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: `${t('org.positions.delete')} 工程师` })).not.toBeInTheDocument()
    }
    scopeKind = 'all'
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.getByRole('button', { name: t('org.positions.add') })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: `${t('org.positions.rename')} 工程师` })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: `${t('org.positions.delete')} 工程师` })).toBeInTheDocument()
    grants = [{ action: 'create', subject: 'Position' }]
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.getByRole('button', { name: t('org.positions.add') })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: `${t('org.positions.rename')} 工程师` })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: `${t('org.positions.delete')} 工程师` })).not.toBeInTheDocument()
    expect(requests.filter((request) => request.method !== 'GET' && request.path.includes('/positions'))).toEqual([])
  })

  it('disposes a position draft on scope revocation and ignores a late rename after regrant', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'update', subject: 'Position' }]
    let finish: ((response: Response) => void) | undefined
    delayedSettings = {
      suffix: '/positions/position-1',
      method: 'PUT',
      response: new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.positions') }))
    await user.click(await screen.findByRole('button', { name: `${t('org.positions.rename')} 工程师` }))
    await user.clear(screen.getByRole('textbox', { name: t('org.positions.name') }))
    await user.type(screen.getByRole('textbox', { name: t('org.positions.name') }), '旧名称')
    await user.click(screen.getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(requests.some((request) => request.method === 'PUT')).toBe(true))
    scopeKind = 'self'
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    scopeKind = 'all'
    await act(async () => {
      await revalidatePolicy()
    })
    await user.click(await screen.findByRole('button', { name: `${t('org.positions.rename')} 工程师` }))
    await user.clear(screen.getByRole('textbox', { name: t('org.positions.name') }))
    await user.type(screen.getByRole('textbox', { name: t('org.positions.name') }), '新名称')
    const catalogReads = requests.filter(
      (request) => request.method === 'GET' && request.path.endsWith('/positions')
    ).length
    await act(async () => {
      finish?.(new Response(JSON.stringify({ code: 0, data: { id: 'position-1', name: '旧名称', sort_order: 0 } })))
    })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: t('org.positions.name') })).toHaveValue('新名称')
    expect(requests.filter((request) => request.method === 'GET' && request.path.endsWith('/positions'))).toHaveLength(
      catalogReads
    )
  })

  it('hides every member action when a creator has no effective row capabilities', async () => {
    creator = true
    directoryRows = [{ ...colleague }]
    openPage()
    const row = (await screen.findByText(colleague.name)).closest('tr') as HTMLElement
    expect(within(row).queryByRole('button', { name: t('common.edit') })).not.toBeInTheDocument()
    expect(within(row).queryByRole('button', { name: t('common.more') })).not.toBeInTheDocument()
  })

  it('offers only the target-specific assignable roles and hides removal of a protected administrator', async () => {
    const user = userEvent.setup()
    grants = [
      { action: 'assign', subject: 'Role' },
      { action: 'delete', subject: 'Member' }
    ]
    directoryRows = [
      { ...colleague, admin: true, can_remove: false, can_assign_role: true, assignable_role_ids: ['safe-role'] }
    ]
    roles = [
      {
        id: 'safe-role',
        code: 'admin',
        name: '允许授予',
        description: '',
        built_in: true,
        sort_order: 1,
        member_count: 1
      },
      {
        id: 'wide-role',
        code: 'custom_wide',
        name: '超出权限上限',
        description: '',
        built_in: false,
        sort_order: 2,
        member_count: 0
      }
    ]
    openPage()
    const row = (await screen.findByText(colleague.name)).closest('tr') as HTMLElement
    expect(within(row).queryByRole('button', { name: t('common.more') })).not.toBeInTheDocument()
    await user.click(within(row).getByRole('button', { name: t('common.edit') }))
    const dialog = await screen.findByRole('dialog')
    await user.click(within(dialog).getByRole('button', { name: new RegExp(t('org.members.setRole')) }))
    expect(await screen.findByRole('option', { name: '允许授予' })).toBeInTheDocument()
    expect(screen.queryByRole('option', { name: '超出权限上限' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('option', { name: '允许授予' }))
    await user.click(within(dialog).getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(requests.filter((row) => row.method === 'PUT')).toEqual([
      { path: `/api/member/orgs/${orgId}/members/user-2/role`, method: 'PUT', body: { role_id: 'safe-role' } }
    ])
  })

  it('refetches a narrowed directory and discards the old broader-scope response', async () => {
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/members',
      method: 'GET',
      response: new Promise((resolve) => {
        finish = resolve
      })
    }
    const oldResponse = delayedSettings.response
    openPage()
    await waitFor(() => expect(requests.some((row) => row.path.endsWith('/members'))).toBe(true))
    scopeKind = 'self'
    delayedSettings = null
    directoryRows = [{ ...colleague, id: 'self-user', name: '范围内本人' }]
    await act(async () => {
      await revalidatePolicy()
    })
    expect(await screen.findByText('范围内本人')).toBeInTheDocument()
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: [colleague] })))
      await oldResponse
    })
    expect(screen.queryByText(colleague.name)).not.toBeInTheDocument()
    expect(screen.getByText('范围内本人')).toBeInTheDocument()
  })

  it('disposes a revoked member editor and ignores its late write after row access is regranted', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'assign', subject: 'Position' }]
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/position',
      method: 'PUT',
      response: new Promise((resolve) => {
        finish = resolve
      })
    }
    const oldResponse = delayedSettings.response
    openPage()
    const row = (await screen.findByText(colleague.name)).closest('tr') as HTMLElement
    await user.click(within(row).getByRole('button', { name: t('common.edit') }))
    const dialog = await screen.findByRole('dialog')
    await user.click(within(dialog).getByRole('button', { name: new RegExp(t('org.members.col.position')) }))
    await user.click(await screen.findByText('工程师'))
    await user.click(within(dialog).getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(requests.some((row) => row.method === 'PUT')).toBe(true))
    directoryRows = [{ ...directoryRows[0], can_assign_position: false }]
    await act(async () => {
      await revalidateDirectory()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    directoryRows = [{ ...directoryRows[0], can_assign_position: true }]
    await act(async () => {
      await revalidateDirectory()
    })
    await user.click(
      within(screen.getByText(colleague.name).closest('tr') as HTMLElement).getByRole('button', {
        name: t('common.edit')
      })
    )
    await screen.findByRole('dialog')
    const reads = requests.filter((row) => row.path.endsWith('/members')).length
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
      await oldResponse
    })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(requests.filter((row) => row.path.endsWith('/members'))).toHaveLength(reads)
  })

  it('reloads ownership candidates after revocation and regrant and ignores the earlier delayed read', async () => {
    const user = userEvent.setup()
    creator = true
    settingsCaps.transfer = true
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/transfer-candidates',
      method: 'GET',
      response: new Promise((resolve) => {
        finish = resolve
      })
    }
    const oldResponse = delayedSettings.response
    openPage('settings')
    await user.click(await screen.findByRole('button', { name: t('org.settings.transferCta') }))
    await waitFor(() => expect(requests.some((row) => row.path.endsWith('/transfer-candidates'))).toBe(true))
    settingsCaps.transfer = false
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    settingsCaps.transfer = true
    delayedSettings = null
    transferCandidates = [{ id: 'fresh-member', name: '当前接手人' }]
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: t('org.settings.transferCta') }))
    const dialog = await screen.findByRole('dialog')
    await user.click(await within(dialog).findByRole('button', { name: new RegExp(t('org.settings.transferPick')) }))
    expect(await screen.findByText('当前接手人')).toBeInTheDocument()
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: [{ id: 'old-member', name: '过期接手人' }] })))
      await oldResponse
    })
    expect(screen.queryByText('过期接手人')).not.toBeInTheDocument()
    expect(screen.getByText('当前接手人')).toBeInTheDocument()
  })

  it('requires each effective settings capability and whole-team profile scope, even for a creator', async () => {
    creator = true
    admin = true
    openPage('settings')
    await screen.findByRole('heading', { name: t('org.nav.settings') })
    expect(screen.queryByRole('textbox', { name: t('org.settings.name') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.settings.transferCta') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.settings.dissolveCta') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.settings.leaveCta') })).not.toBeInTheDocument()
    creator = false
    grants = [{ action: 'update', subject: 'Organization' }]
    settingsCaps = { update: true, transfer: true, delete: true, leave: true }
    scopeKind = 'department'
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('textbox', { name: t('org.settings.name') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.settings.transferCta') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.settings.dissolveCta') })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: t('org.settings.leaveCta') })).toBeInTheDocument()
    scopeKind = 'all'
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.getByRole('textbox', { name: t('org.settings.name') })).toBeInTheDocument()
  })

  it('discards a late profile save after policy changes while retaining the new pending save', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'update', subject: 'Organization' }]
    settingsCaps.update = true
    settingsCaps.leave = true
    let finishOld!: (response: Response) => void
    delayedSettings = {
      suffix: `/orgs/${orgId}`,
      method: 'PUT',
      response: new Promise((resolve) => {
        finishOld = resolve
      })
    }
    const oldResponse = delayedSettings.response
    openPage('settings')
    await user.clear(await screen.findByRole('textbox', { name: t('org.settings.name') }))
    await user.type(screen.getByRole('textbox', { name: t('org.settings.name') }), '旧草稿')
    await user.click(screen.getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(requests.some((row) => row.method === 'PUT')).toBe(true))
    settingsCaps.leave = false
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.getByRole('textbox', { name: t('org.settings.name') })).toHaveValue('权限测试团队')
    let finishNew!: (response: Response) => void
    delayedSettings = {
      suffix: `/orgs/${orgId}`,
      method: 'PUT',
      response: new Promise((resolve) => {
        finishNew = resolve
      })
    }
    const newResponse = delayedSettings.response
    await user.clear(screen.getByRole('textbox', { name: t('org.settings.name') }))
    await user.type(screen.getByRole('textbox', { name: t('org.settings.name') }), '新草稿')
    await user.click(screen.getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(requests.filter((row) => row.method === 'PUT')).toHaveLength(2))
    const reads = requests.filter((row) => row.path === '/api/member-orgs').length
    await act(async () => {
      finishOld(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
      await oldResponse
    })
    expect(screen.getByRole('textbox', { name: t('org.settings.name') })).toHaveValue('新草稿')
    expect(screen.getByRole('button', { name: t('common.save') })).toBeDisabled()
    expect(requests.filter((row) => row.path === '/api/member-orgs')).toHaveLength(reads)
    await act(async () => {
      finishNew(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
      await newResponse
    })
  })

  it('keeps the selected team after a late leave response following revocation and regrant', async () => {
    const user = userEvent.setup()
    settingsCaps.leave = true
    let finish!: (response: Response) => void
    delayedSettings = {
      suffix: '/membership',
      method: 'DELETE',
      response: new Promise((resolve) => {
        finish = resolve
      })
    }
    const pendingResponse = delayedSettings.response
    openPage('settings')
    await user.click(await screen.findByRole('button', { name: t('org.settings.leaveCta') }))
    const dialog = await screen.findByRole('dialog')
    await user.click(within(dialog).getByRole('button', { name: t('org.settings.leaveCta') }))
    await waitFor(() => expect(requests.some((row) => row.method === 'DELETE')).toBe(true))
    settingsCaps.leave = false
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    settingsCaps.leave = true
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    const reads = requests.filter((row) => row.path === '/api/member-orgs').length
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
      await pendingResponse
    })
    expect(readCenterId()).toBe(orgId)
    expect(screen.getByRole('heading', { name: t('org.nav.settings') })).toBeInTheDocument()
    expect(requests.filter((row) => row.path === '/api/member-orgs')).toHaveLength(reads)
  })

  it('requires effective full-team invitation capability even for an administrator with the invite grant', async () => {
    admin = true
    grants = [{ action: 'invite', subject: 'Member' }]
    openPage()
    await screen.findByText(colleague.name)
    expect(screen.queryByRole('button', { name: t('org.invite.open') })).not.toBeInTheDocument()
    inviteCapability = true
    scopeKind = 'department'
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('button', { name: t('org.invite.open') })).not.toBeInTheDocument()
    scopeKind = 'all'
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.getByRole('button', { name: t('org.invite.open') })).toBeInTheDocument()
    expect(requests.filter((request) => request.path.endsWith('/join-invite'))).toEqual([])
  })

  it('lets inviters share a link while configuration requires its independent grant', async () => {
    const user = userEvent.setup()
    admin = true
    inviteCapability = true
    grants = [{ action: 'invite', subject: 'Member' }]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
    expect(await screen.findByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/current-code')
    expect(screen.getByRole('button', { name: t('org.invite.copy') })).toBeEnabled()
    expect(screen.queryByRole('button', { name: t('org.invite.disable') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.join.rotateCta') })).not.toBeInTheDocument()
    grants.push({ action: 'configure', subject: 'JoinInvite' })
    await act(async () => {
      await revalidatePolicy()
    })
    await user.click(await screen.findByRole('button', { name: t('org.join.rotateCta') }))
    expect(screen.getByRole('dialog', { name: t('org.join.rotateCta') })).toBeInTheDocument()
    grants = [{ action: 'invite', subject: 'Member' }]
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('dialog', { name: t('org.join.rotateCta') })).not.toBeInTheDocument()
    expect(await screen.findByRole('button', { name: t('org.invite.copy') })).toBeEnabled()
    expect(requests.filter((request) => request.method === 'PUT' && request.path.endsWith('/join-invite'))).toEqual([])
    grants = [{ action: 'configure', subject: 'JoinInvite' }]
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.invite.open') })).not.toBeInTheDocument()
  })

  it.each([404, 409, 503])(
    'withdraws invitation controls after refused configuration %s and reloads current policy before retry',
    async (status) => {
      const user = userEvent.setup()
      inviteCapability = true
      grants = [
        { action: 'invite', subject: 'Member' },
        { action: 'configure', subject: 'JoinInvite' }
      ]
      delayedConfigure = Promise.resolve(new Response(JSON.stringify({ message: 'configuration refused' }), { status }))
      openPage()
      await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
      await user.click(await screen.findByRole('button', { name: t('org.invite.disable') }))
      await screen.findByRole('button', { name: t('common.retry') })
      expect(screen.queryByRole('textbox', { name: t('org.invite.link') })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: t('org.invite.copy') })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: t('org.join.rotateCta') })).not.toBeInTheDocument()
      grants = [{ action: 'invite', subject: 'Member' }]
      joinInvite = { ...joinInvite, code: 'recovered-link' }
      delayedConfigure = null
      const policyReads = requests.filter((row) => row.path === '/api/member-orgs').length
      await user.click(screen.getByRole('button', { name: t('common.retry') }))
      expect(await screen.findByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/recovered-link')
      expect(requests.filter((row) => row.path === '/api/member-orgs').length).toBeGreaterThan(policyReads)
      expect(screen.getByRole('button', { name: t('org.invite.copy') })).toBeEnabled()
      expect(screen.queryByRole('button', { name: t('org.invite.disable') })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: t('org.join.rotateCta') })).not.toBeInTheDocument()
      expect(requests.filter((row) => row.method === 'PUT' && row.path.endsWith('/join-invite'))).toHaveLength(1)
    }
  )

  it.each(['approve', 'reject'])(
    'withdraws pending invitation actions after a refused %s request',
    async (decision) => {
      const user = userEvent.setup()
      inviteCapability = true
      reviewJoin = true
      grants = [
        { action: 'invite', subject: 'Member' },
        { action: decision, subject: 'JoinRequest' }
      ]
      delayedSettings = {
        suffix: decision === 'approve' ? '/approval' : '/rejection',
        method: 'PUT',
        response: Promise.resolve(new Response(JSON.stringify({ message: 'decision refused' }), { status: 404 }))
      }
      openPage()
      await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
      const actionLabel = t(decision === 'approve' ? 'org.invite.approve' : 'org.invite.reject')
      await user.click(await screen.findByRole('button', { name: actionLabel }))
      await screen.findByRole('button', { name: t('common.retry') })
      const dialog = screen.getByRole('dialog', { name: t('org.invite.open') })
      expect(within(dialog).queryByText('申请人')).not.toBeInTheDocument()
      expect(within(dialog).queryByRole('button', { name: actionLabel })).not.toBeInTheDocument()
      expect(within(dialog).queryByRole('textbox', { name: t('org.invite.link') })).not.toBeInTheDocument()
    }
  )

  it('ignores a late join decision after invitation access was withdrawn and recovered', async () => {
    const user = userEvent.setup()
    inviteCapability = true
    reviewJoin = true
    grants = [
      { action: 'invite', subject: 'Member' },
      { action: 'configure', subject: 'JoinInvite' },
      { action: 'approve', subject: 'JoinRequest' }
    ]
    let finish!: (response: Response) => void
    const oldDecision = new Promise<Response>((resolve) => {
      finish = resolve
    })
    delayedSettings = { suffix: '/approval', method: 'PUT', response: oldDecision }
    delayedConfigure = Promise.resolve(
      new Response(JSON.stringify({ message: 'configuration refused' }), { status: 404 })
    )
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
    await user.click(await screen.findByRole('button', { name: t('org.invite.approve') }))
    await user.click(screen.getByRole('button', { name: t('org.invite.disable') }))
    await screen.findByRole('button', { name: t('common.retry') })
    delayedSettings = null
    delayedConfigure = null
    joinInvite = { ...joinInvite, code: 'recovered-link' }
    await user.click(screen.getByRole('button', { name: t('common.retry') }))
    expect(await screen.findByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/recovered-link')
    const dialog = screen.getByRole('dialog', { name: t('org.invite.open') })
    await within(dialog).findByText('申请人')
    const policyReads = requests.filter((row) => row.path === '/api/member-orgs').length
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: { ok: true } })))
      await oldDecision
    })
    expect(within(dialog).getByText('申请人')).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: t('org.invite.approve') })).toBeEnabled()
    expect(requests.filter((row) => row.path === '/api/member-orgs')).toHaveLength(policyReads)
  })

  it('keeps recovered invitation access after a late refusal from the previous policy', async () => {
    const user = userEvent.setup()
    inviteCapability = true
    grants = [
      { action: 'invite', subject: 'Member' },
      { action: 'configure', subject: 'JoinInvite' }
    ]
    let finish!: (response: Response) => void
    const oldConfiguration = new Promise<Response>((resolve) => {
      finish = resolve
    })
    delayedConfigure = oldConfiguration
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
    await user.click(await screen.findByRole('button', { name: t('org.invite.disable') }))
    grants = [{ action: 'invite', subject: 'Member' }]
    joinInvite = { ...joinInvite, code: 'recovered-link' }
    await act(async () => {
      await revalidatePolicy()
    })
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/recovered-link')
    )
    await act(async () => {
      finish(new Response(JSON.stringify({ message: 'old authority refused' }), { status: 404 }))
      await oldConfiguration
    })
    expect(screen.getByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/recovered-link')
    expect(screen.getByRole('button', { name: t('org.invite.copy') })).toBeEnabled()
    expect(screen.queryByRole('button', { name: t('common.retry') })).not.toBeInTheDocument()
  })

  it.each([undefined, '', 'a'.repeat(63), 'A'.repeat(64), 42])(
    'keeps sharing available but hides configuration without a canonical invitation revision: %s',
    async (revision) => {
      const user = userEvent.setup()
      inviteCapability = true
      grants = [
        { action: 'invite', subject: 'Member' },
        { action: 'configure', subject: 'JoinInvite' }
      ]
      joinInvite = { ...joinInvite, revision: revision as string | undefined }
      openPage()
      await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
      await screen.findByRole('textbox', { name: t('org.invite.link') })
      expect(screen.getByRole('button', { name: t('org.invite.copy') })).toBeEnabled()
      expect(screen.queryByRole('button', { name: t('org.invite.disable') })).not.toBeInTheDocument()
      expect(screen.queryByRole('button', { name: t('org.join.rotateCta') })).not.toBeInTheDocument()
      expect(requests.filter((row) => row.method === 'PUT' && row.path.endsWith('/join-invite'))).toHaveLength(0)
    }
  )

  it('sends the displayed invitation revision when changing configuration', async () => {
    const user = userEvent.setup()
    inviteCapability = true
    grants = [
      { action: 'invite', subject: 'Member' },
      { action: 'configure', subject: 'JoinInvite' }
    ]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
    await user.click(await screen.findByRole('button', { name: t('org.invite.disable') }))
    await waitFor(() =>
      expect(requests.find((row) => row.method === 'PUT' && row.path.endsWith('/join-invite'))?.body).toEqual({
        enabled: false,
        expected_revision: 'a'.repeat(64)
      })
    )
  })

  it('requires a new rotation review after a conflict and sends only the newly loaded revision', async () => {
    const user = userEvent.setup()
    inviteCapability = true
    grants = [
      { action: 'invite', subject: 'Member' },
      { action: 'configure', subject: 'JoinInvite' }
    ]
    delayedConfigure = Promise.resolve(new Response(JSON.stringify({ message: 'invitation changed' }), { status: 409 }))
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
    await user.click(await screen.findByRole('button', { name: t('org.join.rotateCta') }))
    await user.click(
      within(await screen.findByRole('dialog', { name: t('org.join.rotateCta') })).getByRole('button', {
        name: t('org.join.rotateCta')
      })
    )
    await screen.findByRole('button', { name: t('common.retry') })
    expect(screen.queryByRole('dialog', { name: t('org.join.rotateCta') })).not.toBeInTheDocument()
    joinInvite = { ...joinInvite, code: 'newly-reviewed-code', revision: 'b'.repeat(64) }
    delayedConfigure = null
    await user.click(screen.getByRole('button', { name: t('common.retry') }))
    expect(await screen.findByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/newly-reviewed-code')
    expect(requests.filter((row) => row.method === 'PUT' && row.path.endsWith('/join-invite'))).toHaveLength(1)
    expect(screen.queryByRole('dialog', { name: t('org.join.rotateCta') })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: t('org.join.rotateCta') }))
    await user.click(
      within(await screen.findByRole('dialog', { name: t('org.join.rotateCta') })).getByRole('button', {
        name: t('org.join.rotateCta')
      })
    )
    await waitFor(() =>
      expect(
        requests.filter((row) => row.method === 'PUT' && row.path.endsWith('/join-invite')).map((row) => row.body)
      ).toEqual([
        { rotate: true, expected_revision: 'a'.repeat(64) },
        { rotate: true, expected_revision: 'b'.repeat(64) }
      ])
    )
  })

  it('requires the configuration grant to delegate it through the role editor', async () => {
    const user = userEvent.setup()
    grants = [
      { action: 'update', subject: 'Role' },
      { action: 'invite', subject: 'Member' }
    ]
    roles = [editableRole]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    const permission = await screen.findByRole('checkbox', { name: t('org.perm.rule.configureJoinInvite') })
    expect(permission).toHaveAttribute('aria-disabled', 'true')
    await user.click(permission)
    expect(permission).toHaveAttribute('aria-checked', 'false')
    grants.push({ action: 'configure', subject: 'JoinInvite' })
    await act(async () => {
      await revalidatePolicy()
    })
    const authorizedPermission = screen.getByRole('checkbox', { name: t('org.perm.rule.configureJoinInvite') })
    await user.click(authorizedPermission)
    expect(authorizedPermission).toHaveAttribute('aria-checked', 'true')
    await user.click(screen.getByRole('button', { name: t('common.save') }))
    await waitFor(() =>
      expect(
        requests.find((request) => request.method === 'PUT' && request.path.includes('/roles/'))?.body
      ).toMatchObject({
        rules: [{ action: 'configure', subject: 'JoinInvite' }],
        data_scope: { kind: 'self', department_ids: [] },
        expected_revision: editableRole.policy_revision
      })
    )
  })

  it('discards late invitation configuration after revocation and regrant without restoring an old link', async () => {
    const user = userEvent.setup()
    grants = [
      { action: 'invite', subject: 'Member' },
      { action: 'configure', subject: 'JoinInvite' },
      { action: 'approve', subject: 'JoinRequest' }
    ]
    reviewJoin = true
    inviteCapability = true
    let finish!: (response: Response) => void
    delayedConfigure = new Promise<Response>((resolve) => {
      finish = resolve
    })
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.invite.open') }))
    expect(await screen.findByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/current-code')
    await user.click(screen.getByRole('button', { name: t('org.invite.disable') }))
    await waitFor(() =>
      expect(requests.some((request) => request.method === 'PUT' && request.path.endsWith('/join-invite'))).toBe(true)
    )
    grants = [
      { action: 'invite', subject: 'Member' },
      { action: 'approve', subject: 'JoinRequest' }
    ]
    joinInvite = { ...joinInvite, code: 'fresh-code' }
    await act(async () => {
      await revalidatePolicy()
    })
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/fresh-code')
    )
    await act(async () => {
      finish(new Response(JSON.stringify({ code: 0, data: { ...joinInvite, code: 'late-old-code', enabled: false } })))
      await delayedConfigure
    })
    expect(screen.getByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/fresh-code')
    expect(screen.getByRole('button', { name: t('org.invite.copy') })).toBeEnabled()
    expect(screen.queryByRole('button', { name: t('org.invite.disable') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.join.rotateCta') })).not.toBeInTheDocument()
    inviteCapability = false
    await act(async () => {
      await revalidatePolicy()
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    inviteCapability = true
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: t('org.invite.open') }))
    expect(await screen.findByRole('textbox', { name: t('org.invite.link') })).toHaveValue('/join/fresh-code')
  })

  it('limits role controls by server row capabilities and clears an editor after those capabilities are revoked', async () => {
    const user = userEvent.setup()
    grants = ['create', 'update', 'delete'].map((action) => ({ action, subject: 'Role' }))
    grants.push({ action: 'assign', subject: 'Position' })
    roles = [
      {
        id: 'bounded',
        code: 'custom_bounded',
        name: '部门职位维护',
        description: '',
        built_in: false,
        sort_order: 1,
        member_count: 0,
        rules: [],
        data_scope: { kind: 'self', department_ids: [] },
        policy_revision: 'a'.repeat(64),
        can_update: true,
        can_delete: true
      },
      {
        id: 'shared',
        code: 'custom_shared',
        name: '其他部门共享角色',
        description: '',
        built_in: false,
        sort_order: 2,
        member_count: 0,
        rules: [],
        data_scope: { kind: 'self', department_ids: [] },
        policy_revision: 'a'.repeat(64),
        can_update: false,
        can_delete: false
      },
      {
        id: 'unknown',
        code: 'custom_unknown',
        name: '缺少操作许可的角色',
        description: '',
        built_in: false,
        sort_order: 3,
        member_count: 0,
        rules: [],
        data_scope: { kind: 'self', department_ids: [] }
      }
    ]
    openPage()
    await user.click(await screen.findByRole('button', { name: t('org.nav.roles') }))
    await user.click(await screen.findByRole('button', { name: roles[2].name }))
    expect(screen.queryByRole('button', { name: `${t('common.edit')} ${roles[2].name}` })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: t('org.perm.rule.assignPosition') })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: roles[1].name }))
    expect(screen.queryByRole('button', { name: `${t('common.edit')} ${roles[1].name}` })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: `${t('org.roles.delete')} ${roles[1].name}` })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: t('org.perm.rule.assignPosition') })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: roles[0].name }))
    expect(screen.getByRole('button', { name: t('org.perm.rule.assignPosition') })).toBeEnabled()
    expect(screen.getByRole('button', { name: t('org.perm.rule.deleteMember') })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: `${t('common.edit')} ${roles[0].name}` }))
    await screen.findByRole('dialog')
    roles = roles.map((role) => ({ ...role, can_update: false, can_delete: false }))
    await act(async () => {
      await revalidatePolicy()
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: `${t('common.edit')} ${roles[0].name}` })).not.toBeInTheDocument()
    roles = roles.map((role) => ({ ...role, can_update: true, can_delete: true }))
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(requests.filter((request) => request.method === 'PUT')).toEqual([])
  })
  it('does not grant administration buttons from the admin membership flag', async () => {
    admin = true
    openPage()
    await screen.findByText(colleague.name)
    expect(screen.queryByRole('button', { name: t('org.invite.open') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('common.edit') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.members.deptAdd') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.nav.positions') })).not.toBeInTheDocument()
    expect(requests.some((r) => r.path.endsWith('/join-requests'))).toBe(false)
  })

  it.each([undefined, [], [''], ['dept-leaf', 'dept-leaf'], [' dept-leaf'], [42]].map((ids) => ({ ids })))(
    'refuses department editing for a missing or malformed destination list %j',
    async ({ ids }) => {
      grants = [{ action: 'assign', subject: 'Department' }]
      directoryRows = [{ ...directoryRows[0], assignable_department_ids: ids as string[] | undefined }]
      openPage()
      const row = (await screen.findByText('李四')).closest('tr')!
      expect(within(row).queryByRole('button', { name: t('common.edit') })).not.toBeInTheDocument()
      grants = [...grants, { action: 'assign', subject: 'Position' }]
      await act(async () => {
        await revalidatePolicy()
      })
      fireEvent.click(await screen.findByRole('button', { name: t('common.edit') }))
      const dialog = screen.getByRole('dialog')
      expect(within(dialog).getByRole('combobox', { name: t('org.members.col.department') })).toBeDisabled()
      expect(requests.filter((request) => request.method === 'PUT' && request.path.endsWith('/department'))).toEqual([])
    }
  )

  it('disables a denied parent while allowing an authorized descendant department change', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'assign', subject: 'Department' }]
    departmentRows = [
      { id: 'dept-root', parent_id: null, name: '总部门', sort_order: 0 },
      { id: 'dept-leaf', parent_id: 'dept-root', name: '研发部', sort_order: 0 },
      { id: 'dept-destination', parent_id: 'dept-root', name: '测试部', sort_order: 1 }
    ]
    directoryRows = [{ ...directoryRows[0], assignable_department_ids: ['dept-leaf', 'dept-destination'] }]
    openPage()
    const row = (await screen.findByText('李四')).closest('tr')!
    await user.click(within(row).getByRole('button', { name: t('common.edit') }))
    await user.click(screen.getByRole('combobox', { name: t('org.members.col.department') }))
    expect(await screen.findByRole('treeitem', { name: '总部门' })).toBeDisabled()
    expect(screen.getByRole('treeitem', { name: '测试部' })).toBeEnabled()
    await user.click(screen.getByRole('treeitem', { name: '测试部' }))
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: t('common.save') }))
    await waitFor(() =>
      expect(requests.filter((request) => request.method === 'PUT')).toEqual([
        {
          path: `/api/member/orgs/${orgId}/members/user-2/department`,
          method: 'PUT',
          body: { department_id: 'dept-destination' }
        }
      ])
    )
  })

  it('discards a selected destination after its row authorization is narrowed', async () => {
    const user = userEvent.setup()
    grants = [{ action: 'assign', subject: 'Department' }]
    departmentRows = [
      { id: 'dept-root', parent_id: null, name: '总部门', sort_order: 0 },
      { id: 'dept-leaf', parent_id: 'dept-root', name: '研发部', sort_order: 0 }
    ]
    openPage()
    const row = (await screen.findByText('李四')).closest('tr')!
    await user.click(within(row).getByRole('button', { name: t('common.edit') }))
    await user.click(screen.getByRole('combobox', { name: t('org.members.col.department') }))
    await user.click(await screen.findByRole('treeitem', { name: '总部门' }))
    directoryRows = [{ ...directoryRows[0], assignable_department_ids: ['dept-leaf'] }]
    await act(async () => {
      await revalidateDirectory()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    await user.click(
      within((await screen.findByText('李四')).closest('tr')!).getByRole('button', { name: t('common.edit') })
    )
    expect(screen.getByRole('combobox', { name: t('org.members.col.department') })).toHaveTextContent('研发部')
    await user.click(screen.getByRole('combobox', { name: t('org.members.col.department') }))
    expect(await screen.findByRole('treeitem', { name: '总部门' })).toBeDisabled()
    expect(requests.filter((request) => request.method === 'PUT')).toEqual([])
  })

  it('allows only position assignment for a custom member role and renders a scoped department root', async () => {
    grants = [{ action: 'assign', subject: 'Position' }]
    openPage()
    const row = (await screen.findByText(colleague.name)).closest('tr') as HTMLElement
    expect(screen.getByText('研发部', { selector: 'span' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.members.deptAdd') })).not.toBeInTheDocument()
    expect(within(row).queryByRole('button', { name: t('common.more') })).not.toBeInTheDocument()
    fireEvent.click(within(row).getByRole('button', { name: t('common.edit') }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByRole('combobox', { name: t('org.members.col.department') })).toBeDisabled()
    expect(within(dialog).queryByText(t('org.members.col.role'))).not.toBeInTheDocument()
    fireEvent.click(within(dialog).getByRole('button', { name: new RegExp(t('org.members.col.position')) }))
    fireEvent.click(await screen.findByText('工程师'))
    fireEvent.click(within(dialog).getByRole('button', { name: t('common.save') }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(requests.filter((r) => r.method === 'PUT')).toEqual([
      { path: `/api/member/orgs/${orgId}/members/user-2/position`, method: 'PUT', body: { position_id: 'position-1' } }
    ])
    fireEvent.click(screen.getByRole('button', { name: t('org.nav.positions') }))
    await screen.findByRole('heading', { name: t('org.nav.positions') })
    expect(screen.queryByRole('button', { name: t('org.positions.add') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: `${t('org.positions.rename')} 工程师` })).not.toBeInTheDocument()
  })

  it('separates approval from rejection and hides edit and invitation controls', async () => {
    grants = [{ action: 'approve', subject: 'JoinRequest' }]
    reviewJoin = true
    openPage()
    await showPending()
    const row = (await screen.findByText('申请人')).closest('tr') as HTMLElement
    expect(within(row).getByRole('button', { name: t('org.invite.approve') })).toBeInTheDocument()
    expect(within(row).queryByRole('button', { name: t('common.more') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('org.invite.open') })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: t('common.edit') })).not.toBeInTheDocument()
  })

  it('requires server data-scope authority for join buttons and clears them after scope revocation', async () => {
    grants = [
      { action: 'approve', subject: 'JoinRequest' },
      { action: 'reject', subject: 'JoinRequest' }
    ]
    openPage()
    await screen.findByText(colleague.name)
    expect(screen.queryByText('申请人')).not.toBeInTheDocument()
    expect(requests.some((request) => request.path.endsWith('/join-requests'))).toBe(false)
    reviewJoin = true
    await act(async () => {
      await revalidatePolicy()
    })
    await showPending()
    await screen.findByText('申请人')
    expect(screen.getByRole('button', { name: t('org.invite.approve') })).toBeInTheDocument()
    reviewJoin = false
    await act(async () => {
      await revalidatePolicy()
    })
    await waitFor(() => expect(screen.queryByText('申请人')).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: t('org.invite.approve') })).not.toBeInTheDocument()
    expect(requests.filter((request) => request.method !== 'GET' && request.path.includes('/join-requests'))).toEqual(
      []
    )
  })

  it('closes the member editor after permissions are revoked and refreshed', async () => {
    grants = [{ action: 'assign', subject: 'Position' }]
    openPage()
    const row = (await screen.findByText(colleague.name)).closest('tr') as HTMLElement
    fireEvent.click(within(row).getByRole('button', { name: t('common.edit') }))
    await screen.findByRole('dialog')
    grants = []
    await act(async () => {
      await revalidatePolicy()
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: t('common.edit') })).not.toBeInTheDocument()
    expect(requests.filter((r) => r.method === 'PUT')).toEqual([])
    grants = [{ action: 'assign', subject: 'Position' }]
    await act(async () => {
      await revalidatePolicy()
    })
    expect(screen.getByRole('button', { name: t('common.edit') })).toBeInTheDocument()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })
})
