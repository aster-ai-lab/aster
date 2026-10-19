import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { SWRConfig } from 'swr'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { MemberOrg } from '@renderer/data/memberOrgs'

import type * as AdminApi from '../orgAdminApi'
import {
  adminErrorKey,
  isCreditCode,
  isInvitePhone,
  tokenState,
  usedSeats,
  type AccessToken,
  type MemberInvite,
  type OrgSubscription
} from '../orgAdminApi'
import type { OrgMemberRow } from '../orgApi'
import { maskPhone, OrgInvitesTable } from '../OrgInvitesTable'
import { OrgMemberTable } from '../OrgMemberTable'
import { OrgPlanPanel } from '../OrgPlanPanel'
import { OrgTokensPanel } from '../OrgTokensPanel'
import { OrgVerificationCard } from '../OrgVerificationCard'

const admin = vi.hoisted(() => ({
  setOrgMemberStatus: vi.fn(),
  revokeMemberInvite: vi.fn(),
  fetchOrgSubscription: vi.fn(),
  listOrgEntitlements: vi.fn(),
  updateOrgSubscription: vi.fn(),
  listAccessTokens: vi.fn(),
  createAccessToken: vi.fn(),
  revokeAccessToken: vi.fn(),
  fetchOrgVerification: vi.fn(),
  fetchPersonalVerification: vi.fn(),
  submitOrgVerification: vi.fn(),
  submitPersonalVerification: vi.fn()
}))

const toast = vi.hoisted(() => ({ success: vi.fn(), error: vi.fn() }))

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars && Object.keys(vars).length ? `${key} ${JSON.stringify(vars)}` : key,
    i18n: { language: 'zh-CN' }
  })
}))
vi.mock('@renderer/services/toast', () => ({ toast }))
vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))
vi.mock('../orgAdminApi', async (importOriginal) => ({
  ...(await importOriginal<typeof AdminApi>()),
  ...admin
}))

function team(patch: Partial<MemberOrg> = {}): MemberOrg {
  return {
    id: 'org-1',
    name: '北辰科技',
    logo: '',
    membership: 'admin',
    canInviteMembers: true,
    dataScope: { kind: 'all', departmentIds: [] },
    rules: [],
    ...patch
  }
}

function person(id: string, patch: Partial<OrgMemberRow> = {}): OrgMemberRow {
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
    can_update_status: true,
    ...patch
  }
}

function invite(id: string, patch: Partial<MemberInvite> = {}): MemberInvite {
  return {
    id,
    revision: `rev-${id}`,
    org_id: 'org-1',
    phone: '13800001234',
    role_code: null,
    code: `code-${id}`,
    status: 'pending',
    created_at: '2026-10-12T00:00:00Z',
    accepted_at: null,
    ...patch
  }
}

function withSwr(node: React.ReactNode) {
  return render(<SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>{node}</SWRConfig>)
}

const updateMember = { action: 'update', subject: 'Member' }

beforeEach(() => {
  vi.clearAllMocks()
  localStorage.clear()
})

describe('team admin contract helpers', () => {
  it('maps exact owner answers and nothing else', () => {
    expect(adminErrorKey(new Error('cannot change your own status'))).toBe('org.status.selfChange')
    expect(adminErrorKey(new Error('a resigned member must be invited again'))).toBe('org.status.resignedFinal')
    expect(adminErrorKey(new Error('invitation changed; reload before revoking'))).toBe('org.invites.changed')
    expect(adminErrorKey(new Error('self-serve plan change is disabled; contact us'))).toBe('org.plan.selfServeOff')
    expect(adminErrorKey(new Error('every seat is taken; add seats or disable a member first'))).toBe('org.seats.full')
    expect(adminErrorKey(new Error('seats cannot be fewer than active members; disable members first'))).toBe(
      'org.seats.belowActive'
    )
    expect(adminErrorKey(new Error('Cannot change your own status'))).toBe('org.error.generic')
    expect(adminErrorKey('cannot change your own status')).toBe('org.error.generic')
    expect(adminErrorKey(new Error('credit code is invalid'))).toBe('org.verification.creditInvalid')
    expect(adminErrorKey(new Error('personal verification is required'))).toBe('org.verification.personalRequired')
  })

  it('counts seats from active members only, like the server', () => {
    expect(
      usedSeats([
        { status: 'active' },
        { status: 'disabled' },
        { status: 'resigned' },
        { status: 'invited' },
        { status: 'active' }
      ])
    ).toBe(2)
  })

  it('derives token state and validates phone, credit code and masking', () => {
    const now = Date.parse('2026-10-13T00:00:00Z')
    expect(tokenState({ revoked_at: null, expires_at: '2026-10-14T00:00:00Z' }, now)).toBe('active')
    expect(tokenState({ revoked_at: null, expires_at: '2026-10-12T00:00:00Z' }, now)).toBe('expired')
    expect(tokenState({ revoked_at: '2026-10-01T00:00:00Z', expires_at: '2026-10-14T00:00:00Z' }, now)).toBe('revoked')
    expect(isInvitePhone('13800001234')).toBe(true)
    expect(isInvitePhone('+8613800001234')).toBe(true)
    expect(isInvitePhone('2380000123')).toBe(false)
    expect(isCreditCode('91310000ma1fl2xw30')).toBe(true)
    expect(isCreditCode('91310000MA1FL2XW31')).toBe(false)
    expect(isCreditCode('91310000MA1FL2XW3')).toBe(false)
    expect(maskPhone('13800001234')).toBe('138****1234')
  })
})

describe('member status', () => {
  function renderTable(rows: OrgMemberRow[], org = team({ rules: [updateMember] }), onRefresh = vi.fn(async () => {})) {
    return {
      onRefresh,
      ...render(
        <OrgMemberTable
          orgId="org-1"
          rows={rows}
          departmentNames={new Map([['dept-root', '总部']])}
          sourceCount={rows.length}
          org={org}
          query=""
          onRefresh={onRefresh}
        />
      )
    }
  }

  async function openMenu(name: string) {
    const row = screen.getByText(name).closest('tr') as HTMLElement
    fireEvent.click(within(row).getByRole('button', { name: 'common.more' }))
  }

  async function confirmLast(name: string) {
    const buttons = await screen.findAllByRole('button', { name })
    fireEvent.click(buttons[buttons.length - 1])
  }

  it('offers status commands by state and only with update Member', async () => {
    renderTable([person('在职者'), person('停用者', { status: 'disabled' }), person('离职者', { status: 'resigned' })])
    await openMenu('在职者')
    expect(screen.getByRole('button', { name: 'org.status.disable' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'org.status.resign' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.status.restore' })).not.toBeInTheDocument()
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' })
    await openMenu('停用者')
    expect(await screen.findByRole('button', { name: 'org.status.restore' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.status.disable' })).not.toBeInTheDocument()
    const resigned = screen.getByText('离职者').closest('tr') as HTMLElement
    expect(within(resigned).queryByRole('button', { name: 'common.more' })).not.toBeInTheDocument()
  })

  it('hides status commands and selection without the grant or the row capability', () => {
    renderTable([person('张三'), person('李四', { can_update_status: false })], team({ rules: [] }))
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'common.more' })).not.toBeInTheDocument()
  })

  it('disables after a consequence dialog and rolls back on refusal', async () => {
    admin.setOrgMemberStatus.mockRejectedValueOnce(new Error('the enterprise must keep at least one admin'))
    renderTable([person('张三')])
    await openMenu('张三')
    fireEvent.click(screen.getByRole('button', { name: 'org.status.disable' }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText(/org\.status\.confirm\.disabled\.access/)).toBeInTheDocument()
    expect(within(dialog).getByText(/org\.status\.confirm\.disabled\.after/)).toBeInTheDocument()
    await confirmLast('org.status.disable')
    await waitFor(() => expect(admin.setOrgMemberStatus).toHaveBeenCalledWith('org-1', '张三', 'disabled'))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('org.members.lastAdmin'))
    expect(screen.getByText('org.status.active')).toBeInTheDocument()
  })

  it('batch-disables one request per row and reports partial failure', async () => {
    admin.setOrgMemberStatus.mockImplementation(async (_org: string, id: string) => {
      if (id === '李四') throw new Error('boom')
      return { ok: true }
    })
    const { onRefresh } = renderTable([person('张三'), person('李四'), person('王五', { status: 'disabled' })])
    const boxes = screen.getAllByRole('checkbox')
    expect(boxes).toHaveLength(3)
    fireEvent.click(boxes[0])
    expect(await screen.findByRole('region', { name: 'collection.selectedActions' })).toBeInTheDocument()
    const bar = screen.getByRole('region', { name: 'collection.selectedActions' })
    fireEvent.click(within(bar).getByRole('button', { name: 'org.status.disable' }))
    expect(await screen.findByRole('dialog')).toBeInTheDocument()
    await confirmLast('org.status.disable')
    await waitFor(() => expect(admin.setOrgMemberStatus).toHaveBeenCalledTimes(2))
    expect(admin.setOrgMemberStatus).toHaveBeenNthCalledWith(1, 'org-1', '张三', 'disabled')
    expect(admin.setOrgMemberStatus).toHaveBeenNthCalledWith(2, 'org-1', '李四', 'disabled')
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(`org.status.batchPartial ${JSON.stringify({ ok: 1, failed: 1 })}`)
    )
    expect(onRefresh).toHaveBeenCalled()
  })
})

describe('invites', () => {
  function renderInvites(invites: MemberInvite[], onRefresh = vi.fn(async () => {})) {
    return {
      onRefresh,
      ...render(
        <OrgInvitesTable
          orgId="org-1"
          org={team({ rules: [{ action: 'invite', subject: 'Member' }] })}
          invites={invites}
          segments={null}
          onRefresh={onRefresh}
        />
      )
    }
  }

  it('says when nothing was ever sent and when the filter hides everything', async () => {
    const { unmount } = renderInvites([])
    expect(screen.getByText('org.invites.empty')).toBeInTheDocument()
    unmount()
    renderInvites([invite('a', { status: 'accepted' })])
    expect(screen.getByText('org.invites.noPending')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'org.invites.filter.label' }))
    const all = await screen.findByRole('option', { name: 'org.invites.filter.all' })
    fireEvent.click(within(all).getByRole('button'))
    expect(await screen.findByText('138****1234')).toBeInTheDocument()
  })

  it('revokes at the reviewed revision and refreshes on a conflict', async () => {
    admin.revokeMemberInvite.mockRejectedValueOnce(new Error('invitation changed; reload before revoking'))
    const { onRefresh } = renderInvites([invite('a')])
    fireEvent.click(screen.getByRole('button', { name: 'org.invites.revoke 138****1234' }))
    const confirm = await screen.findAllByRole('button', { name: 'org.invites.revoke' })
    fireEvent.click(confirm[confirm.length - 1])
    await waitFor(() =>
      expect(admin.revokeMemberInvite).toHaveBeenCalledWith(expect.objectContaining({ id: 'a', revision: 'rev-a' }))
    )
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('org.invites.changed'))
    expect(onRefresh).toHaveBeenCalled()
    expect(screen.getByText('org.invites.status.pending')).toBeInTheDocument()
  })
})

describe('plan and services', () => {
  const free: OrgSubscription = {
    org_id: 'org-1',
    plan_id: 'team',
    status: 'active',
    seats: 5,
    period_start: '2026-10-01T00:00:00Z',
    period_end: null,
    self_serve: true
  }

  it('shows the plan to everyone and the change only with manage Billing', async () => {
    admin.fetchOrgSubscription.mockResolvedValue(free)
    admin.listOrgEntitlements.mockResolvedValue([])
    withSwr(<OrgPlanPanel orgId="org-1" org={team()} members={[{ status: 'active' }]} />)
    expect(await screen.findByText('org.plan.name.team')).toBeInTheDocument()
    expect(await screen.findByText('org.plan.noEntitlements')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.plan.change' })).not.toBeInTheDocument()
  })

  it('explains a closed self-serve switch instead of offering the change', async () => {
    admin.fetchOrgSubscription.mockResolvedValue({ ...free, self_serve: false })
    admin.listOrgEntitlements.mockResolvedValue([])
    withSwr(
      <OrgPlanPanel orgId="org-1" org={team({ rules: [{ action: 'manage', subject: 'Billing' }] })} members={[]} />
    )
    expect(await screen.findByText('org.plan.selfServeOff')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.plan.change' })).not.toBeInTheDocument()
  })

  it('refuses seats below the active members and shows the server refusal', async () => {
    admin.fetchOrgSubscription.mockResolvedValue({ ...free, plan_id: 'builder', seats: 8 })
    admin.listOrgEntitlements.mockRejectedValue(new Error('down'))
    admin.updateOrgSubscription
      .mockRejectedValueOnce(new Error('seats cannot be fewer than active members; disable members first'))
      .mockResolvedValue({ ...free, plan_id: 'builder', seats: 5 })
    const members = Array.from({ length: 4 }, () => ({ status: 'active' }))
    withSwr(
      <OrgPlanPanel orgId="org-1" org={team({ rules: [{ action: 'manage', subject: 'Billing' }] })} members={members} />
    )
    expect(await screen.findByText('org.error.generic')).toBeInTheDocument()
    fireEvent.click(await screen.findByRole('button', { name: 'org.plan.change' }))
    const seats = await screen.findByLabelText(/org\.plan\.col\.seats/, { selector: 'input' })
    fireEvent.change(seats, { target: { value: '3' } })
    expect(screen.getByRole('alert')).toHaveTextContent('org.plan.belowActive')
    expect(screen.getByRole('button', { name: 'org.plan.confirm' })).toBeDisabled()
    fireEvent.change(seats, { target: { value: '5' } })
    fireEvent.click(screen.getByRole('button', { name: 'org.plan.confirm' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('org.seats.belowActive')
    fireEvent.click(screen.getByRole('button', { name: 'org.plan.confirm' }))
    await waitFor(() => expect(admin.updateOrgSubscription).toHaveBeenLastCalledWith('org-1', 'builder', 5))
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('org.plan.changed'))
  })
})

describe('access tokens', () => {
  const token: AccessToken = {
    id: '11111111-1111-4111-8111-111111111111',
    name: 'laptop',
    scopes: ['repository:read'],
    created_at: '2026-10-01T00:00:00Z',
    expires_at: '2099-01-01T00:00:00Z',
    revoked_at: null,
    last_used_at: null
  }

  it('shows the empty state and the failure retry', async () => {
    admin.listAccessTokens.mockRejectedValueOnce(new Error('down')).mockResolvedValueOnce([])
    withSwr(<OrgTokensPanel />)
    fireEvent.click(await screen.findByRole('button', { name: 'common.retry' }))
    expect(await screen.findByText('org.tokens.empty')).toBeInTheDocument()
  })

  it('shows the secret once and closes only after it is saved', async () => {
    admin.listAccessTokens.mockResolvedValue([])
    admin.createAccessToken.mockResolvedValue({ ...token, token: 'a3s_pat_secret' })
    withSwr(<OrgTokensPanel />)
    fireEvent.click(await screen.findByRole('button', { name: 'org.tokens.create' }))
    const dialog = await screen.findByRole('dialog')
    fireEvent.change(within(dialog).getByRole('textbox'), { target: { value: ' laptop ' } })
    const submit = within(dialog).getAllByRole('button', { name: 'org.tokens.create' })
    fireEvent.click(submit[submit.length - 1])
    await waitFor(() =>
      expect(admin.createAccessToken).toHaveBeenCalledWith({
        name: 'laptop',
        scopes: ['repository:read'],
        expires_in_days: 30
      })
    )
    expect(await screen.findByDisplayValue('a3s_pat_secret')).toBeInTheDocument()
    const done = screen.getByRole('button', { name: 'org.tokens.done' })
    expect(done).toBeDisabled()
    fireEvent.click(screen.getByRole('checkbox', { name: 'org.tokens.savedConfirm' }))
    fireEvent.click(done)
    await waitFor(() => expect(screen.queryByDisplayValue('a3s_pat_secret')).not.toBeInTheDocument())
  })

  it('rolls a refused revoke back to active', async () => {
    admin.listAccessTokens.mockResolvedValue([token])
    admin.revokeAccessToken.mockRejectedValueOnce(new Error('down'))
    withSwr(<OrgTokensPanel />)
    fireEvent.click(await screen.findByRole('button', { name: 'org.tokens.revoke laptop' }))
    const confirm = await screen.findAllByRole('button', { name: 'org.tokens.revoke' })
    await act(async () => {
      fireEvent.click(confirm[confirm.length - 1])
    })
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('org.error.generic'))
    expect(screen.getByText('org.tokens.state.active')).toBeInTheDocument()
  })
})

function orgVerification(patch: Partial<AdminApi.OrgVerification> = {}): AdminApi.OrgVerification {
  return { status: 'unverified', enterprise_name: '', credit_code: '', legal_person: '', reject_reason: '', ...patch }
}

function personalVerification(status: AdminApi.VerificationStatus): AdminApi.PersonalVerification {
  return { status, real_name: status === 'unverified' ? '' : '张三', id_last4: '', reject_reason: '' }
}

describe('enterprise verification card', () => {
  it('shows a non-creator the status only', async () => {
    admin.fetchOrgVerification.mockResolvedValue(orgVerification())
    withSwr(<OrgVerificationCard org={team({ creator: false })} />)
    expect(await screen.findByText('org.verification.creatorOnlyHint')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.verification.submit' })).not.toBeInTheDocument()
    expect(admin.fetchPersonalVerification).not.toHaveBeenCalled()
  })

  it('sends a creator without personal verification to verify first', async () => {
    admin.fetchOrgVerification.mockResolvedValue(orgVerification())
    admin.fetchPersonalVerification.mockResolvedValue(personalVerification('unverified'))
    admin.submitPersonalVerification.mockResolvedValue(personalVerification('pending'))
    withSwr(<OrgVerificationCard org={team({ creator: true })} />)
    fireEvent.click(await screen.findByRole('button', { name: 'org.verification.goPersonal' }))
    const dialog = await screen.findByRole('dialog')
    fireEvent.change(within(dialog).getByLabelText('org.verification.realName'), { target: { value: '张三' } })
    fireEvent.change(within(dialog).getByLabelText('org.verification.idNumber'), { target: { value: '12345' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'org.verification.submit' }))
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('org.verification.idInvalid')
    expect(admin.submitPersonalVerification).not.toHaveBeenCalled()
    fireEvent.change(within(dialog).getByLabelText('org.verification.idNumber'), {
      target: { value: '11010519491231002x' }
    })
    fireEvent.click(within(dialog).getByRole('button', { name: 'org.verification.submit' }))
    await waitFor(() =>
      expect(admin.submitPersonalVerification).toHaveBeenCalledWith({
        real_name: '张三',
        id_number: '11010519491231002X'
      })
    )
    expect(await screen.findByText('org.verification.personalPending')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.verification.goPersonal' })).not.toBeInTheDocument()
  })

  it('submits enterprise details after checking the credit code locally', async () => {
    admin.fetchOrgVerification.mockResolvedValue(orgVerification())
    admin.fetchPersonalVerification.mockResolvedValue(personalVerification('verified'))
    admin.submitOrgVerification.mockResolvedValue(
      orgVerification({
        status: 'pending',
        enterprise_name: '北辰科技有限公司',
        credit_code: '91310000MA1FL2XW30',
        legal_person: '张三'
      })
    )
    withSwr(<OrgVerificationCard org={team({ creator: true })} />)
    fireEvent.click(await screen.findByRole('button', { name: 'org.verification.submit' }))
    const dialog = await screen.findByRole('dialog')
    fireEvent.change(within(dialog).getByLabelText('org.verification.enterpriseName'), {
      target: { value: '北辰科技有限公司' }
    })
    fireEvent.change(within(dialog).getByLabelText('org.verification.creditCode'), {
      target: { value: '91310000MA1FL2XW31' }
    })
    fireEvent.change(within(dialog).getByLabelText('org.verification.legalPerson'), { target: { value: '张三' } })
    fireEvent.click(within(dialog).getByRole('button', { name: 'org.verification.submit' }))
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('org.verification.creditInvalid')
    expect(admin.submitOrgVerification).not.toHaveBeenCalled()
    fireEvent.change(within(dialog).getByLabelText('org.verification.creditCode'), {
      target: { value: '91310000ma1fl2xw30' }
    })
    fireEvent.click(within(dialog).getByRole('button', { name: 'org.verification.submit' }))
    await waitFor(() =>
      expect(admin.submitOrgVerification).toHaveBeenCalledWith('org-1', {
        enterprise_name: '北辰科技有限公司',
        credit_code: '91310000MA1FL2XW30',
        legal_person: '张三'
      })
    )
    expect(await screen.findByText('org.verification.hint.pending')).toBeInTheDocument()
    expect(screen.getByText('91310000MA1FL2XW30')).toBeInTheDocument()
    expect(toast.success).toHaveBeenCalledWith('org.verification.submitted')
  })

  it('shows the rejection reason and lets the creator resubmit with the previous details', async () => {
    admin.fetchOrgVerification.mockResolvedValue(
      orgVerification({
        status: 'rejected',
        enterprise_name: '北辰科技',
        credit_code: '91310000MA1FL2XW30',
        legal_person: '张三',
        reject_reason: '名称与执照不一致'
      })
    )
    admin.fetchPersonalVerification.mockResolvedValue(personalVerification('verified'))
    admin.submitOrgVerification.mockRejectedValue(new Error('credit code is already used'))
    withSwr(<OrgVerificationCard org={team({ creator: true })} />)
    expect(await screen.findByText(/名称与执照不一致/)).toBeInTheDocument()
    fireEvent.click(await screen.findByRole('button', { name: 'org.verification.resubmit' }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByLabelText('org.verification.enterpriseName')).toHaveValue('北辰科技')
    fireEvent.click(within(dialog).getByRole('button', { name: 'org.verification.submit' }))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('org.verification.creditUsed'))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })

  it('fails closed on a malformed owner answer', async () => {
    admin.fetchOrgVerification.mockRejectedValue(new Error('verification response is malformed'))
    withSwr(<OrgVerificationCard org={team({ creator: true })} />)
    expect(await screen.findByText('org.error.generic')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'org.verification.submit' })).not.toBeInTheDocument()
  })
})
